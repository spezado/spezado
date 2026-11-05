// SPDX-License-Identifier: AGPL-3.0-or-later
//! German law for spezado – a first cut: the DATEV standard charts, the German tax codes, the VAT return, the
//! recapitulative statement, the income statement on a cash basis and the HGB statements, and the periods of advance
//! VAT returns. The data comes from librotenado; the rules of what applies when are here. All of it is a draft for
//! review by tax advisers.
//!
//! Sources: § 18 Abs. 1 and 2 UStG (advance returns, their periods and deadline), § 18a UStG (the recapitulative
//! statement), § 46 UStDV (the extension of the deadline), § 108 Abs. 3 AO (deadlines on a weekend or a public
//! holiday), § 4 Abs. 3 EStG (cash basis), §§ 266, 275 HGB (statements). What is not covered yet is marked OPEN
//! where it would apply.

use std::collections::BTreeMap;

use librotenado::{
    AccountId, Chart, ChartTranslation, Date, FilingKind, ReportMapping, ReturnPeriod, TaxSubject, TaxTable,
};
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
                shortened: None,
            });
            // The annual return over the calendar year (§ 18 Abs. 3 UStG), also filed when advance returns were.
            if from == Date::new(2026, 1, 1).expect("a date") && to == Date::new(2026, 12, 31).expect("a date") {
                reports.push(Report {
                    id: "de-ust",
                    name: "Umsatzsteuererklärung",
                    basis: ReportBasis::Period,
                    mapping: ReportMapping::de_ust_2026(),
                    filing: Some(FilingKind::VatReturn),
                    shortened: None,
                });
            }
            // Supplies to businesses in other member states, per customer (§ 18a UStG); none for small businesses
            // (Abs. 4). Which period applies – month, quarter or year (Abs. 1, 2 and 9) – is the filer's.
            reports.push(Report {
                id: "de-zm",
                name: "Zusammenfassende Meldung",
                basis: ReportBasis::Customers,
                mapping: ReportMapping::de_zm_2026(),
                filing: Some(FilingKind::RecapitulativeStatement),
                shortened: None,
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
                    shortened: None,
                });
                // The register of fixed assets, part of the Anlage EÜR. OPEN: filed together with it.
                if year == Some(2026) {
                    reports.push(Report {
                        id: "de-aveuer",
                        name: "Anlage AVEÜR",
                        basis: ReportBasis::Assets,
                        mapping: ReportMapping::de_aveuer_2026(),
                        filing: None,
                        shortened: None,
                    });
                }
            }
            Some("de-double-entry") if year == Some(2026) => {
                reports.push(Report {
                    id: "de-hgb-266",
                    name: "Bilanz (§ 266 HGB)",
                    basis: ReportBasis::Balances,
                    mapping: ReportMapping::de_hgb_266_2026(),
                    // Filed as the E-Bilanz, with mappings of their own (OPEN).
                    filing: None,
                    shortened: None,
                });
                // Corporations may file shortened balance sheets by their size (§ 266 Abs. 1 Satz 3 and 4, § 267,
                // § 267a HGB); which size applies is theirs to determine.
                if facts.legal_form.is_some_and(|form| CORPORATIONS.contains(&form)) {
                    for (id, name, size) in [
                        (
                            "de-hgb-266-small",
                            "Bilanz, verkürzt (kleine Kapitalgesellschaft)",
                            librotenado::HgbSize::Small,
                        ),
                        (
                            "de-hgb-266-micro",
                            "Bilanz, verkürzt (Kleinstkapitalgesellschaft)",
                            librotenado::HgbSize::Micro,
                        ),
                    ] {
                        reports.push(Report {
                            id,
                            name,
                            basis: ReportBasis::Balances,
                            mapping: ReportMapping::de_hgb_266_2026(),
                            filing: None,
                            shortened: Some(size),
                        });
                    }
                }
                reports.push(Report {
                    id: "de-hgb-275-gkv",
                    name: "Gewinn- und Verlustrechnung (§ 275 Abs. 2 HGB)",
                    basis: ReportBasis::Period,
                    mapping: ReportMapping::de_hgb_275_gkv_2026(),
                    filing: None,
                    shortened: None,
                });
            }
            _ => {}
        }
        // The taxes on profit of a business over a financial year (librotenado 0012): corporation tax for bodies
        // under the KStG, trade tax where the configuration names the multiplier of the municipality.
        if configuration.accounting.is_some() {
            let subject = match facts.legal_form {
                Some("de-gmbh" | "de-ug" | "de-ag" | "de-kgaa" | "de-se" | "de-eg") => TaxSubject::Corporation,
                Some("de-ev" | "de-stiftung") => TaxSubject::Association,
                Some("de-gbr" | "de-ohg" | "de-kg" | "de-gmbh-co-kg" | "de-partg") => TaxSubject::Partnership,
                _ => TaxSubject::Individual,
            };
            if matches!(subject, TaxSubject::Corporation | TaxSubject::Association) {
                reports.push(Report {
                    id: "de-kst",
                    name: "Körperschaftsteuer",
                    basis: ReportBasis::ProfitTax { subject, trade: false },
                    mapping: computed("de-kst"),
                    filing: None,
                    shortened: None,
                });
            }
            if configuration.trade_tax_multiplier.is_some() {
                reports.push(Report {
                    id: "de-gewst",
                    name: "Gewerbesteuer",
                    basis: ReportBasis::ProfitTax { subject, trade: true },
                    mapping: computed("de-gewst"),
                    filing: None,
                    shortened: None,
                });
            }
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

    fn customer_periods(
        &self,
        facts: &Facts,
        year: u16,
        fields: &dyn Fn(Date, Date) -> BTreeMap<String, i128>,
    ) -> Vec<(Date, Date, Date)> {
        let (Some(first), Some(last)) = (Date::new(year, 1, 1), Date::new(year, 12, 31)) else { return Vec::new() };
        if !self.reports(facts, first, last).iter().any(|report| report.basis == ReportBasis::Customers) {
            return Vec::new();
        }
        let state = facts.configuration.region.as_ref().map(|region| region.as_str());
        // Supplies of goods, triangular ones included, count towards the threshold; services do not (§ 18a Abs. 1
        // Satz 2 UStG).
        let goods = |(from, to): (Date, Date)| -> i128 {
            fields(from, to)
                .iter()
                .filter(|(field, _)| field.ends_with("/L") || field.ends_with("/D"))
                .map(|f| f.1)
                .sum()
        };
        let mut periods = Vec::new();
        for quarter in 0..4i32 {
            // Quarterly while goods stay at 50,000 euros at most in this quarter and each of the four before it;
            // monthly otherwise (§ 18a Abs. 1). Services alone are quarterly (Abs. 2). OPEN: a threshold crossed
            // within the quarter, which the law reports from the month of crossing on, is shown monthly for the
            // whole quarter; and the threshold in a functional currency other than the euro.
            let monthly = (0..5).any(|back| quarter_of(year, quarter - back).is_some_and(|q| goods(q) > 5_000_000));
            let Some((from, to)) = quarter_of(year, quarter) else { continue };
            if monthly {
                for month in from.month()..=to.month() {
                    let (Some(start), Some(end)) = (Date::new(year, month, 1), last_day(year, month)) else { continue };
                    periods.push((start, end, statement_deadline(end, state)));
                }
            } else {
                periods.push((from, to, statement_deadline(to, state)));
            }
        }
        periods
    }

    fn asset_accounts(
        &self,
        national: &ChartTranslation,
        asset: &librotenado::Asset,
    ) -> Option<(Option<AccountId>, AccountId)> {
        use librotenado::Depreciation;
        // The asset's account as SKR 03 numbers it – both charts translate the same harmonised accounts.
        let number: u16 = ChartTranslation::de_skr03().national(&asset.account)?.parse().ok()?;
        let (depreciation, disposal) = match (asset.depreciation, number) {
            (Depreciation::Immediate, _) => (Some("4855"), "2310"),
            (Depreciation::Pool, _) => (Some("4862"), "2310"),
            (Depreciation::None, 500..=599) => (None, "2312"),
            (Depreciation::None, _) => (None, "2310"),
            (_, 10..=48) => (Some("4822"), "2311"),
            (_, 149) => (Some("4833"), "2310"),
            (_, 80..=199) => (Some("4831"), "2310"),
            (_, 320 | 350) => (Some("4832"), "2310"),
            (_, 500..=599) => (None, "2312"),
            _ => (Some("4830"), "2310"),
        };
        let in_chart = |skr03: &str| -> Option<AccountId> {
            let number =
                if national.chart == "de-skr04" { SKR04_OF.iter().find(|(a, _)| *a == skr03)?.1 } else { skr03 };
            national.accounts.get(number).cloned()
        };
        Some((depreciation.and_then(in_chart), in_chart(disposal)?))
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
            // 70 per cent of business entertainment is deductible (§ 4 Abs. 5 Satz 1 Nr. 2 EStG); the VAT on it in
            // full (§ 15 Abs. 1a Satz 2 UStG).
            partly_deductible: [(account("entertainment")?, (account("entertainment-not-deductible")?, 70))].into(),
            transit: account("transit")?,
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
    ("entertainment", "4650", "6640"),
    ("entertainment-not-deductible", "4654", "6644"),
    ("transit", "1590", "1370"),
];

/// A calendar quarter, counted from the first of `year` (0) – earlier ones negative.
fn quarter_of(year: u16, quarter: i32) -> Option<(Date, Date)> {
    let index = i32::from(year) * 4 + quarter;
    let (year, first) = (u16::try_from(index.div_euclid(4)).ok()?, index.rem_euclid(4) as u8 * 3 + 1);
    Some((Date::new(year, first, 1)?, last_day(year, first + 2)?))
}

/// The day a statement per customer of a period ending on `end` is due: the 25th day after it (§ 18a Abs. 1 and 2
/// UStG) – not extended by § 46 UStDV, which names advance returns only –, on a weekend or a public holiday the next
/// working day (§ 108 Abs. 3 AO).
fn statement_deadline(end: Date, state: Option<&str>) -> Date {
    let index = u32::from(end.month());
    let (year, month) = (end.year() + (index / 12) as u16, (index % 12) as u8 + 1);
    let mut day = Date::new(year, month, 25).expect("the 25th exists");
    while weekday(day) >= 5 || holidays::is_holiday(day, state) {
        day = next_day(day);
    }
    day
}

/// The accounts of depreciation and disposal in SKR 03 and SKR 04, as DATEV numbers them.
const SKR04_OF: &[(&str, &str)] = &[
    ("4822", "6200"),
    ("4830", "6220"),
    ("4831", "6221"),
    ("4832", "6222"),
    ("4833", "6223"),
    ("4855", "6260"),
    ("4862", "6264"),
    ("2310", "6895"),
    ("2311", "6896"),
    ("2312", "6897"),
];

/// The legal forms of corporations and of partnerships without a natural person fully liable, which the shortened
/// balance sheets of § 266 Abs. 1 HGB apply to (§ 264a HGB).
const CORPORATIONS: [&str; 5] = ["de-gmbh", "de-ug", "de-ag", "de-kgaa", "de-gmbh-co-kg"];

/// The mapping of a report computed rather than mapped – the taxes on profit –: only its name, which names its fields.
fn computed(report: &str) -> ReportMapping {
    serde_json::from_value(serde_json::json!({
        "report": report, "country": "DE", "version": "2026", "valid_from": "2026-01-01",
        "into": "librotenado/0.1", "status": "draft",
    }))
    .expect("a mapping without accounts")
}

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
        assert_eq!(ids("2026-01-01", "2026-03-31"), ["de-ustva", "de-zm", "de-euer", "de-aveuer"]);
        assert_eq!(ids("2026-01-01", "2026-12-31"), ["de-ustva", "de-ust", "de-zm", "de-euer", "de-aveuer"]);
        assert_eq!(ids("2025-01-01", "2025-12-31"), ["de-euer"]);
        assert!(ids("2025-12-01", "2026-01-31").is_empty());
        let quarters = Germany.vat_periods(&facts, 2026);
        assert_eq!(quarters.len(), 4);
        assert_eq!(quarters[3], (date("2026-10-01"), date("2026-12-31")));
    }

    #[test]
    fn corporations_may_shorten_their_balance_sheet() {
        let books = configuration("de-double-entry", Some(("de-regular", "month")));
        let ids = |legal_form: Option<&str>| -> Vec<&str> {
            let facts = Facts::new(&books, Some("DE".parse().unwrap()), legal_form);
            Germany.reports(&facts, date("2026-01-01"), date("2026-12-31")).iter().map(|r| r.id).collect()
        };
        let gmbh = ids(Some("de-gmbh"));
        assert!(gmbh.contains(&"de-hgb-266-small") && gmbh.contains(&"de-hgb-266-micro"), "{gmbh:?}");
        assert!(!ids(Some("de-ek")).contains(&"de-hgb-266-small"));
    }

    #[test]
    fn small_businesses_file_no_advance_returns_and_months_end_on_their_last_day() {
        let small = configuration("de-cash-basis", Some(("de-small-business", "year")));
        let facts = Facts::new(&small, None, None);
        assert!(Germany.vat_periods(&facts, 2026).is_empty());
        // No VAT return, but the Anlage EÜR all the same.
        let reports = Germany.reports(&facts, date("2026-01-01"), date("2026-12-31"));
        assert_eq!(reports.iter().map(|report| report.id).collect::<Vec<_>>(), ["de-euer", "de-aveuer"]);
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
    fn recapitulative_statements_are_quarterly_unless_goods_exceed_50000_euros() {
        let regular = configuration("de-double-entry", Some(("de-regular", "month")));
        let facts = Facts::new(&regular, None, None);
        // Goods of 60,000 euros in the second quarter of 2026, services of 80,000 in the first.
        let fields = |from: Date, to: Date| -> BTreeMap<String, i128> {
            let mut fields = BTreeMap::new();
            if from <= date("2026-05-15") && date("2026-05-15") <= to {
                fields.insert("ATU12345678/L".into(), 6_000_000);
            }
            if from <= date("2026-02-15") && date("2026-02-15") <= to {
                fields.insert("FR40303265045/S".into(), 8_000_000);
            }
            fields
        };
        let periods = Germany.customer_periods(&facts, 2026, &fields);
        let ends: Vec<String> = periods.iter().map(|p| p.1.to_string()).collect();
        // Services alone stay quarterly; from the quarter with goods over the threshold on, monthly for the four
        // quarters after it, too – here the rest of the year.
        assert_eq!(
            ends,
            [
                "2026-03-31",
                "2026-04-30",
                "2026-05-31",
                "2026-06-30",
                "2026-07-31",
                "2026-08-31",
                "2026-09-30",
                "2026-10-31",
                "2026-11-30",
                "2026-12-31"
            ]
        );
        // The 25th day after the period; 25 April 2026 is a Saturday, 25 January 2027 a Monday.
        assert_eq!(periods[0].2, date("2026-04-27"));
        assert_eq!(periods[9].2, date("2027-01-25"));
        // Small businesses file none (§ 18a Abs. 4 UStG).
        let small = configuration("de-cash-basis", Some(("de-small-business", "year")));
        assert!(Germany.customer_periods(&Facts::new(&small, None, None), 2026, &fields).is_empty());
    }

    #[test]
    fn both_datev_charts_translate_the_harmonised_chart() {
        let charts = Germany.charts(&Chart::v0_1());
        let names: Vec<&str> = charts.iter().map(|t| t.chart.as_str()).collect();
        assert_eq!(names, ["de-skr03", "de-skr04"]);
    }
}
