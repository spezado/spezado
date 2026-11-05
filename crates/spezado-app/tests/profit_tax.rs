// SPDX-License-Identifier: AGPL-3.0-or-later
//! The taxes on profit (librotenado 0012): corporation tax of a GmbH, trade tax of a sole proprietor, the multiplier
//! changed from a date on.

use std::{fs, path::PathBuf};

use librotenado::{BooksId, Money, Organisation, PartyId, Person};
use spezado_app::{Party, SimpleEntry, Workspace, new_id};

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-profit-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn set_up(scratch: &Scratch, party: impl FnOnce(PartyId) -> Party, accounting: &str) -> (Workspace, BooksId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    workspace.use_keychain(false);
    let owner: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": owner.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": accounting, "vat": {"scheme": "de-regular", "basis": "accrual", "period": "quarter"},
        "trade_tax_multiplier": 400,
    }))
    .unwrap();
    let books = workspace.set_up(party(owner.clone()), configuration).unwrap();
    (workspace, books, owner)
}

/// A payment on an account of SKR 03: gross, VAT by the tax code.
fn pay(workspace: &mut Workspace, books: &BooksId, owner: &PartyId, number: &str, gross: i128, tax: Option<&str>) {
    let standard = workspace.standard_accounts(books, owner, "2026-03-01".parse().unwrap()).unwrap();
    let account = workspace
        .account_choices(books, owner, "2026-03-01".parse().unwrap(), "de")
        .unwrap()
        .into_iter()
        .find(|c| c.number.as_deref() == Some(number))
        .unwrap()
        .id;
    let entry = SimpleEntry {
        attributed: None,
        owner: owner.clone(),
        date: "2026-03-01".parse().unwrap(),
        text: number.into(),
        account,
        gross: Money::new(gross, "EUR".parse().unwrap()).unwrap(),
        tax: tax.map(|t| t.parse().unwrap()),
        payment: standard.bank,
        documents: vec![],
    };
    let booking = workspace.simple_booking(books, &entry).unwrap();
    workspace.execute(books, spezado_core::Command::Book(booking)).unwrap();
}

#[test]
fn a_gmbh_pays_corporation_tax_and_trade_tax() {
    let scratch = Scratch::new("gmbh");
    let organisation = |id: PartyId| {
        Party::Organisation(
            serde_json::from_value::<Organisation>(serde_json::json!({
                "id": id.as_str(), "name": "Beispiel GmbH", "legal_form": "de-gmbh", "country": "DE",
            }))
            .unwrap(),
        )
    };
    let (mut workspace, books, owner) = set_up(&scratch, organisation, "de-double-entry");
    // Sales of 200,000 net, rent of land of 24,000 and business entertainment not deductible of 1,000.
    pay(&mut workspace, &books, &owner, "8400", 23_800_000, Some("de-out-19"));
    pay(&mut workspace, &books, &owner, "4210", 2_400_000, None);
    pay(&mut workspace, &books, &owner, "4654", 100_000, None);
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-12-31".parse().unwrap());
    let reports: Vec<&str> =
        workspace.reports(&books, &owner, from, to).unwrap().into_iter().map(|(id, _)| id).collect();
    assert!(reports.contains(&"de-kst") && reports.contains(&"de-gewst"), "{reports:?}");
    let kst = workspace.report(&books, &owner, "de-kst", from, to).unwrap().fields;
    assert_eq!(kst["result"], 17_500_000);
    assert_eq!(kst["income"], 17_600_000);
    assert_eq!(kst["corporation_tax"], 2_640_000);
    assert_eq!(kst["solidarity"], 145_200);
    let gewst = workspace.report(&books, &owner, "de-gewst", from, to).unwrap().fields;
    // Half of the rent is below 200,000: nothing added; no allowance for a corporation; 3.5 % × 400 %.
    assert_eq!(gewst["financing_e"], 1_200_000);
    assert!(!gewst.contains_key("financing_addition"));
    assert_eq!(gewst["rounded"], 17_600_000);
    assert_eq!(gewst["tax"], 2_464_000);
}

#[test]
fn a_sole_proprietor_pays_trade_tax_from_the_multiplier_on() {
    let scratch = Scratch::new("person");
    let person = |id: PartyId| {
        Party::Person(Person {
            id,
            name: "Ada".into(),
            tax_ids: vec![],
            roles: vec![],
            contact: None,
            bank_accounts: vec![],
        })
    };
    let (mut workspace, books, ada) = set_up(&scratch, person, "de-cash-basis");
    pay(&mut workspace, &books, &ada, "8400", 11_900_000, Some("de-out-19"));
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-12-31".parse().unwrap());
    let reports: Vec<&str> = workspace.reports(&books, &ada, from, to).unwrap().into_iter().map(|(id, _)| id).collect();
    assert!(!reports.contains(&"de-kst") && reports.contains(&"de-gewst"), "{reports:?}");
    let gewst = workspace.report(&books, &ada, "de-gewst", from, to).unwrap().fields;
    // 100,000 less the allowance of 24,500: 75,500 × 3.5 % × 400 %.
    assert_eq!((gewst["rounded"], gewst["allowance"]), (10_000_000, 2_450_000));
    assert_eq!(gewst["tax"], 1_057_000);
    // A freelancer from a date on: no trade tax report for a year that begins after it.
    workspace.change_trade_tax(&books, &ada, "2027-01-01".parse().unwrap(), None).unwrap();
    let next = workspace.reports(&books, &ada, "2027-01-01".parse().unwrap(), "2027-12-31".parse().unwrap()).unwrap();
    assert!(next.iter().all(|(id, _)| *id != "de-gewst"));
}
