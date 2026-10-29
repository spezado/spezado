// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workflows of a freelancer's year: booking from a gross amount, reading documents into booking proposals, the
//! advance VAT returns that fall due, choosing accounts, and the data transfer to an auditor.

use std::{fs, path::Path};

use librotenado::{
    AccountId, AccountKind, Booking, BooksId, Date, Document, DocumentId, DocumentKind, EventHash, InvoiceSide, Money,
    Names, Original, PartyId, Payload, Posting, ProposalAccounts, TaxCode, TaxDirection, en16931, gobd,
    propose_booking,
};
use spezado_core::{Command, split_gross};
use spezado_law::{Facts, StandardAccounts};

use crate::{AppError, Workspace, new_id};

/// Today by the device's clock, in its time zone.
pub fn today() -> Date {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    Date::new(today.year() as u16, today.month() as u8, today.day() as u8).expect("a date of the clock")
}

/// A booking as people think of it: an amount paid or received, on an account, with a tax code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleEntry {
    /// Whose books.
    pub owner: PartyId,
    /// The date – on a cash basis, the day of payment.
    pub date: Date,
    /// What it is about.
    pub text: String,
    /// The account of income or expense.
    pub account: AccountId,
    /// The gross amount, VAT included; negative for a refund.
    pub gross: Money,
    /// The tax code; none for what VAT does not touch.
    pub tax: Option<TaxCode>,
    /// Where the money went from or came to: the bank, the cash.
    pub payment: AccountId,
    /// The documents it rests on.
    pub documents: Vec<DocumentId>,
}

/// A document read from a file, not yet recorded: its record, and a booking proposal if it is an e-invoice.
#[derive(Debug, Clone)]
pub struct ReadDocument {
    /// The document, its original stored by hash.
    pub document: Document,
    /// The booking proposed from an e-invoice.
    pub proposal: Option<Booking>,
    /// Why the file was not read as an e-invoice, if it looked like one.
    pub not_an_invoice: Option<String>,
}

/// An advance VAT return of a period and the day it is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VatReturnDue {
    /// The first day of the period.
    pub from: Date,
    /// The last day of the period.
    pub to: Date,
    /// The day it is due.
    pub due: Date,
}

/// An account to choose from: its number in the national chart, its name, its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountChoice {
    /// The account.
    pub id: AccountId,
    /// Its number in the national chart, if the owner works with one.
    pub number: Option<String>,
    /// Its name in the language asked for.
    pub name: String,
    /// Its kind.
    pub kind: AccountKind,
}

impl Workspace {
    /// A setting of the workspace – of an interface, such as the devices paired with the API.
    pub fn workspace_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.store.setting(None, key)?)
    }

    /// Sets a setting of the workspace.
    pub fn set_workspace_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        Ok(self.store.set_setting(None, key, value)?)
    }

    /// The language the workspace speaks, if one is set (`de`, `en`).
    pub fn language(&self) -> Result<Option<String>, AppError> {
        Ok(self.store.setting(None, "language")?)
    }

    /// Sets the language the workspace speaks.
    pub fn set_language(&self, language: &str) -> Result<(), AppError> {
        Ok(self.store.set_setting(None, "language", language)?)
    }

    /// The standard accounts of the national chart an owner works with on a date.
    pub fn standard_accounts(
        &self,
        books: &BooksId,
        owner: &PartyId,
        date: Date,
    ) -> Result<StandardAccounts, AppError> {
        let (_, _, _, module) = self.law_for(books, owner, date)?;
        let national = self.national_chart(books, owner, date)?.ok_or(AppError::NoStandardAccounts)?;
        module.standard_accounts(&national).ok_or(AppError::NoStandardAccounts)
    }

    /// The booking of a simple entry: a gross amount split into the net amount on the account and the VAT on the VAT
    /// account of the tax code, against the payment account. Income is credited, expenses debited – by the tax code's
    /// direction, else by the account's kind. Nothing is written.
    pub fn simple_booking(&self, books: &BooksId, entry: &SimpleEntry) -> Result<Booking, AppError> {
        let (_, _, _, module) = self.law_for(books, &entry.owner, entry.date)?;
        let kinds = self.get(books)?.events().account_kinds(&self.chart);
        let mut income = kinds.get(&entry.account) == Some(&AccountKind::Income);
        let mut vat = None;
        if let Some(code) = &entry.tax {
            let table = module.tax_table(entry.date).ok_or_else(|| AppError::UnknownTaxCode(code.clone()))?;
            let definition = table.get(code).ok_or_else(|| AppError::UnknownTaxCode(code.clone()))?;
            income = definition.direction == TaxDirection::Output;
            let standard = self.standard_accounts(books, &entry.owner, entry.date)?;
            match (definition.rate, standard.vat.get(code)) {
                (Some(rate), Some(account)) => vat = Some((rate, account.clone())),
                (Some(rate), None) if !rate.is_zero() => return Err(AppError::NeedsLines(code.clone())),
                _ => {}
            }
        }
        let (net, tax) = match &vat {
            Some((rate, _)) => {
                split_gross(&entry.gross, *rate).ok_or(AppError::Balance(librotenado::BalanceError::Overflow))?
            }
            None => (entry.gross.clone(), Money::new(0, entry.gross.currency().clone()).expect("zero")),
        };
        // Expenses on the debit, income on the credit; the payment on the other side.
        let side = |money: &Money| if income { -money.clone() } else { money.clone() };
        let posting = |account: AccountId, amount: Money, tax: Option<TaxCode>| Posting {
            owner: entry.owner.clone(),
            account,
            amount,
            note: None,
            tax,
            exchange: None,
        };
        let mut postings = vec![posting(entry.account.clone(), side(&net), entry.tax.clone())];
        if let Some((_, account)) = vat
            && tax.base_units() != 0
        {
            postings.push(posting(account, side(&tax), entry.tax.clone()));
        }
        postings.push(posting(entry.payment.clone(), -side(&entry.gross), None));
        Ok(Booking {
            date: entry.date,
            text: entry.text.clone(),
            postings,
            documents: entry.documents.clone(),
            entries: vec![],
            transaction: None,
        })
    }

    /// Reads a file as a document of an owner: stores its original and reads an e-invoice (XRechnung, ZUGFeRD) with a
    /// booking proposal against `payment` – on a cash basis, the account it is paid from or to. Nothing is recorded.
    pub fn read_document(
        &self,
        books: &BooksId,
        owner: &PartyId,
        bytes: &[u8],
        name: Option<&str>,
        side: InvoiceSide,
        payment: &AccountId,
    ) -> Result<ReadDocument, AppError> {
        let hash = self.store.put_original(bytes)?;
        let media_type = media_type(bytes);
        let currencies = librotenado::CurrencyTable::v0_1();
        let translated = match media_type {
            "application/pdf" => Some(en16931::read_pdf(bytes, &currencies)),
            "application/xml" => Some(en16931::read(bytes, &currencies)),
            _ => None,
        };
        let received = today();
        let mut not_an_invoice = None;
        let invoice = match translated {
            Some(Ok(invoice)) => {
                for file in invoice.files.values() {
                    self.store.put_original(file)?;
                }
                Some(invoice)
            }
            Some(Err(error)) => {
                // A PDF without an embedded invoice is a document all the same; an XML that is none is worth saying.
                if media_type == "application/xml" {
                    not_an_invoice = Some(error.to_string());
                }
                None
            }
            None => None,
        };
        let original = Original {
            hash,
            size: bytes.len() as u64,
            media_type: media_type.to_owned(),
            format: invoice.as_ref().map(|invoice| invoice.format.to_owned()),
            name: name.map(str::to_owned),
        };
        let document = match invoice {
            Some(invoice) => Document {
                id: new_id(),
                kind: invoice.kind,
                number: Some(invoice.number),
                date: invoice.date,
                received,
                original,
                invoice: Some(Box::new(invoice.invoice)),
                statement: None,
            },
            None => Document {
                id: new_id(),
                kind: DocumentKind::Receipt,
                number: None,
                date: received,
                received,
                original,
                invoice: None,
                statement: None,
            },
        };
        let proposal = match &document.invoice {
            Some(_) => {
                // The accounts of today's configuration: an invoice may be dated before the books begin.
                let standard = self.standard_accounts(books, owner, received)?;
                let accounts = ProposalAccounts {
                    counter: match side {
                        InvoiceSide::Purchase => standard.expenses.clone(),
                        InvoiceSide::Sale => standard.sales.clone(),
                    },
                    party: payment.clone(),
                    vat_output: standard
                        .vat
                        .get(&"de-out-19".parse().expect("a tax code"))
                        .cloned()
                        .unwrap_or(standard.vat_reverse_charge.clone()),
                    vat_input: standard
                        .vat
                        .get(&"de-in-19".parse().expect("a tax code"))
                        .cloned()
                        .unwrap_or(standard.vat_reverse_charge_input.clone()),
                    vat_reverse_charge: standard.vat_reverse_charge.clone(),
                };
                propose_booking(&document, side, owner, &accounts).ok()
            }
            None => None,
        };
        Ok(ReadDocument { document, proposal, not_an_invoice })
    }

    /// Records a document and – if given – a booking resting on it, all or none.
    pub fn record_document(
        &mut self,
        books: &BooksId,
        document: Document,
        booking: Option<Booking>,
    ) -> Result<Vec<EventHash>, AppError> {
        let id = document.id.clone();
        let mut commands = vec![(books.clone(), Command::RecordDocument(document))];
        if let Some(mut booking) = booking {
            if !booking.documents.contains(&id) {
                booking.documents.push(id);
            }
            commands.push((books.clone(), Command::Book(booking)));
        }
        self.execute_all(commands)
    }

    /// The documents recorded in a set of books, in the agreed order, with the events that recorded them.
    pub fn documents(&self, books: &BooksId) -> Result<Vec<(EventHash, Document)>, AppError> {
        Ok(self
            .get(books)?
            .events()
            .ordered()
            .into_iter()
            .filter_map(|(hash, event)| match &event.payload {
                Payload::DocumentRecorded { document } => Some((hash, document.clone())),
                _ => None,
            })
            .collect())
    }

    /// The bytes of a stored original.
    pub fn original(&self, hash: &librotenado::ContentHash) -> Result<Option<Vec<u8>>, AppError> {
        Ok(self.store.original(hash)?)
    }

    /// The advance VAT returns of an owner in a calendar year and when each is due – after the configuration at the
    /// end of each period, so that a region or an extension changed during the year moves the later deadlines.
    pub fn vat_returns(&self, books: &BooksId, owner: &PartyId, year: u16) -> Result<Vec<VatReturnDue>, AppError> {
        let first = Date::new(year, 1, 1).expect("a first of January");
        let (configuration, country, legal_form, module) = self.law_for(books, owner, first)?;
        let facts = Facts::new(&configuration, country, legal_form.as_deref());
        module
            .vat_periods(&facts, year)
            .into_iter()
            .map(|(from, to)| {
                let (configuration, country, legal_form, module) = self.law_for(books, owner, to)?;
                let facts = Facts::new(&configuration, country, legal_form.as_deref());
                Ok(VatReturnDue { from, to, due: module.vat_deadline(&facts, to) })
            })
            .collect()
    }

    /// The accounts an owner can book on: the national chart's, with their numbers, and the books' own; names in
    /// `language` (`de`, `en`), German where it has none.
    pub fn account_choices(
        &self,
        books: &BooksId,
        owner: &PartyId,
        date: Date,
        language: &str,
    ) -> Result<Vec<AccountChoice>, AppError> {
        let names = Names::v0_1(language).or_else(|| Names::v0_1("de")).expect("German names are published");
        let kinds = self.get(books)?.events().account_kinds(&self.chart);
        let name = |id: &AccountId| names.get(id.as_str()).unwrap_or(id.as_str()).to_owned();
        let mut choices: Vec<AccountChoice> = match self.national_chart(books, owner, date)? {
            Some(national) => national
                .accounts
                .iter()
                .filter_map(|(number, id)| {
                    let kind = *kinds.get(id)?;
                    Some(AccountChoice { id: id.clone(), number: Some(number.clone()), name: name(id), kind })
                })
                .collect(),
            None => self
                .chart
                .accounts
                .iter()
                .map(|account| AccountChoice {
                    id: account.id.clone(),
                    number: None,
                    name: name(&account.id),
                    kind: account.kind,
                })
                .collect(),
        };
        // The other side of opening balances, which national charts have no number for.
        let opening = librotenado::Chart::opening_balances_v0_1();
        if !choices.iter().any(|c| c.id == opening) {
            choices.push(AccountChoice { name: name(&opening), id: opening, number: None, kind: AccountKind::Memo });
        }
        for (_, event) in self.get(books)?.events().ordered() {
            if let Payload::AccountOpened { account } = &event.payload {
                // An account of the books has no published name; a bank account goes by its parent and IBAN.
                let own = match (&account.parent, account.attributes.get("iban")) {
                    (Some(parent), Some(iban)) => format!("{} {iban}", name(parent)),
                    (None, Some(iban)) => iban.clone(),
                    _ => name(&account.id),
                };
                choices.push(AccountChoice { id: account.id.clone(), number: None, name: own, kind: account.kind });
            }
        }
        // By number, the numbered ones first.
        choices.sort_by(|a, b| {
            (a.number.is_none(), number_key(&a.number), &a.name).cmp(&(
                b.number.is_none(),
                number_key(&b.number),
                &b.name,
            ))
        });
        Ok(choices)
    }

    /// The files of the data transfer of the GoBD (Z3) for an owner and a period: `index.xml` and its CSV files.
    pub fn gobd_files(
        &self,
        books: &BooksId,
        owner: &PartyId,
        from: Date,
        to: Date,
    ) -> Result<Vec<gobd::ExportFile>, AppError> {
        let (_, _, _, module) = self.law_for(books, owner, to)?;
        let taxes = module.tax_table(to).ok_or(AppError::NoLaw(owner.clone()))?;
        let names = Names::v0_1("de").expect("German names are published");
        let currencies = librotenado::CurrencyTable::v0_1();
        Ok(gobd::export(self.get(books)?.events(), &self.chart, &names, &currencies, &taxes, owner, from, to))
    }

    /// Writes the data transfer of the GoBD (Z3) for an owner and a period into `directory`: `index.xml` and its CSV
    /// files. Returns the files written.
    pub fn gobd_export(
        &self,
        books: &BooksId,
        owner: &PartyId,
        from: Date,
        to: Date,
        directory: &Path,
    ) -> Result<Vec<String>, AppError> {
        let files = self.gobd_files(books, owner, from, to)?;
        let write_error = |path: &Path, error| AppError::Write { path: path.display().to_string(), error };
        fs::create_dir_all(directory).map_err(|error| write_error(directory, error))?;
        let mut written = Vec::new();
        for file in files {
            let path = directory.join(&file.path);
            fs::write(&path, &file.content).map_err(|error| write_error(&path, error))?;
            written.push(file.path);
        }
        Ok(written)
    }
}

/// National numbers sort as numbers where they are, so that 4930 comes before 10000.
fn number_key(number: &Option<String>) -> (usize, String) {
    number.as_ref().map_or((0, String::new()), |n| (n.len(), n.clone()))
}

/// The media type of a file by its first bytes: PDF, XML, PNG, JPEG, else unknown.
fn media_type(bytes: &[u8]) -> &'static str {
    let trimmed = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let start = trimmed.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    match &trimmed[start..] {
        rest if rest.starts_with(b"%PDF") => "application/pdf",
        rest if rest.starts_with(b"<") => "application/xml",
        rest if rest.starts_with(b"\x89PNG") => "image/png",
        rest if rest.starts_with(b"\xff\xd8\xff") => "image/jpeg",
        _ => "application/octet-stream",
    }
}
