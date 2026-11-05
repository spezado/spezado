// SPDX-License-Identifier: AGPL-3.0-or-later
//! A freelancer writes invoices: the seller's details and a customer, an XRechnung in sequence, paid through the bank;
//! a small business without VAT; with double entry, the receivable booked when issued; supplies without VAT – exempt,
//! reverse charge, to other EU countries – with what the invoice has to state, and the recapitulative statement.

use std::fs;
use std::path::PathBuf;

use librotenado::{CurrencyTable, FilingKind, Money, PartyId, Person, en16931};
use spezado_app::{
    Customer, InvoiceFormat, InvoiceProfile, NewInvoice, NewLine, Party, SimpleEntry, Source, Workspace, new_id,
};

const OCTOBER: &[u8] = include_bytes!("data/statement-001.08.xml");

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-invoicing-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn books(scratch: &Scratch, accounting: &str, vat: &str) -> (Workspace, librotenado::BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    let ada: PartyId = new_id();
    let period = if vat == "de-small-business" { "year" } else { "quarter" };
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": ada.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": accounting, "vat": {"scheme": vat, "basis": "cash", "period": period},
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
    let profile = InvoiceProfile {
        name: "Ada Beispiel".into(),
        street: "Musterweg 1".into(),
        postcode: "10115".into(),
        city: "Berlin".into(),
        country: "DE".into(),
        vat_id: (vat != "de-small-business").then(|| "DE123456789".into()),
        tax_number: Some("11/222/33333".into()),
        contact: "Ada Beispiel".into(),
        phone: "+49 30 1234567".into(),
        email: "ada@example.org".into(),
        iban: "DE02 1203 0000 0000 2020 51".into(),
        payment_days: 14,
        prefix: "RE-".into(),
    };
    workspace.set_invoice_profile(&books, &profile).unwrap();
    (workspace, books, ada)
}

fn customer(workspace: &mut Workspace, books: &librotenado::BooksId) -> String {
    let customer = Customer {
        name: "Kunde A GmbH".into(),
        street: "Hauptstraße 5".into(),
        postcode: "80331".into(),
        city: "München".into(),
        country: "DE".into(),
        email: "rechnung@kunde-a.example".into(),
        ..Customer::default()
    };
    workspace.save_customer(books, customer).unwrap()
}

fn euros(cents: i128) -> Money {
    Money::new(cents, "EUR".parse().unwrap()).unwrap()
}

#[test]
fn an_invoice_is_written_paid_and_reported() {
    let scratch = Scratch::new("cash");
    let (mut workspace, books, ada) = books(&scratch, "de-cash-basis", "de-regular");
    let kunde = customer(&mut workspace, &books);
    assert_eq!(workspace.next_invoice_number(&books, 2026).unwrap(), "RE-2026-001");
    let invoice = NewInvoice {
        customer: kunde.clone(),
        date: "2026-09-30".parse().unwrap(),
        delivery: None,
        lines: vec![NewLine {
            name: "Honorar September".into(),
            quantity: "20".parse().unwrap(),
            unit: "HUR".into(),
            price: euros(10_000),
            tax: Some("de-out-19".parse().unwrap()),
        }],
        note: None,
        exemption: None,
        format: InvoiceFormat::default(),
    };
    let written = workspace.issue_invoice(&books, &ada, &invoice).unwrap();
    assert!(written.booking.is_none(), "on a cash basis, booked when paid");
    assert_eq!(written.document.number.as_deref(), Some("RE-2026-001"));
    // The XML reads back to the same invoice.
    let read = en16931::read(written.xml.as_bytes(), &CurrencyTable::v0_1()).unwrap();
    assert_eq!(read.number, "RE-2026-001");
    assert_eq!(read.invoice.totals.due, euros(238_000));
    assert_eq!(read.invoice.totals.tax, euros(38_000));
    en16931::check_xrechnung(&read).unwrap();
    assert_eq!(workspace.original(&written.document.original.hash).unwrap().unwrap(), written.xml.as_bytes());
    assert_eq!(workspace.next_invoice_number(&books, 2026).unwrap(), "RE-2026-002");

    // Paid in October: the bank proposes the invoice, and the Anlage EÜR counts it.
    workspace.import_statements(&books, &ada, OCTOBER, Some("oktober.xml"), None).unwrap();
    let open = workspace.open_entries(&books, &ada).unwrap();
    let paid = open.iter().find(|item| item.entry.amount.base_units() == 238_000).unwrap();
    let suggestion = paid.suggestion.clone().expect("the invoice it pays");
    assert!(matches!(&suggestion.source, Source::Invoice { number: Some(n), .. } if n == "RE-2026-001"));
    workspace.accept_suggestion(&books, &suggestion).unwrap();
    let euer = workspace
        .report(&books, &ada, "de-euer", "2026-01-01".parse().unwrap(), "2026-12-31".parse().unwrap())
        .unwrap();
    assert_eq!((euer.fields.get("1.112"), euer.fields.get("1.140")), (Some(&200_000), Some(&38_000)));
}

#[test]
fn a_small_business_invoices_without_vat() {
    let scratch = Scratch::new("small");
    let (mut workspace, books, ada) = books(&scratch, "de-cash-basis", "de-small-business");
    let kunde = customer(&mut workspace, &books);
    let line = NewLine {
        name: "Beratung".into(),
        quantity: "2.5".parse().unwrap(),
        unit: "HUR".into(),
        price: euros(8_000),
        tax: None,
    };
    let invoice = NewInvoice {
        customer: kunde,
        date: "2026-03-02".parse().unwrap(),
        delivery: None,
        lines: vec![line],
        note: None,
        exemption: None,
        format: InvoiceFormat::default(),
    };
    let mut hybrid = invoice.clone();
    hybrid.format = InvoiceFormat::Zugferd;
    let pdf = workspace.write_invoice(&books, &ada, &hybrid).unwrap().original;
    if let Ok(dir) = std::env::var("SPEZADO_KEEP_PDF") {
        fs::write(PathBuf::from(dir).join("small-business.pdf"), &pdf).unwrap();
    }
    let written = workspace.issue_invoice(&books, &ada, &invoice).unwrap();
    assert!(written.xml.contains("§ 19 UStG"));
    let read = en16931::read(written.xml.as_bytes(), &CurrencyTable::v0_1()).unwrap();
    assert_eq!((read.invoice.totals.tax.base_units(), read.invoice.totals.due.base_units()), (0, 20_000));
}

#[test]
fn with_double_entry_the_receivable_is_booked_when_issued() {
    let scratch = Scratch::new("double");
    let (mut workspace, books, ada) = books(&scratch, "de-double-entry", "de-regular");
    let kunde = customer(&mut workspace, &books);
    let line = NewLine {
        name: "Projekt".into(),
        quantity: "1".parse().unwrap(),
        unit: "C62".into(),
        price: euros(100_000),
        tax: Some("de-out-7".parse().unwrap()),
    };
    let invoice = NewInvoice {
        customer: kunde,
        date: "2026-05-04".parse().unwrap(),
        delivery: None,
        lines: vec![line],
        note: Some("Danke!".into()),
        exemption: None,
        format: InvoiceFormat::default(),
    };
    let written = workspace.issue_invoice(&books, &ada, &invoice).unwrap();
    let booking = written.booking.expect("the receivable");
    let skr03 = librotenado::ChartTranslation::de_skr03();
    let receivable = booking.postings.iter().find(|p| p.account == skr03.accounts["1400"]).unwrap();
    assert_eq!(receivable.amount, euros(107_000));
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());
    // Without the seller's details, no invoice.
    let scratch = Scratch::new("none");
    let mut bare = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
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
    let books = bare.set_up(Party::Person(person), configuration).unwrap();
    assert!(bare.write_invoice(&books, &ada, &invoice).is_err());
}

#[test]
fn supplies_without_vat_state_why_and_reach_their_fields() {
    let scratch = Scratch::new("without");
    let (mut workspace, books, ada) = books(&scratch, "de-double-entry", "de-regular");
    let at = |vat_id: Option<&str>| Customer {
        name: "Kundin B GmbH".into(),
        street: "Ring 1".into(),
        postcode: "1010".into(),
        city: "Wien".into(),
        country: "AT".into(),
        email: "rechnung@kundin-b.example".into(),
        vat_id: vat_id.map(Into::into),
        ..Customer::default()
    };
    let wien = workspace.save_customer(&books, at(Some("ATU12345678"))).unwrap();
    let wien_without = workspace.save_customer(&books, at(None)).unwrap();
    let zuerich = workspace
        .save_customer(
            &books,
            Customer { country: "CH".into(), city: "Zürich".into(), postcode: "8001".into(), ..at(None) },
        )
        .unwrap();
    let berlin = workspace
        .save_customer(
            &books,
            Customer {
                name: "Bau C GmbH".into(),
                street: "Allee 2".into(),
                postcode: "10117".into(),
                city: "Berlin".into(),
                country: "DE".into(),
                email: "rechnung@bau-c.example".into(),
                vat_id: Some("DE987654321".into()),
                ..Customer::default()
            },
        )
        .unwrap();
    let invoice = |customer: &str, code: &str, cents: i128, exemption: Option<&str>| NewInvoice {
        customer: customer.into(),
        date: "2026-04-15".parse().unwrap(),
        delivery: None,
        lines: vec![NewLine {
            name: "Leistung".into(),
            quantity: "1".parse().unwrap(),
            unit: "C62".into(),
            price: euros(cents),
            tax: Some(code.parse().unwrap()),
        }],
        note: None,
        exemption: exemption.map(Into::into),
        format: InvoiceFormat::default(),
    };
    let cases = [
        (
            &berlin,
            "de-out-exempt",
            10_000,
            Some("Steuerfrei nach § 4 Nr. 21 UStG."),
            "Steuerfrei nach § 4 Nr. 21 UStG.",
        ),
        (&berlin, "de-out-13b", 20_000, None, "Steuerschuldnerschaft des Leistungsempfängers"),
        (&wien, "de-out-eu-service", 30_000, None, "Steuerschuldnerschaft des Leistungsempfängers"),
        (&wien, "de-out-ig-supply", 40_000, None, "Steuerfreie innergemeinschaftliche Lieferung"),
        (&zuerich, "de-out-export", 50_000, None, "Steuerfreie Ausfuhrlieferung"),
    ];
    for (customer, code, cents, exemption, statement) in cases {
        // The same as ZUGFeRD, kept for an outside validator when asked.
        let mut hybrid = invoice(customer, code, cents, exemption);
        hybrid.format = InvoiceFormat::Zugferd;
        let pdf = workspace.write_invoice(&books, &ada, &hybrid).unwrap().original;
        en16931::check_xrechnung(&en16931::read_pdf(&pdf, &CurrencyTable::v0_1()).unwrap()).unwrap();
        if let Ok(dir) = std::env::var("SPEZADO_KEEP_PDF") {
            fs::write(PathBuf::from(dir).join(format!("{code}.pdf")), &pdf).unwrap();
        }
        let written = workspace.issue_invoice(&books, &ada, &invoice(customer, code, cents, exemption)).unwrap();
        assert!(written.xml.contains(statement), "{code}");
        let read = en16931::read(written.xml.as_bytes(), &CurrencyTable::v0_1()).unwrap();
        en16931::check_xrechnung(&read).unwrap();
        assert_eq!(read.invoice.totals.due, euros(cents), "{code}: no VAT");
    }
    let ustva = workspace
        .report(&books, &ada, "de-ustva", "2026-04-01".parse().unwrap(), "2026-06-30".parse().unwrap())
        .unwrap();
    let field = |kz: &str| ustva.fields.get(kz).copied();
    assert_eq!(
        (field("48"), field("60"), field("21"), field("41"), field("43")),
        (Some(10_000), Some(20_000), Some(30_000), Some(40_000), Some(50_000))
    );
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());

    // The recapitulative statement (§ 18a UStG): per customer in another member state, by the VAT identification
    // number on the invoice – goods (L) and services (S) apart; filed as a filing of its own.
    let (from, to) = ("2026-04-01".parse().unwrap(), "2026-06-30".parse().unwrap());
    let zm = workspace.report(&books, &ada, "de-zm", from, to).unwrap();
    assert_eq!(zm.fields, [("ATU12345678/L".to_owned(), 40_000), ("ATU12345678/S".to_owned(), 30_000)].into());
    // Listed for the second quarter only, the one with supplies in it, due on the 25th day after it – a Saturday, so
    // the Monday.
    let statements = workspace.customer_statements(&books, &ada, 2026).unwrap();
    assert_eq!(
        statements.iter().map(|s| (s.from, s.to, s.due.to_string())).collect::<Vec<_>>(),
        [(from, to, "2026-07-27".to_owned())]
    );
    let filing = workspace.prepare_filing(&books, &ada, "de-zm", from, to).unwrap();
    assert_eq!(filing.kind, FilingKind::RecapitulativeStatement);
    assert_eq!(filing.content[0].report, "de-zm");
    assert_eq!(filing.content[0].fields["ATU12345678/L"], euros(40_000));

    // What § 14a UStG asks for is checked before anything is written.
    for (customer, code) in [(&wien, "de-out-13b"), (&berlin, "de-out-eu-service"), (&wien_without, "de-out-ig-supply")]
    {
        assert!(workspace.write_invoice(&books, &ada, &invoice(customer, code, 100, None)).is_err(), "{code}");
    }

    // A supply to another member state booked without an invoice naming the customer's VAT identification number is
    // left out of the statement – and goes to the review list.
    let standard = workspace.standard_accounts(&books, &ada, "2026-05-01".parse().unwrap()).unwrap();
    let entry = SimpleEntry {
        attributed: None,
        owner: ada.clone(),
        date: "2026-05-01".parse().unwrap(),
        text: "Lieferung nach Wien".into(),
        account: standard.sales.clone(),
        gross: euros(5_000),
        tax: Some("de-out-ig-supply".parse().unwrap()),
        payment: standard.bank,
        documents: vec![],
    };
    let booking = workspace.simple_booking(&books, &entry).unwrap();
    workspace.execute(&books, spezado_core::Command::Book(booking)).unwrap();
    assert_eq!(workspace.report(&books, &ada, "de-zm", from, to).unwrap().fields, zm.fields);
    let review = workspace.review_in(&books, Some(&ada), "de").unwrap();
    assert_eq!(review.len(), 1, "{review:?}");
    assert!(review[0].contains("„Lieferung nach Wien“") && review[0].contains("keine USt-IdNr."), "{review:?}");
}

#[test]
fn an_invoice_is_written_as_zugferd() {
    let scratch = Scratch::new("zugferd");
    let (mut workspace, books, ada) = books(&scratch, "de-cash-basis", "de-regular");
    let kunde = customer(&mut workspace, &books);
    let line = |name: &str, cents: i128, tax: &str| NewLine {
        name: name.into(),
        quantity: "3".parse().unwrap(),
        unit: "HUR".into(),
        price: euros(cents),
        tax: Some(tax.parse().unwrap()),
    };
    let mut lines: Vec<NewLine> = (1..=30)
        .map(|n| {
            line(
                &format!("Beratung, Teil {n} – Analyse, Konzept und Umsetzung im Projekt Übergröße"),
                9_550,
                "de-out-19",
            )
        })
        .collect();
    lines.push(line("Fachbuch", 2_490, "de-out-7"));
    let invoice = NewInvoice {
        customer: kunde,
        date: "2026-10-29".parse().unwrap(),
        delivery: Some("2026-10-15".parse().unwrap()),
        lines,
        note: Some("Vielen Dank für den Auftrag.".into()),
        exemption: None,
        format: InvoiceFormat::Zugferd,
    };
    let written = workspace.issue_invoice(&books, &ada, &invoice).unwrap();
    let pdf = &written.original;
    assert!(pdf.starts_with(b"%PDF-1.7"));
    assert_eq!(
        (written.document.original.media_type.as_str(), written.document.original.name.as_deref()),
        ("application/pdf", Some("RE-2026-001.pdf"))
    );
    // The PDF carries the invoice: it reads back to the same data.
    let read = en16931::read_pdf(pdf, &CurrencyTable::v0_1()).unwrap();
    assert_eq!(read.format, "en16931-cii");
    let mut recorded = written.document.invoice.as_deref().unwrap().clone();
    recorded.seller.party = None; // the books' party, not part of the XML
    assert_eq!(read.invoice, recorded);
    en16931::check_xrechnung(&read).unwrap();
    assert_eq!(workspace.original(&written.document.original.hash).unwrap().unwrap(), *pdf);
    // A PDF/A-3 with the Factur-X description.
    let text = String::from_utf8_lossy(pdf);
    assert!(text.contains("<pdfaid:part>3</pdfaid:part>") && text.contains("<fx:ConformanceLevel>XRECHNUNG"));
    if let Ok(dir) = std::env::var("SPEZADO_KEEP_PDF") {
        fs::write(PathBuf::from(dir).join("zugferd.pdf"), pdf).unwrap();
    }
}
