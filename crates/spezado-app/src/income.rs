// SPDX-License-Identifier: AGPL-3.0-or-later
//! What a person states for income tax beside the books (librotenado 0012): other income, deductions, losses carried
//! forward, joint assessment, church tax – kept per owner and year in the books' settings.

use librotenado::{BooksId, IncomeTaxFacts, PartyId};
use serde::{Deserialize, Serialize};

use crate::{AppError, Workspace};

/// The facts of income tax as stored: amounts in base units of the euro.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomeFacts {
    /// Assessed jointly with a spouse.
    #[serde(default)]
    pub joint: bool,
    /// Other positive income.
    #[serde(default)]
    pub other_income: i128,
    /// Special expenses, extraordinary burdens, allowances.
    #[serde(default)]
    pub deductions: i128,
    /// Losses carried forward, not yet used.
    #[serde(default)]
    pub losses: i128,
    /// The rate of church tax in percent, 8 or 9; none without.
    #[serde(default)]
    pub church_rate: Option<u8>,
}

impl From<&IncomeFacts> for IncomeTaxFacts {
    fn from(facts: &IncomeFacts) -> Self {
        IncomeTaxFacts {
            joint: facts.joint,
            other_income: facts.other_income,
            deductions: facts.deductions,
            losses: facts.losses,
            church_rate: facts.church_rate,
        }
    }
}

fn key(owner: &PartyId, year: u16) -> String {
    format!("income-tax/{owner}/{year}")
}

impl Workspace {
    /// What the owner stated for income tax of a year; nothing stated, all empty.
    pub fn income_facts(&self, books: &BooksId, owner: &PartyId, year: u16) -> Result<IncomeFacts, AppError> {
        match self.store.setting(Some(books), &key(owner, year))? {
            Some(json) => serde_json::from_str(&json).map_err(|e| AppError::UnknownReport(format!("income tax: {e}"))),
            None => Ok(IncomeFacts::default()),
        }
    }

    /// Stores what the owner states for income tax of a year – on this device; it is not part of the books' events.
    pub fn set_income_facts(
        &self,
        books: &BooksId,
        owner: &PartyId,
        year: u16,
        facts: &IncomeFacts,
    ) -> Result<(), AppError> {
        if facts.church_rate.is_some_and(|rate| !(1..=10).contains(&rate)) {
            return Err(AppError::UnknownReport("a church tax rate is a percentage, 8 or 9".into()));
        }
        let json = serde_json::to_string(facts).expect("facts serialize");
        Ok(self.store.set_setting(Some(books), &key(owner, year), &json)?)
    }
}

/// What the books do not hold of the taxes on profit of a business (librotenado 0012), per owner and financial year:
/// amounts in base units of the euro.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfitFacts {
    /// Losses carried forward for corporation tax or income tax, not yet used (§ 10d EStG).
    #[serde(default)]
    pub income_losses: i128,
    /// Trade losses carried forward, not yet used (§ 10a GewStG).
    #[serde(default)]
    pub trade_losses: i128,
    /// The reduction for land of the business (§ 9 Nr. 1 Satz 1 GewStG): 1.2 % of the Einheitswert, as stated.
    #[serde(default)]
    pub land_reduction: i128,
    /// The turnover and wages of the year, where donations are capped by them (§ 9 Abs. 1 Nr. 2 KStG); none if not
    /// stated.
    #[serde(default)]
    pub turnover_and_wages: Option<i128>,
    /// The share held in the corporations whose dividends the books show: `at_least_15` (default), `10_to_15`,
    /// `below_10` (§ 8b Abs. 4 KStG, § 9 Nr. 2a GewStG).
    #[serde(default)]
    pub holding: Option<String>,
}

impl ProfitFacts {
    /// The holding as librotenado knows it.
    pub fn holding(&self) -> Result<librotenado::Holding, AppError> {
        match self.holding.as_deref() {
            None | Some("at_least_15") => Ok(librotenado::Holding::AtLeast15),
            Some("10_to_15") => Ok(librotenado::Holding::From10To15),
            Some("below_10") => Ok(librotenado::Holding::Below10),
            Some(other) => Err(AppError::UnknownReport(format!("not a holding: {other:?}"))),
        }
    }
}

fn profit_key(owner: &PartyId, year: u16) -> String {
    format!("profit-tax/{owner}/{year}")
}

impl Workspace {
    /// What the owner stated for the taxes on profit of the financial year ending in `year`; nothing stated, empty.
    pub fn profit_facts(&self, books: &BooksId, owner: &PartyId, year: u16) -> Result<ProfitFacts, AppError> {
        match self.store.setting(Some(books), &profit_key(owner, year))? {
            Some(json) => serde_json::from_str(&json).map_err(|e| AppError::UnknownReport(format!("profit tax: {e}"))),
            None => Ok(ProfitFacts::default()),
        }
    }

    /// Stores what the owner states for the taxes on profit of a year – on this device, beside the books.
    pub fn set_profit_facts(
        &self,
        books: &BooksId,
        owner: &PartyId,
        year: u16,
        facts: &ProfitFacts,
    ) -> Result<(), AppError> {
        facts.holding()?;
        let negative = facts.income_losses < 0
            || facts.trade_losses < 0
            || facts.land_reduction < 0
            || facts.turnover_and_wages.is_some_and(|t| t < 0);
        if negative {
            return Err(AppError::UnknownReport("losses, reductions, turnover and wages are not negative".into()));
        }
        let json = serde_json::to_string(facts).expect("facts serialize");
        Ok(self.store.set_setting(Some(books), &profit_key(owner, year), &json)?)
    }
}
