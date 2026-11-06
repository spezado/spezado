// SPDX-License-Identifier: AGPL-3.0-or-later
//! The interface of spezado's law modules.
//!
//! A law module is a pure function of facts: who the owner is and how it keeps its books (spec part 0008 of
//! librotenado) go in; what follows from that comes out – the national charts it may use, the tax codes, the
//! reports and the periods they cover. A module does no I/O and keeps no state. A closed period records the name and
//! version of the modules its figures were derived with.

use std::collections::BTreeMap;

use librotenado::{
    AccountId, Chart, ChartTranslation, Configuration, Country, Date, FilingKind, ReportMapping, TaxCode, TaxTable,
};

/// What a law module needs to know about an owner, on a date.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    /// The country whose law applies: an organisation's country, else the country of the configuration's codes.
    pub country: Option<Country>,
    /// An organisation's legal form (`de-gmbh`); none for a person.
    pub legal_form: Option<&'a str>,
    /// How the owner keeps its books.
    pub configuration: &'a Configuration,
    /// Whether the owner employs people (librotenado spec part 0013); set by whoever knows the books.
    pub employer: bool,
}

impl<'a> Facts<'a> {
    /// The facts of an owner with this configuration; `country` and `legal_form` as known of an organisation.
    pub fn new(configuration: &'a Configuration, country: Option<Country>, legal_form: Option<&'a str>) -> Self {
        let country = country.or_else(|| {
            let code = configuration.accounting.as_ref().or(configuration.vat.as_ref().map(|vat| &vat.scheme))?;
            code.as_str()[..2].to_ascii_uppercase().parse().ok()
        });
        Facts { country, legal_form, configuration, employer: false }
    }
}

/// How a report is computed (librotenado spec part 0009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportBasis {
    /// From the postings dated within the period – a VAT return, an income statement on a cash basis, a profit and
    /// loss account.
    Period,
    /// From the balances at the end of the period – a balance sheet.
    Balances,
    /// From the postings dated within the period, per customer – a recapitulative statement.
    Customers,
    /// From the register of fixed assets – the Anlage AVEÜR (librotenado spec part 0011).
    Assets,
    /// The taxes on profit of a financial year (librotenado spec part 0012): corporation tax, or with `trade` the
    /// trade tax, of an owner of this kind.
    ProfitTax {
        /// Who pays.
        subject: librotenado::TaxSubject,
        /// The trade tax rather than corporation tax.
        trade: bool,
    },
    /// The income tax of a person over a calendar year, with what the person states beside the books (librotenado
    /// spec part 0012).
    IncomeTax,
    /// From the payslips for periods ending within the period – a wage tax return (librotenado spec part 0013).
    Payroll,
    /// The lines of a person's profit in the forms of the income tax return – Anlage G or Anlage S (librotenado spec
    /// part 0012).
    IncomeTaxForm,
}

/// A report that applies to an owner for a period.
#[derive(Debug, Clone)]
pub struct Report {
    /// A short identity, the same across versions of the mapping (`de-ustva`, `de-euer`).
    pub id: &'static str,
    /// Its name in the language of the country (`Umsatzsteuer-Voranmeldung`).
    pub name: &'static str,
    /// How it is computed.
    pub basis: ReportBasis,
    /// The mapping of accounts and tax codes onto its positions.
    pub mapping: ReportMapping,
    /// The kind of filing it is submitted as on its own; none when it is not filed, or only with others.
    pub filing: Option<FilingKind>,
    /// A balance sheet shortened for a small or micro company (§ 266 Abs. 1 HGB); none for the full one.
    pub shortened: Option<librotenado::HgbSize>,
}

/// The accounts of a national chart that workflows book on when nobody chose others: where money is, what is sold
/// and bought, and where VAT goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardAccounts {
    /// The bank account.
    pub bank: AccountId,
    /// The cash account.
    pub cash: AccountId,
    /// Sales at the standard rate.
    pub sales: AccountId,
    /// Other expenses.
    pub expenses: AccountId,
    /// The proprietor's withdrawals.
    pub withdrawals: AccountId,
    /// The proprietor's contributions.
    pub contributions: AccountId,
    /// Receivables from sales, where invoices are booked when issued.
    pub receivables: AccountId,
    /// Per tax code whose VAT is part of a gross amount: the account of that VAT – owed on sales, deductible on
    /// purchases.
    pub vat: BTreeMap<TaxCode, AccountId>,
    /// The VAT a buyer owes under reverse charge, and deducts.
    pub vat_reverse_charge: AccountId,
    /// The deductible VAT under reverse charge.
    pub vat_reverse_charge_input: AccountId,
    /// Expenses deductible in part: account → the account of the part not deductible, and the deductible part in
    /// per cent – business entertainment in Germany.
    pub partly_deductible: BTreeMap<AccountId, (AccountId, u8)>,
    /// Transit items: amounts collected in the name and for the account of others (durchlaufende Posten).
    pub transit: AccountId,
}

/// The law of one country, or of a part of it, as a module.
pub trait LawModule: std::fmt::Debug + Send + Sync {
    /// The module's name, recorded with a closed period (`spezado-law-de`).
    fn name(&self) -> &'static str;

    /// The module's version, recorded with a closed period.
    fn version(&self) -> &'static str;

    /// Whether the module applies to an owner with these facts.
    fn applies(&self, facts: &Facts) -> bool;

    /// The national charts of accounts the owner may work with, as translations of the harmonised `chart`.
    fn charts(&self, chart: &Chart) -> Vec<ChartTranslation>;

    /// The tax codes that apply on `date`.
    fn tax_table(&self, date: Date) -> Option<TaxTable>;

    /// The reports that apply to the owner for the period from `from` to `to`.
    fn reports(&self, facts: &Facts, from: Date, to: Date) -> Vec<Report>;

    /// The periods of advance VAT returns in the calendar year `year`; none when there are none.
    fn vat_periods(&self, facts: &Facts, year: u16) -> Vec<(Date, Date)>;

    /// The day the advance VAT return of a period ending on `end` is due.
    fn vat_deadline(&self, facts: &Facts, end: Date) -> Date;

    /// The periods of the statements per customer in the calendar year `year` – `(first day, last day, due)` – given
    /// the fields of the report per customer from one day to another; none when the owner files none.
    fn customer_periods(
        &self,
        facts: &Facts,
        year: u16,
        fields: &dyn Fn(Date, Date) -> BTreeMap<String, i128>,
    ) -> Vec<(Date, Date, Date)> {
        let _ = (facts, year, fields);
        Vec::new()
    }

    /// For a fixed asset in a national chart: the account of expense its depreciation goes to – none where it is not
    /// depreciated – and the account its book value goes to when it leaves.
    fn asset_accounts(
        &self,
        national: &ChartTranslation,
        asset: &librotenado::Asset,
    ) -> Option<(Option<AccountId>, AccountId)> {
        let _ = (national, asset);
        None
    }

    /// The standard accounts of a national chart the module offers.
    fn standard_accounts(&self, national: &ChartTranslation) -> Option<StandardAccounts>;
}

/// The first module of `modules` that applies to the facts.
pub fn module_for<'m>(modules: &'m [Box<dyn LawModule>], facts: &Facts) -> Option<&'m dyn LawModule> {
    modules.iter().map(AsRef::as_ref).find(|module| module.applies(facts))
}
