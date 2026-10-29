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
}

impl<'a> Facts<'a> {
    /// The facts of an owner with this configuration; `country` and `legal_form` as known of an organisation.
    pub fn new(configuration: &'a Configuration, country: Option<Country>, legal_form: Option<&'a str>) -> Self {
        let country = country.or_else(|| {
            let code = configuration.accounting.as_ref().or(configuration.vat.as_ref().map(|vat| &vat.scheme))?;
            code.as_str()[..2].to_ascii_uppercase().parse().ok()
        });
        Facts { country, legal_form, configuration }
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

    /// The standard accounts of a national chart the module offers.
    fn standard_accounts(&self, national: &ChartTranslation) -> Option<StandardAccounts>;
}

/// The first module of `modules` that applies to the facts.
pub fn module_for<'m>(modules: &'m [Box<dyn LawModule>], facts: &Facts) -> Option<&'m dyn LawModule> {
    modules.iter().map(AsRef::as_ref).find(|module| module.applies(facts))
}
