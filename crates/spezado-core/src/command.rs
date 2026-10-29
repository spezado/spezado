// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commands: what someone wants to happen in the books, checked before it becomes an event.

use std::collections::BTreeMap;

use librotenado::{
    Account, Booking, BookingError, BooksId, Chart, Closing, Configuration, Date, Document, DocumentId, EventHash,
    Invoicing, Organisation, PartyId, Payload, Person, Series,
};

use crate::Books;

/// What someone wants to happen in a set of books – close to the events of the protocol. Workflows that span
/// several commands (setting up books, splitting a bill) are built from these in the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Founds the books under an identity; the first command of every set of books.
    Found(BooksId),
    /// Describes an organisation, or changes its description.
    DescribeOrganisation(Organisation),
    /// Describes a person, or changes their description.
    DescribePerson(Person),
    /// Makes a described organisation or person an owner of the books.
    AddOwner(PartyId),
    /// Records how an owner keeps its books from a date on.
    Configure(Configuration),
    /// Selects the harmonised chart of accounts, as `name/version`.
    SelectChart(String),
    /// Opens an account of the books' own, refining a known one.
    OpenAccount(Account),
    /// Records a booking.
    Book(Booking),
    /// Reverses a recorded booking on a date, with a text.
    Reverse {
        /// The event that recorded the booking.
        event: EventHash,
        /// The date of the reversal.
        date: Date,
        /// Why.
        text: String,
    },
    /// Records a document; its original is stored by its hash, outside the events.
    RecordDocument(Document),
    /// Records how an owner writes invoices.
    ConfigureInvoicing(Invoicing),
    /// Gives a device a series of invoice numbers of an owner.
    AssignSeries(Series),
    /// Closes an owner's books up to a date.
    Close {
        /// The owner.
        owner: PartyId,
        /// The last day closed.
        to: Date,
        /// The software the figures were derived with, name → version.
        software: BTreeMap<String, String>,
    },
}

/// Why a command was refused. Nothing is written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// The books are founded already.
    #[error("the books are founded already, as {0}")]
    AlreadyFounded(BooksId),
    /// The books are not founded yet.
    #[error("the books are not founded yet")]
    NotFounded,
    /// The organisation or person is not described in the books.
    #[error("{0} is not described in the books")]
    UnknownParty(PartyId),
    /// The organisation or person is an owner already.
    #[error("{0} is an owner of the books already")]
    AlreadyOwner(PartyId),
    /// The organisation or person is not an owner of the books.
    #[error("{0} is not an owner of the books")]
    NotAnOwner(PartyId),
    /// The owner has no configuration on the date.
    #[error("{owner} is not configured on {date}")]
    NotConfigured {
        /// The owner.
        owner: PartyId,
        /// The date.
        date: Date,
    },
    /// The date lies in a closed period of the owner.
    #[error("the books of {owner} are closed up to {closed}")]
    Closed {
        /// The owner.
        owner: PartyId,
        /// The last day closed.
        closed: Date,
    },
    /// The books select a chart already.
    #[error("the books use the chart {0} already")]
    ChartSelected(String),
    /// The command needs a chart of accounts, and the books select none.
    #[error("the books select no chart of accounts")]
    NoChart,
    /// The account is neither in the chart nor opened.
    #[error("account {0} is neither in the chart nor opened")]
    UnknownAccount(librotenado::AccountId),
    /// An account of that identity exists already.
    #[error("account {0} exists already")]
    AccountExists(librotenado::AccountId),
    /// The booking is not valid.
    #[error(transparent)]
    InvalidBooking(#[from] BookingError),
    /// A posting in a foreign currency without an exchange into the owner's functional currency.
    #[error("a posting of {owner} in {currency} needs an exchange into {functional}")]
    NoExchange {
        /// The owner.
        owner: PartyId,
        /// The posting's currency.
        currency: librotenado::Currency,
        /// The owner's functional currency.
        functional: librotenado::Currency,
    },
    /// The booking rests on a document that is not recorded.
    #[error("document {0} is not recorded")]
    UnknownDocument(DocumentId),
    /// A document of that identity is recorded already.
    #[error("document {0} is recorded already")]
    DocumentExists(DocumentId),
    /// The booking names an entry that no recorded statement has.
    #[error("entry {} of {} is not an entry of a recorded statement", .0.entry, .0.document)]
    UnknownEntry(librotenado::EntryReference),
    /// The booking names an entry that is booked already.
    #[error("entry {} of {} is booked already, by event {by}", .entry.entry, .entry.document)]
    EntryBooked {
        /// The entry.
        entry: librotenado::EntryReference,
        /// The booking's event.
        by: EventHash,
    },
    /// The event is not a recorded booking.
    #[error("event {0} is not a recorded booking")]
    NotABooking(EventHash),
    /// The booking is reversed already.
    #[error("the booking of event {booking} is reversed already, by {by}")]
    AlreadyReversed {
        /// The booking's event.
        booking: EventHash,
        /// The reversal's event.
        by: EventHash,
    },
    /// A closing must close more than the last one.
    #[error("the books of {owner} are closed up to {closed} already")]
    ClosedAlready {
        /// The owner.
        owner: PartyId,
        /// The last day closed.
        closed: Date,
    },
}

impl Books {
    /// Checks `command` against the books and the harmonised chart `chart`; returns the payload of the event to
    /// write. The books do not change: the application stamps the event, stores it and inserts it.
    pub fn decide(&self, command: Command, chart: &Chart) -> Result<Payload, CommandError> {
        let events = self.events();
        if let Command::Found(books) = command {
            return match self.id() {
                Some(founded) => Err(CommandError::AlreadyFounded(founded.clone())),
                None => Ok(Payload::BooksFounded { books }),
            };
        }
        if self.id().is_none() {
            return Err(CommandError::NotFounded);
        }
        Ok(match command {
            Command::Found(_) => unreachable!("handled above"),
            Command::DescribeOrganisation(organisation) => Payload::OrganisationDescribed { organisation },
            Command::DescribePerson(person) => Payload::PersonDescribed { person },
            Command::AddOwner(owner) => {
                let (organisations, persons) = events.parties();
                if !organisations.contains_key(&owner) && !persons.contains_key(&owner) {
                    return Err(CommandError::UnknownParty(owner));
                }
                if self.is_owner(&owner) {
                    return Err(CommandError::AlreadyOwner(owner));
                }
                Payload::OwnerAdded { owner }
            }
            Command::Configure(configuration) => {
                self.owner_open(&configuration.owner, configuration.from)?;
                Payload::OwnerConfigured { configuration }
            }
            Command::SelectChart(chart) => {
                if let Some(selected) = events.selected_charts().first() {
                    return Err(CommandError::ChartSelected((*selected).to_owned()));
                }
                Payload::ChartSelected { chart }
            }
            Command::OpenAccount(account) => {
                self.has_chart()?;
                if self.knows_account(chart, &account.id) {
                    return Err(CommandError::AccountExists(account.id));
                }
                match &account.parent {
                    Some(parent) if self.knows_account(chart, parent) => {}
                    Some(parent) => return Err(CommandError::UnknownAccount(parent.clone())),
                    None => return Err(CommandError::UnknownAccount(account.id)),
                }
                Payload::AccountOpened { account }
            }
            Command::Book(booking) => {
                self.check_booking(&booking, chart)?;
                Payload::BookingRecorded { booking }
            }
            Command::Reverse { event, date, text } => {
                let Some(Payload::BookingRecorded { booking }) = events.get(&event).map(|e| &e.payload) else {
                    return Err(CommandError::NotABooking(event));
                };
                if let Some(by) = self.reversals().get(&event) {
                    return Err(CommandError::AlreadyReversed { booking: event, by: *by });
                }
                let reversal = booking.reversed(date, text);
                // A reversal names the entries of what it reverses; they are booked, by it.
                let mut checked = reversal.clone();
                checked.entries.clear();
                self.check_booking(&checked, chart)?;
                Payload::BookingReversed { reverses: event, booking: reversal }
            }
            Command::RecordDocument(document) => {
                if self.document_recorded(&document.id) {
                    return Err(CommandError::DocumentExists(document.id));
                }
                Payload::DocumentRecorded { document }
            }
            Command::ConfigureInvoicing(invoicing) => {
                if !self.is_owner(&invoicing.owner) {
                    return Err(CommandError::NotAnOwner(invoicing.owner));
                }
                Payload::InvoicingConfigured { invoicing }
            }
            Command::AssignSeries(series) => {
                if !self.is_owner(&series.owner) {
                    return Err(CommandError::NotAnOwner(series.owner));
                }
                Payload::SeriesAssigned { series }
            }
            Command::Close { owner, to, software } => {
                if !self.is_owner(&owner) {
                    return Err(CommandError::NotAnOwner(owner));
                }
                if let Some(closed) = events.closed_until(&owner).filter(|closed| to <= *closed) {
                    return Err(CommandError::ClosedAlready { owner, closed });
                }
                Payload::PeriodClosed { closing: Closing { owner, to, state: events.state(), software } }
            }
        })
    }

    fn has_chart(&self) -> Result<(), CommandError> {
        if self.events().selected_charts().is_empty() { Err(CommandError::NoChart) } else { Ok(()) }
    }

    fn document_recorded(&self, id: &DocumentId) -> bool {
        self.events()
            .ordered()
            .iter()
            .any(|(_, event)| matches!(&event.payload, Payload::DocumentRecorded { document } if document.id == *id))
    }

    /// `owner` is an owner, and `date` lies after the last day closed in their books.
    fn owner_open(&self, owner: &PartyId, date: Date) -> Result<(), CommandError> {
        if !self.is_owner(owner) {
            return Err(CommandError::NotAnOwner(owner.clone()));
        }
        match self.events().closed_until(owner) {
            Some(closed) if date <= closed => Err(CommandError::Closed { owner: owner.clone(), closed }),
            _ => Ok(()),
        }
    }

    /// A booking may be written: valid, on known accounts, in the open period of configured owners, in their
    /// functional currency or converted into it, resting on recorded documents.
    fn check_booking(&self, booking: &Booking, chart: &Chart) -> Result<(), CommandError> {
        booking.validate()?;
        self.has_chart()?;
        for posting in &booking.postings {
            self.owner_open(&posting.owner, booking.date)?;
            let Some(configuration) = self.events().configuration(&posting.owner, booking.date) else {
                return Err(CommandError::NotConfigured { owner: posting.owner.clone(), date: booking.date });
            };
            if posting.amount.currency() != &configuration.currency && posting.exchange.is_none() {
                return Err(CommandError::NoExchange {
                    owner: posting.owner.clone(),
                    currency: posting.amount.currency().clone(),
                    functional: configuration.currency.clone(),
                });
            }
            if !self.knows_account(chart, &posting.account) {
                return Err(CommandError::UnknownAccount(posting.account.clone()));
            }
        }
        if let Some(document) = booking.documents.iter().find(|d| !self.document_recorded(d)) {
            return Err(CommandError::UnknownDocument(document.clone()));
        }
        let statements: std::collections::BTreeMap<_, _> = self.events().statements().into_iter().collect();
        let booked = self.events().booked_entries();
        for entry in &booking.entries {
            if !statements.get(&entry.document).is_some_and(|s| (entry.entry as usize) <= s.entries.len()) {
                return Err(CommandError::UnknownEntry(entry.clone()));
            }
            if let Some(by) = booked.get(entry) {
                return Err(CommandError::EntryBooked { entry: entry.clone(), by: *by });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use librotenado::{ChartTranslation, Clock, DeviceId, Event, Money, Posting};

    use super::*;
    use librotenado::BookingProblem;

    use crate::Issue;

    const OWNER: &str = "0192f1e4-0000-7000-8000-0000000000a1";

    /// Books on one device that writes what it decides.
    struct Desk {
        books: Books,
        chart: Chart,
        device: DeviceId,
        clock: Clock,
        ms: u64,
    }

    impl Desk {
        /// Books founded for one person, configured from 2026, on the harmonised chart.
        fn new() -> Self {
            let device: DeviceId = "laptop".parse().unwrap();
            let clock = Clock::new(device.clone());
            let mut desk = Desk { books: Books::new(), chart: Chart::v0_1(), device, clock, ms: 1_000 };
            desk.run(Command::Found("0192f1e4-0000-7000-8000-0000000000b1".parse().unwrap())).unwrap();
            let person = serde_json::json!({"id": OWNER, "name": "Ada"});
            desk.run(Command::DescribePerson(serde_json::from_value(person).unwrap())).unwrap();
            desk.run(Command::AddOwner(owner())).unwrap();
            desk.run(Command::Configure(configuration("2026-01-01"))).unwrap();
            desk.run(Command::SelectChart("librotenado/0.1".into())).unwrap();
            desk
        }

        fn run(&mut self, command: Command) -> Result<EventHash, CommandError> {
            let payload = self.books.decide(command, &self.chart)?;
            self.ms += 1;
            let (seq, prev) = self.books.events().chain_head(&self.device);
            Ok(self.books.insert(Event { time: self.clock.now(self.ms), seq, prev, payload }).0)
        }
    }

    fn owner() -> PartyId {
        OWNER.parse().unwrap()
    }

    fn configuration(from: &str) -> Configuration {
        serde_json::from_value(serde_json::json!({
            "owner": OWNER, "from": from, "currency": "EUR", "year_start": 1, "accounting": "de-cash-basis",
        }))
        .unwrap()
    }

    /// A booking from the bank account (SKR 03 1200) onto office supplies (SKR 03 4930).
    fn booking(date: &str, cents: i128) -> Booking {
        let skr03 = ChartTranslation::de_skr03();
        let posting = |number: &str, units: i128| Posting {
            owner: owner(),
            account: skr03.accounts[number].clone(),
            amount: Money::new(units, "EUR".parse().unwrap()).unwrap(),
            note: None,
            tax: None,
            exchange: None,
        };
        Booking {
            date: date.parse().unwrap(),
            text: "office paper".into(),
            postings: vec![posting("4930", cents), posting("1200", -cents)],
            documents: vec![],
            entries: vec![],
            transaction: None,
        }
    }

    #[test]
    fn set_up_books_have_nothing_to_review() {
        let mut desk = Desk::new();
        desk.run(Command::Book(booking("2026-10-28", 1240))).unwrap();
        assert_eq!(desk.books.review(&desk.chart), vec![]);
        assert_eq!(desk.books.journal(Some(&owner()), None, None).len(), 1);
    }

    #[test]
    fn books_are_founded_once_and_first() {
        let mut desk = Desk { books: Books::new(), ..Desk::new() };
        assert_eq!(desk.run(Command::AddOwner(owner())), Err(CommandError::NotFounded));
        desk.run(Command::Found("0192f1e4-0000-7000-8000-0000000000b2".parse().unwrap())).unwrap();
        let again = desk.run(Command::Found("0192f1e4-0000-7000-8000-0000000000b3".parse().unwrap()));
        assert!(matches!(again, Err(CommandError::AlreadyFounded(_))));
        assert_eq!(desk.run(Command::AddOwner(owner())), Err(CommandError::UnknownParty(owner())));
    }

    #[test]
    fn a_booking_needs_a_configured_owner_and_known_accounts() {
        let mut desk = Desk::new();
        let early = desk.run(Command::Book(booking("2025-12-31", 1240)));
        assert_eq!(early, Err(CommandError::NotConfigured { owner: owner(), date: "2025-12-31".parse().unwrap() }));
        let mut unknown = booking("2026-10-28", 1240);
        unknown.postings[0].account = "0192f1e4-0000-7000-8000-00000000dead".parse().unwrap();
        assert!(matches!(desk.run(Command::Book(unknown)), Err(CommandError::UnknownAccount(_))));
        let mut foreign = booking("2026-10-28", 1240);
        for posting in &mut foreign.postings {
            posting.amount = Money::new(posting.amount.base_units(), "USD".parse().unwrap()).unwrap();
        }
        assert!(matches!(desk.run(Command::Book(foreign)), Err(CommandError::NoExchange { .. })));
        let mut unbalanced = booking("2026-10-28", 1240);
        unbalanced.postings.pop();
        assert!(matches!(desk.run(Command::Book(unbalanced)), Err(CommandError::InvalidBooking(_))));
        let mut stranger = booking("2026-10-28", 1240);
        for posting in &mut stranger.postings {
            posting.owner = "0192f1e4-0000-7000-8000-0000000000a2".parse().unwrap();
        }
        assert!(matches!(desk.run(Command::Book(stranger)), Err(CommandError::NotAnOwner(_))));
    }

    #[test]
    fn a_booking_is_reversed_once() {
        let mut desk = Desk::new();
        let recorded = desk.run(Command::Book(booking("2026-10-28", 1240))).unwrap();
        let reverse = |event| Command::Reverse { event, date: "2026-10-29".parse().unwrap(), text: "wrong".into() };
        let reversal = desk.run(reverse(recorded)).unwrap();
        assert_eq!(desk.run(reverse(recorded)), Err(CommandError::AlreadyReversed { booking: recorded, by: reversal }));
        assert_eq!(desk.run(reverse(reversal)), Err(CommandError::NotABooking(reversal)));
        let journal = desk.books.journal(None, None, None);
        assert_eq!(journal[0].reversed_by, Some(reversal));
        assert_eq!(journal[1].reverses, Some(recorded));
        assert_eq!(desk.books.review(&desk.chart), vec![]);
    }

    #[test]
    fn reversals_from_elsewhere_are_reviewed() {
        let mut desk = Desk::new();
        let recorded = desk.run(Command::Book(booking("2026-10-28", 1240))).unwrap();
        // Another device, which does not check, writes a second and a wrong reversal.
        let mut write = |reverses: EventHash, booking: Booking| {
            let payload = Payload::BookingReversed { reverses, booking };
            desk.ms += 1;
            let (seq, prev) = desk.books.events().chain_head(&desk.device);
            desk.books.insert(Event { time: desk.clock.now(desk.ms), seq, prev, payload }).0
        };
        let first = write(recorded, booking("2026-10-28", -1240));
        let second = write(recorded, booking("2026-10-28", -1240));
        let wrong = write(recorded, booking("2026-10-28", -1000));
        let nothing = write(first, booking("2026-10-28", 1240));
        assert_eq!(
            desk.books.review(&desk.chart),
            vec![
                Issue::Booking(BookingProblem::DoubleReversal { event: second, reverses: recorded }),
                Issue::Booking(BookingProblem::DoubleReversal { event: wrong, reverses: recorded }),
                Issue::Booking(BookingProblem::ReversalMismatch { event: wrong, reverses: recorded }),
                Issue::Booking(BookingProblem::UnknownReversal { event: nothing, reverses: first }),
            ]
        );
    }

    #[test]
    fn a_closed_period_takes_no_bookings_and_no_configuration() {
        let mut desk = Desk::new();
        desk.run(Command::Book(booking("2026-03-31", 1240))).unwrap();
        let close = |to: &str| Command::Close { owner: owner(), to: to.parse().unwrap(), software: BTreeMap::new() };
        desk.run(close("2026-03-31")).unwrap();
        let closed = "2026-03-31".parse().unwrap();
        assert_eq!(
            desk.run(Command::Book(booking("2026-03-15", 100))),
            Err(CommandError::Closed { owner: owner(), closed })
        );
        assert_eq!(
            desk.run(Command::Configure(configuration("2026-03-01"))),
            Err(CommandError::Closed { owner: owner(), closed })
        );
        assert_eq!(desk.run(close("2026-02-28")), Err(CommandError::ClosedAlready { owner: owner(), closed }));
        desk.run(Command::Book(booking("2026-04-01", 100))).unwrap();
        desk.run(close("2026-06-30")).unwrap();
        assert_eq!(desk.books.review(&desk.chart), vec![]);
    }
}
