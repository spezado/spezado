// SPDX-License-Identifier: AGPL-3.0-or-later
//! German law for spezado – a first cut: the DATEV standard charts, the German tax codes, the VAT return, the
//! income statement on a cash basis and the HGB statements, and the periods of advance VAT returns. The data comes
//! from librotenado; the rules of what applies when are here. All of it is a draft for review by tax advisers.
//!
//! Sources: § 18 Abs. 1 and 2 UStG (advance returns, their periods and deadline), § 46 UStDV (the extension of the
//! deadline), § 108 Abs. 3 AO (deadlines on a weekend or a public holiday), § 4 Abs. 3 EStG (cash basis), §§ 266, 275 HGB (statements). What is not covered yet is marked OPEN
//! where it would apply.

use std::collections::BTreeMap;

use librotenado::{AccountId, Chart, ChartTranslation, Date, FilingKind, ReportMapping, ReturnPeriod, TaxTable};
use spezado_law::{Facts, LawModule, Report, ReportBasis, StandardAccounts};

use crate::holidays::{next_day, weekday};

mod holidays;

/// The states: their code (ISO 3166-2), German and English name – the regions whose public holidays move deadlines.
pub const STATES: [(&str, &str, &str); 16] = [
    ("DE-BW", "Baden-Württemberg", "Baden-Württemberg"),
    ("DE-BY", "Bayern", "Bavaria"),
    ("DE-BE", "Berlin", "Berlin"),
    ("DE-BB", "Brandenburg", "Brandenburg"),
    ("DE-HB", "Bremen", "Bremen"),
    ("DE-HH", "Hamburg", "Hamburg"),
    ("DE-HE", "Hessen", "Hesse"),
    ("DE-MV", "Mecklenburg-Vorpommern", "Mecklenburg-Western Pomerania"),
    ("DE-NI", "Niedersachsen", "Lower Saxony"),
    ("DE-NW", "Nordrhein-Westfalen", "North Rhine-Westphalia"),
    ("DE-RP", "Rheinland-Pfalz", "Rhineland-Palatinate"),
    ("DE-SL", "Saarland", "Saarland"),
    ("DE-SN", "Sachsen", "Saxony"),
    ("DE-ST", "Sachsen-Anhalt", "Saxony-Anhalt"),
    ("DE-SH", "Schleswig-Holstein", "Schleswig-Holstein"),
    ("DE-TH", "Thüringen", "Thuringia"),
];

/// The German law module.
#[derive(Debug, Default)]
pub struct Germany;

impl LawModule for Germany {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn applies(&self, facts: &Facts) -> bool {
        facts.country.is_some_and(|country| country.as_str() == "DE")
    }

    fn charts(&self, chart: &Chart) -> Vec<ChartTranslation> {
        let into = format!("{}/{}", chart.name, chart.version);
        [ChartTranslation::de_skr03(), ChartTranslation::de_skr04()]
            .into_iter()
            .filter(|translation| translation.into == into)
            .collect()
    }

    fn tax_table(&self, date: Date) -> Option<TaxTable> {
        let table = TaxTable::de();
        (table.valid_from <= date).then_some(table)
    }

    fn reports(&self, facts: &Facts, from: Date, to: Date) -> Vec<Report> {
        let configuration = facts.configuration;
        // The mappings are published per year; a period across years has none. OPEN: mappings of further years.
        let year = (from.year() == to.year()).then_some(from.year());
        let mut reports = Vec::new();
        let regular = configuration.vat.as_ref().is_some_and(|vat| vat.scheme.as_str() == "de-regular");
        if regular && year == Some(2026) {
            let mapping = ReportMapping::de_ustva_2026();
            reports.push(Report {
                id: "de-ustva",
                name: "Umsatzsteuer-Voranmeldung",
                basis: ReportBasis::Period,
                mapping,
                filing: Some(FilingKind::VatReturn),
            });
        }
        match configuration.accounting.as_ref().map(|code| code.as_str()) {
            Some("de-cash-basis") if matches!(year, Some(2025 | 2026)) => {
                let mapping =
                    if year == Some(2025) { ReportMapping::de_euer_2025() } else { ReportMapping::de_euer_2026() };
                reports.push(Report {
                    id: "de-euer",
                    name: "Anlage EÜR",
                    basis: ReportBasis::Period,
                    mapping,
                    filing: Some(FilingKind::CashBasisStatement),
                });
            }
            Some("de-double-entry") if year == Some(2026) => {
                reports.push(Report {
                    id: "de-hgb-266",
                    name: "Bilanz (§ 266 HGB)",
                    basis: ReportBasis::Balances,
                    mapping: ReportMapping::de_hgb_266_2026(),
                    // Filed as the E-Bilanz, with mappings of their own (OPEN).
                    filing: None,
                });
                reports.push(Report {
                    id: "de-hgb-275-gkv",
                    name: "Gewinn- und Verlustrechnung (§ 275 Abs. 2 HGB)",
                    basis: ReportBasis::Period,
                    mapping: ReportMapping::de_hgb_275_gkv_2026(),
                    filing: None,
                });
            }
            _ => {}
        }
        reports
    }

    fn vat_deadline(&self, facts: &Facts, end: Date) -> Date {
        // The tenth day after the end of the period (§ 18 Abs. 1 Satz 1 UStG), a month later with the permanent
        // extension (§ 46 UStDV); on a weekend or a public holiday where the tax office sits, the next working day
        // (§ 108 Abs. 3 AO).
        let months = if facts.configuration.vat.as_ref().is_some_and(|vat| vat.extended) { 2 } else { 1 };
        let index = u32::from(end.month()) - 1 + months;
        let (year, month) = (end.year() + (index / 12) as u16, (index % 12) as u8 + 1);
        let state = facts.configuration.region.as_ref().map(|region| region.as_str());
        let mut day = Date::new(year, month, 10).expect("the tenth exists");
        while weekday(day) >= 5 || holidays::is_holiday(day, state) {
            day = next_day(day);
        }
        day
    }

    fn standard_accounts(&self, national: &ChartTranslation) -> Option<StandardAccounts> {
        let column = match national.chart.as_str() {
            "de-skr03" => 1,
            "de-skr04" => 2,
            _ => return None,
        };
        let account = |role: &str| -> Option<AccountId> {
            let row = STANDARD.iter().find(|row| row.0 == role)?;
            national.accounts.get(if column == 1 { row.1 } else { row.2 }).cloned()
        };
        let vat: Option<BTreeMap<_, _>> = STANDARD
            .iter()
            .filter(|row| row.0.starts_with("de-"))
            .map(|row| Some((row.0.parse().ok()?, account(row.0)?)))
            .collect();
        Some(StandardAccounts {
            bank: account("bank")?,
            cash: account("cash")?,
            sales: account("sales")?,
            expenses: account("expenses")?,
            withdrawals: account("withdrawals")?,
            contributions: account("contributions")?,
            receivables: account("receivables")?,
            vat: vat?,
            vat_reverse_charge: account("reverse-charge")?,
            vat_reverse_charge_input: account("reverse-charge-input")?,
        })
    }

    fn vat_periods(&self, facts: &Facts, year: u16) -> Vec<(Date, Date)> {
        let Some(vat) = &facts.configuration.vat else { return Vec::new() };
        let months = match vat.period {
            ReturnPeriod::Month => 1,
            ReturnPeriod::Quarter => 3,
            ReturnPeriod::Year => return Vec::new(),
        };
        (1..=12)
            .step_by(months)
            .filter_map(|first: u8| {
                let last = first + months as u8 - 1;
                Some((Date::new(year, first, 1)?, last_day(year, last)?))
            })
            .collect()
    }
}

/// The standard accounts by number, in SKR 03 and SKR 04: (role, SKR 03, SKR 04). Names as DATEV lists them.
const STANDARD: &[(&str, &str, &str)] = &[
    ("bank", "1200", "1800"),
    ("cash", "1000", "1600"),
    ("sales", "8400", "4400"),
    ("expenses", "4900", "6300"),
    ("withdrawals", "1800", "2100"),
    ("contributions", "1890", "2180"),
    ("receivables", "1400", "1200"),
    ("de-out-19", "1776", "3806"),
    ("de-out-7", "1771", "3801"),
    ("de-in-19", "1576", "1406"),
    ("de-in-7", "1571", "1401"),
    ("reverse-charge", "1787", "3837"),
    ("reverse-charge-input", "1577", "1407"),
];

/// The last day of a month.
fn last_day(year: u16, month: u8) -> Option<Date> {
    (28..=31).rev().find_map(|day| Date::new(year, month, day))
}

#[cfg(test)]
mod tests {
    use librotenado::Configuration;

    use super::*;

    fn configuration(accounting: &str, vat: Option<(&str, &str)>) -> Configuration {
        let mut json = serde_json::json!({
            "owner": "0192f1e4-0000-7000-8000-0000000000a1", "from": "2025-01-01", "currency": "EUR",
            "year_start": 1, "accounting": accounting,
        });
        if let Some((scheme, period)) = vat {
            json["vat"] = serde_json::json!({"scheme": scheme, "basis": "accrual", "period": period});
        }
        serde_json::from_value(json).unwrap()
    }

    fn date(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn applies_by_the_codes_of_a_person_or_the_country_of_an_organisation() {
        let freelancer = configuration("de-cash-basis", None);
        assert!(Germany.applies(&Facts::new(&freelancer, None, None)));
        let french = configuration("fr-something", None);
        assert!(!Germany.applies(&Facts::new(&french, None, None)));
        assert!(Germany.applies(&Facts::new(&french, Some("DE".parse().unwrap()), Some("de-gmbh"))));
    }

    #[test]
    fn a_freelancer_files_vat_returns_and_the_anlage_euer() {
        let configuration = configuration("de-cash-basis", Some(("de-regular", "quarter")));
        let facts = Facts::new(&configuration, None, None);
        let ids = |from, to| -> Vec<&str> {
            Germany.reports(&facts, date(from), date(to)).iter().map(|report| report.id).collect()
        };
        assert_eq!(ids("2026-01-01", "2026-03-31"), ["de-ustva", "de-euer"]);
        assert_eq!(ids("2025-01-01", "2025-12-31"), ["de-euer"]);
        assert!(ids("2025-12-01", "2026-01-31").is_empty());
        let quarters = Germany.vat_periods(&facts, 2026);
        assert_eq!(quarters.len(), 4);
        assert_eq!(quarters[3], (date("2026-10-01"), date("2026-12-31")));
    }

    #[test]
    fn small_businesses_file_no_advance_returns_and_months_end_on_their_last_day() {
        let small = configuration("de-cash-basis", Some(("de-small-business", "year")));
        let facts = Facts::new(&small, None, None);
        assert!(Germany.vat_periods(&facts, 2026).is_empty());
        // No VAT return, but the Anlage EÜR all the same.
        let reports = Germany.reports(&facts, date("2026-01-01"), date("2026-12-31"));
        assert_eq!(reports.iter().map(|report| report.id).collect::<Vec<_>>(), ["de-euer"]);
        let monthly = configuration("de-double-entry", Some(("de-regular", "month")));
        let months = Germany.vat_periods(&Facts::new(&monthly, None, None), 2028);
        assert_eq!(months.len(), 12);
        assert_eq!(months[1], (date("2028-02-01"), date("2028-02-29")));
    }

    #[test]
    fn advance_returns_are_due_on_the_tenth_or_the_next_working_day() {
        let configuration = configuration("de-cash-basis", Some(("de-regular", "quarter")));
        let facts = Facts::new(&configuration, None, None);
        // 10 April 2026 is a Friday; 10 January 2027 a Sunday; 10 October 2026 a Saturday.
        assert_eq!(Germany.vat_deadline(&facts, date("2026-03-31")), date("2026-04-10"));
        assert_eq!(Germany.vat_deadline(&facts, date("2026-12-31")), date("2027-01-11"));
        assert_eq!(Germany.vat_deadline(&facts, date("2026-09-30")), date("2026-10-12"));
    }

    #[test]
    fn the_standard_accounts_of_both_charts() {
        for translation in [ChartTranslation::de_skr03(), ChartTranslation::de_skr04()] {
            let accounts = Germany.standard_accounts(&translation).unwrap();
            let number = |id: &librotenado::AccountId| translation.national(id).unwrap().to_owned();
            let expected = if translation.chart == "de-skr03" { ("1200", "1576") } else { ("1800", "1406") };
            assert_eq!(
                (number(&accounts.bank), number(&accounts.vat[&"de-in-19".parse().unwrap()])),
                (expected.0.into(), expected.1.into())
            );
            assert_eq!(accounts.vat.len(), 4);
        }
        assert!(Germany.standard_accounts(&ChartTranslation::fr_pcg()).is_none());
    }

    #[test]
    fn holidays_and_the_permanent_extension_move_the_deadline() {
        let mut monthly = configuration("de-cash-basis", Some(("de-regular", "month")));
        let due = |c: &Configuration, end: &str| Germany.vat_deadline(&Facts::new(c, None, None), date(end));
        // The 10th of May 2029 is Ascension Day.
        assert_eq!(due(&monthly, "2029-04-30"), date("2029-05-11"));
        assert_eq!(due(&monthly, "2026-11-30"), date("2026-12-10"));
        assert_eq!(due(&monthly, "2026-12-31"), date("2027-01-11"));
        // A month later with the extension, across the year, too.
        monthly.vat.as_mut().unwrap().extended = true;
        assert_eq!(due(&monthly, "2029-03-31"), date("2029-05-11"));
        assert_eq!(due(&monthly, "2026-11-30"), date("2027-01-11"));
        assert_eq!(due(&monthly, "2026-12-31"), date("2027-02-10"));
        monthly.region = Some("DE-SN".parse().unwrap());
        assert_eq!(due(&monthly, "2026-10-31"), date("2026-12-10"));
    }

    #[test]
    fn both_datev_charts_translate_the_harmonised_chart() {
        let charts = Germany.charts(&Chart::v0_1());
        let names: Vec<&str> = charts.iter().map(|t| t.chart.as_str()).collect();
        assert_eq!(names, ["de-skr03", "de-skr04"]);
    }
}
