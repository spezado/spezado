// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application layer of spezado: a workspace of books on one device. Every interface – the command line, the
//! terminal interface, the API – works through it.
//!
//! The workspace checks each command in the core, stamps the event it becomes (time, device, place in the device's
//! chain), stores it and only then shows it in the books. Several commands, in one set of books or in several, are
//! written all or none. Workflows that take more than one command – setting up books, a bill split between books –
//! live here as well.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use librotenado::{
    BalanceError, Booking, BooksId, Chart, ChartTranslation, Clock, Configuration, Country, Currency, Date, DeviceId,
    Event, EventHash, EventSet, ObjectError, Organisation, PartyId, Person, Transaction, TransactionId,
    review_transactions,
};
use spezado_core::{Books, Command, CommandError, Issue};
use spezado_law::{Facts, LawModule, Report, ReportBasis};
use spezado_store::{Store, StoreError};

mod assets;
mod bank;
mod contacts;
mod filing;
mod income;
mod invoice_pdf;
mod invoicing;
mod opening;
mod payroll;
mod review;
mod sync;
mod workflows;

pub use assets::{AssetItem, NewAsset};
pub use bank::{ImportedStatements, OpenEntry, Source, Suggestion};
pub use contacts::{ContactCard, ContactPerson, ContactSuggestion};
pub use filing::FilingItem;
pub use income::IncomeFacts;
pub use invoicing::{
    BOOKING_TAX_CODES, Customer, INVOICE_TAX_CODES, InvoiceFormat, InvoiceProfile, NewInvoice, NewLine, WrittenInvoice,
};
pub use payroll::NewEmployment;
pub use spezado_law_de::STATES as GERMAN_STATES;
pub use sync::{SyncMember, SyncReport, parse_invitation};
pub use workflows::{AccountChoice, ReadDocument, SimpleEntry, VatReturnDue, today};

/// Why the workspace refused or failed.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The store.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A command was refused; nothing was written.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// The directory holds no workspace yet.
    #[error("no workspace here yet")]
    NoWorkspace,
    /// The directory holds a workspace already.
    #[error("there is a workspace here already, of device {0}")]
    WorkspaceExists(DeviceId),
    /// No books of that identity in the workspace.
    #[error("no books {0} in the workspace")]
    UnknownBooks(BooksId),
    /// No national chart of that name for the books.
    #[error("no national chart {0:?} for these books")]
    UnknownChart(String),
    /// No configuration of the owner on the date.
    #[error("{owner} is not configured on {date}")]
    NotConfigured {
        /// The owner.
        owner: PartyId,
        /// The date.
        date: Date,
    },
    /// No law module applies to the owner.
    #[error("no law module applies to {0}")]
    NoLaw(PartyId),
    /// No report of that identity applies to the owner for the period.
    #[error("no report {0:?} applies for this period")]
    UnknownReport(String),
    /// An event to import could not be read.
    #[error("line {line}: {error}")]
    Import {
        /// The line, from 1.
        line: usize,
        /// What was wrong.
        error: ObjectError,
    },
    /// Imported events that found no books.
    #[error("the events found no books")]
    NotFounded,
    /// A sum does not fit the number range.
    #[error(transparent)]
    Balance(#[from] BalanceError),
    /// The owner works with no national chart the law module knows standard accounts of.
    #[error("no standard accounts for the chart the owner works with")]
    NoStandardAccounts,
    /// A tax code the tax table of the date does not know.
    #[error("tax code {0} is not known on that date")]
    UnknownTaxCode(librotenado::TaxCode),
    /// A tax code a gross amount cannot be split by – reverse charge, intra-Community acquisitions: book its lines.
    #[error("tax code {0} does not split a gross amount; book it line by line")]
    NeedsLines(librotenado::TaxCode),
    /// A file of statements could not be read.
    #[error("not a statement that can be read: {0}")]
    Statement(String),
    /// Fetching from the bank needs a FinTS product registration, which falkOSS can apply for once it is founded.
    #[error(
        "fetching from the bank needs a FinTS product registration, which there is none of yet; read statements from files"
    )]
    FinTsNotRegistered,
    /// Fetching from the bank is not built yet.
    #[error("fetching from the bank by FinTS is not built yet")]
    FinTsNotBuilt,
    /// An invoice that cannot be written.
    #[error("{0}")]
    Invoice(String),
    /// Opening balances that cannot be booked.
    #[error("{0}")]
    Opening(String),
    /// A fixed asset that cannot be registered, depreciated or disposed of.
    #[error("{0}")]
    Asset(String),
    /// An employment or payslip that cannot be described, computed or recorded.
    #[error("{0}")]
    Payroll(String),
    /// Synchronising through a relay failed.
    #[error("{0}")]
    Sync(String),
    /// Preparing, submitting or asking about a filing failed.
    #[error("{0}")]
    Adapter(String),
    /// Books this device may only read.
    #[error("this device may only read the books {0}")]
    ReadOnly(BooksId),
    /// Writing a file.
    #[error("{path}: {error}")]
    Write {
        /// The file.
        path: String,
        /// What went wrong.
        error: std::io::Error,
    },
}

/// The organisation or person whose books are set up.
#[derive(Debug, Clone)]
pub enum Party {
    /// An organisation.
    Organisation(Organisation),
    /// A person.
    Person(Person),
}

impl Party {
    fn id(&self) -> &PartyId {
        match self {
            Party::Organisation(organisation) => &organisation.id,
            Party::Person(person) => &person.id,
        }
    }
}

/// The figures of a report.
#[derive(Debug, Clone)]
pub struct ReportFigures {
    /// Its identity (`de-ustva`).
    pub id: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Where the names of its fields stand in the translation data: `<names>/<field>` (the report of its mapping).
    pub names: String,
    /// Position or field → amount, in base units of `currency`.
    pub fields: BTreeMap<String, i128>,
    /// The owner's functional currency.
    pub currency: Currency,
    /// The law module, `name/version`.
    pub law: String,
    /// For a balance sheet: the remaining terms of the amounts on each position (§ 268 Abs. 4 and 5 HGB).
    pub terms: BTreeMap<String, librotenado::Terms>,
    /// For a balance sheet: how the tax balance sheet differs per position, from bookings in one of them only.
    pub reconciliation: BTreeMap<String, i128>,
}

/// An owner's configuration on a date, its country and legal form if an organisation, and the law module that
/// applies.
type OwnerLaw<'a> = (Configuration, Option<Country>, Option<String>, &'a dyn LawModule);

/// The books of one device: the store, the device's clock and chain, the harmonised chart and the law modules.
#[derive(Debug)]
pub struct Workspace {
    store: Store,
    device: DeviceId,
    clock: Clock,
    chart: Chart,
    law: Vec<Box<dyn LawModule>>,
    books: BTreeMap<BooksId, Books>,
    /// Whether member seeds go into the operating system's key store (off with `SPEZADO_KEYCHAIN=0`).
    keychain: bool,
}

/// The harmonised chart books are set up with.
const CHART: &str = "librotenado/0.1";

/// A new identity: a UUID of version 7, as the protocol recommends for new identities.
pub fn new_id<T: FromStr>() -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::now_v7().to_string().parse().expect("a UUID in its one form")
}

/// Milliseconds since 1970-01-01 UTC by the device's clock.
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

impl Workspace {
    /// Creates a workspace in `directory` for this device.
    pub fn create(directory: &Path, device: DeviceId) -> Result<Self, AppError> {
        let store = Store::open(directory)?;
        if let Some(existing) = store.setting(None, "device")? {
            return Err(AppError::WorkspaceExists(existing.parse().map_err(|_| StoreError::Identity(existing))?));
        }
        store.set_setting(None, "device", device.as_str())?;
        Self::load(store, device)
    }

    /// Opens the workspace in `directory`.
    pub fn open(directory: &Path) -> Result<Self, AppError> {
        if !directory.join("spezado.sqlite").exists() {
            return Err(AppError::NoWorkspace);
        }
        let store = Store::open(directory)?;
        let device = store.setting(None, "device")?.ok_or(AppError::NoWorkspace)?;
        let device = device.parse().map_err(|_| StoreError::Identity(device))?;
        Self::load(store, device)
    }

    fn load(store: Store, device: DeviceId) -> Result<Self, AppError> {
        let mut clock = Clock::new(device.clone());
        let mut books = BTreeMap::new();
        for id in store.books()? {
            let events = store.load(&id)?;
            for (_, event) in events.ordered() {
                clock.observe(&event.time, now_ms());
            }
            books.insert(id, Books::from_events(events));
        }
        let law: Vec<Box<dyn LawModule>> = vec![Box::new(spezado_law_de::Germany)];
        let keychain = std::env::var("SPEZADO_KEYCHAIN").map_or(true, |v| v != "0");
        Ok(Workspace { store, device, clock, chart: Chart::v0_1(), law, books, keychain })
    }

    /// Whether member seeds of books synchronised from now on go into the operating system's key store – the macOS
    /// keychain, the Windows credential manager, the Secret Service on Linux – rather than into the workspace.
    pub fn use_keychain(&mut self, on: bool) {
        self.keychain = on;
    }

    /// This device.
    pub fn device(&self) -> &DeviceId {
        &self.device
    }

    /// The harmonised chart of accounts.
    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    /// All books, by identity.
    pub fn books(&self) -> &BTreeMap<BooksId, Books> {
        &self.books
    }

    /// One set of books.
    pub fn get(&self, books: &BooksId) -> Result<&Books, AppError> {
        self.books.get(books).ok_or_else(|| AppError::UnknownBooks(books.clone()))
    }

    /// Executes a command in a set of books; returns the event written.
    pub fn execute(&mut self, books: &BooksId, command: Command) -> Result<EventHash, AppError> {
        Ok(self.execute_all(vec![(books.clone(), command)])?[0])
    }

    /// Executes commands, in one set of books or several, in order: each is checked against the books as the ones
    /// before it left them. All are written, or – if one is refused – none. Returns the events written.
    pub fn execute_all(&mut self, commands: Vec<(BooksId, Command)>) -> Result<Vec<EventHash>, AppError> {
        for (id, _) in &commands {
            if self.sync_rights(id)? == Some(librotenado::Rights::Read) {
                return Err(AppError::ReadOnly(id.clone()));
            }
        }
        let mut working: BTreeMap<BooksId, Books> = BTreeMap::new();
        let mut rows = Vec::new();
        let mut hashes = Vec::new();
        for (id, command) in commands {
            let books = match working.get_mut(&id) {
                Some(books) => books,
                None => {
                    let current = match (self.books.get(&id), &command) {
                        (Some(books), _) => books.clone(),
                        (None, Command::Found(_)) => Books::new(),
                        (None, _) => return Err(AppError::UnknownBooks(id)),
                    };
                    working.entry(id.clone()).or_insert(current)
                }
            };
            let payload = books.decide(command, &self.chart)?;
            let (seq, prev) = books.events().chain_head(&self.device);
            let event = Event { time: self.clock.now(now_ms()), seq, prev, payload };
            let (hash, _) = books.insert(event);
            let canonical = books.events().canonical(&hash).expect("just inserted").to_vec();
            rows.push((id, hash.to_string(), canonical));
            hashes.push(hash);
        }
        self.store.append(&rows)?;
        self.books.extend(working);
        Ok(hashes)
    }

    /// Sets up books for an organisation or a person: founds them, describes and adds the owner, configures it and
    /// selects the harmonised chart. A national chart in the configuration (`de-skr03`) must be one the law that
    /// applies offers. Returns the identity of the new books.
    pub fn set_up(&mut self, party: Party, configuration: Configuration) -> Result<BooksId, AppError> {
        let id: BooksId = new_id();
        let owner = party.id().clone();
        if let Some(name) = &configuration.chart {
            let (country, legal_form) = match &party {
                Party::Organisation(organisation) => {
                    (Some(organisation.country), Some(organisation.legal_form.as_str()))
                }
                Party::Person(_) => (None, None),
            };
            let facts = Facts::new(&configuration, country, legal_form);
            let module = spezado_law::module_for(&self.law, &facts);
            let charts = module.map(|module| module.charts(&self.chart)).unwrap_or_default();
            if !charts.iter().any(|chart| chart.chart == name.as_str()) {
                return Err(AppError::UnknownChart(name.to_string()));
            }
        }
        let describe = match party {
            Party::Organisation(organisation) => Command::DescribeOrganisation(organisation),
            Party::Person(person) => Command::DescribePerson(person),
        };
        let commands = [
            Command::Found(id.clone()),
            describe,
            Command::AddOwner(owner),
            Command::Configure(configuration),
            Command::SelectChart(CHART.into()),
        ];
        self.execute_all(commands.into_iter().map(|command| (id.clone(), command)).collect())?;
        Ok(id)
    }

    /// Changes the facts that move an owner's deadlines from a date on – the region whose public holidays apply and
    /// whether its VAT returns are extended for good (§ 46 UStDV) – by configuring it anew from that date with
    /// everything else as it was; a configuration from a later date stays as it is. The date must not be in a closed
    /// period.
    pub fn change_deadlines(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        from: Date,
        region: Option<librotenado::Region>,
        extended: bool,
    ) -> Result<(), AppError> {
        let mut configuration = self
            .get(books)?
            .events()
            .configuration(owner, from)
            .cloned()
            .ok_or_else(|| AppError::NotConfigured { owner: owner.clone(), date: from })?;
        configuration.from = from;
        configuration.region = region;
        if let Some(vat) = configuration.vat.as_mut() {
            vat.extended = extended;
        }
        self.execute(books, Command::Configure(configuration))?;
        Ok(())
    }

    /// Changes the trade tax multiplier of the owner's municipality from a date on (librotenado 0008); none when the
    /// owner is not subject to trade tax. Everything else stays as configured.
    pub fn change_trade_tax(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        from: Date,
        multiplier: Option<u32>,
    ) -> Result<(), AppError> {
        let mut configuration = self
            .get(books)?
            .events()
            .configuration(owner, from)
            .cloned()
            .ok_or_else(|| AppError::NotConfigured { owner: owner.clone(), date: from })?;
        configuration.from = from;
        configuration.trade_tax_multiplier = multiplier;
        self.execute(books, Command::Configure(configuration))?;
        Ok(())
    }

    /// Records one business transaction in several sets of books, one booking each, all or none: the bookings are
    /// given a new transaction that names the other books (spec part 0002). Returns the events written.
    pub fn book_across(&mut self, bookings: Vec<(BooksId, Booking)>) -> Result<Vec<EventHash>, AppError> {
        let transaction: TransactionId = new_id();
        let all: Vec<BooksId> = bookings.iter().map(|(id, _)| id.clone()).collect();
        let commands = bookings
            .into_iter()
            .map(|(id, mut booking)| {
                let others = all.iter().filter(|other| **other != id).cloned().collect();
                booking.transaction = Some(Transaction { id: transaction.clone(), books: others });
                (id, Command::Book(booking))
            })
            .collect();
        self.execute_all(commands)
    }

    /// The national chart an owner works with on a date, as its configuration names it; none when it names none.
    pub fn national_chart(
        &self,
        books: &BooksId,
        owner: &PartyId,
        date: Date,
    ) -> Result<Option<ChartTranslation>, AppError> {
        let (configuration, _, _, module) = self.law_for(books, owner, date)?;
        let Some(name) = configuration.chart else { return Ok(None) };
        let translation = module.charts(&self.chart).into_iter().find(|translation| translation.chart == name.as_str());
        translation.map(Some).ok_or_else(|| AppError::UnknownChart(name.to_string()))
    }

    /// Everything that needs review in a set of books – with the transactions it shares with other books held here.
    pub fn review(&self, books: &BooksId) -> Result<Vec<Issue>, AppError> {
        let mut issues = self.get(books)?.review(&self.chart);
        let held: Vec<&EventSet> = self.books.values().map(Books::events).collect();
        let own: BTreeSet<TransactionId> = self
            .get(books)?
            .events()
            .ordered()
            .iter()
            .filter_map(|(_, event)| match &event.payload {
                librotenado::Payload::BookingRecorded { booking }
                | librotenado::Payload::BookingReversed { booking, .. } => {
                    booking.transaction.as_ref().map(|t| t.id.clone())
                }
                _ => None,
            })
            .collect();
        issues.extend(
            review_transactions(&held)
                .into_iter()
                .filter(|problem| concerns(problem, books, &own))
                .map(Issue::Transaction),
        );
        for problem in self.customer_problems(books)? {
            if !issues.contains(&Issue::Customer(problem.clone())) {
                issues.push(Issue::Customer(problem));
            }
        }
        Ok(issues)
    }

    /// The bookings the reports per customer of the law that applies cannot place – a VAT identification number of
    /// the customer missing, or several – over each calendar year with bookings.
    fn customer_problems(&self, books: &BooksId) -> Result<Vec<librotenado::CustomerProblem>, AppError> {
        let events = self.get(books)?.events();
        let years: BTreeSet<u16> = events
            .ordered()
            .iter()
            .filter_map(|(_, event)| match &event.payload {
                librotenado::Payload::BookingRecorded { booking }
                | librotenado::Payload::BookingReversed { booking, .. } => Some(booking.date.year()),
                _ => None,
            })
            .collect();
        let mut problems = Vec::new();
        for owner in events.owners() {
            for &year in &years {
                let (Some(from), Some(to)) = (Date::new(year, 1, 1), Date::new(year, 12, 31)) else { continue };
                // An owner not set up by the end of the year, or without law, has no reports to check here; the
                // review names what is missing on its own.
                let Ok((configuration, country, legal_form, module)) = self.law_for(books, owner, to) else {
                    continue;
                };
                let mut facts = Facts::new(&configuration, country, legal_form.as_deref());
                facts.employer = self.is_employer(books, owner, to)?;
                for report in module.reports(&facts, from, to) {
                    if report.basis == ReportBasis::Customers {
                        problems.extend(events.customer_report(&report.mapping, &self.chart, owner, from, to)?.1);
                    }
                }
            }
        }
        Ok(problems)
    }

    /// Whether an owner employs people – has an employment begun by `to` (librotenado spec part 0013).
    fn is_employer(&self, books: &BooksId, owner: &PartyId, to: Date) -> Result<bool, AppError> {
        Ok(!self.get(books)?.events().employments(owner, to).is_empty())
    }

    /// The configuration of an owner on a date, the law module that applies to it, and its facts' parts.
    fn law_for(&self, books: &BooksId, owner: &PartyId, date: Date) -> Result<OwnerLaw<'_>, AppError> {
        let events = self.get(books)?.events();
        let configuration = events
            .configuration(owner, date)
            .cloned()
            .ok_or_else(|| AppError::NotConfigured { owner: owner.clone(), date })?;
        let (organisations, _) = events.parties();
        let organisation = organisations.get(owner);
        let country = organisation.map(|o| o.country);
        let legal_form = organisation.map(|o| o.legal_form.clone());
        let mut facts = Facts::new(&configuration, country, legal_form.as_deref());
        facts.employer = self.is_employer(books, owner, date)?;
        let module = spezado_law::module_for(&self.law, &facts).ok_or_else(|| AppError::NoLaw(owner.clone()))?;
        Ok((configuration, country, legal_form, module))
    }

    /// The reports that apply to an owner for a period, as `(identity, name)`.
    pub fn reports(
        &self,
        books: &BooksId,
        owner: &PartyId,
        from: Date,
        to: Date,
    ) -> Result<Vec<(&'static str, &'static str)>, AppError> {
        let (configuration, country, legal_form, module) = self.law_for(books, owner, from)?;
        let mut facts = Facts::new(&configuration, country, legal_form.as_deref());
        facts.employer = self.is_employer(books, owner, to)?;
        Ok(module.reports(&facts, from, to).into_iter().map(|report| (report.id, report.name)).collect())
    }

    /// The figures of a report for an owner and a period.
    pub fn report(
        &self,
        books: &BooksId,
        owner: &PartyId,
        report: &str,
        from: Date,
        to: Date,
    ) -> Result<ReportFigures, AppError> {
        let (configuration, country, legal_form, module) = self.law_for(books, owner, from)?;
        let mut facts = Facts::new(&configuration, country, legal_form.as_deref());
        facts.employer = self.is_employer(books, owner, to)?;
        let Report { id, name, basis, mapping, shortened, .. } = module
            .reports(&facts, from, to)
            .into_iter()
            .find(|candidate| candidate.id == report)
            .ok_or_else(|| AppError::UnknownReport(report.to_owned()))?;
        let events = self.get(books)?.events();
        let fields = match basis {
            ReportBasis::Period => {
                let taxes = module.tax_table(to).ok_or_else(|| AppError::UnknownReport(report.to_owned()))?;
                events.period_report(&mapping, &taxes, &self.chart, owner, from, to)?
            }
            ReportBasis::Balances => events.account_report(&mapping, &self.chart, owner, Some(to))?,
            ReportBasis::Customers => events.customer_report(&mapping, &self.chart, owner, from, to)?.0,
            ReportBasis::Assets => events.asset_report(&mapping, &self.chart, owner, from, to),
            ReportBasis::ProfitTax { subject, trade } => {
                let facts = librotenado::ProfitTaxFacts {
                    subject,
                    multiplier: configuration.trade_tax_multiplier,
                    income_losses: 0,
                    trade_losses: 0,
                    land_reduction: 0,
                    turnover_and_wages: None,
                    holding: librotenado::Holding::default(),
                    cash_basis: configuration.accounting.as_ref().is_some_and(|a| a.as_str() == "de-cash-basis"),
                };
                let tax = events
                    .profit_tax(&self.chart, owner, from, to, &facts)
                    .map_err(|e| AppError::UnknownReport(format!("{report}: {e}")))?;
                match (trade, &tax.trade) {
                    (true, Some(trade)) => trade.fields(),
                    (true, None) => BTreeMap::new(),
                    (false, _) => tax.fields(),
                }
            }
            ReportBasis::Payroll => events.payroll_report(&mapping, owner, from, to),
            ReportBasis::IncomeTax => {
                let profit = librotenado::ProfitTaxFacts {
                    subject: librotenado::TaxSubject::Individual,
                    multiplier: configuration.trade_tax_multiplier,
                    income_losses: 0,
                    trade_losses: 0,
                    land_reduction: 0,
                    turnover_and_wages: None,
                    holding: librotenado::Holding::default(),
                    cash_basis: configuration.accounting.as_ref().is_some_and(|a| a.as_str() == "de-cash-basis"),
                };
                let facts = librotenado::IncomeTaxFacts::from(&self.income_facts(books, owner, to.year())?);
                events
                    .income_tax(&self.chart, owner, from, to, &profit, &facts)
                    .map_err(|e| AppError::UnknownReport(format!("{report}: {e}")))?
                    .map(|tax| tax.fields())
                    .ok_or_else(|| AppError::UnknownReport(format!("{report}: no tariff for {}", to.year())))?
            }
        };
        let (mut terms, mut reconciliation) = (BTreeMap::new(), BTreeMap::new());
        if basis == ReportBasis::Balances {
            terms = events.term_report(&mapping, &self.chart, owner, Some(to))?;
            reconciliation = events.reconciliation(&mapping, &self.chart, owner, Some(to))?;
        }
        let shorten = |fields: BTreeMap<String, i128>| match shortened {
            Some(size) => librotenado::hgb_shortened(&fields, size),
            None => fields,
        };
        let fields = shorten(fields);
        let reconciliation = shorten(reconciliation);
        if shortened.is_some() {
            // The terms of the positions a shortened balance sheet keeps: each part shortened alike.
            let part = |get: fn(&librotenado::Terms) -> i128| -> BTreeMap<String, i128> {
                shorten(terms.iter().map(|(position, t)| (position.clone(), get(t))).collect())
            };
            let parts = [
                part(|t| t.up_to_one_year),
                part(|t| t.over_one_year),
                part(|t| t.over_five_years),
                part(|t| t.unknown),
            ];
            let positions: std::collections::BTreeSet<&String> = parts.iter().flat_map(BTreeMap::keys).collect();
            terms = positions
                .into_iter()
                .map(|position| {
                    let get = |i: usize| parts[i].get(position).copied().unwrap_or(0);
                    let t = librotenado::Terms {
                        up_to_one_year: get(0),
                        over_one_year: get(1),
                        over_five_years: get(2),
                        unknown: get(3),
                    };
                    (position.clone(), t)
                })
                .collect();
        }
        let law = format!("{}/{}", module.name(), module.version());
        Ok(ReportFigures {
            id,
            name,
            names: mapping.report,
            fields,
            currency: configuration.currency,
            law,
            terms,
            reconciliation,
        })
    }

    /// The law modules, `name → version`, as a closing records them.
    pub fn software(&self) -> BTreeMap<String, String> {
        let mut software: BTreeMap<String, String> =
            self.law.iter().map(|module| (module.name().to_owned(), module.version().to_owned())).collect();
        software.insert("spezado".into(), env!("CARGO_PKG_VERSION").into());
        software
    }

    /// The events of a set of books as JSON Lines: each in its canonical encoding, in the agreed order.
    pub fn export(&self, books: &BooksId) -> Result<Vec<u8>, AppError> {
        let events = self.get(books)?.events();
        let mut lines = Vec::new();
        for (hash, _) in events.ordered() {
            lines.extend_from_slice(events.canonical(&hash).expect("an event of the set"));
            lines.push(b'\n');
        }
        Ok(lines)
    }

    /// Imports events exported as JSON Lines: a set of books new here, or more events of one held here. Returns the
    /// books' identity and how many events were new.
    pub fn import(&mut self, lines: &[u8]) -> Result<(BooksId, usize), AppError> {
        let mut received = EventSet::new();
        for (number, line) in lines.split(|b| *b == b'\n').enumerate() {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            received.receive(line).map_err(|error| AppError::Import { line: number + 1, error })?;
        }
        let id = received.books().cloned().ok_or(AppError::NotFounded)?;
        let rows: Vec<_> = received
            .ordered()
            .iter()
            .map(|(hash, _)| (id.clone(), hash.to_string(), received.canonical(hash).expect("in the set").to_vec()))
            .collect();
        for (_, event) in received.ordered() {
            self.clock.observe(&event.time, now_ms());
        }
        let new = self.store.append(&rows)?;
        let books = self.books.entry(id.clone()).or_default();
        let mut events = books.events().clone();
        events.merge(&received);
        *books = Books::from_events(events);
        Ok((id, new))
    }
}

/// Whether a problem of a transaction concerns these books: it names them, or a transaction they record.
fn concerns(problem: &librotenado::TransactionProblem, books: &BooksId, own: &BTreeSet<TransactionId>) -> bool {
    use librotenado::TransactionProblem::*;
    match problem {
        Duplicate { books: of, .. } => of == books,
        Missing { books: of, other, transaction, .. } => of == books || other == books || own.contains(transaction),
        BooksMismatch { transaction } | DateMismatch { transaction } | ReversalMismatch { transaction } => {
            own.contains(transaction)
        }
    }
}
