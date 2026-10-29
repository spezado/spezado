// SPDX-License-Identifier: AGPL-3.0-or-later
//! The workflows of a freelancer's year: gross amounts, e-invoices, VAT returns, accounts and the data transfer.

use std::{fs, path::PathBuf};

use librotenado::{BooksId, ChartTranslation, InvoiceSide, Money, PartyId, Person};
use spezado_app::{AppError, Party, SimpleEntry, Workspace, new_id, today};
use spezado_core::Command;

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-workflows-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A freelancer on a cash basis with quarterly VAT returns, in SKR 03 or SKR 04.
fn freelancer(scratch: &Scratch, chart: &str) -> (Workspace, BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let ada: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": chart,
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![], contact: None };
    let books = workspace.set_up(Party::Person(person), configuration).unwrap();
    (workspace, books, ada)
}

fn eur(units: i128) -> Money {
    Money::new(units, "EUR".parse().unwrap()).unwrap()
}

/// The national numbers and amounts of a booking's postings.
fn lines(booking: &librotenado::Booking, chart: &str) -> Vec<(String, i128)> {
    let translation = if chart == "de-skr03" { ChartTranslation::de_skr03() } else { ChartTranslation::de_skr04() };
    booking
        .postings
        .iter()
        .map(|p| (translation.national(&p.account).unwrap().to_owned(), p.amount.base_units()))
        .collect()
}

#[test]
fn gross_amounts_are_split_by_their_tax_code() {
    let scratch = Scratch::new("gross");
    let (mut workspace, books, ada) = freelancer(&scratch, "de-skr03");
    let skr03 = ChartTranslation::de_skr03();
    let standard = workspace.standard_accounts(&books, &ada, "2026-03-01".parse().unwrap()).unwrap();
    let entry = |number: &str, gross: i128, tax: Option<&str>| SimpleEntry {
        owner: ada.clone(),
        date: "2026-03-01".parse().unwrap(),
        text: "test".into(),
        account: skr03.accounts[number].clone(),
        gross: eur(gross),
        tax: tax.map(|t| t.parse().unwrap()),
        payment: standard.bank.clone(),
        documents: vec![],
    };
    // Office paper for 119.00, input VAT 19 %.
    let paper = workspace.simple_booking(&books, &entry("4930", 11_900, Some("de-in-19"))).unwrap();
    assert_eq!(lines(&paper, "de-skr03"), [("4930".into(), 10_000), ("1576".into(), 1_900), ("1200".into(), -11_900)]);
    // A sale for 107.00 at 7 %.
    let sale = workspace.simple_booking(&books, &entry("8300", 10_700, Some("de-out-7"))).unwrap();
    assert_eq!(lines(&sale, "de-skr03"), [("8300".into(), -10_000), ("1771".into(), -700), ("1200".into(), 10_700)]);
    // Without a tax code, the account's kind decides: income is credited.
    let fee = workspace.simple_booking(&books, &entry("4970", 500, None)).unwrap();
    assert_eq!(lines(&fee, "de-skr03"), [("4970".into(), 500), ("1200".into(), -500)]);
    // Reverse charge has no VAT in its gross amount.
    let reverse = workspace.simple_booking(&books, &entry("4930", 10_000, Some("de-in-13b-eu-service")));
    assert!(matches!(reverse, Err(AppError::NeedsLines(_))));
    for booking in [paper, sale, fee] {
        workspace.execute(&books, Command::Book(booking)).unwrap();
    }
    assert!(workspace.review(&books).unwrap().is_empty());
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-03-31".parse().unwrap());
    let vat = workspace.report(&books, &ada, "de-ustva", from, to).unwrap();
    assert_eq!(vat.fields.get("86"), Some(&10_000), "sales at 7 %: {:?}", vat.fields);
    assert_eq!(vat.fields.get("66"), Some(&1_900), "input VAT: {:?}", vat.fields);
}

#[test]
fn skr_04_has_its_own_numbers() {
    let scratch = Scratch::new("skr04");
    let (workspace, books, ada) = freelancer(&scratch, "de-skr04");
    let skr04 = ChartTranslation::de_skr04();
    let standard = workspace.standard_accounts(&books, &ada, "2026-03-01".parse().unwrap()).unwrap();
    let entry = SimpleEntry {
        owner: ada.clone(),
        date: "2026-03-01".parse().unwrap(),
        text: "telephone".into(),
        account: skr04.accounts["6805"].clone(),
        gross: eur(5_950),
        tax: Some("de-in-19".parse().unwrap()),
        payment: standard.bank,
        documents: vec![],
    };
    let booking = workspace.simple_booking(&books, &entry).unwrap();
    assert_eq!(lines(&booking, "de-skr04"), [("6805".into(), 5_000), ("1406".into(), 950), ("1800".into(), -5_950)]);
}

#[test]
fn an_e_invoice_becomes_a_document_and_a_proposal() {
    let scratch = Scratch::new("invoice");
    let (mut workspace, books, ada) = freelancer(&scratch, "de-skr03");
    let xml = include_bytes!("data/01.01a-INVOICE_ubl.xml");
    let standard = workspace.standard_accounts(&books, &ada, today()).unwrap();
    let read =
        workspace.read_document(&books, &ada, xml, Some("invoice.xml"), InvoiceSide::Purchase, &standard.bank).unwrap();
    assert_eq!(read.document.number.as_deref(), Some("123456XX"));
    assert_eq!(read.document.original.format.as_deref(), Some("en16931-ubl"));
    assert_eq!(workspace.original(&read.document.original.hash).unwrap().as_deref(), Some(&xml[..]));
    let mut proposal = read.proposal.expect("a proposal from the invoice");
    // On a cash basis the booking is dated when paid.
    proposal.date = "2026-10-01".parse().unwrap();
    let events = workspace.record_document(&books, read.document.clone(), Some(proposal)).unwrap();
    assert_eq!(events.len(), 2);
    let journal = workspace.get(&books).unwrap().journal(None, None, None);
    assert_eq!(journal[0].booking.documents, std::slice::from_ref(&read.document.id));
    assert_eq!(workspace.documents(&books).unwrap().len(), 1);
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books));

    // A photo is kept as a document of its own, without a proposal.
    let photo = workspace
        .read_document(&books, &ada, b"\xff\xd8\xff\xe0 a photo", None, InvoiceSide::Purchase, &standard.bank)
        .unwrap();
    assert_eq!((photo.document.original.media_type.as_str(), photo.proposal.is_none()), ("image/jpeg", true));
    let garbage =
        workspace.read_document(&books, &ada, b"<html/>", None, InvoiceSide::Purchase, &standard.bank).unwrap();
    assert!(garbage.not_an_invoice.is_some());
}

#[test]
fn vat_returns_fall_due_and_accounts_can_be_chosen() {
    let scratch = Scratch::new("due");
    let (workspace, books, ada) = freelancer(&scratch, "de-skr03");
    let returns = workspace.vat_returns(&books, &ada, 2026).unwrap();
    assert_eq!(returns.len(), 4);
    assert_eq!(returns[0].due, "2026-04-10".parse().unwrap());
    let choices = workspace.account_choices(&books, &ada, "2026-03-01".parse().unwrap(), "en").unwrap();
    let telephone = choices.iter().find(|c| c.number.as_deref() == Some("4920")).unwrap();
    assert_eq!(telephone.name, "Telephone");
    let numbers: Vec<&str> = choices.iter().filter_map(|c| c.number.as_deref()).take(3).collect();
    assert!(numbers.windows(2).all(|w| w[0].len() <= w[1].len()), "{numbers:?}");
}

#[test]
fn deadlines_change_from_a_date_on() {
    let scratch = Scratch::new("change");
    let (mut workspace, books, ada) = freelancer(&scratch, "de-skr03");
    workspace
        .change_deadlines(&books, &ada, "2026-07-01".parse().unwrap(), Some("DE-BY".parse().unwrap()), true)
        .unwrap();
    let due: Vec<String> =
        workspace.vat_returns(&books, &ada, 2026).unwrap().iter().map(|r| r.due.to_string()).collect();
    assert_eq!(due, ["2026-04-10", "2026-07-10", "2026-11-10", "2027-02-10"]);
    let configuration =
        workspace.get(&books).unwrap().events().configuration(&ada, "2026-08-01".parse().unwrap()).cloned().unwrap();
    assert_eq!(configuration.region.map(|r| r.to_string()).as_deref(), Some("DE-BY"));
    assert_eq!(configuration.chart.map(|c| c.to_string()).as_deref(), Some("de-skr03"), "the rest stays");
    assert!(workspace.review(&books).unwrap().is_empty());
}

#[test]
fn the_data_transfer_is_written_into_a_directory() {
    let scratch = Scratch::new("gobd");
    let (mut workspace, books, ada) = freelancer(&scratch, "de-skr03");
    let skr03 = ChartTranslation::de_skr03();
    let standard = workspace.standard_accounts(&books, &ada, "2026-03-01".parse().unwrap()).unwrap();
    let entry = SimpleEntry {
        owner: ada.clone(),
        date: "2026-03-01".parse().unwrap(),
        text: "paper".into(),
        account: skr03.accounts["4930"].clone(),
        gross: eur(11_900),
        tax: Some("de-in-19".parse().unwrap()),
        payment: standard.bank,
        documents: vec![],
    };
    let booking = workspace.simple_booking(&books, &entry).unwrap();
    workspace.execute(&books, Command::Book(booking)).unwrap();
    let target = scratch.0.join("export");
    let files = workspace
        .gobd_export(&books, &ada, "2026-01-01".parse().unwrap(), "2026-12-31".parse().unwrap(), &target)
        .unwrap();
    assert!(files.contains(&"index.xml".to_owned()), "{files:?}");
    assert!(target.join("index.xml").exists());
    workspace.set_language("en").unwrap();
    assert_eq!(workspace.language().unwrap().as_deref(), Some("en"));
}
