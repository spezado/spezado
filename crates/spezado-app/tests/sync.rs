// SPDX-License-Identifier: AGPL-3.0-or-later
//! Books synchronised through a relay, end-to-end encrypted: a laptop starts, a phone joins by invitation, both
//! write, a tablet comes back with the recovery code; a stranger without an invitation stays out.

use std::fs;
use std::path::PathBuf;

use librotenado::{BooksId, Person};
use spezado_app::{Party, Workspace, new_id};

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
    let person = Person { id: ada.clone(), name: "Ada".into(), tax_ids: vec![], roles: vec![] };
    let books = laptop.set_up(Party::Person(person), configuration).unwrap();
    laptop.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let code = laptop.start_sync(&books, &relay, Some("Laptop")).unwrap();
    assert!(laptop.start_sync(&books, &relay, None).is_err(), "only once");

    // The phone joins by invitation: it waits until the laptop has added it, then receives everything.
    let invitation = laptop.invite(&books).unwrap();
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
