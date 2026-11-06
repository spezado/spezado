// SPDX-License-Identifier: AGPL-3.0-or-later
//! spezado on the command line: set up books, book, reverse, read the journal, balances and reports, close periods,
//! prepare and submit filings, review, export and import. Everything goes through the workspace of `spezado-app`.

use std::{collections::BTreeMap, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use librotenado::{
    AccountId, Booking, BooksId, ChartTranslation, Configuration, Currency, Date, DeviceId, Names, Organisation,
    PartyId, Person, Posting,
};
use spezado_app::{Party, Workspace, new_id};
use spezado_core::{Command, format_amount, parse_amount};

mod autostart;
mod start;
#[cfg(feature = "tray")]
mod tray;

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

/// On or off.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Switch {
    On,
    Off,
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
    /// Sets the trade tax multiplier of the municipality (Hebesatz, in percent) from a date on; without one, the owner
    /// is not subject to trade tax – a freelancer.
    TradeTax {
        /// The multiplier in percent, at least 200.
        multiplier: Option<u32>,
        /// From when it applies; default: today.
        #[arg(long)]
        from: Option<Date>,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// What a person states for income tax beside the books, for a year (shown in the report de-est); without
    /// options, shows what is stated.
    IncomeTax {
        /// The calendar year.
        #[arg(long)]
        year: u16,
        /// Assessed jointly with a spouse (splitting).
        #[arg(long)]
        joint: Option<bool>,
        /// Other positive income, e.g. 12000.00.
        #[arg(long)]
        other_income: Option<String>,
        /// Special expenses, extraordinary burdens, allowances.
        #[arg(long)]
        deductions: Option<String>,
        /// Losses carried forward, not yet used.
        #[arg(long)]
        losses: Option<String>,
        /// The rate of church tax in percent, 8 or 9; 0 for none.
        #[arg(long)]
        church: Option<u8>,
        /// The child benefit received for the year.
        #[arg(long)]
        child_benefit: Option<String>,
        /// Extraordinary income taxed by the fifth (§ 34 EStG).
        #[arg(long)]
        extraordinary: Option<String>,
        /// Shares of profits of partnerships.
        #[arg(long)]
        partnership_income: Option<String>,
        /// The share of their trade tax base amounts (§ 35 Abs. 2 EStG).
        #[arg(long)]
        partnership_trade_base: Option<String>,
        /// The share of their trade tax.
        #[arg(long)]
        partnership_trade_tax: Option<String>,
        /// Capital income not taxed at source (§ 32d EStG).
        #[arg(long)]
        capital_income: Option<String>,
        /// The allowances for children in tenths: 10 a child in full.
        #[arg(long)]
        child_allowances: Option<u32>,
        /// Children of a single parent (§ 24b EStG).
        #[arg(long)]
        single_parent_children: Option<u8>,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Shows or states what the books do not hold of the taxes on profit of a year: losses carried forward, the
    /// reduction for land, turnover and wages, the share held in corporations.
    ProfitTax {
        /// The year the financial year ends in.
        #[arg(long)]
        year: u16,
        /// Losses carried forward for corporation or income tax (§ 10d EStG).
        #[arg(long)]
        income_losses: Option<String>,
        /// Trade losses carried forward (§ 10a GewStG).
        #[arg(long)]
        trade_losses: Option<String>,
        /// The reduction for land (§ 9 Nr. 1 GewStG), 1.2 % of the Einheitswert.
        #[arg(long)]
        land_reduction: Option<String>,
        /// Turnover and wages of the year, for the cap of donations.
        #[arg(long)]
        turnover_and_wages: Option<String>,
        /// The share held in corporations: at_least_15, 10_to_15, below_10.
        #[arg(long)]
        holding: Option<String>,
        /// Hidden profit distributions (§ 8 Abs. 3 KStG).
        #[arg(long)]
        hidden_distributions: Option<String>,
        /// Hidden contributions booked as income.
        #[arg(long)]
        hidden_contributions: Option<String>,
        /// In a tax group as member: the compensation paid to outside shareholders.
        #[arg(long)]
        group_compensation: Option<String>,
        /// In a tax group as parent: the income attributed from the members.
        #[arg(long)]
        group_income: Option<String>,
        /// In a tax group as parent: their trade income.
        #[arg(long)]
        group_trade_income: Option<String>,
        /// Interest carried forward by the interest barrier.
        #[arg(long)]
        interest_carryforward: Option<String>,
        /// EBITDA carried forward.
        #[arg(long)]
        ebitda_carryforward: Option<String>,
        /// The part in a tax group: member or parent; empty for none.
        #[arg(long)]
        group: Option<String>,
        /// An exception of the interest barrier applies (§ 4h Abs. 2 EStG).
        #[arg(long)]
        interest_exception: Option<bool>,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Lists the employments of an owner, or adds one (librotenado 0013).
    Employment {
        #[command(subcommand)]
        action: Option<EmploymentAction>,
    },
    /// Computes the payslip of an employment for a month; with --record records and books it.
    Payslip {
        /// The employment; left out for a correction.
        #[arg(long, required_unless_present = "corrects")]
        employment: Option<librotenado::payroll::EmploymentId>,
        /// The month, e.g. 2026-01; left out for a correction.
        #[arg(long, required_unless_present = "corrects")]
        month: Option<String>,
        /// Corrects a recorded payslip (Rückrechnung): the same employment and month with these items.
        #[arg(long)]
        corrects: Option<librotenado::payroll::PayslipId>,
        /// For a correction, the day the difference is paid; default today.
        #[arg(long)]
        paid: Option<Date>,
        /// Current wages of the month, e.g. 4000.00.
        #[arg(long)]
        wage: Option<String>,
        /// A one-off payment: a bonus, holiday or Christmas pay.
        #[arg(long)]
        one_off: Option<String>,
        /// Compensation for the loss of the employment (§ 24 Nr. 1 EStG).
        #[arg(long)]
        compensation: Option<String>,
        /// A benefit in kind.
        #[arg(long)]
        in_kind: Option<String>,
        /// Paid free of tax and contributions.
        #[arg(long)]
        tax_free: Option<String>,
        /// Deducted from the net pay.
        #[arg(long)]
        deduction: Option<String>,
        /// A pension paid by the employer (Versorgungsbezug).
        #[arg(long)]
        pension: Option<String>,
        /// Records the payslip and books it.
        #[arg(long)]
        record: bool,
    },
    /// Lists what the owner has to file or pay in a calendar year, and when.
    Obligations {
        /// The calendar year; default this one.
        #[arg(long)]
        year: Option<u16>,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Shows the Lohnsteuerbescheinigung of an employment for a calendar year.
    WageTaxCertificate {
        /// The employment.
        #[arg(long)]
        employment: librotenado::payroll::EmploymentId,
        /// The calendar year.
        #[arg(long)]
        year: u16,
    },
    /// Shows the Beitragsnachweis of an owner to one health insurer for the months of a period.
    Contributions {
        /// The company number (Betriebsnummer) of the insurer.
        #[arg(long)]
        insurer: String,
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
    /// Reopens the latest closing of an owner: the period it closed is open again; nothing is deleted.
    Reopen {
        /// Why it is reopened.
        #[arg(long)]
        reason: String,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
    /// Lists the filings of the books, or prepares, submits or asks about one.
    Filing {
        #[command(subcommand)]
        action: Option<FilingAction>,
    },
    /// Shows the adapter that submits filings in a country, or sets it – with the token from
    /// SPEZADO_ADAPTER_TOKEN, in the environment rather than on the command line.
    Adapter {
        /// The country, e.g. DE.
        country: librotenado::Country,
        /// Its address: https://…, or http://127.0.0.1:… on this device.
        url: Option<String>,
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
    /// Serves the API and the web app on this device, at http://127.0.0.1:PORT – or, with --listen, to other
    /// devices over TLS with a certificate of this workspace's own. Creates the workspace if there is none; takes
    /// the pairing code from SPEZADO_PAIRING_CODE if set (for apps that start spezado themselves).
    Serve {
        /// The port on this device.
        #[arg(long, default_value_t = 8048)]
        port: u16,
        /// An address to listen on for other devices, e.g. 0.0.0.0:8443; they are served over TLS only.
        #[arg(long)]
        listen: Option<std::net::SocketAddr>,
        /// The name other devices reach this one by; default: the name of this computer.
        #[arg(long)]
        host: Option<String>,
    },
    /// Runs the server on this device with an icon in the menu bar or the system tray: open the web app, pair a
    /// device, start at login, quit. Creates the workspace if there is none; takes the pairing code from
    /// SPEZADO_PAIRING_CODE if set.
    #[cfg(feature = "tray")]
    Tray {
        /// The port on this device.
        #[arg(long, default_value_t = 8048)]
        port: u16,
    },
    /// Whether spezado starts at login – with its icon where this build has one, else as the server alone.
    Autostart {
        /// on or off; without, tells whether it does.
        state: Option<Switch>,
        /// The port the server listens on.
        #[arg(long, default_value_t = 8048)]
        port: u16,
    },
    /// Writes the description of the API in OpenAPI to standard output.
    Openapi,
    /// Synchronises books with other devices through a relay, end-to-end encrypted; without an action, every set
    /// of books that is synchronised.
    Sync {
        #[command(subcommand)]
        action: Option<SyncAction>,
    },
    /// Runs a relay: it stores and passes on the encrypted events of books, without reading them. Put it behind a
    /// reverse proxy that speaks TLS.
    Relay {
        /// Where the relay keeps its data; default: spezado-relay in the user's data directory.
        #[arg(long)]
        data: Option<PathBuf>,
        /// The address to listen on.
        #[arg(long, default_value = "127.0.0.1:8049")]
        listen: std::net::SocketAddr,
    },
}

#[derive(Debug, Subcommand)]
enum SyncAction {
    /// Starts synchronising the books through a relay and prints their recovery code.
    Start {
        /// The address of the relay, e.g. https://relay.example.org.
        relay: String,
        /// What to call this device, e.g. Laptop.
        #[arg(long)]
        name: Option<String>,
    },
    /// Prints an invitation for another device, good for a day: one of yours, or someone else's with less rights.
    Invite {
        /// What the invited device may do: read, write or manage (your own devices).
        #[arg(long, default_value = "manage")]
        rights: String,
        /// The last day it may read and write, e.g. for an auditor (YYYY-MM-DD); removed at the next
        /// synchronisation after it.
        #[arg(long)]
        until: Option<String>,
    },
    /// Removes a device or person from the books, by its name or device name; the key of the books changes.
    Remove {
        /// The name or device name.
        who: String,
    },
    /// Asks to join books with an invitation; they arrive once the inviting device has synchronised.
    Join {
        /// The invitation.
        invitation: String,
        /// What to call this device.
        #[arg(long)]
        name: Option<String>,
    },
    /// Brings books back to this device with their recovery code, when no other device is left.
    Recover {
        /// The address of the relay.
        relay: String,
        /// The recovery code.
        code: String,
        /// What to call this device.
        #[arg(long)]
        name: Option<String>,
    },
    /// Lists the devices of the books.
    Devices,
}

#[derive(Debug, Subcommand)]
enum EmploymentAction {
    /// Adds an employment under German law; the employee is described as a person by name.
    Add {
        /// The employee's name.
        #[arg(long)]
        name: String,
        /// The first day.
        #[arg(long)]
        start: Date,
        /// The last day, if it ends.
        #[arg(long)]
        end: Option<Date>,
        /// The employer's number for the employee.
        #[arg(long)]
        number: Option<String>,
        /// The employee's day of birth.
        #[arg(long)]
        born: Option<Date>,
        /// The tax class, 1 to 6; left out with --flat.
        #[arg(long)]
        class: Option<u8>,
        /// Taxed at the flat rate of 2 % (minor employment).
        #[arg(long)]
        flat: bool,
        /// Taxed at the flat rate of 25 % (short-term employment, --person-group 110 --groups 0000).
        #[arg(long)]
        short_term: bool,
        /// A minor employment in a private household.
        #[arg(long)]
        household: bool,
        /// Work after the standard retirement age, with the employer's pension contributions (Aktivrente).
        #[arg(long)]
        active_pension: bool,
        /// For pensions paid: the year they began.
        #[arg(long, requires = "pension_start_monthly")]
        pension_start_year: Option<u16>,
        /// For pensions paid: the pension of the first whole month.
        #[arg(long, requires = "pension_start_year")]
        pension_start_monthly: Option<String>,
        /// The number of child allowances, e.g. 1.5.
        #[arg(long)]
        children: Option<String>,
        /// The religious community that levies church tax: ev, rk, …
        #[arg(long)]
        church: Option<String>,
        /// The company number of the health insurer – for a minor employment the Minijob-Zentrale.
        #[arg(long)]
        insurer: String,
        /// The key of the contribution groups, e.g. 1111, 6500.
        #[arg(long, default_value = "1111")]
        groups: String,
        /// The key of the group of persons, e.g. 101, 109.
        #[arg(long, default_value = "101")]
        person_group: String,
        /// The insurer's additional rate in percent, e.g. 2.9.
        #[arg(long)]
        additional_rate: Option<String>,
        /// The employee pays the care surcharge for having no children.
        #[arg(long)]
        childless: bool,
        /// The children under 25 the care insurance counts.
        #[arg(long, default_value_t = 0)]
        care_children: u8,
        /// The employee works in Saxony.
        #[arg(long)]
        saxony: bool,
        /// The rate of the levy U1 in percent.
        #[arg(long)]
        u1: Option<String>,
        /// The rate of the levy U2 in percent.
        #[arg(long)]
        u2: Option<String>,
        /// The owner; may be left out while the books have one.
        #[arg(long)]
        owner: Option<PartyId>,
    },
}

#[derive(Debug, Subcommand)]
enum FilingAction {
    /// Prepares the filing of a report for a period from the books as they are.
    Prepare {
        /// The report: de-ustva, de-zm, de-euer.
        report: String,
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
    /// Submits a prepared filing through the adapter of its country.
    Submit {
        /// The filing.
        filing: librotenado::FilingId,
        /// A test submission the authority does not process; nothing is recorded.
        #[arg(long)]
        test: bool,
    },
    /// Asks the adapter where a submitted filing stands.
    Refresh {
        /// The filing.
        filing: librotenado::FilingId,
    },
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
    /// The day of that month it begins on, 2 to 28, if not the first.
    #[arg(long)]
    year_start_day: Option<u8>,
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
    /// The trade tax multiplier of the municipality in percent (Hebesatz); none for a freelancer.
    #[arg(long)]
    trade_tax_multiplier: Option<u32>,
}

/// Whether a command creates the workspace if there is none: the terminal interface and the server.
fn starts_from_nothing(command: &Commands) -> bool {
    match command {
        Commands::Tui { .. } | Commands::Serve { .. } => true,
        #[cfg(feature = "tray")]
        Commands::Tray { .. } => true,
        _ => false,
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let directory = start::directory(cli.dir.clone())?;
    if let Commands::Openapi = cli.command {
        use spezado_api::utoipa_openapi;
        println!("{}", utoipa_openapi());
        return Ok(());
    }
    if let Commands::Relay { data, listen } = &cli.command {
        let data = match data {
            Some(data) => data.clone(),
            None => dirs::data_dir().context("no data directory; give --data")?.join("spezado-relay"),
        };
        spezado_relay::run(&data, *listen, |address| {
            println!("spezado relay listens at http://{address}, data in {}", data.display());
            println!("Put a reverse proxy with TLS in front of it. Stop with Ctrl-C.");
        })?;
        return Ok(());
    }
    if let Commands::Autostart { state, port } = cli.command {
        match state {
            Some(Switch::On) => {
                let (program, arguments) = autostart::launcher(cli.dir.as_deref(), port)?;
                autostart::enable(&program, &arguments)?;
            }
            Some(Switch::Off) => autostart::disable()?,
            None => {}
        }
        println!("{}", if autostart::enabled()? { "on" } else { "off" });
        return Ok(());
    }
    if let Commands::Init { device } = cli.command {
        let device = device.unwrap_or_else(|| {
            let id: String = start::uuid_suffix();
            format!("device-{id}").parse().expect("a valid device name")
        });
        let workspace = Workspace::create(&directory, device)?;
        println!("workspace for device {} in {}", workspace.device(), directory.display());
        return Ok(());
    }
    // The terminal interface and the server start from nothing: a workspace for this device, then setting up books.
    if starts_from_nothing(&cli.command) {
        start::ensure_workspace(&directory)?;
    }
    let mut workspace = Workspace::open(&directory)
        .with_context(|| format!("{} – create one with `spezado init`", directory.display()))?;
    match cli.command {
        Commands::Init { .. } | Commands::Autostart { .. } => unreachable!("handled above"),
        Commands::Books => list_books(&workspace),
        Commands::SetUp(setup) => {
            let id = set_up(&mut workspace, setup)?;
            println!("{id}");
            Ok(())
        }
        Commands::Serve { port, listen, host } => {
            let listen = listen.unwrap_or_else(|| std::net::SocketAddr::from(([127, 0, 0, 1], port)));
            let host = host.unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned());
            let pairing = start::pairing_code_given()?;
            spezado_api::serve(
                workspace,
                listen,
                &host,
                pairing,
                |serving| {
                    println!("spezado serves at {}", serving.link.split('#').next().unwrap_or_default());
                    println!("Pair a device with the code {} – or open {}", serving.code, serving.link);
                    if let Some(fingerprint) = &serving.fingerprint {
                        println!("The certificate's fingerprint is {fingerprint}; apps check it, browsers ask once.");
                        if let Ok(code) = qrcode::QrCode::new(serving.link.as_bytes()) {
                            use qrcode::render::unicode::Dense1x2;
                            let image = code
                                .render::<Dense1x2>()
                                .dark_color(Dense1x2::Light)
                                .light_color(Dense1x2::Dark)
                                .build();
                            println!("{image}");
                        }
                    }
                    println!("The code is valid for 15 minutes. Stop with Ctrl-C.");
                },
                async {
                    let _ = tokio::signal::ctrl_c().await;
                },
            )?;
            Ok(())
        }
        #[cfg(feature = "tray")]
        Commands::Tray { port } => {
            let pairing = start::pairing_code_given()?;
            let again = autostart::launcher(cli.dir.as_deref(), port)?;
            tray::run(workspace, std::net::SocketAddr::from(([127, 0, 0, 1], port)), pairing, again)
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
        Commands::Sync { action } => sync(&mut workspace, cli.books, action),
        Commands::Adapter { country, url } => {
            if let Some(url) = url {
                let token =
                    std::env::var("SPEZADO_ADAPTER_TOKEN").context("give the token in SPEZADO_ADAPTER_TOKEN")?;
                workspace.set_adapter(country, &url, &token)?;
            }
            match workspace.adapter(country)? {
                Some(url) => println!("{country}: {url}"),
                None => println!("{country}: no adapter"),
            }
            Ok(())
        }
        command => {
            let books = choose_books(&workspace, cli.books)?;
            run(&mut workspace, &books, command)
        }
    }
}

/// Eight hexadecimal digits from a fresh identity – for a device name nobody chose.
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
                    let money = |units: i128| -> Result<String> {
                        Ok(format_amount(
                            &librotenado::Money::new(units, figures.currency.clone()).context("overflow")?,
                        ))
                    };
                    for (field, units) in &figures.fields {
                        println!("{field:>16} {:>16}", money(*units)?);
                        // The remaining terms the balance sheet notes (§ 268 Abs. 4 and 5 HGB).
                        if let Some(t) = figures.terms.get(field).filter(|t| t.over_one_year != 0 || t.unknown != 0) {
                            println!("{:>16} {:>16}  of which due within a year", "", money(t.up_to_one_year)?);
                            println!("{:>16} {:>16}  of which due after more than a year", "", money(t.over_one_year)?);
                            if t.unknown != 0 {
                                println!(
                                    "{:>16} {:>16}  without a remaining term – to be divided",
                                    "",
                                    money(t.unknown)?
                                );
                            }
                        }
                    }
                    if !figures.reconciliation.is_empty() {
                        println!("Reconciliation to the tax balance sheet:");
                        for (field, units) in &figures.reconciliation {
                            println!("{field:>16} {:>16}", money(*units)?);
                        }
                    }
                }
            }
        }
        Commands::Close { to, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            let software = workspace.software();
            println!("{}", workspace.execute(books, Command::Close { owner, to, software })?);
        }
        Commands::TradeTax { multiplier, from, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            anyhow::ensure!(multiplier.is_none_or(|m| m >= 200), "a multiplier is at least 200 % (§ 16 Abs. 4 GewStG)");
            let from = from.unwrap_or_else(spezado_app::today);
            workspace.change_trade_tax(books, &owner, from, multiplier)?;
            match multiplier {
                Some(m) => println!("trade tax at {m} % from {from}"),
                None => println!("no trade tax from {from}"),
            }
        }
        Commands::IncomeTax {
            year,
            joint,
            other_income,
            deductions,
            losses,
            church,
            child_benefit,
            extraordinary,
            partnership_income,
            partnership_trade_base,
            partnership_trade_tax,
            capital_income,
            child_allowances,
            single_parent_children,
            owner,
        } => {
            let owner = choose_owner(workspace, books, owner)?;
            let mut facts = workspace.income_facts(books, &owner, year)?;
            let euros = |text: &str| -> Result<i128> {
                let money = parse_amount(text, &"EUR".parse().expect("EUR"))?;
                Ok(money.base_units())
            };
            let mut changed = joint.is_some()
                || other_income.is_some()
                || deductions.is_some()
                || losses.is_some()
                || church.is_some();
            for (field, text) in [
                (&mut facts.child_benefit, child_benefit),
                (&mut facts.extraordinary, extraordinary),
                (&mut facts.partnership_income, partnership_income),
                (&mut facts.partnership_trade_base, partnership_trade_base),
                (&mut facts.partnership_trade_tax, partnership_trade_tax),
                (&mut facts.capital_income, capital_income),
            ] {
                if let Some(text) = text {
                    *field = euros(&text)?;
                    changed = true;
                }
            }
            if let Some(n) = child_allowances {
                facts.child_allowances = n;
                changed = true;
            }
            if let Some(n) = single_parent_children {
                facts.single_parent_children = n;
                changed = true;
            }
            if let Some(joint) = joint {
                facts.joint = joint;
            }
            if let Some(text) = other_income {
                facts.other_income = euros(&text)?;
            }
            if let Some(text) = deductions {
                facts.deductions = euros(&text)?;
            }
            if let Some(text) = losses {
                facts.losses = euros(&text)?;
            }
            if let Some(rate) = church {
                facts.church_rate = (rate > 0).then_some(rate);
            }
            if changed {
                workspace.set_income_facts(books, &owner, year, &facts)?;
            }
            let eur = |units: i128| {
                format_amount(&librotenado::Money::new(units, "EUR".parse().expect("EUR")).expect("an amount"))
            };
            println!("joint assessment   {}", if facts.joint { "yes" } else { "no" });
            println!("other income       {:>16}", eur(facts.other_income));
            println!("deductions         {:>16}", eur(facts.deductions));
            println!("losses carried     {:>16}", eur(facts.losses));
            println!("church tax         {}", facts.church_rate.map_or("none".into(), |r| format!("{r} %")));
        }
        Commands::ProfitTax {
            year,
            income_losses,
            trade_losses,
            land_reduction,
            turnover_and_wages,
            holding,
            hidden_distributions,
            hidden_contributions,
            group_compensation,
            group_income,
            group_trade_income,
            interest_carryforward,
            ebitda_carryforward,
            group,
            interest_exception,
            owner,
        } => {
            let owner = choose_owner(workspace, books, owner)?;
            let mut facts = workspace.profit_facts(books, &owner, year)?;
            let euros =
                |text: &str| -> Result<i128> { Ok(parse_amount(text, &"EUR".parse().expect("EUR"))?.base_units()) };
            let mut changed = income_losses.is_some()
                || trade_losses.is_some()
                || land_reduction.is_some()
                || turnover_and_wages.is_some()
                || holding.is_some();
            for (field, text) in [
                (&mut facts.hidden_distributions, hidden_distributions),
                (&mut facts.hidden_contributions, hidden_contributions),
                (&mut facts.group_compensation, group_compensation),
                (&mut facts.group_income, group_income),
                (&mut facts.group_trade_income, group_trade_income),
                (&mut facts.interest_carryforward, interest_carryforward),
                (&mut facts.ebitda_carryforward, ebitda_carryforward),
            ] {
                if let Some(text) = text {
                    *field = euros(&text)?;
                    changed = true;
                }
            }
            if let Some(group) = group {
                facts.group = (!group.is_empty()).then_some(group);
                changed = true;
            }
            if let Some(exception) = interest_exception {
                facts.interest_exception = exception;
                changed = true;
            }
            if let Some(text) = income_losses {
                facts.income_losses = euros(&text)?;
            }
            if let Some(text) = trade_losses {
                facts.trade_losses = euros(&text)?;
            }
            if let Some(text) = land_reduction {
                facts.land_reduction = euros(&text)?;
            }
            if let Some(text) = turnover_and_wages {
                facts.turnover_and_wages = (!text.is_empty()).then(|| euros(&text)).transpose()?;
            }
            if let Some(text) = holding {
                facts.holding = Some(text);
            }
            if changed {
                workspace.set_profit_facts(books, &owner, year, &facts)?;
            }
            let eur = |units: i128| {
                format_amount(&librotenado::Money::new(units, "EUR".parse().expect("EUR")).expect("an amount"))
            };
            println!("losses (§ 10d)     {:>16}", eur(facts.income_losses));
            println!("trade losses       {:>16}", eur(facts.trade_losses));
            println!("land reduction     {:>16}", eur(facts.land_reduction));
            println!("turnover and wages {:>16}", facts.turnover_and_wages.map(eur).unwrap_or_else(|| "–".into()));
            println!("holding            {}", facts.holding.as_deref().unwrap_or("at_least_15"));
        }
        Commands::Employment { action } => employment(workspace, books, action)?,
        Commands::Payslip {
            employment,
            month,
            corrects,
            paid,
            wage,
            one_off,
            compensation,
            in_kind,
            tax_free,
            deduction,
            pension,
            record,
        } => {
            use librotenado::payroll::{PayItem, PayItemKind};
            let eur: Currency = "EUR".parse().expect("EUR");
            let mut items = Vec::new();
            for (kind, name, text) in [
                (PayItemKind::Wage, "Lohn/Gehalt", wage),
                (PayItemKind::OneOff, "Einmalzahlung", one_off),
                (PayItemKind::Compensation, "Abfindung", compensation),
                (PayItemKind::InKind, "Sachbezug", in_kind),
                (PayItemKind::TaxFree, "Steuerfrei", tax_free),
                (PayItemKind::Deduction, "Abzug", deduction),
                (PayItemKind::Pension, "Versorgungsbezug", pension),
            ] {
                if let Some(text) = text {
                    items.push(PayItem { kind, name: name.into(), amount: parse_amount(&text, &eur)? });
                }
            }
            let payslip = match corrects {
                Some(replaced) => {
                    workspace.correction(books, &replaced, items, paid.unwrap_or_else(spezado_app::today))?
                }
                None => {
                    let month = month.ok_or_else(|| anyhow!("a payslip needs --month"))?;
                    let (year, number) = month
                        .split_once('-')
                        .and_then(|(y, m)| Some((y.parse::<u16>().ok()?, m.parse::<u8>().ok()?)))
                        .ok_or_else(|| anyhow!("a month is written 2026-01"))?;
                    let employment = employment.ok_or_else(|| anyhow!("a payslip needs --employment"))?;
                    workspace.payslip(books, &employment, year, number, items)?
                }
            };
            println!("{} {} to {}, paid {}", payslip.id, payslip.from, payslip.to, payslip.paid);
            for (name, amount) in &payslip.figures {
                println!("{name:<28} {:>16}", format_amount(amount));
            }
            if record {
                workspace.record_payslip(books, payslip)?;
                println!("recorded and booked");
            }
        }
        Commands::Obligations { year, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            let year = year.unwrap_or_else(|| spezado_app::today().year());
            for o in workspace.obligations(books, &owner, year)? {
                let due = o.due.map(|d| d.to_string()).unwrap_or_else(|| "–".into());
                println!("{due:<12} {:<10} {} – {}  {}", o.id, o.from, o.to, o.name);
            }
        }
        Commands::WageTaxCertificate { employment, year } => {
            let eur: Currency = "EUR".parse().expect("EUR");
            match workspace.wage_tax_certificate(books, &employment, year)? {
                None => println!("none: wages taxed at a flat rate, or no payslips in {year}"),
                Some(lines) => {
                    for (line, units) in lines {
                        let amount = librotenado::Money::new(units, eur.clone()).expect("an amount");
                        println!("{line:<6} {:>16}", format_amount(&amount));
                    }
                }
            }
        }
        Commands::Contributions { insurer, from, to, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            let eur: Currency = "EUR".parse().expect("EUR");
            for (field, units) in workspace.contribution_statement(books, &owner, &insurer, from, to)? {
                let amount = librotenado::Money::new(units, eur.clone()).expect("an amount");
                println!("{field:<16} {:>16}", format_amount(&amount));
            }
        }
        Commands::Reopen { reason, owner } => {
            let owner = choose_owner(workspace, books, owner)?;
            println!("{}", workspace.execute(books, Command::Reopen { owner, reason })?);
        }
        Commands::Filing { action } => filing(workspace, books, action)?,
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
        Commands::Tui { .. } | Commands::Autostart { .. } => unreachable!("handled in main"),
        #[cfg(feature = "tray")]
        Commands::Tray { .. } => unreachable!("handled in main"),
        Commands::Serve { .. }
        | Commands::Openapi
        | Commands::Sync { .. }
        | Commands::Relay { .. }
        | Commands::Adapter { .. } => {
            unreachable!("handled in main")
        }
        Commands::Init { .. } | Commands::Books | Commands::SetUp(_) | Commands::Import { .. } => {
            unreachable!("handled in main")
        }
    }
    Ok(())
}

fn filing(workspace: &mut Workspace, books: &BooksId, action: Option<FilingAction>) -> Result<()> {
    let status = match action {
        None => {
            let filings = workspace.filings(books)?;
            if filings.is_empty() {
                println!("no filings – prepare one with `spezado filing prepare <report> --from … --to …`");
            }
            for item in filings {
                let filing = &item.filing;
                let report = filing.content.first().map_or("", |c| c.report.as_str());
                let outdated = if item.outdated { ", outdated" } else { "" };
                println!("{} {report:<10} {} – {} {:?}{outdated}", filing.id, filing.from, filing.to, item.state);
            }
            return Ok(());
        }
        Some(FilingAction::Prepare { report, from, to, owner }) => {
            let owner = choose_owner(workspace, books, owner)?;
            let filing = workspace.prepare_filing(books, &owner, &report, from, to)?;
            println!("{}", filing.id);
            if let Some(corrects) = &filing.corrects {
                println!("corrects {corrects}");
            }
            for content in &filing.content {
                for (field, amount) in &content.fields {
                    println!("{field:>12} {:>16}", format_amount(amount));
                }
            }
            return Ok(());
        }
        Some(FilingAction::Submit { filing, test }) => workspace.submit_filing(books, &filing, test)?,
        Some(FilingAction::Refresh { filing }) => workspace.refresh_filing(books, &filing)?,
    };
    println!("{:?}", status.state);
    if let Some(message) = &status.message {
        println!("{message}");
    }
    for receipt in &status.receipts {
        println!("received {}", receipt.name);
    }
    Ok(())
}

fn sync(workspace: &mut Workspace, books: Option<BooksId>, action: Option<SyncAction>) -> Result<()> {
    match action {
        None => {
            let reports = workspace.sync_all()?;
            if reports.is_empty() {
                println!("no books are synchronised – start with `spezado sync start <relay>`");
            }
            for (books, report) in reports {
                if report.waiting {
                    println!("{books}: waiting to be added by the device that invited this one");
                    continue;
                }
                println!("{books}: {} received, {} sent", report.received, report.sent);
                for name in report.added {
                    println!("{books}: added {name}");
                }
            }
        }
        Some(SyncAction::Start { relay, name }) => {
            let books = choose_books(workspace, books)?;
            let code = workspace.start_sync(&books, &relay, name.as_deref())?;
            println!("The books {books} are synchronised through {relay}.");
            println!();
            println!("Recovery code – write it down and keep it safe:");
            println!();
            println!("    {code}");
            println!();
            println!("With it, the books come back to a new device if every device is lost:");
            println!("    spezado --books {books} sync recover {relay} <code>");
            println!("Without it and without a device, the books are lost. spezado shows it only now.");
            match workspace.sync_key_store(&books)? {
                Some("keychain") => println!("This device's key of the books is in the key store of the system."),
                _ => println!("This device's key of the books is in the workspace – protect its folder."),
            }
        }
        Some(SyncAction::Invite { rights, until }) => {
            let books = choose_books(workspace, books)?;
            let rights = match rights.as_str() {
                "read" => librotenado::Rights::Read,
                "write" => librotenado::Rights::Write,
                "manage" => librotenado::Rights::Manage,
                _ => bail!("rights are read, write or manage"),
            };
            let until =
                until.map(|d| d.parse::<librotenado::Date>()).transpose().map_err(|_| anyhow!("--until is a date"))?;
            println!("{}", workspace.invite(&books, rights, until)?);
            println!();
            println!("On the other device: spezado sync join <invitation> – then run spezado sync here.");
        }
        Some(SyncAction::Join { invitation, name }) => {
            let books = workspace.join(&invitation, name.as_deref())?;
            println!("Asked to join the books {books}. Run spezado sync on the inviting device, then here.");
        }
        Some(SyncAction::Recover { relay, code, name }) => {
            let books = books.context("give the books with --books")?;
            let code = code.parse().map_err(|_| anyhow!("not a valid recovery code"))?;
            let report = workspace.recover(&relay, &books, &code, name.as_deref())?;
            println!("The books {books} are back on this device: {} received.", report.received);
        }
        Some(SyncAction::Remove { who }) => {
            let books = choose_books(workspace, books)?;
            let name = workspace.remove_member(&books, &who)?;
            println!(
                "Removed {name}. The books have a new key, which it does not get; what it read, it may have kept."
            );
        }
        Some(SyncAction::Devices) => {
            let books = choose_books(workspace, books)?;
            for member in workspace.sync_members(&books)? {
                let what = if member.recovery { "recovery code".to_owned() } else { member.device.unwrap_or_default() };
                let me = if member.me { " (this device)" } else { "" };
                println!("{:<20} {:<8} {what}{me}", member.name, format!("{:?}", member.rights).to_lowercase());
            }
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

/// Lists the employments of an owner, or adds one.
fn employment(workspace: &mut Workspace, books: &BooksId, action: Option<EmploymentAction>) -> Result<()> {
    match action {
        None => {
            let owner = choose_owner(workspace, books, None)?;
            let (_, persons) = workspace.get(books)?.events().parties();
            let names: BTreeMap<PartyId, String> =
                persons.iter().map(|(id, p)| ((*id).clone(), p.name.clone())).collect();
            for e in workspace.employments(books, &owner, spezado_app::today())? {
                let name = names.get(&e.employee).cloned().unwrap_or_default();
                let end = e.end.map(|d| d.to_string()).unwrap_or_default();
                println!("{}  {name}  {} – {end}", e.id, e.start);
            }
        }
        Some(EmploymentAction::Add {
            name,
            start,
            end,
            number,
            born,
            class,
            flat,
            short_term,
            household,
            active_pension,
            pension_start_year,
            pension_start_monthly,
            children,
            church,
            insurer,
            groups,
            person_group,
            additional_rate,
            childless,
            care_children,
            saxony,
            u1,
            u2,
            owner,
        }) => {
            use librotenado::payroll::{GermanEmployment, GermanInsurance, GermanTax};
            let owner = choose_owner(workspace, books, owner)?;
            let decimal = |text: Option<String>| -> Result<Option<librotenado::Decimal>> {
                text.map(|t| t.parse().map_err(|_| anyhow!("not a number: {t}"))).transpose()
            };
            let employee: PartyId = new_id();
            let person: Person = serde_json::from_value(serde_json::json!({"id": employee.as_str(), "name": name}))?;
            workspace.execute(books, Command::DescribePerson(person))?;
            let de = GermanEmployment {
                tax: GermanTax {
                    class,
                    factor: None,
                    children: decimal(children)?,
                    church,
                    allowance: None,
                    addition: None,
                    flat,
                    short_term,
                    pension_start: match (pension_start_year, pension_start_monthly) {
                        (Some(year), Some(monthly)) => Some(librotenado::payroll::PensionStart {
                            year,
                            monthly: parse_amount(&monthly, &"EUR".parse().expect("EUR"))?,
                        }),
                        _ => None,
                    },
                    active_pension,
                },
                insurance: GermanInsurance {
                    insurer,
                    contribution_group: groups,
                    person_group,
                    additional_rate: decimal(additional_rate)?,
                    private: None,
                    childless,
                    children: care_children,
                    saxony,
                    u1: decimal(u1)?,
                    u2: decimal(u2)?,
                    household,
                },
            };
            let new = spezado_app::NewEmployment { owner, employee, start, end, number, born, de };
            println!("{}", workspace.add_employment(books, new)?);
        }
    }
    Ok(())
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
            contact: None,
            bank_accounts: vec![],
        }),
        None => Party::Person(Person {
            id: id.clone(),
            name: setup.name,
            tax_ids: vec![],
            roles: vec![],
            contact: None,
            bank_accounts: vec![],
        }),
    };
    let mut configuration = serde_json::json!({
        "owner": id.as_str(), "from": setup.from.to_string(), "currency": setup.currency.as_str(),
        "year_start": setup.year_start,
    });
    if let Some(day) = setup.year_start_day {
        configuration["year_start_day"] = day.into();
    }
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
    if let Some(multiplier) = setup.trade_tax_multiplier {
        configuration["trade_tax_multiplier"] = multiplier.into();
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
    Ok(Booking {
        basis: None,
        attributed: None,
        date,
        text,
        postings,
        documents: vec![],
        entries: vec![],
        transaction: None,
    })
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
