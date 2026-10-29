// SPDX-License-Identifier: AGPL-3.0-or-later
//! The bank: statements read from files (camt.053, MT940, CSV), the bank account of each IBAN, the entries not booked
//! yet with a proposal for each, and the placeholder for fetching statements from the bank directly (FinTS).

use std::collections::{BTreeMap, BTreeSet};

use librotenado::{
    Account, AccountId, AccountKind, Booking, BooksId, CurrencyTable, Document, DocumentId, EntryReference, EventHash,
    InvoiceSide, Original, PartyId, Payload, ProposalAccounts, StatementEntry, StatementProblem, TaxCode,
    propose_booking, read_statements,
};
use spezado_core::Command;

use crate::{AppError, SimpleEntry, Workspace, new_id, today};

/// What reading a file of statements did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedStatements {
    /// The statements recorded, as documents.
    pub documents: Vec<DocumentId>,
    /// Statements left out: the same file was read before.
    pub known: usize,
    /// Entries new to the books.
    pub entries: usize,
    /// Entries shown by an earlier statement as well, not to be booked again.
    pub shown_before: usize,
}

/// Where a proposal for an entry comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// An invoice recorded but not booked: its amount due and its number or party match.
    Invoice {
        /// The invoice.
        document: DocumentId,
        /// Its number.
        number: Option<String>,
    },
    /// The same counterparty as an entry booked before: its account and tax code.
    History(EventHash),
    /// A payment booked before its statement came – the same amount on the bank, tied to no entry: it is reversed and
    /// booked again on the statement's account, naming the entry, on its own date.
    Booked(EventHash),
}

/// A booking proposed for an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Where it comes from.
    pub source: Source,
    /// The booking, naming the entry and the statement.
    pub booking: Booking,
}

/// An entry of a recorded statement that is not booked yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEntry {
    /// The entry.
    pub reference: EntryReference,
    /// The IBAN of its account.
    pub iban: String,
    /// The account of the books with that IBAN.
    pub account: Option<AccountId>,
    /// The entry itself.
    pub entry: StatementEntry,
    /// A proposal, if there is one.
    pub suggestion: Option<Suggestion>,
}

impl OpenEntry {
    /// The other party's name, the purpose: what the entry says.
    pub fn text(&self) -> String {
        let detail = self.entry.details.first();
        let name = detail.and_then(|d| d.counterparty.as_ref()).and_then(|c| c.name.clone());
        let purpose = detail.and_then(|d| d.purpose.clone());
        match (name, purpose) {
            (Some(name), Some(purpose)) => format!("{name} – {purpose}"),
            (Some(text), None) | (None, Some(text)) => text,
            (None, None) => self.entry.code.clone().unwrap_or_default(),
        }
    }
}

/// Days since 1970-01-01.
fn days(date: librotenado::Date) -> i64 {
    chrono::NaiveDate::from_ymd_opt(i32::from(date.year()), u32::from(date.month()), u32::from(date.day()))
        .map_or(0, |d| d.signed_duration_since(chrono::NaiveDate::default()).num_days())
}

/// The IBAN without spaces.
fn compact(iban: &str) -> String {
    iban.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether two entries have the same other party: the same IBAN, or – where one has none – the same name.
fn same_party(a: &StatementEntry, b: &StatementEntry) -> bool {
    let party = |e: &StatementEntry| e.details.first().and_then(|d| d.counterparty.clone());
    match (party(a), party(b)) {
        (Some(a), Some(b)) => match (&a.iban, &b.iban) {
            (Some(x), Some(y)) => compact(x) == compact(y),
            _ => a.name.as_ref().zip(b.name.as_ref()).is_some_and(|(x, y)| x.to_lowercase() == y.to_lowercase()),
        },
        _ => false,
    }
}

impl Workspace {
    /// The account of the books for an IBAN: the one opened with it.
    pub fn bank_account(&self, books: &BooksId, iban: &str) -> Result<Option<AccountId>, AppError> {
        let iban = compact(iban);
        Ok(self.get(books)?.events().ordered().into_iter().find_map(|(_, event)| match &event.payload {
            Payload::AccountOpened { account }
                if account.attributes.get("iban").is_some_and(|i| compact(i) == iban) =>
            {
                Some(account.id.clone())
            }
            _ => None,
        }))
    }

    /// Reads the statements of a file – camt.053, MT940, or a bank's CSV file, for which `iban` names the account
    /// where the file does not –, records each as a document, and opens a bank account for an IBAN new to the books,
    /// all or none. A file read before is left out.
    pub fn import_statements(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        bytes: &[u8],
        name: Option<&str>,
        iban: Option<&str>,
    ) -> Result<ImportedStatements, AppError> {
        let read =
            read_statements(bytes, &CurrencyTable::v0_1(), iban).map_err(|e| AppError::Statement(e.to_string()))?;
        let hash = self.store.put_original(bytes)?;
        let recorded: BTreeSet<String> = self
            .documents(books)?
            .iter()
            .filter(|(_, d)| d.statement.is_some())
            .map(|(_, d)| format!("{}|{}", d.original.hash, d.number.clone().unwrap_or_default()))
            .collect();
        let standard = self.standard_accounts(books, owner, today())?;
        let mut commands = Vec::new();
        let mut opened: BTreeSet<String> = BTreeSet::new();
        let mut imported = ImportedStatements { documents: Vec::new(), known: 0, entries: 0, shown_before: 0 };
        for statement in read {
            let key = format!("{hash}|{}", statement.number.clone().unwrap_or_default());
            if recorded.contains(&key) {
                imported.known += 1;
                continue;
            }
            let iban = compact(&statement.statement.account.iban);
            if self.bank_account(books, &iban)?.is_none() && opened.insert(iban.clone()) {
                let attributes = BTreeMap::from([("iban".to_owned(), iban.clone())]);
                let account =
                    Account { id: new_id(), kind: AccountKind::Asset, parent: Some(standard.bank.clone()), attributes };
                commands.push((books.clone(), Command::OpenAccount(account)));
            }
            imported.entries += statement.statement.entries.len();
            let document = Document {
                id: new_id(),
                kind: statement.kind,
                number: statement.number,
                date: statement.date,
                received: today(),
                original: Original {
                    hash,
                    size: bytes.len() as u64,
                    media_type: if statement.format == "iso20022-camt.053" { "application/xml" } else { "text/plain" }
                        .into(),
                    format: Some(statement.format.into()),
                    name: name.map(str::to_owned),
                },
                invoice: None,
                statement: Some(Box::new(statement.statement)),
            };
            imported.documents.push(document.id.clone());
            commands.push((books.clone(), Command::RecordDocument(document)));
        }
        self.execute_all(commands)?;
        imported.shown_before = self
            .get(books)?
            .events()
            .review_statements()
            .iter()
            .filter(|p| matches!(p, StatementProblem::ShownTwice { again, .. } if imported.documents.contains(&again.document)))
            .count();
        imported.entries -= imported.shown_before;
        Ok(imported)
    }

    /// The entries of recorded statements that are not booked – and not shown by an earlier statement already –, in
    /// the order booked, each with a proposal where there is one.
    pub fn open_entries(&self, books: &BooksId, owner: &PartyId) -> Result<Vec<OpenEntry>, AppError> {
        let events = self.get(books)?.events();
        let booked = events.booked_entries();
        let shown_before: BTreeSet<EntryReference> = events
            .review_statements()
            .into_iter()
            .filter_map(|p| match p {
                StatementProblem::ShownTwice { again, .. } => Some(again),
                _ => None,
            })
            .collect();
        let statements = events.statements();
        let entry_of: BTreeMap<EntryReference, &StatementEntry> = statements
            .iter()
            .flat_map(|(id, s)| {
                s.entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (EntryReference { document: id.clone(), entry: i as u32 + 1 }, e))
            })
            .collect();
        let mut open = Vec::new();
        let mut taken = BTreeSet::new();
        for (document, statement) in &statements {
            let iban = compact(&statement.account.iban);
            let account = self.bank_account(books, &iban)?;
            for (i, entry) in statement.entries.iter().enumerate() {
                let reference = EntryReference { document: document.clone(), entry: i as u32 + 1 };
                if booked.contains_key(&reference) || shown_before.contains(&reference) {
                    continue;
                }
                let mut item = OpenEntry {
                    reference,
                    iban: iban.clone(),
                    account: account.clone(),
                    entry: entry.clone(),
                    suggestion: None,
                };
                if item.account.is_some() {
                    item.suggestion = self.suggest(books, owner, &item, &booked, &entry_of, &mut taken);
                }
                open.push(item);
            }
        }
        open.sort_by_key(|item| item.entry.booked);
        Ok(open)
    }

    /// A proposal for an entry: the payment booked before the statement came, else an unbooked invoice it pays, else
    /// the account of the same party's last entry. A payment booked before is proposed for one entry only (`taken`).
    fn suggest(
        &self,
        books: &BooksId,
        owner: &PartyId,
        item: &OpenEntry,
        booked: &BTreeMap<EntryReference, EventHash>,
        entry_of: &BTreeMap<EntryReference, &StatementEntry>,
        taken: &mut BTreeSet<EventHash>,
    ) -> Option<Suggestion> {
        let account = item.account.clone()?;
        if let Some(found) = self.booked_before(books, owner, item, &account, taken) {
            return Some(found);
        }
        let events = self.get(books).ok()?.events();
        let money_in = item.entry.amount.base_units() > 0;
        let text = item.text().to_lowercase();
        let detail = item.entry.details.first();
        let end_to_end = detail.and_then(|d| d.end_to_end.clone()).unwrap_or_default().to_lowercase();
        // Invoices recorded and not booked, whose amount due is the entry's.
        let based: BTreeSet<DocumentId> = events
            .ordered()
            .into_iter()
            .flat_map(|(_, event)| match &event.payload {
                Payload::BookingRecorded { booking } => booking.documents.clone(),
                _ => Vec::new(),
            })
            .collect();
        let standard = self.standard_accounts(books, owner, item.entry.booked).ok()?;
        for (_, document) in self.documents(books).ok()? {
            let Some(invoice) = &document.invoice else { continue };
            if based.contains(&document.id) || invoice.totals.due.base_units() != item.entry.amount.base_units().abs() {
                continue;
            }
            let (side, party) = if money_in {
                (InvoiceSide::Sale, &invoice.buyer.name)
            } else {
                (InvoiceSide::Purchase, &invoice.seller.name)
            };
            let number = document.number.clone().unwrap_or_default().to_lowercase();
            let by_number = !number.is_empty() && (text.contains(&number) || end_to_end.contains(&number));
            let by_party = !party.is_empty() && text.contains(&party.to_lowercase());
            if !(by_number || by_party) {
                continue;
            }
            let accounts = ProposalAccounts {
                counter: if money_in { standard.sales.clone() } else { standard.expenses.clone() },
                party: account.clone(),
                vat_output: standard.vat.get(&"de-out-19".parse().ok()?).cloned()?,
                vat_input: standard.vat.get(&"de-in-19".parse().ok()?).cloned()?,
                vat_reverse_charge: standard.vat_reverse_charge.clone(),
            };
            let mut booking = propose_booking(&document, side, owner, &accounts).ok()?;
            // A small business's own invoices are exempt by § 19 UStG, not by § 4: a field of their own.
            let small = self
                .law_for(books, owner, item.entry.booked)
                .ok()
                .and_then(|(c, ..)| c.vat)
                .is_some_and(|v| v.scheme.as_str() == "de-small-business");
            if small && side == InvoiceSide::Sale {
                for posting in &mut booking.postings {
                    if posting.tax.as_ref().is_some_and(|t| t.as_str() == "de-out-exempt") {
                        posting.tax = "de-out-small-business".parse().ok();
                    }
                }
            }
            booking.date = item.entry.booked;
            booking.documents.push(item.reference.document.clone());
            booking.entries = vec![item.reference.clone()];
            let source = Source::Invoice { document: document.id.clone(), number: document.number.clone() };
            return Some(Suggestion { source, booking });
        }
        // The same party as an entry booked before: its account and tax code, the latest first.
        let vat_accounts: BTreeSet<&AccountId> =
            standard.vat.values().chain([&standard.vat_reverse_charge, &standard.vat_reverse_charge_input]).collect();
        let mut earlier: Vec<(&EntryReference, &EventHash)> = booked
            .iter()
            .filter(|(reference, _)| entry_of.get(reference).is_some_and(|e| same_party(e, &item.entry)))
            .collect();
        earlier.sort_by_key(|(reference, _)| entry_of[reference].booked);
        let (_, event) = earlier.last()?;
        let Payload::BookingRecorded { booking: before } = &events.get(event)?.payload else { return None };
        let counter = before.postings.iter().find(|p| {
            p.owner == *owner
                && p.account != account
                && !vat_accounts.contains(&p.account)
                && !self.is_bank(books, &p.account)
        })?;
        let booking = self
            .entry_booking(books, owner, item, counter.account.clone(), counter.tax.clone(), before.text.clone())
            .ok()?;
        Some(Suggestion { source: Source::History(**event), booking })
    }

    /// The payment booked before an entry's statement came: a booking that stands, names no entry, and posts the
    /// entry's amount on its bank account or on the standard bank account, dated at most 60 days before the entry and
    /// 10 after – the nearest. Proposed as booked again on the bank account, naming the entry.
    fn booked_before(
        &self,
        books: &BooksId,
        owner: &PartyId,
        item: &OpenEntry,
        account: &AccountId,
        taken: &mut BTreeSet<EventHash>,
    ) -> Option<Suggestion> {
        let events = self.get(books).ok()?.events();
        let standard = self.standard_accounts(books, owner, item.entry.booked).ok()?;
        let reversed: BTreeSet<EventHash> = events
            .ordered()
            .into_iter()
            .filter_map(|(_, event)| match &event.payload {
                Payload::BookingReversed { reverses, .. } => Some(*reverses),
                _ => None,
            })
            .collect();
        let day = |date: librotenado::Date| days(date);
        let entry_day = day(item.entry.booked);
        let mut best: Option<(i64, EventHash, Booking, usize)> = None;
        for (hash, event) in events.ordered() {
            let Payload::BookingRecorded { booking } = &event.payload else { continue };
            let distance = day(booking.date) - entry_day;
            if reversed.contains(&hash)
                || !booking.entries.is_empty()
                || taken.contains(&hash)
                || !(-60..=10).contains(&distance)
            {
                continue;
            }
            let Some(index) = booking.postings.iter().position(|p| {
                p.owner == *owner
                    && p.amount == item.entry.amount
                    && (p.account == *account || p.account == standard.bank)
            }) else {
                continue;
            };
            if best.as_ref().is_none_or(|(d, ..)| distance.abs() < d.abs()) {
                best = Some((distance, hash, booking.clone(), index));
            }
        }
        let (_, hash, mut booking, index) = best?;
        taken.insert(hash);
        booking.postings[index].account = account.clone();
        booking.entries = vec![item.reference.clone()];
        if !booking.documents.contains(&item.reference.document) {
            booking.documents.push(item.reference.document.clone());
        }
        Some(Suggestion { source: Source::Booked(hash), booking })
    }

    /// What accepting a proposal writes: its booking, and for a payment booked before, first its reversal on the
    /// same day – so that no figure of a period changes.
    pub fn suggestion_commands(&self, suggestion: &Suggestion) -> Result<Vec<Command>, AppError> {
        let mut commands = Vec::new();
        if let Source::Booked(event) = &suggestion.source {
            let german = self.language()?.is_none_or(|l| l.starts_with("de"));
            let text = if german { "dem Kontoauszug zugeordnet" } else { "matched to the bank statement" };
            commands.push(Command::Reverse { event: *event, date: suggestion.booking.date, text: text.into() });
        }
        commands.push(Command::Book(suggestion.booking.clone()));
        Ok(commands)
    }

    /// Accepts a proposal: writes what [`Workspace::suggestion_commands`] says, all or none.
    pub fn accept_suggestion(&mut self, books: &BooksId, suggestion: &Suggestion) -> Result<Vec<EventHash>, AppError> {
        let commands = self.suggestion_commands(suggestion)?;
        self.execute_all(commands.into_iter().map(|command| (books.clone(), command)).collect())
    }

    /// What an owner holds in the bank at the end of a day, in base units of its functional currency: the standard
    /// bank account and the accounts opened for IBANs below it.
    pub fn bank_holdings(&self, books: &BooksId, owner: &PartyId, until: librotenado::Date) -> Result<i128, AppError> {
        let standard = self.standard_accounts(books, owner, until)?;
        let (configuration, ..) = self.law_for(books, owner, until)?;
        let events = self.get(books)?.events();
        let balances = events.functional_balances(Some(until))?;
        let mut accounts = vec![standard.bank.clone()];
        accounts.extend(events.ordered().into_iter().filter_map(|(_, event)| match &event.payload {
            Payload::AccountOpened { account } if account.parent.as_ref() == Some(&standard.bank) => {
                Some(account.id.clone())
            }
            _ => None,
        }));
        Ok(accounts.iter().map(|account| balances.get(owner, account, &configuration.currency)).sum())
    }

    fn is_bank(&self, books: &BooksId, account: &AccountId) -> bool {
        self.get(books).is_ok_and(|b| {
            b.events().ordered().iter().any(|(_, event)| {
                matches!(&event.payload, Payload::AccountOpened { account: a } if a.id == *account && a.attributes.contains_key("iban"))
            })
        })
    }

    /// The booking of an entry on an account with a tax code: its amount as the gross amount, against its bank account,
    /// naming the entry and resting on its statement. Nothing is written.
    pub fn entry_booking(
        &self,
        books: &BooksId,
        owner: &PartyId,
        item: &OpenEntry,
        account: AccountId,
        tax: Option<TaxCode>,
        text: String,
    ) -> Result<Booking, AppError> {
        let payment = item.account.clone().ok_or(AppError::NoStandardAccounts)?;
        let amount = item.entry.amount.clone();
        let mut entry = SimpleEntry {
            owner: owner.clone(),
            date: item.entry.booked,
            text,
            account,
            gross: if amount.base_units() < 0 { -amount.clone() } else { amount.clone() },
            tax,
            payment: payment.clone(),
            documents: vec![item.reference.document.clone()],
        };
        let mut booking = self.simple_booking(books, &entry)?;
        // The bank's side must be the entry's: money in a debit of the bank account. Income paid back, an expense
        // refunded: the gross amount the other way.
        let bank = booking.postings.iter().find(|p| p.account == payment).map(|p| p.amount.base_units()).unwrap_or(0);
        if bank.signum() != amount.base_units().signum() {
            entry.gross = -entry.gross;
            booking = self.simple_booking(books, &entry)?;
        }
        booking.entries = vec![item.reference.clone()];
        Ok(booking)
    }

    /// Books every open entry that has a proposal, all or none; returns how many.
    pub fn accept_suggestions(&mut self, books: &BooksId, owner: &PartyId) -> Result<usize, AppError> {
        let suggestions: Vec<Suggestion> =
            self.open_entries(books, owner)?.into_iter().filter_map(|item| item.suggestion).collect();
        let count = suggestions.len();
        let mut commands = Vec::new();
        for suggestion in &suggestions {
            commands.extend(self.suggestion_commands(suggestion)?.into_iter().map(|command| (books.clone(), command)));
        }
        self.execute_all(commands)?;
        Ok(count)
    }

    /// The product registration number of FinTS, once falkOSS has one (set by the maintainer).
    pub fn fints_product(&self) -> Result<Option<String>, AppError> {
        Ok(self.store.setting(None, "fints-product")?)
    }

    /// Fetches statements from the bank directly by FinTS – a placeholder: every FinTS dialogue sends the product
    /// registration number the Deutsche Kreditwirtschaft gives, and there is none yet. Until then, statements are read
    /// from files.
    pub fn fetch_statements(&mut self, _books: &BooksId, _iban: &str) -> Result<ImportedStatements, AppError> {
        match self.fints_product()? {
            None => Err(AppError::FinTsNotRegistered),
            Some(_) => Err(AppError::FinTsNotBuilt),
        }
    }
}
