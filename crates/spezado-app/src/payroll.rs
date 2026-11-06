// SPDX-License-Identifier: AGPL-3.0-or-later
//! Payroll (librotenado spec part 0013): employments, and a payslip per month – computed by librotenado's rules,
//! recorded, and booked.

use std::collections::BTreeMap;

use librotenado::payroll::{Employment, EmploymentId, GermanEmployment, PayItem, Payslip, PayslipId};
use librotenado::{Booking, Date, PartyId};
use spezado_core::Command;

use crate::{AppError, BooksId, Workspace, new_id};

/// An employment to describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEmployment {
    /// The employer, an owner of the books.
    pub owner: PartyId,
    /// The employee, a person described in the books.
    pub employee: PartyId,
    /// The first day of the employment.
    pub start: Date,
    /// Its last day, if it ends.
    pub end: Option<Date>,
    /// The employer's number for the employee.
    pub number: Option<String>,
    /// The employee's day of birth.
    pub born: Option<Date>,
    /// What German law asks about it.
    pub de: GermanEmployment,
}

/// The last day of a month.
fn last_day(year: u16, month: u8) -> Date {
    (28..=31).rev().find_map(|day| Date::new(year, month, day)).expect("a month has a last day")
}

impl Workspace {
    /// The employments of an owner as described on `day`.
    pub fn employments(&self, books: &BooksId, owner: &PartyId, day: Date) -> Result<Vec<Employment>, AppError> {
        Ok(self.get(books)?.events().employments(owner, day).into_values().collect())
    }

    /// Describes a new employment; returns its identity.
    pub fn add_employment(&mut self, books: &BooksId, new: NewEmployment) -> Result<EmploymentId, AppError> {
        let (_, persons) = self.get(books)?.events().parties();
        if !persons.contains_key(&new.employee) {
            return Err(AppError::Payroll(format!("{} is not a person described in the books", new.employee)));
        }
        if new.end.is_some_and(|end| end < new.start) {
            return Err(AppError::Payroll("an employment ends after it begins".into()));
        }
        let employment = Employment {
            id: new_id(),
            owner: new.owner,
            employee: new.employee,
            from: new.start,
            start: new.start,
            end: new.end,
            number: new.number.filter(|n| !n.trim().is_empty()),
            born: new.born,
            de: Some(new.de),
        };
        let id = employment.id.clone();
        self.execute(books, Command::DescribeEmployment(Box::new(employment)))?;
        Ok(id)
    }

    /// Changes the description of an employment from a day on – the tax class, the insurer, the end.
    pub fn change_employment(&mut self, books: &BooksId, employment: Employment) -> Result<(), AppError> {
        self.execute(books, Command::DescribeEmployment(Box::new(employment)))?;
        Ok(())
    }

    /// The payslip of an employment for a month: the part of it the employment lasts, paid on its last day, with the
    /// figures librotenado's rules give. Nothing is recorded.
    pub fn payslip(
        &self,
        books: &BooksId,
        employment: &EmploymentId,
        year: u16,
        month: u8,
        items: Vec<PayItem>,
    ) -> Result<Payslip, AppError> {
        let first = Date::new(year, month, 1).ok_or_else(|| AppError::Payroll(format!("no month {year}-{month}")))?;
        let last = last_day(year, month);
        let events = self.get(books)?.events();
        let described = events
            .employment(employment, last)
            .ok_or_else(|| AppError::Payroll(format!("employment {employment} has not begun by {last}")))?;
        let from = first.max(described.start);
        let to = described.end.map_or(last, |end| end.min(last));
        if to < from {
            return Err(AppError::Payroll(format!("employment {employment} does not last into {year}-{month:02}")));
        }
        if items.is_empty() {
            return Err(AppError::Payroll("a payslip needs at least one item".into()));
        }
        let figures =
            events.payslip_figures(employment, from, to, &items).map_err(|e| AppError::Payroll(e.to_string()))?;
        Ok(Payslip {
            id: new_id(),
            employment: employment.clone(),
            from,
            to,
            paid: last,
            replaces: None,
            items,
            figures,
        })
    }

    /// A correction of a recorded payslip (Rückrechnung): the same employment and period with other items, paid on
    /// `paid`, with the figures the rules give now. Nothing is recorded.
    pub fn correction(
        &self,
        books: &BooksId,
        replaced: &PayslipId,
        items: Vec<PayItem>,
        paid: Date,
    ) -> Result<Payslip, AppError> {
        let events = self.get(books)?.events();
        let old = events
            .owners()
            .into_iter()
            .flat_map(|owner| events.payslips(owner))
            .find(|p| p.id == *replaced)
            .ok_or_else(|| AppError::Payroll(format!("payslip {replaced} is not one that applies")))?;
        if items.is_empty() {
            return Err(AppError::Payroll("a payslip needs at least one item".into()));
        }
        let figures = events
            .payslip_figures(&old.employment, old.from, old.to, &items)
            .map_err(|e| AppError::Payroll(e.to_string()))?;
        Ok(Payslip {
            id: new_id(),
            employment: old.employment,
            from: old.from,
            to: old.to,
            paid,
            replaces: Some(old.id),
            items,
            figures,
        })
    }

    /// The Lohnsteuerbescheinigung of an employment for a year: its lines in cents; none for wages taxed at a flat
    /// rate or a year without payslips.
    pub fn wage_tax_certificate(
        &self,
        books: &BooksId,
        employment: &EmploymentId,
        year: u16,
    ) -> Result<Option<BTreeMap<String, i128>>, AppError> {
        Ok(self
            .get(books)?
            .events()
            .wage_tax_certificate(employment, year)
            .map(|lines| lines.into_iter().map(|(line, units)| (line.to_owned(), units)).collect()))
    }

    /// Records a payslip and books it: the wages and what the employer pays as expenses against the liabilities, on
    /// the day paid (librotenado `Payslip::postings`).
    pub fn record_payslip(&mut self, books: &BooksId, payslip: Payslip) -> Result<(), AppError> {
        let events = self.get(books)?.events();
        let employment = events
            .employment(&payslip.employment, payslip.to)
            .ok_or_else(|| AppError::Payroll(format!("employment {} is not described", payslip.employment)))?;
        let (_, persons) = events.parties();
        let name =
            persons.get(&employment.employee).map_or_else(|| employment.employee.to_string(), |p| p.name.clone());
        let minor = employment.de.as_ref().is_some_and(|de| de.insurance.person_group == "109");
        let postings = match &payslip.replaces {
            None => payslip.postings(&employment.owner, minor),
            Some(id) => {
                let replaced = events
                    .recorded_payslips(&employment.owner)
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| AppError::Payroll(format!("payslip {id} is not recorded")))?;
                payslip.correction_postings(&replaced, &employment.owner, minor)
            }
        };
        let correction = if payslip.replaces.is_some() { "Korrektur " } else { "" };
        let booking = Booking {
            basis: None,
            attributed: None,
            date: payslip.paid,
            text: format!("{correction}Lohn {:02}/{} {name}", payslip.to.month(), payslip.to.year()),
            postings,
            documents: vec![],
            entries: vec![],
            transaction: None,
        };
        self.execute(books, Command::RecordPayslip(payslip))?;
        self.execute(books, Command::Book(booking))?;
        Ok(())
    }

    /// The payslips of an owner for periods ending from `from` to `to`.
    pub fn payslips(&self, books: &BooksId, owner: &PartyId, from: Date, to: Date) -> Result<Vec<Payslip>, AppError> {
        Ok(self.get(books)?.events().payslips_for(owner, from, to).into_iter().map(|(p, _)| p).collect())
    }

    /// The Beitragsnachweis of an owner to one insurer for the months from `from` to `to`, by the groups of the
    /// record, in cents.
    pub fn contribution_statement(
        &self,
        books: &BooksId,
        owner: &PartyId,
        insurer: &str,
        from: Date,
        to: Date,
    ) -> Result<BTreeMap<String, i128>, AppError> {
        Ok(self
            .get(books)?
            .events()
            .contribution_statement(owner, insurer, from, to)
            .into_iter()
            .map(|(field, units)| (field.to_owned(), units))
            .collect())
    }
}

/// The capital accounts a partner gets: the kind of the Kapitalkontenentwicklung and the account of SKR 03 – the same
/// harmonised account in SKR 04 – it is opened below.
fn partner_accounts(liability: librotenado::Liability) -> &'static [(&'static str, &'static str)] {
    match liability {
        // Festkapital, variables Kapital, Privatkonto.
        librotenado::Liability::Unlimited => &[("FK", "0870"), ("VK", "0880"), ("VK", "1800")],
        // Kommandit-Kapital as fixed capital (KK is closed since 2022), variables Kapital, Verlustausgleichskonto,
        // Privatkonto.
        librotenado::Liability::Limited => &[("FK", "0900"), ("VK", "0880"), ("VVK", "0910"), ("VK", "1800")],
    }
}

impl Workspace {
    /// Opens the capital accounts of a partner of a partnership – fixed and variable capital, losses carried for a
    /// limited partner, a private account –, with the attributes the Kapitalkontenentwicklung of the E-Bilanz reads
    /// (`partner`, `liability`, `capital`, librotenado 0009). Returns the accounts opened with their kind.
    pub fn open_partner_accounts(
        &mut self,
        books: &BooksId,
        partner: &PartyId,
        liability: librotenado::Liability,
    ) -> Result<Vec<(librotenado::AccountId, &'static str)>, AppError> {
        let skr03 = librotenado::ChartTranslation::de_skr03();
        let mut opened = Vec::new();
        for (kind, number) in partner_accounts(liability) {
            let parent =
                skr03.accounts.get(*number).cloned().ok_or_else(|| AppError::UnknownChart(number.to_string()))?;
            let attributes = std::collections::BTreeMap::from([
                ("partner".to_owned(), partner.to_string()),
                (
                    "liability".to_owned(),
                    match liability {
                        librotenado::Liability::Unlimited => "unlimited".to_owned(),
                        librotenado::Liability::Limited => "limited".to_owned(),
                    },
                ),
                ("capital".to_owned(), (*kind).to_owned()),
            ]);
            let account = librotenado::Account {
                id: new_id(),
                kind: librotenado::AccountKind::Equity,
                parent: Some(parent),
                attributes,
            };
            let id = account.id.clone();
            self.execute(books, Command::OpenAccount(account))?;
            opened.push((id, *kind));
        }
        Ok(opened)
    }
}
