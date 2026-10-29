// SPDX-License-Identifier: AGPL-3.0-or-later
//! The interface driven by keys, drawn on a test terminal.

use std::{fs, path::PathBuf};

use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
};
use spezado_app::Workspace;

use crate::{
    App, Lang,
    app::{Mode, Tab},
    view,
};

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-tui-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn press(app: &mut App, code: KeyCode) {
    app.handle(KeyEvent::from(code));
}

fn type_in(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// What the screen shows, as text.
fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            view::draw(frame, app);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let width = buffer.area.width as usize;
    buffer.content().chunks(width).map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>() + "\n").collect()
}

/// Sets up Ada's books through the form shown on first start: name typed, everything else as offered.
fn set_up(app: &mut App) {
    assert!(matches!(app.mode, Mode::Form(..)), "the set-up form on first start");
    type_in(app, "Ada Beispiel");
    for _ in 0..9 {
        press(app, KeyCode::Enter);
    }
    assert!(app.books.is_some(), "{:?}", app.status);
}

#[test]
fn a_freelancer_sets_up_books_and_books_a_receipt() {
    let scratch = Scratch::new("book");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    assert!(screen(&app).contains("Bücher einrichten"));
    set_up(&mut app);
    let shown = screen(&app);
    assert!(shown.contains("Ada Beispiel") && shown.contains("SKR 03") && shown.contains("Übersicht"), "{shown}");

    // Office paper for 119,00 €, input VAT 19 %, paid from the bank.
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Enter); // today
    type_in(&mut app, "Papier");
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "büro");
    assert!(screen(&app).contains("Bürobedarf"), "accounts are offered by name");
    press(&mut app, KeyCode::Enter); // takes the account; the tax code follows its kind
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "119,00");
    for _ in 0..3 {
        press(&mut app, KeyCode::Enter);
    }
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    assert_eq!(app.view.journal.len(), 1);
    let postings: Vec<i128> = app.view.journal[0].booking.postings.iter().map(|p| p.amount.base_units()).collect();
    assert_eq!(postings, [10_000, 1_900, -11_900]);

    press(&mut app, KeyCode::Char('2'));
    let shown = screen(&app);
    assert!(
        shown.contains("Papier") && shown.contains("119,00") && shown.contains("Abziehbare Vorsteuer 19 %"),
        "{shown}"
    );

    // And in English.
    press(&mut app, KeyCode::Char('s'));
    let shown = screen(&app);
    assert!(shown.contains("Journal") && shown.contains("119.00") && shown.contains("Office supplies"), "{shown}");
    assert_eq!(app.workspace.language().unwrap().as_deref(), Some("en"));
}

#[test]
fn a_booking_is_reversed_and_the_vat_return_shows_it() {
    let scratch = Scratch::new("reverse");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::En);
    set_up(&mut app);
    // A sale in lines, then the same as a gross amount.
    press(&mut app, KeyCode::Char('z'));
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "Invoice 1");
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "1200=1,190.00 8400=-1000@de-out-19 1776=-190@de-out-19");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Enter); // today
    type_in(&mut app, "typo");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.journal.len(), 2, "{:?}", app.status);
    assert!(screen(&app).contains("reversed"));
    assert!(app.view.issues.is_empty(), "{:?}", app.view.issues);

    // The reports of the year: four advance returns, the Anlage EÜR.
    press(&mut app, KeyCode::Char('6'));
    assert_eq!(app.view.reports.len(), 5, "{:?}", app.view.reports);
    let quarter = (
        crate::app::App::rows(&app),
        app.view.reports.iter().position(|r| r.from <= spezado_app::today() && spezado_app::today() <= r.to),
    );
    app.selected[5] = quarter.1.unwrap_or(0);
    press(&mut app, KeyCode::Enter);
    let shown = screen(&app);
    assert!(shown.contains("Draft") && shown.contains("spezado-law-de"), "{shown}");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.tab, Tab::Reports);
}

#[test]
fn an_e_invoice_is_read_into_a_proposal_and_booked() {
    let scratch = Scratch::new("invoice");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    let invoice = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../spezado-app/tests/data/01.01a-INVOICE_ubl.xml");
    press(&mut app, KeyCode::Char('4'));
    press(&mut app, KeyCode::Char('i'));
    type_in(&mut app, invoice.to_str().unwrap());
    for _ in 0..3 {
        press(&mut app, KeyCode::Enter);
    }
    assert!(matches!(app.mode, Mode::Proposal(_)), "{:?}", app.status);
    assert!(screen(&app).contains("Buchungsvorschlag"));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    assert_eq!((app.view.documents.len(), app.view.journal.len()), (1, 1));
    assert_eq!(app.view.journal[0].booking.documents, [app.view.documents[0].1.id.clone()]);
    assert!(screen(&app).contains("123456XX"));
}

#[test]
fn a_period_is_closed_and_the_data_transfer_written() {
    let scratch = Scratch::new("close");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    press(&mut app, KeyCode::Char('8'));
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Enter);
    assert!(app.view.closed.is_some(), "{:?}", app.status);
    press(&mut app, KeyCode::Char('g'));
    for _ in 0..10 {
        press(&mut app, KeyCode::Backspace);
    }
    type_in(&mut app, scratch.0.join("gobd").to_str().unwrap());
    for _ in 0..3 {
        press(&mut app, KeyCode::Enter);
    }
    assert!(scratch.0.join("gobd/index.xml").exists(), "{:?}", app.status);
    assert!(screen(&app).contains("Festgeschrieben bis"));
}

#[test]
fn amounts_and_dates_in_both_languages() {
    let eur = "EUR".parse().unwrap();
    let money = |units| librotenado::Money::new(units, "EUR".parse().unwrap()).unwrap();
    assert_eq!(Lang::De.amount(&money(12_345_678)), "123.456,78");
    assert_eq!(Lang::En.amount(&money(-12_345_678)), "-123,456.78");
    assert_eq!(Lang::De.parse_amount("1.234,5", &eur).unwrap().base_units(), 123_450);
    assert_eq!(Lang::De.parse_amount("12.40", &eur).unwrap().base_units(), 1_240);
    assert_eq!(Lang::En.parse_amount("1,234.50", &eur).unwrap().base_units(), 123_450);
    let date = "2026-10-29".parse().unwrap();
    assert_eq!((Lang::De.date(date), Lang::En.date(date)), ("29.10.2026".into(), "2026-10-29".into()));
    assert_eq!(Lang::De.parse_date("29.10.", 2026), Some(date));
    assert_eq!(Lang::De.parse_date("2026-10-29", 2000), Some(date));
}

#[test]
#[ignore = "prints the screens, for looking at them"]
fn print_screens() {
    let scratch = Scratch::new("print");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    for (text, account, gross) in
        [("Papier", "4930", "119,00"), ("Telefon Oktober", "4920", "59,50"), ("Honorar Kunde A", "8400", "2.380,00")]
    {
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Enter);
        type_in(&mut app, text);
        press(&mut app, KeyCode::Enter);
        type_in(&mut app, account);
        press(&mut app, KeyCode::Enter);
        type_in(&mut app, gross);
        press(&mut app, KeyCode::Enter);
        let tax = if account == "8400" { 3 } else { 1 };
        for _ in 0..tax {
            press(&mut app, KeyCode::Right);
        }
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
    }
    println!("{}", screen(&app));
    press(&mut app, KeyCode::Char('2'));
    println!("{}", screen(&app));
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "reise");
    println!("{}", screen(&app));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../spezado-app/tests/data");
    press(&mut app, KeyCode::Char('3'));
    for file in ["statement-001.08.xml", "statement.mt940"] {
        press(&mut app, KeyCode::Char('i'));
        type_in(&mut app, data.join(file).to_str().unwrap());
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
    }
    println!("{}", screen(&app));
}

/// Draws the screen, then clicks on the first place that shows `text`, as a terminal reports it.
fn click(app: &mut App, text: &str) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut hits = view::Hits::default();
    terminal.draw(|frame| hits = view::draw(frame, app)).unwrap();
    app.hits = hits;
    let buffer = terminal.backend().buffer();
    let width = buffer.area.width as usize;
    let rows: Vec<Vec<&str>> =
        buffer.content().chunks(width).map(|row| row.iter().map(|c| c.symbol()).collect()).collect();
    let found = rows.iter().enumerate().find_map(|(y, row)| {
        let line: String = row.concat();
        let byte = line.find(text)?;
        Some((line[..byte].chars().count() as u16, y as u16))
    });
    let (column, row) = found.unwrap_or_else(|| panic!("{text:?} is not on the screen:\n{}", screen(app)));
    let kind = MouseEventKind::Down(MouseButton::Left);
    app.handle_mouse(MouseEvent { kind, column: column + 1, row, modifiers: KeyModifiers::NONE });
}

fn scroll(app: &mut App, kind: MouseEventKind) {
    app.handle_mouse(MouseEvent { kind, column: 10, row: 10, modifiers: KeyModifiers::NONE });
}

#[test]
fn everything_can_be_clicked() {
    let scratch = Scratch::new("mouse");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    // While the set-up form is open, the tabs wait.
    click(&mut app, "2 Journal");
    assert!(matches!(app.mode, Mode::Form(..)) && app.tab == Tab::Overview);
    // Setting up: click into the name, choose SKR 04 by clicking the choice, confirm by the button.
    click(&mut app, "Name");
    type_in(&mut app, "Ada");
    click(&mut app, "‹ SKR 03 ›");
    assert!(screen(&app).contains("‹ SKR 04 ›"));
    click(&mut app, "↵ übernehmen");
    assert!(app.books.is_some(), "{:?}", app.status);

    click(&mut app, "2 Journal");
    assert_eq!(app.tab, Tab::Journal);
    click(&mut app, "n neu");
    click(&mut app, "Konto");
    type_in(&mut app, "telefon");
    click(&mut app, "6805 Telefon");
    click(&mut app, "Brutto");
    type_in(&mut app, "59,50");
    click(&mut app, "↵ übernehmen");
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    let postings: Vec<i128> = app.view.journal[0].booking.postings.iter().map(|p| p.amount.base_units()).collect();
    assert_eq!(postings, [5_000, 950, -5_950], "the tax code followed the account clicked");

    // Rows: a click selects, a second opens; the wheel moves.
    click(&mut app, "6 Berichte");
    scroll(&mut app, MouseEventKind::ScrollDown);
    assert_eq!(app.selected[5], 1);
    click(&mut app, "Umsatzsteuer-Voranmeldung  01.01.2026");
    assert_eq!(app.selected[5], 0);
    click(&mut app, "Umsatzsteuer-Voranmeldung  01.01.2026");
    assert!(matches!(app.mode, Mode::Figures(_)));
    click(&mut app, "Esc zurück");
    assert!(matches!(app.mode, Mode::Normal));

    // The mouse can be switched off.
    click(&mut app, "m Maus aus");
    assert!(!app.mouse);
    assert!(screen(&app).contains("m Maus an"));
    let kind = MouseEventKind::Down(MouseButton::Left);
    app.handle_mouse(MouseEvent { kind, column: 20, row: 1, modifiers: KeyModifiers::NONE });
    assert_eq!(app.tab, Tab::Reports, "no clicks while the mouse is off");
    press(&mut app, KeyCode::Char('m'));
    assert!(app.mouse);
}

#[test]
fn bank_entries_are_read_booked_and_proposed() {
    let scratch = Scratch::new("bank");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../spezado-app/tests/data");
    press(&mut app, KeyCode::Char('3'));
    assert!(screen(&app).contains("Keine offenen Umsätze"));
    press(&mut app, KeyCode::Char('i'));
    type_in(&mut app, data.join("statement-001.08.xml").to_str().unwrap());
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.open_entries.len(), 3, "{:?}", app.status);
    assert!(screen(&app).contains("Telefon AG – Kundennummer 123 Oktober"));
    assert!(screen(&app).contains("Offene Bankumsätze") || app.tab == Tab::Bank);

    // The telephone, booked by hand on 4920 with input VAT.
    app.selected[2] = 1;
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "4920");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Right); // de-in-19
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    assert_eq!(app.view.open_entries.len(), 2);

    // January brings the telephone again – proposed as before, booked with the others' proposals.
    press(&mut app, KeyCode::Char('i'));
    type_in(&mut app, data.join("statement.mt940").to_str().unwrap());
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(screen(&app).contains("wie zuletzt"), "{}", screen(&app));
    let before = app.view.open_entries.len();
    press(&mut app, KeyCode::Char('a'));
    assert_eq!(app.view.open_entries.len(), before - 1, "{:?}", app.status);

    // Fetching from the bank waits for the registration.
    press(&mut app, KeyCode::Char('f'));
    assert!(app.status.as_ref().is_some_and(|(error, text)| *error && text.contains("FinTS")), "{:?}", app.status);
}

#[test]
fn opening_balances_are_booked_on_the_accounts_tab() {
    let scratch = Scratch::new("opening");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    press(&mut app, KeyCode::Char('5'));
    assert_eq!(app.tab, Tab::Accounts);
    press(&mut app, KeyCode::Char('o'));
    assert!(screen(&app).contains("Anfangsbestände"));
    press(&mut app, KeyCode::Enter); // the first day of the books
    type_in(&mut app, "1200=3.160,50 1000=200");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    let shown = screen(&app);
    assert!(shown.contains("3.160,50") && shown.contains("200,00"), "{shown}");
    assert!(app.view.issues.is_empty(), "{:?}", app.view.issues);
}

#[test]
fn deadlines_are_changed_on_the_overview() {
    let scratch = Scratch::new("deadlines");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    assert_eq!(app.tab, Tab::Overview);
    press(&mut app, KeyCode::Char('d'));
    assert!(screen(&app).contains("Fristen ändern"));
    press(&mut app, KeyCode::Enter); // from today
    press(&mut app, KeyCode::Right); // the first state
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Right); // extended
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    let (books, owner) = (app.books.clone().unwrap(), app.owner.clone().unwrap());
    let set = app.workspace.get(&books).unwrap();
    let configuration = set.events().configuration(&owner, spezado_app::today()).cloned().unwrap();
    assert!(configuration.region.is_some() && configuration.vat.unwrap().extended);
}

#[test]
fn an_invoice_is_written_in_the_terminal() {
    let scratch = Scratch::new("writing");
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(app.tab, Tab::Documents);
    // Without the seller's details, the form for them comes first.
    press(&mut app, KeyCode::Char('r'));
    assert!(screen(&app).contains("Absender auf Rechnungen"));
    press(&mut app, KeyCode::Enter); // the name of the books' owner
    for text in ["Musterweg 1", "10115", "Berlin"] {
        type_in(&mut app, text);
        press(&mut app, KeyCode::Enter);
    }
    press(&mut app, KeyCode::Enter); // DE
    type_in(&mut app, "DE123456789");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter); // no tax number
    for text in ["Ada Beispiel", "+49 30 1234567", "ada@example.org", "DE02120300000000202051"] {
        type_in(&mut app, text);
        press(&mut app, KeyCode::Enter);
    }
    press(&mut app, KeyCode::Enter); // 14 days
    press(&mut app, KeyCode::Enter); // RE-
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    // A customer, then the invoice.
    press(&mut app, KeyCode::Char('k'));
    for text in ["Kunde A GmbH", "Hauptstraße 5", "80331", "München"] {
        type_in(&mut app, text);
        press(&mut app, KeyCode::Enter);
    }
    press(&mut app, KeyCode::Enter); // DE
    type_in(&mut app, "rechnung@kunde-a.example");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Enter); // the customer
    press(&mut app, KeyCode::Enter); // today
    type_in(&mut app, "Beratung");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Backspace);
    type_in(&mut app, "2,5");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter); // hours
    type_in(&mut app, "80,00");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter); // 19 %
    press(&mut app, KeyCode::Right); // ZUGFeRD
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.status);
    let shown = screen(&app);
    assert!(shown.contains("RE-"), "{shown}");
    assert_eq!(app.view.documents.len(), 1);
    let (_, document) = &app.view.documents[0];
    assert_eq!(document.original.media_type, "application/pdf");
}

/// A relay on a free port of this machine, running until the test ends; its address.
fn relay(scratch: &Scratch) -> String {
    let relay = spezado_relay::Relay::open(&scratch.0).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, spezado_relay::router(relay)).await.unwrap();
        });
    });
    format!("http://{address}")
}

#[test]
fn synchronisation_is_set_up_on_the_overview() {
    let (scratch, relay_dir) = (Scratch::new("sync"), Scratch::new("relay"));
    let relay = relay(&relay_dir);
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    workspace.use_keychain(false);
    let mut app = App::new(&mut workspace, Lang::De);
    set_up(&mut app);
    assert!(screen(&app).contains("nur auf diesem Gerät"));
    press(&mut app, KeyCode::Char('y'));
    assert!(screen(&app).contains("Synchronisation einrichten"));
    for _ in 0.."https://".len() {
        press(&mut app, KeyCode::Backspace);
    }
    type_in(&mut app, &relay);
    press(&mut app, KeyCode::Enter);
    type_in(&mut app, "Laptop");
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Notice(crate::i18n::Text::RecoveryCode, _)), "{:?}", app.status);
    press(&mut app, KeyCode::Esc);
    assert!(screen(&app).contains(&relay));
    press(&mut app, KeyCode::Char('v'));
    let Mode::Notice(crate::i18n::Text::Invitation, lines) = &app.mode else { panic!("{:?}", app.status) };
    assert!(lines[0].contains("#join="));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('y'));
    assert!(
        app.status.as_ref().is_some_and(|(error, text)| !error && text.starts_with("Synchronisiert")),
        "{:?}",
        app.status
    );
}
