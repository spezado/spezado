// SPDX-License-Identifier: AGPL-3.0-or-later
//! A set of books and what can be read from it.

use std::collections::BTreeMap;

use librotenado::{
    AccountId, AccountKind, AccountProblem, Booking, BookingProblem, BooksId, BooksProblem, ChainProblem, Chart,
    ClosingProblem, Date, DocumentProblem, Event, EventHash, EventSet, FilingProblem, ObjectError, PartyId,
    PartyProblem, Payload, StatementProblem, TransactionProblem,
};

/// Something in the books a person has to look at. Nothing on this list is resolved silently; it is resolved by a
/// new event.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Issue {
    /// A device's chain of events is not clean (spec part 0002).
    #[error("{0}")]
    Chain(ChainProblem),
    /// The identity of the books (spec part 0002).
    #[error("{0}")]
    Books(BooksProblem),
    /// The accounts (spec part 0002).
    #[error("{0}")]
    Account(AccountProblem),
    /// The parties and their configuration (spec part 0008).
    #[error("{0}")]
    Party(PartyProblem),
    /// The documents (spec part 0005).
    #[error("{0}")]
    Document(DocumentProblem),
    /// The filings (spec part 0007).
    #[error("{0}")]
    Filing(FilingProblem),
    /// A closed period (spec part 0002).
    #[error("{0}")]
    Closing(ClosingProblem),
    /// A transaction shared with other books (spec part 0002); found only where those books are held, too.
    #[error("{0}")]
    Transaction(TransactionProblem),
    /// The books select no chart of accounts.
    #[error("the books select no chart of accounts")]
    NoChart,
    /// The books select more than one chart of accounts.
    #[error("the books select more than one chart of accounts: {}", .0.join(", "))]
    SeveralCharts(Vec<String>),
    /// The bookings and reversals (spec part 0002).
    #[error("{0}")]
    Booking(BookingProblem),
    /// Bank statements and the bookings of their entries (spec part 0005).
    #[error("{0}")]
    Statement(StatementProblem),
}

/// A booking as the journal shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    /// The event that recorded it.
    pub event: EventHash,
    /// The booking.
    pub booking: Booking,
    /// For a reversal: the event it reverses.
    pub reverses: Option<EventHash>,
    /// The event that reversed it, if any.
    pub reversed_by: Option<EventHash>,
}

/// A set of books: its events, and what follows from them.
#[derive(Debug, Clone, Default)]
pub struct Books {
    events: EventSet,
}

impl Books {
    /// Empty books, not yet founded.
    pub fn new() -> Self {
        Self::default()
    }

    /// The books of a set of events.
    pub fn from_events(events: EventSet) -> Self {
        Books { events }
    }

    /// The events.
    pub fn events(&self) -> &EventSet {
        &self.events
    }

    /// Adds an event; returns its hash and whether it was new.
    pub fn insert(&mut self, event: Event) -> (EventHash, bool) {
        self.events.insert(event)
    }

    /// Adds an event received as JSON from elsewhere; returns its hash and whether it was new.
    pub fn receive(&mut self, json: &[u8]) -> Result<(EventHash, bool), ObjectError> {
        self.events.receive(json)
    }

    /// The identity of the books, once founded.
    pub fn id(&self) -> Option<&BooksId> {
        self.events.books()
    }

    /// Whether `owner` is an owner of the books.
    pub fn is_owner(&self, owner: &PartyId) -> bool {
        self.events.owners().contains(&owner)
    }

    /// Whether an account is in `chart` or was opened in the books.
    pub fn knows_account(&self, chart: &Chart, account: &AccountId) -> bool {
        chart.get(account).is_some()
            || self.events.ordered().iter().any(|(_, event)| {
                matches!(&event.payload, Payload::AccountOpened { account: opened } if opened.id == *account)
            })
    }

    /// Per reversed booking: the event that reversed it first, in the agreed order.
    pub fn reversals(&self) -> BTreeMap<EventHash, EventHash> {
        let mut reversals = BTreeMap::new();
        for (hash, event) in self.events.ordered() {
            if let Payload::BookingReversed { reverses, .. } = &event.payload {
                reversals.entry(*reverses).or_insert(hash);
            }
        }
        reversals
    }

    /// The bookings and reversals in which `owner` has postings – all owners' if none is given –, dated from `from`
    /// to `to` inclusive, by date and then in the agreed order of events.
    pub fn journal(&self, owner: Option<&PartyId>, from: Option<Date>, to: Option<Date>) -> Vec<JournalEntry> {
        let reversals = self.reversals();
        let mut entries: Vec<JournalEntry> = self
            .events
            .ordered()
            .into_iter()
            .filter_map(|(hash, event)| {
                let (booking, reverses) = match &event.payload {
                    Payload::BookingRecorded { booking } => (booking, None),
                    Payload::BookingReversed { reverses, booking } => (booking, Some(*reverses)),
                    _ => return None,
                };
                let concerns = owner.is_none_or(|owner| booking.postings.iter().any(|p| p.owner == *owner));
                let within = from.is_none_or(|from| booking.date >= from) && to.is_none_or(|to| booking.date <= to);
                (concerns && within).then(|| JournalEntry {
                    event: hash,
                    booking: booking.clone(),
                    reverses,
                    reversed_by: reversals.get(&hash).copied(),
                })
            })
            .collect();
        // A stable sort keeps the agreed order within a day.
        entries.sort_by_key(|entry| entry.booking.date);
        entries
    }

    /// The income and the expenses of `owner` in the bookings dated from `from` to `to`, in base units of the
    /// functional currency, both positive as earned and spent: postings on accounts of income and of expense only.
    pub fn income_and_expenses(&self, chart: &Chart, owner: &PartyId, from: Date, to: Date) -> (i128, i128) {
        let kinds = self.events.account_kinds(chart);
        let (mut income, mut expenses) = (0i128, 0i128);
        for entry in self.journal(Some(owner), Some(from), Some(to)) {
            for posting in entry.booking.postings.iter().filter(|p| p.owner == *owner) {
                let units = posting.functional().base_units();
                match kinds.get(&posting.account) {
                    Some(AccountKind::Income) => income = income.saturating_sub(units),
                    Some(AccountKind::Expense) => expenses = expenses.saturating_add(units),
                    _ => {}
                }
            }
        }
        (income, expenses)
    }

    /// Everything in the books that needs review, against the harmonised chart `chart`.
    pub fn review(&self, chart: &Chart) -> Vec<Issue> {
        let events = &self.events;
        let mut issues: Vec<Issue> = Vec::new();
        issues.extend(events.verify().into_iter().map(Issue::Chain));
        issues.extend(events.review_books().into_iter().map(Issue::Books));
        match events.selected_charts().as_slice() {
            [] if !events.is_empty() => issues.push(Issue::NoChart),
            [] | [_] => {}
            several => issues.push(Issue::SeveralCharts(several.iter().map(|c| (*c).to_owned()).collect())),
        }
        issues.extend(events.review_accounts(chart).into_iter().map(Issue::Account));
        issues.extend(events.review_parties().into_iter().map(Issue::Party));
        issues.extend(events.review_documents().into_iter().map(Issue::Document));
        issues.extend(events.review_filings().into_iter().map(Issue::Filing));
        issues.extend(events.review_closings().into_iter().map(Issue::Closing));
        issues.extend(events.review_bookings().into_iter().map(Issue::Booking));
        issues.extend(events.review_statements().into_iter().map(Issue::Statement));
        issues
    }
}
