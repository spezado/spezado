// SPDX-License-Identifier: AGPL-3.0-or-later
//! What an owner has to file or pay in a year, and when: advance VAT returns, statements per customer, wage tax
//! returns, statements of contributions, certificates of wage tax, the annual returns.

use librotenado::{Date, PartyId};
use spezado_law::Facts;

use crate::{AppError, BooksId, Workspace};

/// Something to file or to pay, for a period, by a day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligation {
    /// The report or what it is: `de-ustva`, `de-zm`, `de-lsta`, `de-bn` (Beitragsnachweis), `de-lstb`, an annual
    /// report.
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// The first day of its period.
    pub from: Date,
    /// The last day of its period.
    pub to: Date,
    /// The day it is due; none where the law sets none.
    pub due: Option<Date>,
}

impl Workspace {
    /// What `owner` has to file or pay for the calendar year `year`, by the day it is due – those without one last –,
    /// then by period.
    pub fn obligations(&self, books: &BooksId, owner: &PartyId, year: u16) -> Result<Vec<Obligation>, AppError> {
        let (first, last) = (Date::new(year, 1, 1).expect("a date"), Date::new(year, 12, 31).expect("a date"));
        let mut out = Vec::new();
        let item = |id: &'static str, name: &'static str, from: Date, to: Date, due: Option<Date>| Obligation {
            id,
            name,
            from,
            to,
            due,
        };
        for r in self.vat_returns(books, owner, year)? {
            out.push(item("de-ustva", "Umsatzsteuer-Voranmeldung", r.from, r.to, Some(r.due)));
        }
        for r in self.customer_statements(books, owner, year)? {
            out.push(item("de-zm", "Zusammenfassende Meldung", r.from, r.to, Some(r.due)));
        }
        let (configuration, country, legal_form, module) = self.law_for(books, owner, last)?;
        let mut facts = Facts::new(&configuration, country, legal_form.as_deref());
        let events = self.get(books)?.events();
        facts.employer = !events.employments(owner, last).is_empty();
        if facts.employer {
            // The wage tax of the year before decides the period; for an employer new in the year, that of its
            // first month with payslips for a whole year (§ 41a Abs. 2 Satz 4 EStG).
            let tax = |from: Date, to: Date| -> i128 {
                events
                    .payslips_for(owner, from, to)
                    .iter()
                    .flat_map(|(p, _)| {
                        ["wage_tax", "flat_wage_tax"].map(|f| p.figures.get(f).map_or(0, |m| m.base_units()))
                    })
                    .sum()
            };
            let before = tax(Date::new(year - 1, 1, 1).expect("a date"), Date::new(year - 1, 12, 31).expect("a date"));
            let previous = if before > 0 {
                before
            } else {
                let mut months = events.payslips_for(owner, first, last);
                months.sort_by_key(|(p, _)| p.to);
                months.first().map_or(0, |(p, _)| {
                    let start = Date::new(p.to.year(), p.to.month(), 1).expect("a date");
                    tax(start, p.to) * 12
                })
            };
            for (from, to, due) in module.wage_tax_periods(&facts, year, previous) {
                out.push(item("de-lsta", "Lohnsteuer-Anmeldung", from, to, Some(due)));
            }
            for month in 1..=12 {
                let (Some(from), Some((statement, _))) =
                    (Date::new(year, month, 1), module.contribution_deadlines(&facts, year, month))
                else {
                    continue;
                };
                let to = (28..=31).rev().find_map(|d| Date::new(year, month, d)).expect("a last day");
                // Only months someone is employed in.
                if events.employments(owner, to).values().all(|e| e.start > to || e.end.is_some_and(|end| end < from)) {
                    continue;
                }
                out.push(item("de-bn", "Beitragsnachweis", from, to, Some(statement)));
            }
            out.push(item(
                "de-lstb",
                "Lohnsteuerbescheinigungen",
                first,
                last,
                module.annual_deadline(&facts, "de-lstb", year),
            ));
        }
        let periodic = ["de-ustva", "de-zm", "de-lsta"];
        for report in module.reports(&facts, first, last) {
            if periodic.contains(&report.id) {
                continue;
            }
            out.push(item(report.id, report.name, first, last, module.annual_deadline(&facts, report.id, year)));
        }
        // By the day due, those without one last.
        out.sort_by(|a, b| (a.due.is_none(), a.due, a.from, a.id).cmp(&(b.due.is_none(), b.due, b.from, b.id)));
        Ok(out)
    }
}
