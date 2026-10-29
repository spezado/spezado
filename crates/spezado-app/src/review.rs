// SPDX-License-Identifier: AGPL-3.0-or-later
//! The review list in words: each issue named by the bookings, documents, accounts and amounts it is about – not by
//! the hashes of events –, in German or in English.

use std::collections::BTreeMap;

use librotenado::{
    AccountProblem, BookingError, BookingProblem, BooksProblem, ChainProblem, ClosingProblem, Document, DocumentId,
    DocumentProblem, EntryReference, EventHash, EventSet, FilingProblem, Money, PartyId, PartyProblem, Payload,
    StatementProblem, TransactionId, TransactionProblem,
};
use spezado_core::{Issue, format_amount_in, format_date_in};

use crate::{AppError, Workspace, today};

impl Workspace {
    /// What needs review in a set of books, each issue in a sentence of a language (`de`, `en`) – accounts named as
    /// `owner` sees them.
    pub fn review_in(
        &self,
        books: &librotenado::BooksId,
        owner: Option<&PartyId>,
        language: &str,
    ) -> Result<Vec<String>, AppError> {
        let issues = self.review(books)?;
        let accounts = match owner {
            Some(owner) => self
                .account_choices(books, owner, today(), language)?
                .into_iter()
                .map(|c| (c.id.to_string(), c.number.map_or(c.name.clone(), |n| format!("{n} {}", c.name))))
                .collect(),
            None => BTreeMap::new(),
        };
        let words = Words {
            de: language.starts_with("de"),
            language,
            events: self.get(books)?.events(),
            documents: self.documents(books)?.into_iter().map(|(_, d)| (d.id.clone(), d)).collect(),
            accounts,
        };
        Ok(issues.iter().map(|issue| capitalised(words.issue(issue))).collect())
    }
}

fn capitalised(text: String) -> String {
    let mut chars = text.chars();
    chars.next().map_or(text.clone(), |first| first.to_uppercase().chain(chars).collect())
}

/// The first characters of an identity – enough to tell it apart on screen.
fn short(id: impl ToString) -> String {
    id.to_string().chars().take(8).collect()
}

/// What a sentence about the books needs to name things.
struct Words<'a> {
    de: bool,
    language: &'a str,
    events: &'a EventSet,
    documents: BTreeMap<DocumentId, Document>,
    accounts: BTreeMap<String, String>,
}

impl Words<'_> {
    /// The German or the English.
    fn say(&self, de: String, en: String) -> String {
        if self.de { de } else { en }
    }

    fn money(&self, money: &Money) -> String {
        format!("{} {}", format_amount_in(money, self.language), money.currency())
    }

    fn date(&self, date: librotenado::Date) -> String {
        format_date_in(date, self.language)
    }

    fn account(&self, account: &impl ToString) -> String {
        let id = account.to_string();
        self.accounts.get(&id).cloned().unwrap_or_else(|| short(id))
    }

    fn party(&self, party: &PartyId) -> String {
        let (organisations, persons) = self.events.parties();
        persons
            .get(party)
            .map(|p| p.name.clone())
            .or_else(|| organisations.get(party).map(|o| o.name.clone()))
            .unwrap_or_else(|| short(party))
    }

    /// A booking, a document, an account opened – what an event recorded.
    fn event(&self, hash: &EventHash) -> String {
        match self.events.get(hash).map(|event| &event.payload) {
            Some(Payload::BookingRecorded { booking }) => self.say(
                format!("Buchung vom {} „{}“", self.date(booking.date), booking.text),
                format!("the booking of {} “{}”", self.date(booking.date), booking.text),
            ),
            Some(Payload::BookingReversed { booking, .. }) => self.say(
                format!("Stornobuchung vom {} „{}“", self.date(booking.date), booking.text),
                format!("the reversal of {} “{}”", self.date(booking.date), booking.text),
            ),
            Some(Payload::DocumentRecorded { document }) => self.document(&document.id),
            Some(Payload::AccountOpened { account }) => self.say(
                format!("Eröffnung des Kontos {}", self.account(&account.id)),
                format!("the opening of account {}", self.account(&account.id)),
            ),
            Some(Payload::OwnerConfigured { .. }) => {
                self.say("Einrichtung der Bücher".into(), "the set-up of the books".into())
            }
            Some(Payload::PeriodClosed { closing }) => self.say(
                format!("Abschluss bis {}", self.date(closing.to)),
                format!("the closing up to {}", self.date(closing.to)),
            ),
            _ => self.say(format!("Ereignis {}", short(hash)), format!("event {}", short(hash))),
        }
    }

    /// The date a closing closes up to.
    fn closing(&self, hash: &EventHash) -> String {
        match self.events.get(hash).map(|event| &event.payload) {
            Some(Payload::PeriodClosed { closing }) => self.date(closing.to),
            _ => short(hash),
        }
    }

    fn document(&self, document: &DocumentId) -> String {
        let Some(found) = self.documents.get(document) else {
            return self.say(format!("Beleg {}", short(document)), format!("document {}", short(document)));
        };
        let label =
            found.number.clone().or_else(|| found.original.name.clone()).unwrap_or_else(|| self.date(found.date));
        if found.statement.is_some() {
            self.say(format!("Kontoauszug {label}"), format!("statement {label}"))
        } else if found.invoice.is_some() {
            self.say(format!("Rechnung {label}"), format!("invoice {label}"))
        } else {
            self.say(format!("Beleg {label}"), format!("document {label}"))
        }
    }

    /// An entry of a statement, with its date and amount.
    fn entry(&self, entry: &EntryReference) -> String {
        let statement = self.document(&entry.document);
        let found = self
            .events
            .statements()
            .into_iter()
            .find(|(id, _)| *id == entry.document)
            .and_then(|(_, data)| data.entries.get(entry.entry.checked_sub(1)? as usize).cloned());
        match found {
            Some(found) => self.say(
                format!(
                    "Umsatz {} aus {statement} ({}, {})",
                    entry.entry,
                    self.date(found.booked),
                    self.money(&found.amount)
                ),
                format!(
                    "entry {} of {statement} ({}, {})",
                    entry.entry,
                    self.date(found.booked),
                    self.money(&found.amount)
                ),
            ),
            None => self.say(
                format!("Umsatz {} aus {statement}", entry.entry),
                format!("entry {} of {statement}", entry.entry),
            ),
        }
    }

    /// A transaction shared with other books, by the booking that records it here.
    fn transaction(&self, transaction: &TransactionId) -> String {
        let booking = self.events.ordered().into_iter().find_map(|(_, event)| match &event.payload {
            Payload::BookingRecorded { booking }
                if booking.transaction.as_ref().is_some_and(|t| t.id == *transaction) =>
            {
                Some(booking.clone())
            }
            _ => None,
        });
        match booking {
            Some(booking) => self.say(
                format!("„{}“ vom {}", booking.text, self.date(booking.date)),
                format!("“{}” of {}", booking.text, self.date(booking.date)),
            ),
            None => short(transaction),
        }
    }

    fn booking_error(&self, error: &BookingError) -> String {
        match error {
            BookingError::TooFewPostings => {
                self.say("sie hat weniger als zwei Zeilen".into(), "it has fewer than two lines".into())
            }
            BookingError::ZeroPosting(i) => {
                self.say(format!("Zeile {} ist null", i + 1), format!("line {} is zero", i + 1))
            }
            BookingError::Unbalanced { difference, .. } => self.say(
                format!("Soll und Haben unterscheiden sich um {}", self.money(difference)),
                format!("debit and credit differ by {}", self.money(difference)),
            ),
            BookingError::Overflow => self.say("die Beträge sind zu groß".into(), "the amounts are too large".into()),
            BookingError::Exchange(i) => self.say(
                format!("Zeile {} ist nicht in die Hauswährung umgerechnet", i + 1),
                format!("line {} is not converted into the functional currency", i + 1),
            ),
            BookingError::UnbalancedFunctional { difference, .. } => self.say(
                format!("in der Hauswährung unterscheiden sich Soll und Haben um {}", self.money(difference)),
                format!("in the functional currency, debit and credit differ by {}", self.money(difference)),
            ),
        }
    }

    fn issue(&self, issue: &Issue) -> String {
        match issue {
            Issue::Chain(problem) => self.chain(problem),
            Issue::Books(problem) => self.books(problem),
            Issue::Account(problem) => self.account_problem(problem),
            Issue::Party(problem) => self.party_problem(problem),
            Issue::Document(problem) => self.document_problem(problem),
            Issue::Filing(problem) => self.filing(problem),
            Issue::Closing(problem) => self.closing_problem(problem),
            Issue::Transaction(problem) => self.transaction_problem(problem),
            Issue::NoChart => self
                .say("Die Bücher wählen keinen Kontenrahmen.".into(), "The books select no chart of accounts.".into()),
            Issue::SeveralCharts(charts) => self.say(
                format!("Die Bücher wählen mehr als einen Kontenrahmen: {}.", charts.join(", ")),
                format!("The books select more than one chart of accounts: {}.", charts.join(", ")),
            ),
            Issue::Booking(problem) => self.booking(problem),
            Issue::Statement(problem) => self.statement(problem),
        }
    }

    fn chain(&self, problem: &ChainProblem) -> String {
        match problem {
            ChainProblem::Fork { device, seq } => self.say(
                format!(
                    "Gerät {} hat zwei Ereignisse an Position {seq} – seine Daten widersprechen sich.",
                    short(device)
                ),
                format!("Device {} has two events at position {seq} – its data contradict each other.", short(device)),
            ),
            ChainProblem::Gap { device, from, to } => self.say(
                format!(
                    "Von Gerät {} fehlen die Ereignisse {from} bis {to} – vielleicht noch nicht übertragen.",
                    short(device)
                ),
                format!("Events {from} to {to} of device {} are missing – perhaps not transferred yet.", short(device)),
            ),
            ChainProblem::BrokenLink { device, event } => self.say(
                format!("{} von Gerät {} schließt nicht an dessen vorige Daten an.", self.event(event), short(device)),
                format!("{} of device {} does not follow the device's earlier data.", self.event(event), short(device)),
            ),
        }
    }

    fn books(&self, problem: &BooksProblem) -> String {
        match problem {
            BooksProblem::NotFounded => self.say(
                "Die Bücher sind nicht gegründet – vielleicht fehlen noch Daten.".into(),
                "The books have not been founded – perhaps data are still missing.".into(),
            ),
            BooksProblem::FoundedTwice { books, .. } => self.say(
                format!("Die Bücher werden ein zweites Mal gegründet, als {}.", short(books)),
                format!("The books are founded a second time, as {}.", short(books)),
            ),
        }
    }

    fn account_problem(&self, problem: &AccountProblem) -> String {
        match problem {
            AccountProblem::UnknownAccount { event, account } => self.say(
                format!("{} bucht auf ein unbekanntes Konto ({}).", self.event(event), short(account)),
                format!("{} posts on an unknown account ({}).", self.event(event), short(account)),
            ),
            AccountProblem::UnknownParent { account, .. } => self.say(
                format!("Konto {} gehört zu keinem bekannten Konto.", self.account(account)),
                format!("Account {} refines no known account.", self.account(account)),
            ),
            AccountProblem::KindMismatch { account, .. } => self.say(
                format!("Konto {} hat eine andere Art als das Konto, zu dem es gehört.", self.account(account)),
                format!("Account {} is of another kind than the account it refines.", self.account(account)),
            ),
            AccountProblem::Duplicate { account, .. } => self.say(
                format!("Konto {} wird ein zweites Mal eröffnet.", self.account(account)),
                format!("Account {} is opened a second time.", self.account(account)),
            ),
        }
    }

    fn party_problem(&self, problem: &PartyProblem) -> String {
        match problem {
            PartyProblem::NoOwner => self.say(
                "Die Bücher gehören niemandem – keine Organisation und keine Person ist als Eigentümer eingetragen."
                    .into(),
                "The books belong to no one – no organisation and no person is entered as owner.".into(),
            ),
            PartyProblem::UnknownOwner { event, owner } => self.say(
                format!("{} nennt einen Eigentümer, der nicht beschrieben ist ({}).", self.event(event), short(owner)),
                format!("{} names an owner who is not described ({}).", self.event(event), short(owner)),
            ),
            PartyProblem::UnknownOrganisation { person, organisation } => self.say(
                format!(
                    "{} hat eine Rolle in einer Organisation, die nicht beschrieben ist ({}).",
                    self.party(person),
                    short(organisation)
                ),
                format!(
                    "{} has a role in an organisation that is not described ({}).",
                    self.party(person),
                    short(organisation)
                ),
            ),
            PartyProblem::Shares { organisation, nominal } => self.say(
                format!(
                    "Die Anteile an {} ergeben nicht ihr Kapital von {}.",
                    self.party(organisation),
                    self.money(nominal)
                ),
                format!(
                    "The shares in {} do not add up to its capital of {}.",
                    self.party(organisation),
                    self.money(nominal)
                ),
            ),
            PartyProblem::NotConfigured { event, owner } => self.say(
                format!("{} bucht für {}, bevor die Bücher eingerichtet sind.", self.event(event), self.party(owner)),
                format!("{} posts for {} before the books are set up.", self.event(event), self.party(owner)),
            ),
            PartyProblem::SeriesTaken { owner, series, devices } => {
                let devices = devices.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                let series = if series.is_empty() { "ohne Kennbuchstaben".to_owned() } else { series.clone() };
                self.say(
                    format!(
                        "Die Rechnungsnummern-Serie {series} von {} ist mehreren Geräten zugeteilt ({devices}) – sie \
                         könnten dieselben Nummern vergeben. Einem Gerät eine andere Serie geben.",
                        self.party(owner)
                    ),
                    format!(
                        "The series {series} of invoice numbers of {} is assigned to several devices ({devices}) – they \
                         may write the same numbers. Give one device another series.",
                        self.party(owner)
                    ),
                )
            }
            PartyProblem::ConfigurationOfNonOwner { event, owner } => self.say(
                format!(
                    "{0} gilt {1} – {1} ist aber kein Eigentümer der Bücher.",
                    self.event(event),
                    self.party(owner)
                ),
                format!("{} is for {}, who is not an owner of the books.", self.event(event), self.party(owner)),
            ),
            PartyProblem::NotAnOwner { event, owner } => self.say(
                format!(
                    "{0} bucht für {1} – {1} ist aber kein Eigentümer der Bücher.",
                    self.event(event),
                    self.party(owner)
                ),
                format!("{} posts for {}, who is not an owner of the books.", self.event(event), self.party(owner)),
            ),
        }
    }

    fn document_problem(&self, problem: &DocumentProblem) -> String {
        match problem {
            DocumentProblem::UnknownDocument { event, .. } => self.say(
                format!("{} beruht auf einem Beleg, der nicht erfasst ist.", self.event(event)),
                format!("{} is based on a document that has not been recorded.", self.event(event)),
            ),
            DocumentProblem::Duplicate { document, .. } => self.say(
                format!("{} ist doppelt erfasst.", self.document(document)),
                format!("{} is recorded twice.", self.document(document)),
            ),
        }
    }

    fn filing(&self, problem: &FilingProblem) -> String {
        match problem {
            FilingProblem::Rejected(filing) => self.say(
                format!("Meldung {} wurde abgelehnt.", short(filing)),
                format!("Filing {} was rejected.", short(filing)),
            ),
            FilingProblem::Unknown(filing) => self.say(
                format!("Meldung {} wurde nie vorbereitet.", short(filing)),
                format!("Filing {} was never prepared.", short(filing)),
            ),
            FilingProblem::Duplicate(filing) => self.say(
                format!("Meldung {} wurde zweimal vorbereitet.", short(filing)),
                format!("Filing {} was prepared twice.", short(filing)),
            ),
        }
    }

    fn closing_problem(&self, problem: &ClosingProblem) -> String {
        match problem {
            ClosingProblem::StateMismatch { closing } => self.say(
                format!(
                    "Seit dem Abschluss bis {} sind Daten hinzugekommen, die er nicht kannte – seine Zahlen stimmen nicht mehr.",
                    self.closing(closing)
                ),
                format!(
                    "Data the closing up to {} did not know have arrived since – its figures no longer hold.",
                    self.closing(closing)
                ),
            ),
            ClosingProblem::LateBooking { event, closing } => self.say(
                format!("{} liegt im abgeschlossenen Zeitraum (abgeschlossen bis {}).", self.event(event), self.closing(closing)),
                format!("{} falls in the closed period (closed up to {}).", self.event(event), self.closing(closing)),
            ),
            ClosingProblem::LateConfiguration { event, closing } => self.say(
                format!("{} ändert die Einrichtung im abgeschlossenen Zeitraum (abgeschlossen bis {}).", self.event(event), self.closing(closing)),
                format!("{} changes the set-up in the closed period (closed up to {}).", self.event(event), self.closing(closing)),
            ),
        }
    }

    fn transaction_problem(&self, problem: &TransactionProblem) -> String {
        match problem {
            TransactionProblem::Duplicate { event, .. } => self.say(
                format!(
                    "{} erfasst einen gemeinsamen Geschäftsvorfall, den es in diesen Büchern schon gibt.",
                    self.event(event)
                ),
                format!("{} records a shared transaction these books hold already.", self.event(event)),
            ),
            TransactionProblem::Missing { event, other, .. } => self.say(
                format!(
                    "{} gehört zu einem Geschäftsvorfall mit den Büchern {}, dort fehlt die Gegenbuchung.",
                    self.event(event),
                    short(other)
                ),
                format!(
                    "{} belongs to a transaction with books {}, which lack their side of it.",
                    self.event(event),
                    short(other)
                ),
            ),
            TransactionProblem::BooksMismatch { transaction } => self.say(
                format!(
                    "Die Buchungen des gemeinsamen Geschäftsvorfalls {} nennen nicht dieselben Bücher.",
                    self.transaction(transaction)
                ),
                format!(
                    "The bookings of the shared transaction {} do not name the same books.",
                    self.transaction(transaction)
                ),
            ),
            TransactionProblem::DateMismatch { transaction } => self.say(
                format!(
                    "Die Buchungen des gemeinsamen Geschäftsvorfalls {} haben verschiedene Daten.",
                    self.transaction(transaction)
                ),
                format!(
                    "The bookings of the shared transaction {} have different dates.",
                    self.transaction(transaction)
                ),
            ),
            TransactionProblem::ReversalMismatch { transaction } => self.say(
                format!(
                    "Der gemeinsame Geschäftsvorfall {} ist nur in einem Teil der Bücher storniert.",
                    self.transaction(transaction)
                ),
                format!(
                    "The shared transaction {} is reversed in some of the books only.",
                    self.transaction(transaction)
                ),
            ),
        }
    }

    fn booking(&self, problem: &BookingProblem) -> String {
        match problem {
            BookingProblem::Invalid { event, error } => self.say(
                format!("{} ist ungültig: {}.", self.event(event), self.booking_error(error)),
                format!("{} is not valid: {}.", self.event(event), self.booking_error(error)),
            ),
            BookingProblem::UnknownReversal { event, .. } => self.say(
                format!("{} storniert etwas, das keine erfasste Buchung ist.", self.event(event)),
                format!("{} reverses something that is not a recorded booking.", self.event(event)),
            ),
            BookingProblem::DoubleReversal { event, reverses } => self.say(
                format!("{} wird zweimal storniert, zuletzt durch {}.", self.event(reverses), self.event(event)),
                format!("{} is reversed twice, the second time by {}.", self.event(reverses), self.event(event)),
            ),
            BookingProblem::ReversalMismatch { event, reverses } => self.say(
                format!("{} hebt {} nicht genau auf.", self.event(event), self.event(reverses)),
                format!("{} does not cancel {} exactly.", self.event(event), self.event(reverses)),
            ),
        }
    }

    fn statement(&self, problem: &StatementProblem) -> String {
        match problem {
            StatementProblem::UnknownEntry { event, .. } => self.say(
                format!("{} bezieht sich auf einen Umsatz, den kein erfasster Kontoauszug enthält.", self.event(event)),
                format!("{} refers to an entry that no recorded statement holds.", self.event(event)),
            ),
            StatementProblem::BookedTwice { event, entry } => self.say(
                format!("{} ist doppelt gebucht, zuletzt mit {}.", self.entry(entry), self.event(event)),
                format!("{} is booked twice, the second time by {}.", self.entry(entry), self.event(event)),
            ),
            StatementProblem::ShownTwice { first, again } => self.say(
                format!("{} zeigt denselben Umsatz wie {} – er wird nur einmal gebucht.", self.entry(again), self.entry(first)),
                format!("{} shows the same entry as {} – it is booked once only.", self.entry(again), self.entry(first)),
            ),
            StatementProblem::BalanceMismatch { document, closing, booked } => self.say(
                format!(
                    "{} endet mit einem Saldo von {}, die Bücher zeigen {} – fehlt ein Anfangsbestand oder eine Buchung?",
                    self.document(document),
                    self.money(closing),
                    self.money(booked)
                ),
                format!(
                    "{} closes at {}, the books show {} – is an opening balance or a booking missing?",
                    self.document(document),
                    self.money(closing),
                    self.money(booked)
                ),
            ),
        }
    }
}
