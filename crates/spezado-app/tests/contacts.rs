// SPDX-License-Identifier: AGPL-3.0-or-later
//! Contacts: customers and suppliers with their numbers, contact persons and bank accounts; an e-invoice read in is
//! offered the contact its seller is – or kept as a new one.

use std::{fs, path::PathBuf};

use librotenado::{InvoiceSide, MatchBy, PartyId, Person};
use spezado_app::{ContactCard, ContactPerson, Customer, Party, Workspace, new_id};

const INVOICE: &[u8] = include_bytes!("data/01.01a-INVOICE_ubl.xml");

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-contacts-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn books(scratch: &Scratch) -> (Workspace, librotenado::BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let ada: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-cash-basis", "vat": {"scheme": "de-regular", "basis": "cash", "period": "quarter"},
    }))
    .unwrap();
    let person = Person {
        id: ada.clone(),
        name: "Ada".into(),
        tax_ids: vec![],
        roles: vec![],
        contact: None,
        bank_accounts: vec![],
    };
    let books = workspace.set_up(Party::Person(person), configuration).unwrap();
    (workspace, books, ada)
}

#[test]
fn customers_and_suppliers_are_kept_numbered_and_offered_for_documents() {
    let scratch = Scratch::new("kept");
    let (mut workspace, books, ada) = books(&scratch);

    // A customer from the invoice form gets the first customer number; a supplier the first supplier number.
    let customer = Customer {
        name: "Kunde A GmbH".into(),
        street: "Hauptstraße 5".into(),
        postcode: "80331".into(),
        city: "München".into(),
        country: "DE".into(),
        email: "rechnung@kunde-a.example".into(),
        ..Customer::default()
    };
    let kunde = workspace.save_customer(&books, customer).unwrap();
    let supplier = ContactCard {
        name: "Büro Bedarf KG".into(),
        supplier: true,
        country: "DE".into(),
        iban: Some("DE79 0000 0000 1234 5678 90".into()),
        persons: vec![ContactPerson { name: "Eva Müller".into(), phone: "+49 30 1".into(), ..Default::default() }],
        ..ContactCard::default()
    };
    let lieferant = workspace.save_contact(&books, &ada, supplier).unwrap();
    let contacts = workspace.contacts(&books, &ada).unwrap();
    let find = |id: &str| contacts.iter().find(|c| c.id == id).unwrap().clone();
    assert_eq!((find(&kunde).number.as_deref(), find(&kunde).customer), (Some("10001"), true));
    let found = find(&lieferant);
    assert_eq!((found.number.as_deref(), found.supplier, found.customer), (Some("70001"), true, false));
    assert_eq!(found.persons.len(), 1);
    // A contact person is no contact of its own.
    assert_eq!(contacts.len(), 2);

    // The customer supplies, too: the number and what the invoice form does not show stay.
    let both = ContactCard { supplier: true, phone: "+49 89 2".into(), ..find(&kunde) };
    workspace.save_contact(&books, &ada, both).unwrap();
    let kept = workspace.contacts(&books, &ada).unwrap().into_iter().find(|c| c.id == kunde).unwrap();
    assert_eq!((kept.customer, kept.supplier, kept.number.as_deref()), (true, true, Some("10001")));
    let again =
        Customer { id: kunde.clone(), city: "Augsburg".into(), ..workspace.customers(&books).unwrap()[0].clone() };
    workspace.save_customer(&books, again).unwrap();
    let kept = workspace.contacts(&books, &ada).unwrap().into_iter().find(|c| c.id == kunde).unwrap();
    assert_eq!((kept.city.as_str(), kept.supplier, kept.phone.as_str()), ("Augsburg", true, "+49 89 2"));
    // The contact person is taken off.
    let without = ContactCard { persons: vec![], ..find(&lieferant) };
    workspace.save_contact(&books, &ada, without).unwrap();
    let kept = workspace.contacts(&books, &ada).unwrap().into_iter().find(|c| c.id == lieferant).unwrap();
    assert!(kept.persons.is_empty());

    // An e-invoice of the supplier: found by the account it asks to be paid to, and named in the document.
    let bank = workspace.standard_accounts(&books, &ada, "2026-03-01".parse().unwrap()).unwrap().bank;
    let read =
        workspace.read_document(&books, &ada, INVOICE, Some("invoice.xml"), InvoiceSide::Purchase, &bank).unwrap();
    assert_eq!(read.contacts.len(), 1);
    assert_eq!((read.contacts[0].party.to_string(), read.contacts[0].by), (lieferant.clone(), MatchBy::BankAccount));
    let seller = &read.document.invoice.as_ref().unwrap().seller;
    assert_eq!(seller.party.as_ref().map(ToString::to_string), Some(lieferant.clone()));
    // What it says of the seller, ready to keep as a new contact.
    let new = read.new_contact.unwrap();
    assert_eq!((new.name.as_str(), new.supplier, new.vat_id.as_deref()), ("[Seller name]", true, Some("DE 123456789")));

    // Kept as a new contact, it is found by its VAT identification number next time.
    let id = workspace.save_contact(&books, &ada, new).unwrap();
    let read =
        workspace.read_document(&books, &ada, INVOICE, Some("invoice.xml"), InvoiceSide::Purchase, &bank).unwrap();
    assert_eq!((read.contacts[0].party.to_string(), read.contacts[0].by), (id, MatchBy::VatId));
    assert_eq!(read.contacts.len(), 2, "the supplier with the same account follows");
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());

    // Recorded without a contact, the document names one later – a correction; its original stays.
    let mut document = read.document.clone();
    document.invoice.as_mut().unwrap().seller.party = None;
    let original = document.original.clone();
    workspace.record_document(&books, document.clone(), None).unwrap();
    let contact: PartyId = lieferant.parse().unwrap();
    workspace.assign_contact(&books, &document.id, Some(contact.clone())).unwrap();
    let documents = workspace.documents(&books).unwrap();
    let stored = &documents.iter().find(|(_, d)| d.id == document.id).unwrap().1;
    assert_eq!(stored.invoice.as_ref().unwrap().seller.party.as_ref(), Some(&contact));
    assert_eq!(stored.original, original);
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());
}
