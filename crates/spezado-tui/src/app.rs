// SPDX-License-Identifier: AGPL-3.0-or-later
//! The state of the terminal interface and what each key does to it – without a terminal, so that it can be tested.

use std::{collections::BTreeMap, path::PathBuf};

use librotenado::{
    AccountId, Booking, BooksId, Currency, Date, Document, DocumentId, EventHash, InvoiceSide, Money, Names, PartyId,
    Person, Posting,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use spezado_app::{
    AccountChoice, AppError, GERMAN_STATES, OpenEntry, Party, ReadDocument, SimpleEntry, Source, Suggestion, Workspace,
    new_id, today,
};
use spezado_core::{Command, JournalEntry};

use crate::{
    form::{Field, Form, Outcome, resolve},
    i18n::{Lang, Text},
    view::{Hits, Target},
};

/// The tabs, in the order of their keys 1 to 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Balances, the year so far, what is due.
    Overview,
    /// The bookings.
    Journal,
    /// The entries of bank statements, to be booked.
    Bank,
    /// The documents.
    Documents,
    /// The accounts and their balances.
    Accounts,
    /// The VAT returns and the annual report.
    Reports,
    /// What needs review.
    Review,
    /// Closing periods and the data transfer.
    Closing,
}

impl Tab {
    /// All tabs.
    pub const ALL: [Tab; 8] = [
        Tab::Overview,
        Tab::Journal,
        Tab::Bank,
        Tab::Documents,
        Tab::Accounts,
        Tab::Reports,
        Tab::Review,
        Tab::Closing,
    ];

    /// Its title.
    pub fn title(self) -> Text {
        match self {
            Tab::Overview => Text::Overview,
            Tab::Journal => Text::Journal,
            Tab::Bank => Text::Bank,
            Tab::Documents => Text::Documents,
            Tab::Accounts => Text::Accounts,
            Tab::Reports => Text::Reports,
            Tab::Review => Text::Review,
            Tab::Closing => Text::Closing,
        }
    }

    /// Its place, from 0.
    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|tab| *tab == self).expect("a tab")
    }
}

/// What a form is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// Setting up the first books.
    SetUp,
    /// A booking from a gross amount, perhaps resting on a document read just now.
    Simple(Option<Box<Document>>),
    /// A booking in lines.
    Lines,
    /// Reversing a booking.
    Reverse(EventHash),
    /// Reading a document from a file.
    ReadDocument,
    /// Closing a period.
    Close,
    /// The GoBD data transfer.
    Gobd,
    /// Exporting the figures shown.
    ExportReport,
    /// Reading a bank statement from a file.
    ReadStatement,
    /// Booking an entry of a bank statement on an account.
    Entry(Box<OpenEntry>),
    /// Booking opening balances.
    Opening,
    /// The seller's details on invoices.
    Profile,
    /// A new customer.
    Customer,
    /// Writing an invoice.
    Invoice,
    /// Changing what moves the deadlines.
    Deadlines,
    /// Starting to synchronise through a relay.
    SyncStart,
}

/// A report on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figures {
    /// Its title.
    pub title: String,
    /// The law module it was derived with.
    pub law: String,
    /// Field, its name, the amount.
    pub rows: Vec<(String, String, Money)>,
    /// What it is called in a file name.
    pub file: String,
}

/// A booking proposed – from a document read, or for an entry of a bank statement –, to be confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// The document read, recorded with the booking; none for an entry of a statement recorded already.
    pub document: Option<Document>,
    /// The booking proposed.
    pub booking: Booking,
    /// Where the proposal for an entry of a statement comes from; none for a document read.
    pub source: Option<Source>,
    /// Date and text, to change before booking.
    pub form: Form,
}

/// What is on the screen besides the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// The tab.
    Normal,
    /// A form.
    Form(Purpose, Form),
    /// A report.
    Figures(Figures),
    /// The bookings of one account.
    Ledger(AccountId),
    /// A booking proposal.
    Proposal(Box<Proposal>),
    /// Something to read and write down – a recovery code, an invitation –, with its title.
    Notice(Text, Vec<String>),
}

/// A report that can be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportItem {
    /// Its identity (`de-ustva`).
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its period.
    pub from: Date,
    /// Its period.
    pub to: Date,
    /// When it is due, for an advance VAT return.
    pub due: Option<Date>,
}

/// What the screens show, read from the workspace after each change.
#[derive(Debug, Clone, Default)]
pub struct View {
    /// The bookings of the year.
    pub journal: Vec<JournalEntry>,
    /// The documents.
    pub documents: Vec<(EventHash, Document)>,
    /// Number, name and balance of every account with one.
    pub balances: Vec<(AccountId, String, String, Money)>,
    /// The reports of the year.
    pub reports: Vec<ReportItem>,
    /// What needs review.
    pub issues: Vec<String>,
    /// Bank, cash, income, expenses.
    pub money: Option<(Money, Money, Money, Money)>,
    /// The next advance VAT return, from today.
    pub next_return: Option<ReportItem>,
    /// The relay the books are synchronised through, if they are.
    pub relay: Option<String>,
    /// The last day closed.
    pub closed: Option<Date>,
    /// The accounts to choose from.
    pub accounts: Vec<AccountChoice>,
    /// The owner's name.
    pub owner_name: String,
    /// The national chart, for the title.
    pub chart: Option<String>,
    /// Tax codes to choose from: (shown, code).
    pub tax_codes: Vec<(String, String)>,
    /// The entries of bank statements not booked yet.
    pub open_entries: Vec<OpenEntry>,
}

/// The interface: the workspace, where one is, and what is shown.
#[derive(Debug)]
pub struct App<'w> {
    /// The workspace.
    pub workspace: &'w mut Workspace,
    /// The language.
    pub lang: Lang,
    /// The books shown.
    pub books: Option<BooksId>,
    /// Their owner.
    pub owner: Option<PartyId>,
    /// The year shown.
    pub year: u16,
    /// The tab shown.
    pub tab: Tab,
    /// The row selected in each tab.
    pub selected: [usize; 8],
    /// What is shown besides.
    pub mode: Mode,
    /// A message: whether it is an error, and what it says.
    pub status: Option<(bool, String)>,
    /// What the screens show.
    pub view: View,
    /// Whether to end.
    pub quit: bool,
    /// The figures an export form is about.
    pending_figures: Option<Figures>,
    /// Whether the mouse is used; when it is, the terminal's own selection of text needs a modifier key.
    pub mouse: bool,
    /// What was drawn where, the last time the screen was drawn.
    pub hits: Hits,
}

/// The currency of the books shown.
fn eur() -> Currency {
    "EUR".parse().expect("a currency")
}

impl<'w> App<'w> {
    /// The interface on a workspace, in a language, on its first books – or setting up books if there are none.
    pub fn new(workspace: &'w mut Workspace, lang: Lang) -> Self {
        let books = workspace.books().keys().next().cloned();
        let mut app = App {
            workspace,
            lang,
            books,
            owner: None,
            year: today().year(),
            tab: Tab::Overview,
            selected: [0; 8],
            mode: Mode::Normal,
            status: None,
            view: View::default(),
            quit: false,
            pending_figures: None,
            mouse: true,
            hits: Hits::default(),
        };
        if app.books.is_none() {
            app.mode = Mode::Form(Purpose::SetUp, app.set_up_form());
        }
        app.refresh();
        app
    }

    fn t(&self, text: Text) -> &'static str {
        self.lang.t(text)
    }

    fn say(&mut self, message: impl Into<String>) {
        self.status = Some((false, message.into()));
    }

    fn fail(&mut self, message: impl Into<String>) {
        self.status = Some((true, format!("{}: {}", self.t(Text::Error), message.into())));
    }

    /// Reads what the screens show from the workspace.
    pub fn refresh(&mut self) {
        let Some(books) = self.books.clone() else { return };
        let Ok(set) = self.workspace.get(&books) else { return };
        let events = set.events();
        self.owner = events.owners().first().map(|owner| (*owner).clone());
        let Some(owner) = self.owner.clone() else { return };
        let (first, last) =
            (Date::new(self.year, 1, 1).expect("a date"), Date::new(self.year, 12, 31).expect("a date"));
        let (_, persons) = events.parties();
        let (organisations, _) = events.parties();
        self.view.owner_name = persons
            .get(&owner)
            .map(|p| p.name.clone())
            .or_else(|| organisations.get(&owner).map(|o| o.name.clone()))
            .unwrap_or_else(|| owner.to_string());
        let date = today().max(first).min(last);
        self.view.chart = self.workspace.national_chart(&books, &owner, date).ok().flatten().map(|t| t.chart);
        self.view.journal = set.journal(Some(&owner), Some(first), Some(last));
        self.view.issues = self.workspace.review_in(&books, Some(&owner), self.lang.code()).unwrap_or_default();
        self.view.closed = events.closed_until(&owner);
        self.view.relay = self.workspace.sync_relay(&books).ok().flatten();
        self.view.documents = self.workspace.documents(&books).unwrap_or_default();
        self.view.accounts = self.workspace.account_choices(&books, &owner, date, self.lang.code()).unwrap_or_default();
        let by_id: BTreeMap<&AccountId, &AccountChoice> = self.view.accounts.iter().map(|a| (&a.id, a)).collect();
        let balances = events.functional_balances(Some(last)).unwrap_or_default();
        self.view.balances = balances
            .iter()
            .filter(|(party, _, _, units)| **party == owner && *units != 0)
            .map(|(_, account, currency, units)| {
                let choice = by_id.get(account);
                let number = choice.and_then(|c| c.number.clone()).unwrap_or_default();
                let name = choice.map_or_else(|| account.to_string(), |c| c.name.clone());
                (account.clone(), number, name, Money::new(units, currency.clone()).expect("a balance"))
            })
            .collect();
        self.view.balances.sort_by(|a, b| (a.1.len(), &a.1).cmp(&(b.1.len(), &b.1)));
        let money = |units: i128| Money::new(units, eur()).expect("an amount");
        self.view.money = self.workspace.standard_accounts(&books, &owner, date).ok().map(|standard| {
            let balance = |account: &AccountId| balances.get(&owner, account, &eur());
            let (income, expenses) = set.income_and_expenses(self.workspace.chart(), &owner, first, last);
            let bank = self.workspace.bank_holdings(&books, &owner, last).unwrap_or_else(|_| balance(&standard.bank));
            (money(bank), money(balance(&standard.cash)), money(income), money(expenses))
        });
        let returns = self.workspace.vat_returns(&books, &owner, self.year).unwrap_or_default();
        let mut reports: Vec<ReportItem> = returns
            .iter()
            .map(|r| ReportItem {
                id: "de-ustva",
                name: self.t(Text::VatReturn),
                from: r.from,
                to: r.to,
                due: Some(r.due),
            })
            .collect();
        let annual = self.workspace.reports(&books, &owner, first, last).unwrap_or_default();
        reports.extend(annual.into_iter().filter(|(id, _)| *id != "de-ustva").map(|(id, name)| ReportItem {
            id,
            name,
            from: first,
            to: last,
            due: None,
        }));
        self.view.reports = reports;
        let mut upcoming = self.workspace.vat_returns(&books, &owner, today().year()).unwrap_or_default();
        upcoming.retain(|r| r.due >= today());
        self.view.next_return = upcoming.first().map(|r| ReportItem {
            id: "de-ustva",
            name: self.t(Text::VatReturn),
            from: r.from,
            to: r.to,
            due: Some(r.due),
        });
        let names = Names::v0_1(self.lang.code());
        let mut codes = vec![(self.t(Text::NoTax).to_owned(), String::new())];
        if let Ok(configuration) = events.configuration(&owner, date).ok_or(()) {
            let small = configuration.vat.as_ref().is_some_and(|vat| vat.scheme.as_str() == "de-small-business");
            for code in [
                "de-in-19",
                "de-in-7",
                "de-out-19",
                "de-out-7",
                "de-out-0",
                "de-out-exempt",
                "de-out-13b",
                "de-out-eu-service",
                "de-out-ig-supply",
                "de-out-export",
                "de-out-small-business",
                "de-in-no-deduction",
            ] {
                if small && code.starts_with("de-out-") && code != "de-out-small-business" {
                    continue;
                }
                let name = names.as_ref().and_then(|n| n.get(code)).unwrap_or(code);
                codes.push((format!("{code} – {name}"), code.to_owned()));
            }
        }
        self.view.tax_codes = codes;
        self.view.open_entries = self.workspace.open_entries(&books, &owner).unwrap_or_default();
    }

    /// Handles a key.
    pub fn handle(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => self.normal(key),
            Mode::Form(purpose, form) => self.in_form(purpose, form, key),
            Mode::Figures(figures) => match key.code {
                KeyCode::Char('e') => {
                    let name = format!("{}.csv", figures.file);
                    self.mode = Mode::Form(
                        Purpose::ExportReport,
                        Form::new(Text::KeyExport, vec![Field::text(Text::Path, name)]),
                    );
                    self.pending_figures = Some(figures);
                }
                KeyCode::Esc | KeyCode::Char('q') => {}
                _ => self.mode = Mode::Figures(figures),
            },
            Mode::Ledger(account) => {
                if !matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.mode = Mode::Ledger(account);
                }
            }
            Mode::Proposal(proposal) => self.in_proposal(*proposal, key),
            Mode::Notice(title, lines) => {
                if !matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                    self.mode = Mode::Notice(title, lines);
                }
            }
        }
    }
}

impl App<'_> {
    /// The rows of the tab shown.
    pub fn rows(&self) -> usize {
        match self.tab {
            Tab::Journal => self.view.journal.len(),
            Tab::Bank => self.view.open_entries.len(),
            Tab::Documents => self.view.documents.len(),
            Tab::Accounts => self.view.balances.len(),
            Tab::Reports => self.view.reports.len(),
            Tab::Review => self.view.issues.len(),
            Tab::Overview | Tab::Closing => 0,
        }
    }

    fn normal(&mut self, key: KeyEvent) {
        self.status = None;
        let selected = self.selected[self.tab.index()];
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(c @ '1'..='8') => self.tab = Tab::ALL[c as usize - '1' as usize],
            KeyCode::Tab => self.tab = Tab::ALL[(self.tab.index() + 1) % Tab::ALL.len()],
            KeyCode::BackTab => self.tab = Tab::ALL[(self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len()],
            KeyCode::Char('[') => {
                self.year -= 1;
                self.refresh();
            }
            KeyCode::Char(']') => {
                self.year += 1;
                self.refresh();
            }
            KeyCode::Char('m') => self.mouse = !self.mouse,
            KeyCode::Char('s') => {
                self.lang = self.lang.other();
                let _ = self.workspace.set_language(self.lang.code());
                self.refresh();
            }
            KeyCode::Up => self.selected[self.tab.index()] = selected.saturating_sub(1),
            KeyCode::Down => self.selected[self.tab.index()] = (selected + 1).min(self.rows().saturating_sub(1)),
            KeyCode::Char('n') => self.mode = Mode::Form(Purpose::Simple(None), self.simple_form(None)),
            KeyCode::Char('z') => self.mode = Mode::Form(Purpose::Lines, self.lines_form()),
            KeyCode::Char('i') if self.tab == Tab::Bank => {
                let fields = vec![Field::text(Text::Path, ""), Field::text(Text::Iban, "")];
                self.mode = Mode::Form(Purpose::ReadStatement, Form::new(Text::ReadStatement, fields));
            }
            KeyCode::Char('i') => self.mode = Mode::Form(Purpose::ReadDocument, self.read_form()),
            KeyCode::Char('a') if self.tab == Tab::Bank => {
                let (Some(books), Some(owner)) = (self.books.clone(), self.owner.clone()) else { return };
                match self.workspace.accept_suggestions(&books, &owner) {
                    Ok(count) => {
                        self.say(format!("{}: {count}", self.t(Text::Accepted)));
                        self.refresh();
                    }
                    Err(error) => self.fail(error.to_string()),
                }
            }
            KeyCode::Char('f') if self.tab == Tab::Bank => {
                let Some(books) = self.books.clone() else { return };
                let iban = self.view.open_entries.first().map(|e| e.iban.clone()).unwrap_or_default();
                if let Err(error) = self.workspace.fetch_statements(&books, &iban) {
                    self.fail(error.to_string());
                }
            }
            KeyCode::Char('r') if self.tab == Tab::Journal => {
                if let Some(entry) = self.view.journal.get(selected)
                    && entry.reverses.is_none()
                    && entry.reversed_by.is_none()
                {
                    let form = Form::new(
                        Text::ReverseBooking,
                        vec![Field::text(Text::Date, self.lang.date(today())), Field::text(Text::Reason, "")],
                    );
                    self.mode = Mode::Form(Purpose::Reverse(entry.event), form);
                }
            }
            KeyCode::Char('p') if self.tab == Tab::Documents => {
                self.mode = Mode::Form(Purpose::Profile, self.profile_form());
            }
            KeyCode::Char('k') if self.tab == Tab::Documents => {
                let fields = [
                    Text::Name,
                    Text::Street,
                    Text::Postcode,
                    Text::City,
                    Text::Country,
                    Text::Email,
                    Text::VatId,
                    Text::Reference,
                ]
                .into_iter()
                .map(|label| Field::text(label, if label == Text::Country { "DE" } else { "" }))
                .collect();
                self.mode = Mode::Form(Purpose::Customer, Form::new(Text::NewCustomer, fields));
            }
            KeyCode::Char('r') if self.tab == Tab::Documents => {
                let Some(books) = self.books.clone() else { return };
                if !matches!(self.workspace.invoice_profile(&books), Ok(Some(_))) {
                    self.mode = Mode::Form(Purpose::Profile, self.profile_form());
                    return;
                }
                let customers: Vec<(String, String)> =
                    self.workspace.customers(&books).unwrap_or_default().into_iter().map(|c| (c.name, c.id)).collect();
                if customers.is_empty() {
                    self.fail(self.t(Text::NoCustomers).to_owned());
                    return;
                }
                let option = |text: Text, value: &str| (self.t(text).to_owned(), value.to_owned());
                let names = Names::v0_1(self.lang.code());
                let mut invoice_codes: Vec<(String, String)> = spezado_app::INVOICE_TAX_CODES
                    .iter()
                    .map(|(code, _)| {
                        (names.as_ref().and_then(|n| n.get(code)).unwrap_or(code).to_owned(), (*code).to_owned())
                    })
                    .collect();
                invoice_codes.push(option(Text::NoTax, ""));
                let fields = vec![
                    Field::choice(Text::Customer, customers, 0),
                    Field::text(Text::Date, self.lang.date(today())),
                    Field::text(Text::Service, ""),
                    Field::text(Text::Quantity, "1"),
                    Field::choice(
                        Text::Unit,
                        vec![option(Text::Hours, "HUR"), option(Text::Days, "DAY"), option(Text::Pieces, "C62")],
                        0,
                    ),
                    Field::text(Text::UnitPrice, ""),
                    Field::choice(Text::TaxCode, invoice_codes, 0),
                    Field::choice(
                        Text::InvoiceFile,
                        vec![
                            ("XRechnung (XML)".into(), "xrechnung".into()),
                            ("ZUGFeRD (PDF)".into(), "zugferd".into()),
                        ],
                        0,
                    ),
                ];
                self.mode = Mode::Form(Purpose::Invoice, Form::new(Text::NewInvoice, fields));
            }
            KeyCode::Char('y') if self.tab == Tab::Overview => {
                let Some(books) = self.books.clone() else { return };
                if self.view.relay.is_none() {
                    let fields = vec![Field::text(Text::Relay, "https://"), Field::text(Text::DeviceName, "")];
                    let mut form = Form::new(Text::SyncStart, fields);
                    form.help = Some(Text::SyncHelp);
                    self.mode = Mode::Form(Purpose::SyncStart, form);
                    return;
                }
                match self.workspace.sync(&books) {
                    Ok(report) => {
                        let added = if report.added.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}: {}", self.t(Text::Added), report.added.join(", "))
                        };
                        self.say(format!(
                            "{}: {} ↓ · {} ↑{added}",
                            self.t(Text::Synchronised),
                            report.received,
                            report.sent
                        ));
                        self.refresh();
                    }
                    Err(error) => self.fail(error.to_string()),
                }
            }
            KeyCode::Char('v') if self.tab == Tab::Overview && self.view.relay.is_some() => {
                let Some(books) = self.books.clone() else { return };
                match self.workspace.invite(&books) {
                    Ok(invitation) => {
                        let lines = vec![invitation, String::new(), self.t(Text::InvitationHelp).to_owned()];
                        self.mode = Mode::Notice(Text::Invitation, lines);
                    }
                    Err(error) => self.fail(error.to_string()),
                }
            }
            KeyCode::Char('d') if self.tab == Tab::Overview => {
                let (Some(books), Some(owner)) = (self.books.clone(), self.owner.clone()) else { return };
                let Ok(set) = self.workspace.get(&books) else { return };
                let configuration = set.events().configuration(&owner, today()).cloned();
                let region = configuration.as_ref().and_then(|c| c.region.as_ref()).map(ToString::to_string);
                let extended = configuration.as_ref().and_then(|c| c.vat.as_ref()).is_some_and(|v| v.extended);
                let regions = self.regions();
                let at = regions.iter().position(|(_, code)| Some(code) == region.as_ref()).unwrap_or(0);
                let option = |text: Text, value: &str| (self.t(text).to_owned(), value.to_owned());
                let fields = vec![
                    Field::text(Text::ValidFrom, self.lang.date(today())),
                    Field::choice(Text::Region, regions, at),
                    Field::choice(
                        Text::Extension,
                        vec![option(Text::No, ""), option(Text::Yes, "extended")],
                        usize::from(extended),
                    ),
                ];
                self.mode = Mode::Form(Purpose::Deadlines, Form::new(Text::Deadlines, fields));
            }
            KeyCode::Char('o') if self.tab == Tab::Accounts => {
                let (Some(books), Some(owner)) = (self.books.clone(), self.owner.clone()) else { return };
                let start = self.workspace.books_start(&books, &owner).ok().flatten().unwrap_or_else(today);
                let fields = vec![Field::text(Text::Date, self.lang.date(start)), Field::text(Text::Account, "")];
                let mut form = Form::new(Text::OpeningBalances, fields);
                form.help = Some(Text::OpeningHelp);
                self.mode = Mode::Form(Purpose::Opening, form);
            }
            KeyCode::Char('c') if self.tab == Tab::Closing => {
                let form =
                    Form::new(Text::ClosePeriod, vec![Field::text(Text::CloseUpTo, self.lang.date(last_month_end()))]);
                self.mode = Mode::Form(Purpose::Close, form);
            }
            KeyCode::Char('g') if self.tab == Tab::Closing => {
                let fields = vec![
                    Field::text(Text::Directory, format!("gobd-{}", self.year)),
                    Field::text(Text::From, self.lang.date(Date::new(self.year, 1, 1).expect("a date"))),
                    Field::text(Text::To, self.lang.date(Date::new(self.year, 12, 31).expect("a date"))),
                ];
                self.mode = Mode::Form(Purpose::Gobd, Form::new(Text::GobdExport, fields));
            }
            KeyCode::Enter => self.open(selected),
            _ => {}
        }
    }

    fn open(&mut self, selected: usize) {
        match self.tab {
            Tab::Bank => {
                let Some(item) = self.view.open_entries.get(selected).cloned() else { return };
                match &item.suggestion {
                    Some(suggestion) => {
                        let fields = vec![
                            Field::text(Text::Date, self.lang.date(suggestion.booking.date)),
                            Field::text(Text::Text_, suggestion.booking.text.clone()),
                        ];
                        let form = Form::new(Text::Proposal, fields);
                        let (booking, source) = (suggestion.booking.clone(), Some(suggestion.source.clone()));
                        self.mode = Mode::Proposal(Box::new(Proposal { document: None, booking, source, form }));
                    }
                    None => {
                        let fields = vec![
                            Field::text(Text::Text_, item.text()),
                            Field::account(Text::Account, ""),
                            Field::choice(Text::TaxCode, self.view.tax_codes.clone(), 0),
                        ];
                        self.mode = Mode::Form(Purpose::Entry(Box::new(item)), Form::new(Text::BookEntry, fields));
                    }
                }
            }
            Tab::Accounts => {
                if let Some((account, ..)) = self.view.balances.get(selected) {
                    self.mode = Mode::Ledger(account.clone());
                }
            }
            Tab::Reports => {
                if let Some(item) = self.view.reports.get(selected).cloned() {
                    self.open_report(&item);
                }
            }
            _ => {}
        }
    }

    fn open_report(&mut self, item: &ReportItem) {
        let (Some(books), Some(owner)) = (self.books.clone(), self.owner.clone()) else { return };
        match self.workspace.report(&books, &owner, item.id, item.from, item.to) {
            Ok(figures) => {
                let names = Names::v0_1(self.lang.code());
                let rows = figures
                    .fields
                    .iter()
                    .map(|(field, units)| {
                        let key = format!("{}/{field}", figures.id);
                        let name = names.as_ref().and_then(|n| n.get(&key)).unwrap_or("").to_owned();
                        (field.clone(), name, Money::new(*units, figures.currency.clone()).expect("an amount"))
                    })
                    .collect();
                let title = format!("{} {} – {}", item.name, self.lang.date(item.from), self.lang.date(item.to));
                let file = format!("{}-{}-{}", item.id, item.from, item.to);
                self.mode = Mode::Figures(Figures { title, law: figures.law, rows, file });
            }
            Err(error) => self.fail(error.to_string()),
        }
    }

    fn in_form(&mut self, purpose: Purpose, mut form: Form, key: KeyEvent) {
        match form.handle(key, &self.view.accounts) {
            Outcome::Editing => self.mode = Mode::Form(purpose, form),
            Outcome::Cancel => {
                if purpose == Purpose::SetUp && self.books.is_none() {
                    self.quit = true;
                }
            }
            Outcome::AccountTaken(index) => {
                self.follow_account(&mut form, index);
                self.mode = Mode::Form(purpose, form);
            }
            Outcome::Submit => match self.submit(&purpose, &form) {
                Ok(()) => self.refresh(),
                Err(message) => {
                    self.fail(message);
                    self.mode = Mode::Form(purpose, form);
                }
            },
        }
    }

    /// After an account is chosen: a tax code nobody chose follows its kind.
    fn follow_account(&self, form: &mut Form, index: usize) {
        if form.fields[index].label != Text::Account || !form.fields.iter().any(|f| f.label == Text::TaxCode) {
            return;
        }
        let Some(account) = resolve(&form.fields[index].value, &self.view.accounts) else { return };
        let tax = form.field_mut(Text::TaxCode);
        if tax.touched {
            return;
        }
        let code = match account.kind {
            librotenado::AccountKind::Income => "de-out-19",
            librotenado::AccountKind::Expense => "de-in-19",
            _ => "",
        };
        tax.choose(code);
    }

    fn in_proposal(&mut self, mut proposal: Proposal, key: KeyEvent) {
        let Some(books) = self.books.clone() else { return };
        match key.code {
            KeyCode::Char('a') if proposal.form.matches.is_empty() && proposal.document.is_some() => {
                let document = proposal.document.expect("a document");
                match self.workspace.record_document(&books, document, None) {
                    Ok(_) => {
                        self.say(self.t(Text::Filed));
                        self.refresh();
                    }
                    Err(error) => self.fail(error.to_string()),
                }
            }
            _ => match proposal.form.handle(key, &self.view.accounts) {
                Outcome::Cancel => {}
                Outcome::Submit => {
                    let date = self.lang.parse_date(&proposal.form.field(Text::Date).value, self.year);
                    let Some(date) = date else {
                        self.fail(self.t(Text::InvalidDate));
                        self.mode = Mode::Proposal(Box::new(proposal));
                        return;
                    };
                    let mut booking = proposal.booking.clone();
                    booking.date = date;
                    booking.text = proposal.form.field(Text::Text_).value.clone();
                    let written = match (proposal.document.clone(), proposal.source.clone()) {
                        (Some(document), _) => self.workspace.record_document(&books, document, Some(booking)),
                        (None, Some(source)) => {
                            self.workspace.accept_suggestion(&books, &Suggestion { source, booking })
                        }
                        (None, None) => self.workspace.execute(&books, Command::Book(booking)).map(|event| vec![event]),
                    };
                    match written {
                        Ok(_) => {
                            self.say(self.t(Text::Booked));
                            self.refresh();
                        }
                        Err(error) => {
                            self.fail(error.to_string());
                            self.mode = Mode::Proposal(Box::new(proposal));
                        }
                    }
                }
                _ => self.mode = Mode::Proposal(Box::new(proposal)),
            },
        }
    }

    fn submit(&mut self, purpose: &Purpose, form: &Form) -> Result<(), String> {
        match purpose {
            Purpose::SetUp => self.submit_set_up(form),
            Purpose::Simple(document) => self.submit_simple(form, document.as_deref()),
            Purpose::Lines => self.submit_lines(form),
            Purpose::Opening => self.submit_opening(form),
            Purpose::SyncStart => {
                let books = self.books.clone().ok_or("no books")?;
                let relay = form.field(Text::Relay).value.trim().to_owned();
                let name = form.field(Text::DeviceName).value.trim().to_owned();
                let name = (!name.is_empty()).then_some(name);
                let code = self.workspace.start_sync(&books, &relay, name.as_deref()).map_err(|e| e.to_string())?;
                let lines = vec![code.to_string(), String::new(), self.t(Text::RecoveryHelp).to_owned()];
                self.mode = Mode::Notice(Text::RecoveryCode, lines);
                Ok(())
            }
            Purpose::Deadlines => {
                let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
                let from = self.date(form, Text::ValidFrom)?;
                let region = form.field(Text::Region).get();
                let region =
                    if region.is_empty() { None } else { Some(region.parse().map_err(|_| self.t(Text::Invalid))?) };
                let extended = form.field(Text::Extension).get() == "extended";
                self.workspace.change_deadlines(&books, &owner, from, region, extended).map_err(|e| e.to_string())?;
                self.say(self.t(Text::DeadlinesChanged));
                Ok(())
            }
            Purpose::Profile => self.submit_profile(form),
            Purpose::Customer => {
                let books = self.books.clone().ok_or("no books")?;
                let value = |label: Text| form.field(label).value.trim().to_owned();
                let optional = |label: Text| Some(value(label)).filter(|v| !v.is_empty());
                let customer = spezado_app::Customer {
                    id: String::new(),
                    name: value(Text::Name),
                    street: value(Text::Street),
                    postcode: value(Text::Postcode),
                    city: value(Text::City),
                    country: value(Text::Country).to_uppercase(),
                    email: value(Text::Email),
                    vat_id: optional(Text::VatId),
                    reference: optional(Text::Reference),
                };
                self.workspace.save_customer(&books, customer).map_err(|e| e.to_string())?;
                self.say(self.t(Text::Saved));
                Ok(())
            }
            Purpose::Invoice => self.submit_invoice(form),
            Purpose::ReadStatement => {
                let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
                let path = PathBuf::from(form.field(Text::Path).value.trim());
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let iban = Some(form.field(Text::Iban).value.trim()).filter(|i| !i.is_empty());
                let name = path.file_name().and_then(|n| n.to_str());
                let imported =
                    self.workspace.import_statements(&books, &owner, &bytes, name, iban).map_err(|e| e.to_string())?;
                self.say(format!(
                    "{}: {} · {}: {}",
                    self.t(Text::Imported),
                    imported.entries,
                    self.t(Text::ShownBefore),
                    imported.shown_before + imported.known,
                ));
                Ok(())
            }
            Purpose::Entry(item) => {
                let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
                let account = self.account(&form.field(Text::Account).value)?;
                let tax = form.field(Text::TaxCode).get();
                let tax = if tax.is_empty() { None } else { Some(tax.parse().map_err(|_| self.t(Text::Invalid))?) };
                let text = form.field(Text::Text_).value.trim().to_owned();
                let booking = self
                    .workspace
                    .entry_booking(&books, &owner, item, account, tax, text)
                    .map_err(|e| e.to_string())?;
                self.execute(Command::Book(booking))?;
                self.say(self.t(Text::Booked));
                Ok(())
            }
            Purpose::Reverse(event) => {
                let date = self.date(form, Text::Date)?;
                let text = form.field(Text::Reason).value.clone();
                self.execute(Command::Reverse { event: *event, date, text })?;
                self.say(self.t(Text::Booked));
                Ok(())
            }
            Purpose::ReadDocument => self.submit_read(form),
            Purpose::Close => {
                let to = self.date(form, Text::CloseUpTo)?;
                let owner = self.owner.clone().ok_or("no owner")?;
                let software = self.workspace.software();
                self.execute(Command::Close { owner, to, software })?;
                self.say(format!("{} {}", self.t(Text::Closed), self.lang.date(to)));
                Ok(())
            }
            Purpose::Gobd => {
                let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
                let (from, to) = (self.date(form, Text::From)?, self.date(form, Text::To)?);
                let directory = PathBuf::from(form.field(Text::Directory).value.trim());
                self.workspace.gobd_export(&books, &owner, from, to, &directory).map_err(|e| e.to_string())?;
                self.say(format!("{} {}", self.t(Text::Exported), directory.display()));
                Ok(())
            }
            Purpose::ExportReport => {
                let figures = self.pending_figures.take().ok_or("no report")?;
                let path = PathBuf::from(form.field(Text::Path).value.trim());
                let mut csv = String::from("field;name;amount\n");
                for (field, name, amount) in &figures.rows {
                    csv.push_str(&format!(
                        "{field};\"{}\";{}\n",
                        name.replace('"', "\"\""),
                        spezado_core::format_amount(amount)
                    ));
                }
                std::fs::write(&path, csv).map_err(|e| format!("{}: {e}", path.display()))?;
                self.say(format!("{} {}", self.t(Text::Exported), path.display()));
                Ok(())
            }
        }
    }

    fn execute(&mut self, command: Command) -> Result<EventHash, String> {
        let books = self.books.clone().ok_or("no books")?;
        self.workspace.execute(&books, command).map_err(|e| e.to_string())
    }

    fn date(&self, form: &Form, label: Text) -> Result<Date, String> {
        self.lang
            .parse_date(&form.field(label).value, self.year)
            .ok_or_else(|| format!("{}: {}", self.t(label), self.t(Text::InvalidDate)))
    }

    fn amount(&self, text: &str) -> Result<Money, String> {
        self.lang.parse_amount(text, &eur()).map_err(|e| format!("{}: {e}", self.t(Text::InvalidAmount)))
    }

    fn account(&self, text: &str) -> Result<AccountId, String> {
        resolve(text, &self.view.accounts)
            .map(|a| a.id.clone())
            .ok_or_else(|| format!("{}: {text}", self.t(Text::UnknownAccount)))
    }

    /// The bank or the cash account, as chosen in a form.
    fn payment(&self, form: &Form) -> Result<AccountId, String> {
        let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
        let standard = self.workspace.standard_accounts(&books, &owner, today()).map_err(|e| e.to_string())?;
        Ok(if form.field(Text::Payment).get() == "cash" { standard.cash } else { standard.bank })
    }

    fn submit_set_up(&mut self, form: &Form) -> Result<(), String> {
        let name = form.field(Text::Name).value.trim().to_owned();
        if name.is_empty() {
            return Err(format!("{}: {}", self.t(Text::Name), self.t(Text::Invalid)));
        }
        let from = self.date(form, Text::StartDate)?;
        let id: PartyId = new_id();
        let mut configuration = serde_json::json!({
            "owner": id.as_str(), "from": from.to_string(), "currency": "EUR", "year_start": 1,
            "accounting": form.field(Text::Accounting).get(), "chart": form.field(Text::Chart).get(),
            "vat": {
                "scheme": form.field(Text::VatScheme).get(), "basis": form.field(Text::VatBasis).get(),
                "period": form.field(Text::VatPeriod).get(),
            },
        });
        if form.field(Text::VatScheme).get() == "de-small-business" {
            configuration["vat"]["period"] = "year".into();
        } else if form.field(Text::Extension).get() == "extended" {
            configuration["vat"]["extended"] = true.into();
        }
        if !form.field(Text::Region).get().is_empty() {
            configuration["region"] = form.field(Text::Region).get().into();
        }
        let configuration = serde_json::from_value(configuration).map_err(|e| e.to_string())?;
        let person = Person { id, name, tax_ids: vec![], roles: vec![], contact: None };
        let books = self.workspace.set_up(Party::Person(person), configuration).map_err(|e: AppError| e.to_string())?;
        self.books = Some(books);
        self.year = from.year();
        Ok(())
    }

    fn submit_simple(&mut self, form: &Form, document: Option<&Document>) -> Result<(), String> {
        let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
        let tax = form.field(Text::TaxCode).get();
        let entry = SimpleEntry {
            owner,
            date: self.date(form, Text::Date)?,
            text: form.field(Text::Text_).value.trim().to_owned(),
            account: self.account(&form.field(Text::Account).value)?,
            gross: self.amount(&form.field(Text::Gross).value)?,
            tax: if tax.is_empty() { None } else { Some(tax.parse().map_err(|_| self.t(Text::Invalid))?) },
            payment: self.payment(form)?,
            documents: document.map(|d| vec![d.id.clone()]).unwrap_or_default(),
        };
        let booking = self.workspace.simple_booking(&books, &entry).map_err(|e| e.to_string())?;
        match document {
            Some(document) => {
                self.workspace.record_document(&books, document.clone(), Some(booking)).map_err(|e| e.to_string())?;
            }
            None => {
                self.execute(Command::Book(booking))?;
            }
        }
        self.say(self.t(Text::Booked));
        Ok(())
    }

    fn submit_lines(&mut self, form: &Form) -> Result<(), String> {
        let owner = self.owner.clone().ok_or("no owner")?;
        let mut postings = Vec::new();
        for line in form.field(Text::Account).value.split_whitespace() {
            let (account, rest) = line.split_once('=').ok_or_else(|| format!("{}: {line}", self.t(Text::Invalid)))?;
            let (amount, tax) = match rest.split_once('@') {
                Some((amount, tax)) => {
                    (amount, Some(tax.parse().map_err(|_| format!("{}: {tax}", self.t(Text::Invalid)))?))
                }
                None => (rest, None),
            };
            postings.push(Posting {
                owner: owner.clone(),
                account: self.account(account)?,
                amount: self.amount(amount)?,
                note: None,
                tax,
                exchange: None,
            });
        }
        let booking = Booking {
            date: self.date(form, Text::Date)?,
            text: form.field(Text::Text_).value.trim().to_owned(),
            postings,
            documents: vec![],
            entries: vec![],
            transaction: None,
        };
        self.execute(Command::Book(booking))?;
        self.say(self.t(Text::Booked));
        Ok(())
    }

    fn profile_form(&self) -> Form {
        let profile =
            self.books.as_ref().and_then(|b| self.workspace.invoice_profile(b).ok().flatten()).unwrap_or_else(|| {
                spezado_app::InvoiceProfile {
                    name: self.view.owner_name.clone(),
                    country: "DE".into(),
                    payment_days: 14,
                    prefix: "RE-".into(),
                    ..Default::default()
                }
            });
        let fields = vec![
            Field::text(Text::Name, profile.name),
            Field::text(Text::Street, profile.street),
            Field::text(Text::Postcode, profile.postcode),
            Field::text(Text::City, profile.city),
            Field::text(Text::Country, profile.country),
            Field::text(Text::VatId, profile.vat_id.unwrap_or_default()),
            Field::text(Text::TaxNumber, profile.tax_number.unwrap_or_default()),
            Field::text(Text::Contact, profile.contact),
            Field::text(Text::Phone, profile.phone),
            Field::text(Text::Email, profile.email),
            Field::text(Text::BankIban, profile.iban),
            Field::text(Text::PaymentDays, profile.payment_days.to_string()),
            Field::text(Text::Prefix, profile.prefix),
        ];
        Form::new(Text::SellerDetails, fields)
    }

    fn submit_profile(&mut self, form: &Form) -> Result<(), String> {
        let books = self.books.clone().ok_or("no books")?;
        let value = |label: Text| form.field(label).value.trim().to_owned();
        let optional = |label: Text| Some(value(label)).filter(|v| !v.is_empty());
        let profile = spezado_app::InvoiceProfile {
            name: value(Text::Name),
            street: value(Text::Street),
            postcode: value(Text::Postcode),
            city: value(Text::City),
            country: value(Text::Country).to_uppercase(),
            vat_id: optional(Text::VatId),
            tax_number: optional(Text::TaxNumber),
            contact: value(Text::Contact),
            phone: value(Text::Phone),
            email: value(Text::Email),
            iban: value(Text::BankIban),
            payment_days: value(Text::PaymentDays)
                .parse()
                .map_err(|_| format!("{}: {}", self.t(Text::PaymentDays), self.t(Text::Invalid)))?,
            prefix: value(Text::Prefix),
        };
        self.workspace.set_invoice_profile(&books, &profile).map_err(|e| e.to_string())?;
        self.say(self.t(Text::Saved));
        Ok(())
    }

    fn submit_invoice(&mut self, form: &Form) -> Result<(), String> {
        let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
        // A quantity as people of the language write it: `2,5` or `2.5`.
        let typed = form.field(Text::Quantity).value.trim().replace(' ', "");
        let plain = match self.lang {
            Lang::De => typed.replace('.', "").replace(',', "."),
            Lang::En => typed.replace(',', ""),
        };
        let quantity = plain
            .parse::<librotenado::Decimal>()
            .map_err(|_| format!("{}: {}", self.t(Text::Quantity), self.t(Text::Invalid)))?;
        let tax = form.field(Text::TaxCode).get();
        let line = spezado_app::NewLine {
            name: form.field(Text::Service).value.trim().to_owned(),
            quantity,
            unit: form.field(Text::Unit).get().to_owned(),
            price: self.amount(&form.field(Text::UnitPrice).value)?,
            tax: if tax.is_empty() { None } else { Some(tax.parse().map_err(|_| self.t(Text::Invalid))?) },
        };
        let invoice = spezado_app::NewInvoice {
            customer: form.field(Text::Customer).get().to_owned(),
            date: self.date(form, Text::Date)?,
            delivery: None,
            lines: vec![line],
            note: None,
            exemption: None,
            format: if form.field(Text::InvoiceFile).get() == "zugferd" {
                spezado_app::InvoiceFormat::Zugferd
            } else {
                spezado_app::InvoiceFormat::XRechnung
            },
        };
        let written = self.workspace.issue_invoice(&books, &owner, &invoice).map_err(|e| e.to_string())?;
        self.say(format!("{} {}", written.document.number.unwrap_or_default(), self.t(Text::InvoiceWritten)));
        Ok(())
    }

    fn submit_opening(&mut self, form: &Form) -> Result<(), String> {
        let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
        let mut lines = Vec::new();
        for line in form.field(Text::Account).value.split_whitespace() {
            let (account, amount) = line.split_once('=').ok_or_else(|| format!("{}: {line}", self.t(Text::Invalid)))?;
            lines.push((self.account(account)?, self.amount(amount)?));
        }
        let date = self.date(form, Text::Date)?;
        let text = self.t(Text::OpeningBalances).to_owned();
        self.workspace.book_opening(&books, &owner, date, &lines, &text).map_err(|e| e.to_string())?;
        self.say(self.t(Text::OpeningBooked));
        Ok(())
    }

    fn submit_read(&mut self, form: &Form) -> Result<(), String> {
        let (books, owner) = (self.books.clone().ok_or("no books")?, self.owner.clone().ok_or("no owner")?);
        let path = PathBuf::from(form.field(Text::Path).value.trim());
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path.file_name().and_then(|n| n.to_str());
        let side = if form.field(Text::Direction).get() == "sale" { InvoiceSide::Sale } else { InvoiceSide::Purchase };
        let payment = self.payment(form)?;
        let ReadDocument { document, proposal, not_an_invoice } =
            self.workspace.read_document(&books, &owner, &bytes, name, side, &payment).map_err(|e| e.to_string())?;
        if let Some(reason) = not_an_invoice {
            self.say(format!("{}: {reason}", self.t(Text::NotAnInvoice)));
        }
        match proposal {
            Some(booking) => {
                let fields = vec![
                    Field::text(Text::Date, self.lang.date(today())),
                    Field::text(Text::Text_, booking.text.clone()),
                ];
                let form = Form::new(Text::Proposal, fields);
                self.mode =
                    Mode::Proposal(Box::new(Proposal { document: Some(document), booking, source: None, form }));
            }
            None => {
                if self.status.is_none() {
                    self.say(self.t(Text::NoProposal));
                }
                let form = self.simple_form(Some(&document));
                self.mode = Mode::Form(Purpose::Simple(Some(Box::new(document))), form);
            }
        }
        Ok(())
    }

    /// The form of a booking from a gross amount.
    pub fn simple_form(&self, document: Option<&Document>) -> Form {
        let text = document.and_then(|d| d.original.name.clone().or_else(|| d.number.clone())).unwrap_or_default();
        let fields = vec![
            Field::text(Text::Date, self.lang.date(today())),
            Field::text(Text::Text_, text),
            Field::account(Text::Account, ""),
            Field::text(Text::Gross, ""),
            Field::choice(Text::TaxCode, self.view.tax_codes.clone(), 0),
            self.payment_field(),
        ];
        Form::new(Text::NewBooking, fields)
    }

    fn lines_form(&self) -> Form {
        let fields = vec![
            Field::text(Text::Date, self.lang.date(today())),
            Field::text(Text::Text_, ""),
            Field::text(Text::Account, ""),
        ];
        let mut form = Form::new(Text::NewLines, fields);
        form.help = Some(Text::LinesHelp);
        form
    }

    fn read_form(&self) -> Form {
        let directions = vec![
            (self.t(Text::Purchase).to_owned(), "purchase".to_owned()),
            (self.t(Text::Sale).to_owned(), "sale".to_owned()),
        ];
        let fields =
            vec![Field::text(Text::Path, ""), Field::choice(Text::Direction, directions, 0), self.payment_field()];
        Form::new(Text::ReadDocument, fields)
    }

    fn payment_field(&self) -> Field {
        let options = vec![
            (self.t(Text::Bank).to_owned(), "bank".to_owned()),
            (self.t(Text::Cash).to_owned(), "cash".to_owned()),
        ];
        Field::choice(Text::Payment, options, 0)
    }

    /// The form that sets up a freelancer's books.
    pub fn set_up_form(&self) -> Form {
        let option = |text: Text, value: &str| (self.t(text).to_owned(), value.to_owned());
        let fields = vec![
            Field::text(Text::Name, ""),
            Field::text(Text::StartDate, self.lang.date(Date::new(today().year(), 1, 1).expect("a date"))),
            Field::choice(
                Text::Accounting,
                vec![option(Text::CashBasis, "de-cash-basis"), option(Text::DoubleEntry, "de-double-entry")],
                0,
            ),
            Field::choice(
                Text::VatScheme,
                vec![option(Text::Regular, "de-regular"), option(Text::SmallBusiness, "de-small-business")],
                0,
            ),
            Field::choice(Text::VatBasis, vec![option(Text::CashVat, "cash"), option(Text::Accrual, "accrual")], 0),
            Field::choice(
                Text::VatPeriod,
                vec![option(Text::Quarterly, "quarter"), option(Text::Monthly, "month"), option(Text::Yearly, "year")],
                0,
            ),
            Field::choice(
                Text::Chart,
                vec![("SKR 03".into(), "de-skr03".into()), ("SKR 04".into(), "de-skr04".into())],
                0,
            ),
            Field::choice(Text::Extension, vec![option(Text::No, ""), option(Text::Yes, "extended")], 0),
            Field::choice(Text::Region, self.regions(), 0),
        ];
        Form::new(Text::SetUp, fields)
    }

    /// The states to choose from, none first.
    fn regions(&self) -> Vec<(String, String)> {
        let states = GERMAN_STATES
            .iter()
            .map(|(code, de, en)| ((if self.lang == Lang::De { *de } else { *en }).to_owned(), (*code).to_owned()));
        std::iter::once((self.t(Text::NoRegion).to_owned(), String::new())).chain(states).collect()
    }

    /// The bookings of an account, for the ledger: date, text, amount, and the running balance.
    pub fn ledger(&self, account: &AccountId) -> Vec<(Date, String, Money, Money)> {
        let mut balance = 0i128;
        let mut rows = Vec::new();
        for entry in &self.view.journal {
            for posting in
                entry.booking.postings.iter().filter(|p| p.account == *account && Some(&p.owner) == self.owner.as_ref())
            {
                balance += posting.functional().base_units();
                let currency = posting.functional().currency().clone();
                rows.push((
                    entry.booking.date,
                    entry.booking.text.clone(),
                    posting.amount.clone(),
                    Money::new(balance, currency).expect("a balance"),
                ));
            }
        }
        rows
    }

    /// The name of an account with its number, as chosen.
    pub fn account_label(&self, account: &AccountId) -> String {
        match self.view.accounts.iter().find(|a| a.id == *account) {
            Some(choice) => format!("{:>6} {}", choice.number.as_deref().unwrap_or(""), choice.name),
            None => account.to_string(),
        }
    }

    /// The documents of a booking, for the journal.
    pub fn document_label(&self, id: &DocumentId) -> String {
        self.view.documents.iter().find(|(_, d)| d.id == *id).map_or_else(
            || id.to_string(),
            |(_, d)| d.number.clone().or_else(|| d.original.name.clone()).unwrap_or_else(|| id.to_string()),
        )
    }
}

impl App<'_> {
    /// Handles the mouse: a click does what the key or the row under it does, the wheel moves like the arrow keys.
    pub fn handle_mouse(&mut self, event: MouseEvent) {
        if !self.mouse {
            return;
        }
        let key = |code| KeyEvent::from(code);
        match event.kind {
            MouseEventKind::ScrollUp => self.handle(key(KeyCode::Up)),
            MouseEventKind::ScrollDown => self.handle(key(KeyCode::Down)),
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(target) = self.hits.at(event.column, event.row) else { return };
                self.click(target);
            }
            _ => {}
        }
    }

    fn click(&mut self, target: Target) {
        let key = |code| KeyEvent::from(code);
        let form = match &mut self.mode {
            Mode::Form(_, form) => Some(form),
            Mode::Proposal(proposal) => Some(&mut proposal.form),
            _ => None,
        };
        match (target, form) {
            (Target::Key(KeyCode::Null), _) => {}
            (Target::Key(KeyCode::Enter), Some(form)) => {
                // The button confirms the whole form, wherever the focus is.
                form.matches.clear();
                form.focus = form.fields.len() - 1;
                self.handle(key(KeyCode::Enter));
            }
            (Target::Key(code), _) => self.handle(key(code)),
            (Target::Field(i), Some(form)) => {
                form.focus = i;
                form.matches.clear();
            }
            (Target::Choice(i), Some(form)) => {
                form.focus = i;
                form.matches.clear();
                self.handle(key(KeyCode::Right));
            }
            (Target::Match(j), Some(form)) => {
                form.selected = j;
                self.handle(key(KeyCode::Enter));
            }
            (Target::Tab(tab), None) => {
                self.mode = Mode::Normal;
                self.tab = tab;
            }
            (Target::Row(row), None) => {
                let index = self.tab.index();
                if self.selected[index] == row {
                    // A click on the row selected opens it, like a double click.
                    self.handle(key(KeyCode::Enter));
                } else {
                    self.selected[index] = row;
                }
            }
            _ => {}
        }
    }
}

/// The last day of the month before this one.
fn last_month_end() -> Date {
    let today = today();
    let (year, month) = if today.month() == 1 { (today.year() - 1, 12) } else { (today.year(), today.month() - 1) };
    (28..=31).rev().find_map(|day| Date::new(year, month, day)).expect("a last day")
}
