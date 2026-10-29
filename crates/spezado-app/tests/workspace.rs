// SPDX-License-Identifier: AGPL-3.0-or-later
//! A freelancer's workspace: set up, booked, reopened, shared with private books, exported and reported.

use std::{fs, path::PathBuf};

use librotenado::{Booking, BooksId, ChartTranslation, Configuration, Money, PartyId, Person, Posting};
use spezado_app::{AppError, Party, Workspace, new_id};
use spezado_core::{Command, CommandError};

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-app-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn person(id: &PartyId, name: &str) -> Party {
    Party::Person(Person { id: id.clone(), name: name.into(), tax_ids: vec![], roles: vec![] })
}

fn configuration(owner: &PartyId, accounting: Option<&str>, vat: bool) -> Configuration {
    let mut json =
        serde_json::json!({"owner": owner.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1});
    if let Some(accounting) = accounting {
        json["accounting"] = accounting.into();
        json["chart"] = "de-skr03".into();
    }
    if vat {
        json["vat"] = serde_json::json!({"scheme": "de-regular", "basis": "accrual", "period": "quarter"});
    }
    serde_json::from_value(json).unwrap()
}

/// A booking in SKR 03 accounts: `(number, cents, tax code)`.
fn booking(owner: &PartyId, date: &str, text: &str, lines: &[(&str, i128, Option<&str>)]) -> Booking {
    let skr03 = ChartTranslation::de_skr03();
    Booking {
        date: date.parse().unwrap(),
        text: text.into(),
        postings: lines
            .iter()
            .map(|(number, cents, tax)| Posting {
                owner: owner.clone(),
                account: skr03.accounts[*number].clone(),
                amount: Money::new(*cents, "EUR".parse().unwrap()).unwrap(),
                note: None,
                tax: tax.map(|code| code.parse().unwrap()),
                exchange: None,
            })
            .collect(),
        documents: vec![],
        entries: vec![],
        transaction: None,
    }
}

/// A freelancer's books, set up in a new workspace.
fn freelancer(scratch: &Scratch) -> (Workspace, BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let ada: PartyId = new_id();
    let configuration = configuration(&ada, Some("de-cash-basis"), true);
    let books = workspace.set_up(person(&ada, "Ada"), configuration).unwrap();
    (workspace, books, ada)
}

#[test]
fn books_are_set_up_booked_and_found_again() {
    let scratch = Scratch::new("reopen");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let sale = booking(
        &ada,
        "2026-02-10",
        "invoice 1",
        &[("1200", 119_000, None), ("8400", -100_000, Some("de-out-19")), ("1776", -19_000, Some("de-out-19"))],
    );
    workspace.execute(&books, Command::Book(sale)).unwrap();
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books));
    assert!(matches!(Workspace::create(&scratch.0, "other".parse().unwrap()), Err(AppError::WorkspaceExists(_))));
    drop(workspace);

    let workspace = Workspace::open(&scratch.0).unwrap();
    assert_eq!(workspace.device().as_str(), "laptop");
    assert_eq!(workspace.get(&books).unwrap().journal(Some(&ada), None, None).len(), 1);
    let chart = workspace.national_chart(&books, &ada, "2026-02-10".parse().unwrap()).unwrap();
    assert_eq!(chart.unwrap().chart, "de-skr03");
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-03-31".parse().unwrap());
    assert_eq!(
        workspace.reports(&books, &ada, from, to).unwrap(),
        [("de-ustva", "Umsatzsteuer-Voranmeldung"), ("de-euer", "Anlage EÜR")]
    );
    let vat = workspace.report(&books, &ada, "de-ustva", from, to).unwrap();
    // Kennzahl 81 takes the base at 19 %; the form computes the tax from it.
    assert_eq!(vat.fields, [("81".to_owned(), 100_000)].into());
    assert!(vat.law.starts_with("spezado-law-de/"));
}

#[test]
fn a_refused_command_writes_nothing() {
    let scratch = Scratch::new("refused");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let good = booking(&ada, "2026-03-01", "paper", &[("4930", 1_240, None), ("1200", -1_240, None)]);
    let early = booking(&ada, "2025-12-01", "paper", &[("4930", 1_240, None), ("1200", -1_240, None)]);
    let result =
        workspace.execute_all(vec![(books.clone(), Command::Book(good)), (books.clone(), Command::Book(early))]);
    assert!(matches!(result, Err(AppError::Command(CommandError::NotConfigured { .. }))));
    assert!(workspace.get(&books).unwrap().journal(None, None, None).is_empty());
    drop(workspace);
    let workspace = Workspace::open(&scratch.0).unwrap();
    assert!(workspace.get(&books).unwrap().journal(None, None, None).is_empty());
}

#[test]
fn a_bill_is_split_between_business_and_private_books() {
    let scratch = Scratch::new("split");
    let (mut workspace, business, ada) = freelancer(&scratch);
    let private = workspace.set_up(person(&ada, "Ada"), configuration(&ada, None, false)).unwrap();
    // Internet 60.00 from the business account, one fifth private (SKR 03 4920 telephone, 1800 withdrawals; the
    // private books have no national chart and book on SKR 03 accounts here only for the test).
    let business_side = booking(
        &ada,
        "2026-10-28",
        "internet",
        &[("4920", 4_800, None), ("1800", 1_200, None), ("1200", -6_000, None)],
    );
    let private_side = booking(&ada, "2026-10-28", "internet", &[("4920", 1_200, None), ("1890", -1_200, None)]);
    let events =
        workspace.book_across(vec![(business.clone(), business_side), (private.clone(), private_side)]).unwrap();
    assert_eq!(events.len(), 2);
    assert!(workspace.review(&business).unwrap().is_empty(), "{:?}", workspace.review(&business));
    assert!(workspace.review(&private).unwrap().is_empty());
    let shared = |books: &BooksId| {
        workspace.get(books).unwrap().journal(None, None, None)[0].booking.transaction.clone().unwrap()
    };
    assert_eq!(shared(&business).id, shared(&private).id);
    assert_eq!(shared(&business).books, std::slice::from_ref(&private));
    // Reversing only one side is for review.
    let date = "2026-10-29".parse().unwrap();
    workspace.execute(&business, Command::Reverse { event: events[0], date, text: "wrong".into() }).unwrap();
    assert_eq!(workspace.review(&private).unwrap().len(), 1);
}

#[test]
fn books_travel_as_json_lines() {
    let scratch = Scratch::new("export");
    let (mut workspace, books, ada) = freelancer(&scratch);
    let paper = booking(&ada, "2026-03-01", "paper", &[("4930", 1_240, None), ("1200", -1_240, None)]);
    workspace.execute(&books, Command::Book(paper)).unwrap();
    let exported = workspace.export(&books).unwrap();

    let other = Scratch::new("import");
    let mut phone = Workspace::create(&other.0, "phone".parse().unwrap()).unwrap();
    assert_eq!(phone.import(&exported).unwrap(), (books.clone(), 6));
    assert_eq!(phone.import(&exported).unwrap(), (books.clone(), 0));
    assert_eq!(phone.get(&books).unwrap().events(), workspace.get(&books).unwrap().events());
    // The phone writes on with its own chain.
    let more = booking(&ada, "2026-03-02", "pens", &[("4930", 300, None), ("1200", -300, None)]);
    phone.execute(&books, Command::Book(more)).unwrap();
    assert!(phone.review(&books).unwrap().is_empty());
    assert!(matches!(phone.import(b"{\"no\": 1}\n"), Err(AppError::Import { line: 1, .. })));
}
