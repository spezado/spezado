// SPDX-License-Identifier: AGPL-3.0-or-later
//! A freelancer's bank: statements read, entries booked by hand, by an earlier entry of the same party and by the
//! invoice they pay; statements that overlap; the balance the books show; FinTS still to come.

use std::{fs, path::PathBuf};

use librotenado::{ChartTranslation, InvoiceSide, Money, PartyId, Person, StatementProblem};
use spezado_app::{AppError, Party, Source, Workspace, new_id};
use spezado_core::{Command, CommandError, Issue};

const OCTOBER: &[u8] = include_bytes!("data/statement-001.08.xml");
const MID_OCTOBER: &[u8] = include_bytes!("data/statement-001.02.xml");
const JANUARY: &[u8] = include_bytes!("data/statement.mt940");
const INVOICE: &[u8] = include_bytes!("data/01.01a-INVOICE_ubl.xml");

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-bank-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn freelancer(scratch: &Scratch) -> (Workspace, librotenado::BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let ada: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![], contact: None };
    let books = workspace.set_up(Party::Person(person), configuration).unwrap();
    (workspace, books, ada)
}

fn numbers(booking: &librotenado::Booking) -> Vec<(String, i128)> {
    let skr03 = ChartTranslation::de_skr03();
    booking
        .postings
        .iter()
        .map(|p| (skr03.national(&p.account).map_or_else(|| "bank".to_owned(), str::to_owned), p.amount.base_units()))
        .collect()
}

#[test]
fn a_year_of_bank_entries() {
    let scratch = Scratch::new("year");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let skr03 = ChartTranslation::de_skr03();

    // October: three entries; read again, nothing new.
    let imported = workspace.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    assert_eq!((imported.documents.len(), imported.entries, imported.known), (1, 3, 0));
    let again = workspace.import_statements(&books, &ada, OCTOBER, None, None).unwrap();
    assert_eq!((again.documents.len(), again.known), (0, 1));
    let bank = workspace.bank_account(&books, "DE89 3704 0044 0532 0130 00").unwrap().expect("opened for the IBAN");
    let choices = workspace.account_choices(&books, &ada, spezado_app::today(), "de").unwrap();
    assert_eq!(choices.iter().find(|c| c.id == bank).unwrap().name, "Bank DE89370400440532013000");
    let open = workspace.open_entries(&books, &ada).unwrap();
    assert_eq!(open.len(), 3);
    assert!(open.iter().all(|item| item.account.as_ref() == Some(&bank) && item.suggestion.is_none()));
    assert_eq!(open[1].text(), "Telefon AG – Kundennummer 123 Oktober");

    // The telephone, by hand: 59.50 with input VAT 19 %.
    let telephone = &open[1];
    let booking = workspace
        .entry_booking(
            &books,
            &ada,
            telephone,
            skr03.accounts["4920"].clone(),
            Some("de-in-19".parse().unwrap()),
            "Telefon".into(),
        )
        .unwrap();
    assert_eq!(numbers(&booking), [("4920".into(), 5_000), ("1576".into(), 950), ("bank".into(), -5_950)]);
    workspace.execute(&books, Command::Book(booking.clone())).unwrap();
    let twice = workspace.execute(&books, Command::Book(booking));
    assert!(matches!(twice, Err(AppError::Command(CommandError::EntryBooked { .. }))));
    assert_eq!(workspace.open_entries(&books, &ada).unwrap().len(), 2);

    // A statement of the middle of October shows the same debit: nothing new to book.
    let mid = workspace.import_statements(&books, &ada, MID_OCTOBER, None, None).unwrap();
    assert_eq!((mid.entries, mid.shown_before), (0, 1));
    assert_eq!(workspace.open_entries(&books, &ada).unwrap().len(), 2);

    // January, in MT940: the telephone again, proposed as before.
    workspace.import_statements(&books, &ada, JANUARY, None, None).unwrap();
    let open = workspace.open_entries(&books, &ada).unwrap();
    let january = open.iter().find(|item| item.entry.amount.base_units() == -5_950).unwrap();
    let suggestion = january.suggestion.as_ref().expect("the same party as in October");
    assert!(matches!(suggestion.source, Source::History(_)));
    assert_eq!(numbers(&suggestion.booking), [("4920".into(), 5_000), ("1576".into(), 950), ("bank".into(), -5_950)]);
    assert_eq!(workspace.accept_suggestions(&books, &ada).unwrap(), 1);

    // October closes at 3,160.50 once the opening balance and every entry of it are booked.
    let mismatch = |workspace: &Workspace| {
        workspace.review(&books).unwrap().iter().any(|issue| {
            matches!(issue, Issue::Statement(StatementProblem::BalanceMismatch { document, .. }) if *document == imported.documents[0])
        })
    };
    assert!(mismatch(&workspace));
    let opening = librotenado::Booking {
        date: "2026-09-30".parse().unwrap(),
        text: "Anfangsbestand".into(),
        postings: vec![
            librotenado::Posting {
                owner: ada.clone(),
                account: bank.clone(),
                amount: Money::new(100_000, "EUR".parse().unwrap()).unwrap(),
                note: None,
                tax: None,
                exchange: None,
            },
            librotenado::Posting {
                owner: ada.clone(),
                account: skr03.accounts["1890"].clone(),
                amount: Money::new(-100_000, "EUR".parse().unwrap()).unwrap(),
                note: None,
                tax: None,
                exchange: None,
            },
        ],
        documents: vec![],
        entries: vec![],
        transaction: None,
    };
    // Before the books begin, an opening balance cannot be booked – configured from 2026-01-01, it can.
    workspace.execute(&books, Command::Book(opening)).unwrap();
    for item in workspace.open_entries(&books, &ada).unwrap() {
        if item.reference.document != imported.documents[0] {
            continue;
        }
        let (number, tax) =
            if item.entry.amount.base_units() > 0 { ("8400", "de-out-19") } else { ("4930", "de-in-19") };
        let booking = workspace
            .entry_booking(&books, &ada, &item, skr03.accounts[number].clone(), Some(tax.parse().unwrap()), item.text())
            .unwrap();
        workspace.execute(&books, Command::Book(booking)).unwrap();
    }
    assert!(!mismatch(&workspace), "{:?}", workspace.review(&books));

    // FinTS waits for its registration.
    assert!(matches!(workspace.fetch_statements(&books, "DE89370400440532013000"), Err(AppError::FinTsNotRegistered)));
}

#[test]
fn an_entry_pays_an_invoice_filed_before() {
    let scratch = Scratch::new("invoice");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let bank = {
        workspace.import_statements(&books, &ada, OCTOBER, None, None).unwrap();
        workspace.bank_account(&books, "DE89370400440532013000").unwrap().unwrap()
    };
    // An invoice to Kunde A, filed without booking: its amount due is what came in on 5 October.
    let mut read = workspace.read_document(&books, &ada, INVOICE, None, InvoiceSide::Sale, &bank).unwrap();
    let invoice = read.document.invoice.as_mut().unwrap();
    invoice.buyer.name = "Kunde A GmbH".into();
    invoice.totals.due = Money::new(238_000, "EUR".parse().unwrap()).unwrap();
    read.document.number = Some("RE-2026-001".into());
    workspace.record_document(&books, read.document.clone(), None).unwrap();
    let open = workspace.open_entries(&books, &ada).unwrap();
    let payment = open.iter().find(|item| item.entry.amount.base_units() == 238_000).unwrap();
    let suggestion = payment.suggestion.as_ref().expect("the invoice it pays");
    assert_eq!(
        suggestion.source,
        Source::Invoice { document: read.document.id.clone(), number: Some("RE-2026-001".into()) }
    );
    assert_eq!(suggestion.booking.entries, std::slice::from_ref(&payment.reference));
    assert!(suggestion.booking.documents.contains(&read.document.id));
    assert_eq!(suggestion.booking.date, payment.entry.booked);
}

#[test]
fn the_review_list_names_statements_and_amounts() {
    let scratch = Scratch::new("review");
    let (mut workspace, books, ada) = freelancer(&scratch);
    workspace.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let de = workspace.review_in(&books, Some(&ada), "de").unwrap();
    let en = workspace.review_in(&books, Some(&ada), "en").unwrap();
    assert_eq!(
        de,
        ["Kontoauszug 2026-10-001 endet mit einem Saldo von 3.160,50 EUR, die Bücher zeigen 0,00 EUR – fehlt ein \
          Anfangsbestand oder eine Buchung?"]
    );
    assert_eq!(
        en,
        ["Statement 2026-10-001 closes at 3,160.50 EUR, the books show 0.00 EUR – is an opening balance or a booking \
          missing?"]
    );
}

#[test]
fn an_opening_balance_makes_the_statement_fit() {
    let scratch = Scratch::new("opening");
    let (mut workspace, books, ada) = freelancer(&scratch);
    workspace.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let bank = workspace.bank_account(&books, "DE89370400440532013000").unwrap().unwrap();
    let start = workspace.books_start(&books, &ada).unwrap().unwrap();
    assert_eq!(start.to_string(), "2026-01-01");

    // Nothing to book, and nothing on the opening balance sheet itself.
    let opening = librotenado::Chart::opening_balances_v0_1();
    let nothing = Money::new(0, "EUR".parse().unwrap()).unwrap();
    assert!(workspace.opening_booking(&books, &ada, start, &[(bank.clone(), nothing)], "x").is_err());
    let euros = Money::new(316050, "EUR".parse().unwrap()).unwrap();
    assert!(workspace.opening_booking(&books, &ada, start, &[(opening, euros.clone())], "x").is_err());

    // The statement closes at 3,160.50 with none of its entries booked: that is what the account held.
    workspace.book_opening(&books, &ada, start, &[(bank.clone(), euros.clone())], "Anfangsbestände").unwrap();
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());
    assert_eq!(workspace.opening_balances(&books, &ada).unwrap(), vec![(bank, euros)]);
    assert_eq!(workspace.bank_holdings(&books, &ada, "2026-12-31".parse().unwrap()).unwrap(), 316_050);
    // No result, no VAT.
    let euer = workspace.report(&books, &ada, "de-euer", start, librotenado::Date::new(2026, 12, 31).unwrap()).unwrap();
    assert!(euer.fields.is_empty(), "{:?}", euer.fields);
}

#[test]
fn a_payment_booked_before_the_statement_is_matched_to_its_entry() {
    let scratch = Scratch::new("matched");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let skr03 = ChartTranslation::de_skr03();
    // The telephone bill paid from the bank and booked on the 14th, before the statement came.
    let entry = spezado_app::SimpleEntry {
        owner: ada.clone(),
        date: "2026-10-14".parse().unwrap(),
        text: "Telefon Oktober".into(),
        account: skr03.accounts["4920"].clone(),
        gross: Money::new(5_950, "EUR".parse().unwrap()).unwrap(),
        tax: Some("de-in-19".parse().unwrap()),
        payment: skr03.accounts["1200"].clone(),
        documents: vec![],
    };
    let paid = workspace.execute(&books, Command::Book(workspace.simple_booking(&books, &entry).unwrap())).unwrap();
    workspace.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let bank = workspace.bank_account(&books, "DE89370400440532013000").unwrap().unwrap();
    let open = workspace.open_entries(&books, &ada).unwrap();
    let telephone = open.iter().find(|item| item.entry.amount.base_units() == -5_950).unwrap();
    let suggestion = telephone.suggestion.clone().expect("the payment booked before");
    assert_eq!(suggestion.source, Source::Booked(paid));
    assert_eq!(suggestion.booking.date.to_string(), "2026-10-14");
    assert_eq!(suggestion.booking.entries, vec![telephone.reference.clone()]);
    assert!(suggestion.booking.postings.iter().any(|p| p.account == bank && p.amount.base_units() == -5_950));

    // Accepted: reversed and booked again on the bank account, the same day – the VAT return does not change.
    let before = workspace
        .report(&books, &ada, "de-ustva", "2026-10-01".parse().unwrap(), "2026-12-31".parse().unwrap())
        .unwrap();
    workspace.accept_suggestion(&books, &suggestion).unwrap();
    let after = workspace
        .report(&books, &ada, "de-ustva", "2026-10-01".parse().unwrap(), "2026-12-31".parse().unwrap())
        .unwrap();
    assert_eq!(before.fields, after.fields);
    assert_eq!(workspace.open_entries(&books, &ada).unwrap().len(), 2);
    let journal = workspace.get(&books).unwrap().journal(Some(&ada), None, None);
    assert_eq!(journal.len(), 3);
    assert!(journal.iter().find(|e| e.event == paid).unwrap().reversed_by.is_some());
    // Proposed for one entry only, and not again once matched.
    assert!(
        workspace
            .open_entries(&books, &ada)
            .unwrap()
            .iter()
            .all(|i| !matches!(i.suggestion.as_ref().map(|s| &s.source), Some(Source::Booked(_))))
    );
}
