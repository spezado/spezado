// SPDX-License-Identifier: AGPL-3.0-or-later
//! Opening balances: what a bank account, the cash or a debt amounts to on the day the books start, booked once
//! against the opening balance sheet (librotenado part 0004). Balances carry on between years by themselves.

use std::collections::BTreeMap;

use librotenado::{AccountId, Booking, BooksId, Chart, Date, Money, PartyId, Posting};
use spezado_core::Command;

use crate::{AppError, Workspace};

/// A sum so far and one more amount of its currency.
fn add(sum: Option<&Money>, amount: &Money) -> Result<Money, AppError> {
    let units = sum.map_or(Some(0), |s| Some(s.base_units())).and_then(|s| s.checked_add(amount.base_units()));
    units
        .and_then(|units| Money::new(units, amount.currency().clone()))
        .ok_or_else(|| AppError::Opening("the amounts are too large".into()))
}

impl Workspace {
    /// The first day of an owner's books: the date of its first configuration.
    pub fn books_start(&self, books: &BooksId, owner: &PartyId) -> Result<Option<Date>, AppError> {
        Ok(self.get(books)?.events().ordered().into_iter().find_map(|(_, event)| match &event.payload {
            librotenado::Payload::OwnerConfigured { configuration } if configuration.owner == *owner => {
                Some(configuration.from)
            }
            _ => None,
        }))
    }

    /// The opening balances booked for an owner, by account and currency – debit positive; reversed bookings are
    /// left out.
    pub fn opening_balances(&self, books: &BooksId, owner: &PartyId) -> Result<Vec<(AccountId, Money)>, AppError> {
        let opening = Chart::opening_balances_v0_1();
        let mut sums: BTreeMap<(AccountId, String), Money> = BTreeMap::new();
        for entry in self.get(books)?.journal(Some(owner), None, None) {
            let booking = &entry.booking;
            if entry.reversed_by.is_some()
                || entry.reverses.is_some()
                || !booking.postings.iter().any(|p| p.account == opening)
            {
                continue;
            }
            for posting in booking.postings.iter().filter(|p| p.account != opening && p.owner == *owner) {
                let key = (posting.account.clone(), posting.amount.currency().to_string());
                let sum = add(sums.get(&key), &posting.amount)?;
                sums.insert(key, sum);
            }
        }
        Ok(sums
            .into_iter()
            .filter(|(_, m)| m.base_units() != 0)
            .map(|((account, _), money)| (account, money))
            .collect())
    }

    /// The booking of opening balances: each line on its account – debit positive –, the opposite on the opening
    /// balance sheet, dated `date`. Lines of nothing are left out.
    pub fn opening_booking(
        &self,
        books: &BooksId,
        owner: &PartyId,
        date: Date,
        lines: &[(AccountId, Money)],
        text: &str,
    ) -> Result<Booking, AppError> {
        self.get(books)?;
        let opening = Chart::opening_balances_v0_1();
        let posting = |account: AccountId, amount: Money| Posting {
            owner: owner.clone(),
            account,
            amount,
            note: None,
            tax: None,
            exchange: None,
        };
        let mut postings = Vec::new();
        let mut totals: BTreeMap<String, Money> = BTreeMap::new();
        for (account, amount) in lines.iter().filter(|(_, amount)| amount.base_units() != 0) {
            if *account == opening {
                return Err(AppError::Opening("an opening balance belongs on another account".into()));
            }
            let currency = amount.currency().to_string();
            let total = add(totals.get(&currency), amount)?;
            totals.insert(currency, total);
            postings.push(posting(account.clone(), amount.clone()));
        }
        if postings.is_empty() {
            return Err(AppError::Opening("no opening balance".into()));
        }
        for total in totals.into_values().filter(|t| t.base_units() != 0) {
            postings.push(posting(opening.clone(), -total));
        }
        Ok(Booking { date, text: text.to_owned(), postings, documents: vec![], entries: vec![], transaction: None })
    }

    /// Books opening balances, as [`Workspace::opening_booking`] builds them.
    pub fn book_opening(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        date: Date,
        lines: &[(AccountId, Money)],
        text: &str,
    ) -> Result<librotenado::EventHash, AppError> {
        let booking = self.opening_booking(books, owner, date, lines, text)?;
        self.execute(books, Command::Book(booking))
    }
}
