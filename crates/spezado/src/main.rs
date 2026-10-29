// SPDX-License-Identifier: AGPL-3.0-or-later
//! spezado on the command line: set up books, book, reverse, read the journal, balances and reports, close periods,
//! review, export and import. Everything goes through the workspace of `spezado-app`.

use std::{collections::BTreeMap, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use librotenado::{
    AccountId, Booking, BooksId, ChartTranslation, Configuration, Currency, Date, DeviceId, Names, Organisation,
    PartyId, Person, Posting,
};
use spezado_app::{Party, Workspace, new_id};
use spezado_core::{Command, format_amount, parse_amount};

/// Free accounting for everyone.
#[derive(Debug, Parser)]
#[command(name = "spezado", version, about)]
struct Cli {
    /// The workspace directory; default: $SPEZADO_DIR, else spezado in the user's data directory.
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    /// The books to work in; may be left out while the workspace holds one set of books.
    #[arg(long, global = true)]
    books: Option<BooksId>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Creates a workspace for this device.
    Init {
        /// The name of this device in the books: a-z, 0-9 and -; default: a random one.
        #[arg(long)]
        device: Option<DeviceId>,
    },
    /// Lists the books of the workspace.
    Books,
    /// Sets up books for a person or an organisation.
    SetUp(SetUp),
    /// Records a booking: lines of ACCOUNT=AMOUNT, optionally @TAXCODE – e.g. 4930=12.40 1200=-12.40.
    Book {
        /// The date, YYYY-MM-DD.
        #[arg(long)]
        date: Date,
        /// What it is about.
        #[arg(long)]
        text: String,
        /// The owner whose books it is in; may be left out while the books have one owner.
        #[arg(long)]
        owner: Option<PartyId>,
        /// The lines: a national account number (or an account's UUID), `=`, the amount – positive debit,
        /// negative credit – and optionally `@` and a tax code.
        #[arg(required = true, allow_negative_numbers = true)]
        lines: Vec<String>,
    },
    /// Reverses a booking by the event that recorded it.
    Reverse {
        /// The event, sha256:…
        event: librotenado::EventHash,
        /// The date of the reversal.
        #[arg(long)]
        date: Date,
        /// Why.
        #[arg(long)]
        text: String,
    },
    /// Shows the journal.
    Journal {
        /// From this date on.
        #[arg(long)]
        from: Option<Date>,
        /// Up to this date.
        #[arg(long)]
        to: Option<Date>,
    },
    /// Shows the balance of every account.
    Balance {
        /// On this date; default: everything.
        #[arg(long)]
        until: Option<Date>,
    },
    /// Shows a report for a period, or – without one – the reports that apply.
    Report {
        /// The report, e.g. de-ustva.
        report: Option<String>,
        /// The first day.
        #[arg(long)]
        from: Date,
        /// The last day.
        #[arg(long)]
        to: Date,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Closes the books of an owner up to a date.
    Close {
        /// The last day closed.
        #[arg(long)]
        to: Date,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Shows what needs review.
    Check,
    /// Writes the events of the books as JSON Lines to standard output.
    Export,
    /// Reads events exported as JSON Lines: new books, or more events of books held here.
    Import {
        /// The file.
        file: PathBuf,
    },
    /// Starts the terminal interface – also to set up the first books.
    Tui {
        /// The language: de or en; default: the workspace's, else the system's.
        #[arg(long)]
        lang: Option<String>,
    },
    /// Serves the API and the web app on this device, at http://127.0.0.1:PORT.
    Serve {
        /// The port.
        #[arg(long, default_value_t = 8048)]
        port: u16,
    },
    /// Writes the description of the API in OpenAPI to standard output.
    Openapi,
}

#[derive(Debug, clap::Args)]
struct SetUp {
    /// The name of the person or organisation.
    #[arg(long)]
    name: String,
    /// The legal form of an organisation (de-gmbh, de-ev, …); without it, the books are a person's.
    #[arg(long)]
    legal_form: Option<String>,
    /// The country of an organisation.
    #[arg(long, default_value = "DE")]
    country: String,
    /// The first day the configuration applies.
    #[arg(long)]
    from: Date,
    /// The functional currency.
    #[arg(long, default_value = "EUR")]
    currency: Currency,
    /// The month the financial year begins with.
    #[arg(long, default_value_t = 1)]
    year_start: u8,
    /// How profit is determined: de-cash-basis, de-double-entry; none for a private household.
    #[arg(long)]
    accounting: Option<String>,
    /// The VAT scheme: de-regular, de-small-business; none when not liable.
    #[arg(long)]
    vat: Option<String>,
    /// When VAT is due: accrual or cash.
    #[arg(long, default_value = "accrual")]
    vat_basis: String,
    /// The period of advance VAT returns: month, quarter or year.
    #[arg(long, default_value = "quarter")]
    vat_period: String,
    /// The national chart to work with: de-skr03, de-skr04.
    #[arg(long)]
    chart: Option<String>,
    /// Whether the deadlines of advance VAT returns are extended for good (Dauerfristverlängerung).
    #[arg(long)]
    vat_extended: bool,
    /// The state whose public holidays move deadlines, ISO 3166-2: DE-BY, DE-NW, …
    #[arg(long)]
    region: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let directory = match cli.dir {
        Some(directory) => directory,
        None => match std::env::var_os("SPEZADO_DIR") {
            Some(directory) => directory.into(),
            None => dirs::data_dir().context("no data directory; give --dir")?.join("spezado"),
        },
    };
    if let Commands::Openapi = cli.command {
        use spezado_api::utoipa_openapi;
        println!("{}", utoipa_openapi());
        return Ok(());
    }
    if let Commands::Init { device } = cli.command {
        let device = device.unwrap_or_else(|| {
            let id: String = uuid_suffix();
            format!("device-{id}").parse().expect("a valid device name")
        });
        let workspace = Workspace::create(&directory, device)?;
        println!("workspace for device {} in {}", workspace.device(), directory.display());
        return Ok(());
    }
    // The terminal interface starts from nothing: a workspace for this device, then setting up books.
    if matches!(cli.command, Commands::Tui { .. }) && !directory.join("spezado.sqlite").exists() {
        let device = format!("device-{}", uuid_suffix()).parse().expect("a valid device name");
        Workspace::create(&directory, device)?;
    }
    let mut workspace = Workspace::open(&directory)
        .with_context(|| format!("{} – create one with `spezado init`", directory.display()))?;
    match cli.command {
        Commands::Init { .. } => unreachable!("handled above"),
        Commands::Books => list_books(&workspace),
        Commands::SetUp(setup) => {
            let id = set_up(&mut workspace, setup)?;
            println!("{id}");
            Ok(())
        }
        Commands::Serve { port } => {
            spezado_api::serve(workspace, port, |address, code| {
                println!("spezado serves at http://{address}");
                println!("Pair a device with the code {code} – or open http://{address}/#pair={code}");
                println!("The code is valid for 15 minutes. Stop with Ctrl-C.");
            })?;
            Ok(())
        }
        Commands::Tui { lang } => {
            let lang = spezado_tui::language(&workspace, lang.as_deref());
            spezado_tui::run(&mut workspace, lang)?;
            Ok(())
        }
        Commands::Import { file } => {
            let bytes = std::fs::read(&file).with_context(|| file.display().to_string())?;
            let (id, new) = workspace.import(&bytes)?;
            println!("{new} new events in books {id}");
            Ok(())
        }
        command => {
            let books = choose_books(&workspace, cli.books)?;
            run(&mut workspace, &books, command)
        }
    }
}

/// Eight hexadecimal digits from a fresh identity – for a device name nobody chose.
fn uuid_suffix() -> String {
    let id: PartyId = new_id();
    id.as_str()[id.as_str().len() - 8..].to_owned()
}

fn run(workspace: &mut Workspace, books: &BooksId, command: Commands) -> Result<()> {
    match command {
        Commands::Book { date, text, owner, lines } => {
            let owner = choose_owner(workspace, books, owner)?;
            let booking = parse_booking(workspace, books, &owner, date, text, &lines)?;
            println!("{}", workspace.execute(books, Command::Book(booking))?);
        }
        Commands::Reverse { event, date, text } => {
            println!("{}", workspace.execute(books, Command::Reverse { event, date, text })?);
        }
        Commands::Journal { from, to } => journal(workspace, books, from, to)?,
        Commands::Balance { until } => balance(workspace, books, until)?,
        Commands::Report { report, from, to, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            match report {
                None => {
                    for (id, name) in workspace.reports(books, &owner, from, to)? {
                        println!("{id:<16} {name}");
                    }
                }
                Some(report) => {
                    let figures = workspace.report(books, &owner, &report, from, to)?;
                    println!("{} ({}), {from} – {to}, {}", figures.name, figures.id, figures.law);
                    for (field, units) in &figures.fields {
                        let amount = librotenado::Money::new(*units, figures.currency.clone()).context("overflow")?;
                        println!("{field:>12} {:>16}", format_amount(&amount));
                    }
                }
            }
        }
        Commands::Close { to, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            let software = workspace.software();
            println!("{}", workspace.execute(books, Command::Close { owner, to, software })?);
        }
        Commands::Check => {
            let issues = workspace.review(books)?;
            if issues.is_empty() {
                println!("nothing to review");
            }
            for issue in issues {
                println!("{issue}");
            }
        }
        Commands::Export => std::io::stdout().write_all(&workspace.export(books)?)?,
        Commands::Tui { .. } => unreachable!("handled in main"),
        Commands::Serve { .. } | Commands::Openapi => unreachable!("handled in main"),
        Commands::Init { .. } | Commands::Books | Commands::SetUp(_) | Commands::Import { .. } => {
            unreachable!("handled in main")
        }
    }
    Ok(())
}

fn choose_books(workspace: &Workspace, books: Option<BooksId>) -> Result<BooksId> {
    if let Some(books) = books {
        workspace.get(&books)?;
        return Ok(books);
    }
    let mut all = workspace.books().keys();
    match (all.next(), all.next()) {
        (Some(only), None) => Ok(only.clone()),
        (None, _) => bail!("no books yet – set some up with `spezado set-up`"),
        _ => bail!("the workspace holds several books – choose with --books (see `spezado books`)"),
    }
}

fn choose_owner(workspace: &Workspace, books: &BooksId, owner: Option<PartyId>) -> Result<PartyId> {
    if let Some(owner) = owner {
        return Ok(owner);
    }
    let owners = workspace.get(books)?.events().owners();
    match owners.as_slice() {
        [only] => Ok((*only).clone()),
        _ => bail!("the books have several owners – choose with --owner"),
    }
}

/// The name of an organisation or person of the books.
fn party_name(workspace: &Workspace, books: &BooksId, party: &PartyId) -> String {
    let Ok(books) = workspace.get(books) else { return party.to_string() };
    let (organisations, persons) = books.events().parties();
    organisations
        .get(party)
        .map(|o| o.name.clone())
        .or_else(|| persons.get(party).map(|p| p.name.clone()))
        .unwrap_or_else(|| party.to_string())
}

fn list_books(workspace: &Workspace) -> Result<()> {
    if workspace.books().is_empty() {
        println!("no books yet – set some up with `spezado set-up`");
    }
    for (id, books) in workspace.books() {
        let owners: Vec<String> =
            books.events().owners().into_iter().map(|owner| party_name(workspace, id, owner)).collect();
        let mut charts = Charts::default();
        let mut chart = "–".to_owned();
        for owner in books.events().owners() {
            if let Some(translation) = charts.of(workspace, id, owner) {
                chart.clone_from(&translation.chart);
                break;
            }
        }
        println!("{id}  {:<30} {chart:<9} {} events", owners.join(", "), books.events().len());
    }
    Ok(())
}

fn set_up(workspace: &mut Workspace, setup: SetUp) -> Result<BooksId> {
    let id: PartyId = new_id();
    let party = match setup.legal_form {
        Some(legal_form) => Party::Organisation(Organisation {
            id: id.clone(),
            name: setup.name,
            legal_form,
            country: setup.country.parse()?,
            registrations: vec![],
            tax_ids: vec![],
            shares: vec![],
        }),
        None => Party::Person(Person { id: id.clone(), name: setup.name, tax_ids: vec![], roles: vec![] }),
    };
    let mut configuration = serde_json::json!({
        "owner": id.as_str(), "from": setup.from.to_string(), "currency": setup.currency.as_str(),
        "year_start": setup.year_start,
    });
    if let Some(accounting) = setup.accounting {
        configuration["accounting"] = accounting.into();
    }
    if let Some(chart) = setup.chart {
        configuration["chart"] = chart.into();
    }
    if let Some(scheme) = setup.vat {
        configuration["vat"] =
            serde_json::json!({"scheme": scheme, "basis": setup.vat_basis, "period": setup.vat_period});
        if setup.vat_extended {
            configuration["vat"]["extended"] = true.into();
        }
    }
    if let Some(region) = setup.region {
        configuration["region"] = region.into();
    }
    let configuration: Configuration = serde_json::from_value(configuration).context("configuration")?;
    Ok(workspace.set_up(party, configuration)?)
}

/// An account by its national number in the books' national chart, or by its identity.
fn account(translation: Option<&ChartTranslation>, text: &str) -> Result<AccountId> {
    if let Ok(id) = text.parse() {
        return Ok(id);
    }
    let translation = translation.ok_or_else(|| anyhow!("{text:?}: the books have no national chart; give a UUID"))?;
    translation
        .accounts
        .get(text)
        .cloned()
        .ok_or_else(|| anyhow!("{text:?} is not an account of {}", translation.chart))
}

fn parse_booking(
    workspace: &Workspace,
    books: &BooksId,
    owner: &PartyId,
    date: Date,
    text: String,
    lines: &[String],
) -> Result<Booking> {
    let translation = workspace.national_chart(books, owner, date)?;
    let configuration = workspace
        .get(books)?
        .events()
        .configuration(owner, date)
        .ok_or_else(|| anyhow!("{owner} is not configured on {date}"))?;
    let postings = lines
        .iter()
        .map(|line| {
            let (number, rest) = line.split_once('=').ok_or_else(|| anyhow!("{line:?}: ACCOUNT=AMOUNT expected"))?;
            let (amount, tax) = match rest.split_once('@') {
                Some((amount, tax)) => (amount, Some(tax.parse()?)),
                None => (rest, None),
            };
            Ok(Posting {
                owner: owner.clone(),
                account: account(translation.as_ref(), number)?,
                amount: parse_amount(amount, &configuration.currency)?,
                note: None,
                tax,
                exchange: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Booking { date, text, postings, documents: vec![], entries: vec![], transaction: None })
}

/// The national chart of each owner, as last configured – read once per owner.
#[derive(Debug, Default)]
struct Charts(BTreeMap<PartyId, Option<ChartTranslation>>);

impl Charts {
    fn of(&mut self, workspace: &Workspace, books: &BooksId, owner: &PartyId) -> Option<&ChartTranslation> {
        self.0
            .entry(owner.clone())
            .or_insert_with(|| {
                let latest = Date::new(9999, 12, 31).expect("a valid date");
                workspace.national_chart(books, owner, latest).ok().flatten()
            })
            .as_ref()
    }
}

/// An account as people know it: its national number, if any, and its name.
fn account_label(names: &Names, translation: Option<&ChartTranslation>, account: &AccountId) -> String {
    let number = translation.and_then(|t| t.national(account)).unwrap_or("");
    let name = names.get(account.as_str()).unwrap_or(account.as_str());
    format!("{number:>6} {name}")
}

fn journal(workspace: &Workspace, books: &BooksId, from: Option<Date>, to: Option<Date>) -> Result<()> {
    let names = Names::v0_1("de").context("names")?;
    let mut charts = Charts::default();
    for entry in workspace.get(books)?.journal(None, from, to) {
        let mark = match (entry.reverses, entry.reversed_by) {
            (Some(_), _) => " (reversal)",
            (None, Some(_)) => " (reversed)",
            (None, None) => "",
        };
        println!("{}  {}{mark}\n            {}", entry.booking.date, entry.booking.text, entry.event);
        for posting in &entry.booking.postings {
            let tax = posting.tax.as_ref().map(|t| format!(" @{t}")).unwrap_or_default();
            let label = account_label(&names, charts.of(workspace, books, &posting.owner), &posting.account);
            println!("    {label:<60} {:>14} {}{tax}", format_amount(&posting.amount), posting.amount.currency());
        }
    }
    Ok(())
}

fn balance(workspace: &Workspace, books: &BooksId, until: Option<Date>) -> Result<()> {
    let names = Names::v0_1("de").context("names")?;
    let mut charts = Charts::default();
    let balances = workspace.get(books)?.events().balances(until)?;
    let mut rows: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (owner, account, currency, units) in balances.iter().filter(|(.., units)| *units != 0) {
        let amount = librotenado::Money::new(units, currency.clone()).context("overflow")?;
        let label = account_label(&names, charts.of(workspace, books, owner), account);
        rows.entry(party_name(workspace, books, owner))
            .or_default()
            .push(format!("    {label:<60} {:>14} {currency}", format_amount(&amount)));
    }
    for (owner, lines) in rows {
        println!("{owner}");
        let mut lines = lines;
        lines.sort();
        for line in lines {
            println!("{line}");
        }
    }
    Ok(())
}
