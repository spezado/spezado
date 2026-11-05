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
