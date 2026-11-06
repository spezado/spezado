// SPDX-License-Identifier: AGPL-3.0-or-later
//! Payroll (librotenado 0013): an employee of a GmbH paid for two months, a bonus, the Lohnsteuer-Anmeldung, the
//! Beitragsnachweis, and the booking.

use std::{fs, path::PathBuf};

use librotenado::payroll::{GermanEmployment, PayItem, PayItemKind};
use librotenado::{BooksId, Money, Organisation, PartyId, Person};
use spezado_app::{NewEmployment, Party, Workspace, new_id};
use spezado_core::Command;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("spezado-payroll-{name}-{}", std::process::id()));
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

fn set_up(scratch: &Scratch) -> (Workspace, BooksId, PartyId, PartyId) {
    let mut workspace = Workspace::create(&scratch.0, "laptop".parse().unwrap()).unwrap();
    workspace.use_keychain(false);
    let owner: PartyId = new_id();
    let configuration = serde_json::from_value(serde_json::json!({
        "owner": owner.as_str(), "from": "2026-01-01", "currency": "EUR", "year_start": 1, "chart": "de-skr03",
        "accounting": "de-double-entry", "region": "DE-BY",
    }))
    .unwrap();
    let organisation: Organisation = serde_json::from_value(serde_json::json!({
        "id": owner.as_str(), "name": "Beispiel GmbH", "legal_form": "de-gmbh", "country": "DE",
    }))
    .unwrap();
    let books = workspace.set_up(Party::Organisation(organisation), configuration).unwrap();
    let employee: PartyId = new_id();
    let person: Person =
        serde_json::from_value(serde_json::json!({"id": employee.as_str(), "name": "Erika Muster"})).unwrap();
    workspace.execute(&books, Command::DescribePerson(person)).unwrap();
    (workspace, books, owner, employee)
}

fn german(group: &str, person: &str) -> GermanEmployment {
    serde_json::from_value(serde_json::json!({
        "tax": {"class": 1, "church": "rk"},
        "insurance": {"insurer": "87654321", "contribution_group": group, "person_group": person,
            "additional_rate": "2.5", "u1": "1.5", "u2": "0.4"},
    }))
    .unwrap()
}

fn item(kind: PayItemKind, units: i128) -> PayItem {
    PayItem { kind, name: "Gehalt".into(), amount: eur(units) }
}

#[test]
fn an_employee_is_paid_and_reported() {
    let scratch = Scratch::new("employee");
    let (mut workspace, books, owner, employee) = set_up(&scratch);
    let (jan, mar) = ("2026-01-01".parse().unwrap(), "2026-03-31".parse().unwrap());
    // Not an employer yet: no Lohnsteuer-Anmeldung.
    assert!(!workspace.reports(&books, &owner, jan, mar).unwrap().iter().any(|(id, _)| *id == "de-lsta"));
    let new = NewEmployment {
        owner: owner.clone(),
        employee,
        start: "2026-01-15".parse().unwrap(),
        end: None,
        number: Some("1".into()),
        born: Some("1985-03-01".parse().unwrap()),
        de: german("1111", "101"),
    };
    let employment = workspace.add_employment(&books, new).unwrap();
    // January from the 15th, February with a bonus.
    let january = workspace.payslip(&books, &employment, 2026, 1, vec![item(PayItemKind::Wage, 180_000)]).unwrap();
    assert_eq!(january.from, "2026-01-15".parse().unwrap());
    let figures = january.figures.clone();
    workspace.record_payslip(&books, january.clone()).unwrap();
    // The same payslip again is refused; one with other figures, too.
    assert!(workspace.record_payslip(&books, january).is_err());
    let mut february = workspace
        .payslip(
            &books,
            &employment,
            2026,
            2,
            vec![item(PayItemKind::Wage, 400_000), item(PayItemKind::OneOff, 200_000)],
        )
        .unwrap();
    let mut forged = february.clone();
    forged.figures.insert("wage_tax".into(), eur(1));
    assert!(workspace.record_payslip(&books, forged).is_err());
    february.id = new_id();
    workspace.record_payslip(&books, february.clone()).unwrap();

    // The Lohnsteuer-Anmeldung of the quarter: wage tax and church tax (rk, Bavaria 8 %) of both.
    let reports = workspace.reports(&books, &owner, jan, mar).unwrap();
    assert!(reports.iter().any(|(id, _)| *id == "de-lsta"));
    let lsta = workspace.report(&books, &owner, "de-lsta", jan, mar).unwrap();
    let sum = |name: &str| {
        figures.get(name).map_or(0, |m| m.base_units()) + february.figures.get(name).map_or(0, |m| m.base_units())
    };
    assert_eq!(lsta.fields["42"], sum("wage_tax"));
    assert_eq!(lsta.fields["62"], sum("church"));
    assert!(sum("church") > 0);
    // The Beitragsnachweis of February to the insurer.
    let feb = ("2026-02-01".parse().unwrap(), "2026-02-28".parse().unwrap());
    let bn = workspace.contribution_statement(&books, &owner, "87654321", feb.0, feb.1).unwrap();
    let both = |name: &str| {
        february.figures.get(&format!("{name}_employee")).map_or(0, |m| m.base_units())
            + february.figures.get(&format!("{name}_employer")).map_or(0, |m| m.base_units())
    };
    assert_eq!(bn["1000"], both("health"));
    assert_eq!(bn["0100"], both("pension"));
    // Booked: the net pay owed on 1740.
    let journal = workspace.get(&books).unwrap().journal(Some(&owner), None, None);
    assert_eq!(journal.iter().filter(|e| e.booking.text.starts_with("Lohn ")).count(), 2);
    assert!(workspace.get(&books).unwrap().events().review_payroll().is_empty());

    // January was 2,000, corrected in March: the difference is booked, the certificate counts the correction.
    let correction = workspace
        .correction(&books, &figures_id(&workspace, &books, &owner), vec![item(PayItemKind::Wage, 200_000)], mar)
        .unwrap();
    workspace.record_payslip(&books, correction.clone()).unwrap();
    let journal = workspace.get(&books).unwrap().journal(Some(&owner), None, None);
    assert!(journal.iter().any(|e| e.booking.text.starts_with("Korrektur Lohn 01/2026")));
    let certificate = workspace.wage_tax_certificate(&books, &employment, 2026).unwrap().unwrap();
    assert_eq!(certificate["3"], 200_000 + 600_000);
    // February was computed from the January replaced: it is to be corrected too.
    let review = workspace.get(&books).unwrap().events().review_payroll();
    assert!(review.iter().any(|p| matches!(p, librotenado::payroll::PayrollProblem::Figures { .. })), "{review:?}");
}

/// The identity of the January payslip.
fn figures_id(workspace: &Workspace, books: &BooksId, owner: &PartyId) -> librotenado::payroll::PayslipId {
    let (from, to) = ("2026-01-01".parse().unwrap(), "2026-01-31".parse().unwrap());
    workspace.payslips(books, owner, from, to).unwrap().remove(0).id
}
