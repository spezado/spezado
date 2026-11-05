// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixed assets (librotenado spec part 0011): registered, depreciated once a year by a booking, and booked out at
//! their book value when they leave.

use librotenado::{
    AccountId, Asset, AssetId, AssetState, Booking, Date, Depreciation, Disposal, Money, PartyId, Posting,
};
use spezado_core::Command;

use crate::{AppError, BooksId, Workspace, new_id};

/// A fixed asset to register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAsset {
    /// Whose asset.
    pub owner: PartyId,
    /// What it is.
    pub name: String,
    /// The account of fixed assets it is kept on.
    pub account: AccountId,
    /// The day it was acquired.
    pub acquired: Date,
    /// Its cost, without deductible VAT.
    pub cost: Money,
    /// How it is depreciated.
    pub depreciation: Depreciation,
}

/// A fixed asset as a list shows it: what the register knows, and its book value at the end of a day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetItem {
    /// The asset and its disposal, if it left.
    pub state: AssetState,
    /// The book value, in base units.
    pub book_value: i128,
}

/// The text of the booking of a year's depreciation.
fn depreciation_text(year: u16) -> String {
    format!("Abschreibungen {year}")
}

impl Workspace {
    /// The fixed assets of an owner, in the order registered, with their book values at the end of `day`.
    pub fn assets(&self, books: &BooksId, owner: &PartyId, day: Date) -> Result<Vec<AssetItem>, AppError> {
        Ok(self
            .get(books)?
            .events()
            .assets(owner)
            .into_iter()
            .map(|state| {
                let book_value = state.book_value(day);
                AssetItem { state, book_value }
            })
            .collect())
    }

    /// Registers a fixed asset; returns its identity.
    pub fn register_asset(&mut self, books: &BooksId, new: NewAsset) -> Result<AssetId, AppError> {
        if new.name.trim().is_empty() {
            return Err(AppError::Asset("a fixed asset needs a name".into()));
        }
        if new.cost.base_units() <= 0 {
            return Err(AppError::Asset("a fixed asset costs more than nothing".into()));
        }
        if matches!(new.depreciation, Depreciation::StraightLine { months: 0 }) {
            return Err(AppError::Asset("a straight line needs a useful life".into()));
        }
        let asset = Asset {
            id: new_id(),
            owner: new.owner,
            name: new.name.trim().to_owned(),
            account: new.account,
            acquired: new.acquired,
            cost: new.cost,
            depreciation: new.depreciation,
        };
        let id = asset.id.clone();
        self.execute(books, Command::AcquireAsset(asset))?;
        Ok(id)
    }

    /// The booking of an owner's depreciation of a calendar year: per asset that depreciates in it, its expense
    /// against its account, dated the last day of the year; none when nothing depreciates.
    pub fn depreciation_booking(
        &self,
        books: &BooksId,
        owner: &PartyId,
        year: u16,
    ) -> Result<Option<Booking>, AppError> {
        let (from, to) = (Date::new(year, 1, 1).expect("a date"), Date::new(year, 12, 31).expect("a date"));
        let national = self.national_chart(books, owner, to)?.ok_or(AppError::NoStandardAccounts)?;
        let (_, _, _, module) = self.law_for(books, owner, to)?;
        let mut postings = Vec::new();
        for state in self.get(books)?.events().assets(owner) {
            let units = state.depreciation(from, to);
            if units == 0 {
                continue;
            }
            let Some((Some(expense), _)) = module.asset_accounts(&national, &state.asset) else { continue };
            let money = |units| Money::new(units, state.asset.cost.currency().clone()).expect("within the cost");
            let posting = |account: AccountId, units: i128| Posting {
                owner: owner.clone(),
                account,
                amount: money(units),
                note: Some(state.asset.name.clone()),
                tax: None,
                exchange: None,
            };
            postings.push(posting(expense, units));
            postings.push(posting(state.asset.account.clone(), -units));
        }
        if postings.is_empty() {
            return Ok(None);
        }
        Ok(Some(Booking {
            basis: None,
            attributed: None,
            date: to,
            text: depreciation_text(year),
            postings,
            documents: vec![],
            entries: vec![],
            transaction: None,
        }))
    }

    /// Books an owner's depreciation of a calendar year – once: a year booked and not reversed is refused. Returns
    /// whether there was anything to book.
    pub fn book_depreciation(&mut self, books: &BooksId, owner: &PartyId, year: u16) -> Result<bool, AppError> {
        let text = depreciation_text(year);
        let booked = self
            .get(books)?
            .journal(Some(owner), None, None)
            .into_iter()
            .any(|entry| entry.booking.text == text && entry.reverses.is_none() && entry.reversed_by.is_none());
        if booked {
            return Err(AppError::Asset(format!("the depreciation of {year} is booked already")));
        }
        match self.depreciation_booking(books, owner, year)? {
            Some(booking) => {
                self.execute(books, Command::Book(booking))?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Records that a fixed asset left the business, and books its book value out: the account of disposals against
    /// the asset's account, on the day it left. What it was sold for is booked as a sale of its own.
    pub fn dispose_asset(
        &mut self,
        books: &BooksId,
        asset: &AssetId,
        date: Date,
        proceeds: Option<Money>,
    ) -> Result<(), AppError> {
        let events = self.get(books)?.events();
        let state = events
            .owners()
            .into_iter()
            .flat_map(|owner| events.assets(owner))
            .find(|state| state.asset.id == *asset)
            .ok_or_else(|| AppError::Asset(format!("no fixed asset {asset}")))?;
        let owner = state.asset.owner.clone();
        // The register depreciates up to the month it leaves; a year booked already would count the rest twice.
        let text = depreciation_text(date.year());
        let booked = self
            .get(books)?
            .journal(Some(&owner), None, None)
            .into_iter()
            .any(|entry| entry.booking.text == text && entry.reverses.is_none() && entry.reversed_by.is_none());
        if booked {
            return Err(AppError::Asset(format!(
                "the depreciation of {} is booked already: reverse it, record the disposal, and book it again",
                date.year()
            )));
        }
        let disposal = Disposal { asset: asset.clone(), date, proceeds };
        let mut left = state.clone();
        left.disposal = Some(disposal.clone());
        let value = left.value_at_disposal().unwrap_or(0);
        let mut commands = vec![(books.clone(), Command::DisposeAsset(disposal))];
        if value != 0 && state.asset.depreciation != Depreciation::Pool {
            let national = self.national_chart(books, &owner, date)?.ok_or(AppError::NoStandardAccounts)?;
            let (_, _, _, module) = self.law_for(books, &owner, date)?;
            let (_, out) = module.asset_accounts(&national, &state.asset).ok_or(AppError::NoStandardAccounts)?;
            let money = Money::new(value, state.asset.cost.currency().clone()).expect("within the cost");
            let posting = |account: AccountId, amount: Money| Posting {
                owner: owner.clone(),
                account,
                amount,
                note: Some(state.asset.name.clone()),
                tax: None,
                exchange: None,
            };
            let booking = Booking {
                basis: None,
                attributed: None,
                date,
                text: format!("Anlagenabgang {}", state.asset.name),
                postings: vec![posting(out, money.clone()), posting(state.asset.account.clone(), -money)],
                documents: vec![],
                entries: vec![],
                transaction: None,
            };
            commands.push((books.clone(), Command::Book(booking)));
        }
        self.execute_all(commands)?;
        Ok(())
    }
}
