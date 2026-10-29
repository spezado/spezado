// SPDX-License-Identifier: AGPL-3.0-or-later
//! Books synchronised through a relay, end-to-end encrypted: a laptop starts, a phone joins by invitation, both
//! write, a tablet comes back with the recovery code; a stranger without an invitation stays out.

use std::fs;
use std::path::PathBuf;

use librotenado::Money;
use librotenado::{BooksId, Person};
use spezado_app::{Customer, InvoiceFormat, InvoiceProfile, NewInvoice, NewLine, Party, Workspace, new_id};

const OCTOBER: &[u8] = include_bytes!("data/statement-001.08.xml");

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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

fn device(scratch: &Scratch, name: &str) -> Workspace {
    Workspace::create(&scratch.0, name.parse().unwrap()).unwrap()
}

/// The hashes of all events of the books, for comparing devices.
fn events(workspace: &Workspace, books: &BooksId) -> Vec<String> {
    workspace.get(books).unwrap().events().ordered().iter().map(|(hash, _)| hash.to_string()).collect()
}

#[test]
fn own_devices_share_books_through_a_relay() {
    let (relay_dir, laptop_dir, phone_dir, tablet_dir, stranger_dir) = (
        Scratch::new("relay"),
        Scratch::new("laptop"),
        Scratch::new("phone"),
        Scratch::new("tablet"),
        Scratch::new("stranger"),
    );
    let relay = relay(&relay_dir);

    // The laptop keeps books with a bank statement, and starts synchronising them.
    let mut laptop = device(&laptop_dir, "laptop");
    let ada: librotenado::PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![], contact: None };
    let books = laptop.set_up(Party::Person(person), configuration).unwrap();
    laptop.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let profile = InvoiceProfile {
        name: "Ada".into(),
        street: "Musterweg 1".into(),
        postcode: "10115".into(),
        city: "Berlin".into(),
        country: "DE".into(),
        vat_id: Some("DE123456789".into()),
        tax_number: None,
        contact: "Ada".into(),
        phone: "+49 30 1234567".into(),
        email: "ada@example.org".into(),
        iban: "DE02120300000000202051".into(),
        payment_days: 14,
        prefix: "RE-".into(),
    };
    laptop.set_invoice_profile(&books, &profile).unwrap();
    let customer = Customer {
        name: "Kunde A GmbH".into(),
        street: "Hauptstraße 5".into(),
        postcode: "80331".into(),
        city: "München".into(),
        country: "DE".into(),
        email: "rechnung@kunde-a.example".into(),
        ..Customer::default()
    };
    let kunde = laptop.save_customer(&books, customer).unwrap();
    let invoice = |day: &str| NewInvoice {
        customer: kunde.clone(),
        date: day.parse().unwrap(),
        delivery: None,
        lines: vec![NewLine {
            name: "Beratung".into(),
            quantity: "1".parse().unwrap(),
            unit: "HUR".into(),
            price: Money::new(10_000, "EUR".parse().unwrap()).unwrap(),
            tax: Some("de-out-19".parse().unwrap()),
        }],
        note: None,
        exemption: None,
        format: InvoiceFormat::XRechnung,
    };
    assert_eq!(
        laptop.issue_invoice(&books, &ada, &invoice("2026-10-01")).unwrap().document.number.as_deref(),
        Some("RE-2026-001")
    );
    let code = laptop.start_sync(&books, &relay, Some("Laptop")).unwrap();
    assert!(laptop.start_sync(&books, &relay, None).is_err(), "only once");

    // The phone joins by invitation: it waits until the laptop has added it, then receives everything.
    let invitation = laptop.invite(&books, librotenado::Rights::Manage).unwrap();
    let mut phone = device(&phone_dir, "phone");
    assert_eq!(phone.join(&invitation, Some("Phone")).unwrap(), books);
    assert!(phone.sync(&books).unwrap().waiting);
    // A stranger who guessed the books but has no invitation is not added.
    let mut stranger = device(&stranger_dir, "stranger");
    let forged = format!("{relay}#join={books}.AAAAAAAAAAAAAAAAAAAAAA");
    stranger.join(&forged, None).unwrap();
    let added = laptop.sync(&books).unwrap();
    assert_eq!((added.added, added.received), (vec!["Phone".to_owned()], 0), "its own envelopes are not received");
    let report = phone.sync(&books).unwrap();
    assert!(!report.waiting && report.received > 0);
    assert_eq!(events(&phone, &books), events(&laptop, &books));
    let statement = &phone.documents(&books).unwrap()[0].1;
    assert!(phone.original(&statement.original.hash).unwrap().is_some(), "the original came along");
    assert!(stranger.sync(&books).unwrap().waiting);

    // The seller's details and the customers came along; the phone writes invoices in a series of its own.
    assert_eq!(phone.invoice_profile(&books).unwrap(), Some(profile));
    assert_eq!(phone.customers(&books).unwrap().len(), 1);
    let numbered = phone.issue_invoice(&books, &ada, &invoice("2026-10-02")).unwrap();
    assert_eq!(numbered.document.number.as_deref(), Some("RE-2026-B001"));
    assert_eq!(laptop.next_invoice_number(&books, 2026).unwrap(), "RE-2026-002");
    phone.sync(&books).unwrap();
    laptop.sync(&books).unwrap();

    // Both write; each receives the other's.
    let close = spezado_core::Command::Close {
        owner: ada.clone(),
        to: "2026-01-31".parse().unwrap(),
        software: Default::default(),
    };
    phone.execute(&books, close).unwrap();
    assert_eq!(phone.sync(&books).unwrap().sent, 1);
    assert_eq!(laptop.sync(&books).unwrap().received, 1);
    assert_eq!(events(&phone, &books), events(&laptop, &books));
    assert_eq!(laptop.sync(&books).unwrap(), Default::default(), "nothing travels twice");
    let names: Vec<String> = laptop.sync_members(&books).unwrap().into_iter().map(|m| m.name).collect();
    assert_eq!(names.len(), 3, "{names:?}");

    // Every device lost: the recovery code brings the books to a new one.
    let mut tablet = device(&tablet_dir, "tablet");
    tablet.recover(&relay, &books, &code, Some("Tablet")).unwrap();
    assert_eq!(events(&tablet, &books), events(&laptop, &books));
    assert_eq!(tablet.review(&books).unwrap(), laptop.review(&books).unwrap(), "{:?}", laptop.review(&books).unwrap());
}

#[test]
fn books_are_shared_with_rights_and_members_removed() {
    let (relay_dir, owner_dir, adviser_dir, bookkeeper_dir) =
        (Scratch::new("relay2"), Scratch::new("owner"), Scratch::new("adviser"), Scratch::new("bookkeeper"));
    let relay = relay(&relay_dir);
    let mut owner = device(&owner_dir, "laptop");
    let ada: librotenado::PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![], contact: None };
    let books = owner.set_up(Party::Person(person), configuration).unwrap();
    owner.start_sync(&books, &relay, Some("Laptop")).unwrap();

    // A tax adviser may read, a bookkeeper may write.
    let mut adviser = device(&adviser_dir, "adviser");
    adviser.join(&owner.invite(&books, librotenado::Rights::Read).unwrap(), Some("Steuerberaterin")).unwrap();
    let mut bookkeeper = device(&bookkeeper_dir, "bookkeeper");
    bookkeeper.join(&owner.invite(&books, librotenado::Rights::Write).unwrap(), Some("Buchhalter")).unwrap();
    assert_eq!(owner.sync(&books).unwrap().added.len(), 2);
    adviser.sync(&books).unwrap();
    bookkeeper.sync(&books).unwrap();
    assert_eq!(adviser.sync_rights(&books).unwrap(), Some(librotenado::Rights::Read));
    let close = |to: &str| spezado_core::Command::Close {
        owner: ada.clone(),
        to: to.parse().unwrap(),
        software: Default::default(),
    };
    assert!(matches!(adviser.execute(&books, close("2026-01-31")), Err(spezado_app::AppError::ReadOnly(_))));
    assert!(adviser.invite(&books, librotenado::Rights::Read).is_err(), "only who manages invites");
    bookkeeper.execute(&books, close("2026-01-31")).unwrap();
    bookkeeper.sync(&books).unwrap();
    assert_eq!(owner.sync(&books).unwrap().received, 1);

    // The adviser is removed: the key changes, and nothing new reaches her.
    assert_eq!(owner.remove_member(&books, "Steuerberaterin").unwrap(), "Steuerberaterin");
    assert!(owner.remove_member(&books, "Laptop").is_err(), "not itself");
    owner.execute(&books, close("2026-02-28")).unwrap();
    owner.sync(&books).unwrap();
    assert!(adviser.sync(&books).is_err(), "removed");
    assert_eq!(bookkeeper.sync(&books).unwrap().received, 1, "the others read on with the new key");
    assert_eq!(events(&bookkeeper, &books), events(&owner, &books));
    let rights: Vec<_> = owner.sync_members(&books).unwrap().into_iter().map(|m| (m.name, m.rights)).collect();
    assert!(rights.contains(&("Buchhalter".into(), librotenado::Rights::Write)), "{rights:?}");
}
