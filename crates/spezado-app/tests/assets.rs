// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixed assets: a laptop bought, registered, depreciated once a year by a booking that reaches the Anlage EÜR, shown
//! in the Anlage AVEÜR, and sold.

use std::{fs, path::PathBuf};

use librotenado::{ChartTranslation, Depreciation, Money, PartyId, Person};
use spezado_app::{NewAsset, Party, SimpleEntry, Workspace, new_id};
use spezado_core::Command;

/// A temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-assets-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn eur(units: i128) -> Money {
    Money::new(units, "EUR".parse().unwrap()).unwrap()
}

#[test]
fn a_laptop_is_depreciated_reported_and_sold() {
    let scratch = Scratch::new("laptop");
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
    let skr03 = ChartTranslation::de_skr03();
    let standard = workspace.standard_accounts(&books, &ada, "2026-10-15".parse().unwrap()).unwrap();

    // Bought for 1,800.00 + 342.00 VAT on office equipment, and registered over 36 months.
    let purchase = SimpleEntry {
        attributed: None,
        owner: ada.clone(),
        date: "2026-10-15".parse().unwrap(),
        text: "Laptop".into(),
        account: skr03.accounts["0420"].clone(),
        gross: eur(214_200),
        tax: Some("de-in-19".parse().unwrap()),
        payment: standard.bank.clone(),
        documents: vec![],
    };
    let booking = workspace.simple_booking(&books, &purchase).unwrap();
    workspace.execute(&books, Command::Book(booking)).unwrap();
    let laptop = workspace
        .register_asset(
            &books,
            NewAsset {
                owner: ada.clone(),
                name: "Laptop".into(),
                account: skr03.accounts["0420"].clone(),
                acquired: "2026-10-15".parse().unwrap(),
                cost: eur(180_000),
                depreciation: Depreciation::StraightLine { months: 36 },
            },
        )
        .unwrap();

    // The depreciation of 2026: three months, 150.00, on the expense of depreciation – once.
    assert!(workspace.book_depreciation(&books, &ada, 2026).unwrap());
    assert!(workspace.book_depreciation(&books, &ada, 2026).is_err());
    let year = ("2026-01-01".parse().unwrap(), "2026-12-31".parse().unwrap());
    let euer = workspace.report(&books, &ada, "de-euer", year.0, year.1).unwrap();
    assert_eq!(euer.fields.get("2.130"), Some(&15_000), "{:?}", euer.fields);
    let aveuer = workspace.report(&books, &ada, "de-aveuer", year.0, year.1).unwrap();
    let field = |kz: &str| aveuer.fields.get(kz).copied();
    assert_eq!((field("412"), field("414"), field("416")), (Some(180_000), Some(15_000), Some(165_000)));
    let items = workspace.assets(&books, &ada, year.1).unwrap();
    assert_eq!((items.len(), items[0].book_value), (1, 165_000));

    // Sold in March 2027: refused while the year's depreciation is booked; recorded first, the book value goes out on
    // the account of disposals, and the depreciation of 2027 runs up to March.
    assert!(workspace.book_depreciation(&books, &ada, 2027).unwrap());
    assert!(workspace.dispose_asset(&books, &laptop, "2027-03-20".parse().unwrap(), Some(eur(100_000))).is_err());
    let booked = workspace.get(&books).unwrap().journal(Some(&ada), None, None);
    let afa = booked.iter().find(|e| e.booking.text == "Abschreibungen 2027").unwrap().event;
    workspace
        .execute(&books, Command::Reverse { event: afa, date: "2027-03-20".parse().unwrap(), text: "Storno".into() })
        .unwrap();
    workspace.dispose_asset(&books, &laptop, "2027-03-20".parse().unwrap(), Some(eur(100_000))).unwrap();
    assert!(workspace.book_depreciation(&books, &ada, 2027).unwrap());
    // The asset account is empty: 1,800 bought, 150 + 150 depreciated, 1,500 booked out.
    let account = skr03.accounts["0420"].clone();
    let balance: i128 = workspace
        .get(&books)
        .unwrap()
        .journal(Some(&ada), None, None)
        .iter()
        .flat_map(|e| e.booking.postings.iter())
        .filter(|p| p.account == account)
        .map(|p| p.amount.base_units())
        .sum();
    assert_eq!(balance, 0);
    assert_eq!(workspace.assets(&books, &ada, "2027-12-31".parse().unwrap()).unwrap()[0].book_value, 0);
    assert!(workspace.review(&books).unwrap().is_empty(), "{:?}", workspace.review(&books).unwrap());
}
