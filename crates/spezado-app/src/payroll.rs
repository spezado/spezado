// SPDX-License-Identifier: AGPL-3.0-or-later
//! Payroll (librotenado spec part 0013): employments, and a payslip per month – computed by librotenado's rules,
//! recorded, and booked.

use std::collections::BTreeMap;

use librotenado::payroll::{Employment, EmploymentId, GermanEmployment, PayItem, Payslip};
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
        Ok(Payslip { id: new_id(), employment: employment.clone(), from, to, paid: last, items, figures })
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
        let booking = Booking {
            basis: None,
            attributed: None,
            date: payslip.paid,
            text: format!("Lohn {:02}/{} {name}", payslip.to.month(), payslip.to.year()),
            postings: payslip.postings(&employment.owner, minor),
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
