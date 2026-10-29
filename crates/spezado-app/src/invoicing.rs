// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writing invoices: the seller's details and the customers of a set of books, invoice numbers in sequence, and the
//! invoice itself as an XRechnung 3.0 in UBL (EN 16931) or as ZUGFeRD – a PDF with the XRechnung in CII attached –,
//! recorded as a document whose original is the file written. On a cash
//! basis an invoice is booked when it is paid – the bank proposes it for the entry –, with double entry when issued.
//!
//! Supported: VAT at 19 %, 7 % and 0 %, small businesses (§ 19 UStG), exempt supplies (§ 4 UStG), reverse charge at
//! home (§ 13b UStG) and for services to businesses in other EU countries (§ 3a Abs. 2 UStG), intra-Community
//! supplies and exports – each with the statement § 14 Abs. 4 Nr. 8 and § 14a UStG ask for. Number ranges per device
//! follow once several devices write (synchronisation).
//!
//! Without a VAT identification number the seller's tax number is also its identifier (BT-29): EN 16931 requires one
//! of BT-29, BT-30 or BT-31 (rule BR-CO-26, fatal; <https://docs.peppol.eu/poac/eu/pint-eu/trn-invoice/rule/BR-CO-26/>),
//! and the tax number (BT-32) is none of them.

use std::collections::BTreeMap;

use librotenado::{
    BooksId, Contact, CurrencyTable, Date, Decimal, Document, DocumentKind, InvoiceSide, Invoicing, Money,
    Organisation, Original, PartyId, Person, PostalAddress, ProposalAccounts, Series, TaxCode, TaxId, en16931,
    propose_booking,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use spezado_core::Command;

use crate::{AppError, Workspace, new_id};

/// Where the seller's details are kept, per set of books.
const PROFILE: &str = "invoice-profile";
/// Where the customers are kept, per set of books.
const CUSTOMERS: &str = "customers";

/// The seller's details on invoices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceProfile {
    /// The seller's name.
    pub name: String,
    /// Street and number.
    pub street: String,
    /// The postcode.
    pub postcode: String,
    /// The city.
    pub city: String,
    /// The country, ISO 3166-1 alpha-2.
    pub country: String,
    /// The VAT identification number, if there is one.
    #[serde(default)]
    pub vat_id: Option<String>,
    /// The tax number at the tax office.
    #[serde(default)]
    pub tax_number: Option<String>,
    /// Whom to contact.
    pub contact: String,
    /// The phone number.
    pub phone: String,
    /// The e-mail address – also where invoices are sent from.
    pub email: String,
    /// The account to pay to.
    pub iban: String,
    /// The days until payment is due.
    pub payment_days: u16,
    /// Invoice numbers start with this, followed by the year and a sequence: `RE-2026-001`.
    pub prefix: String,
}

/// A customer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Customer {
    /// Its identity in the list.
    pub id: String,
    /// The name.
    pub name: String,
    /// Street and number.
    pub street: String,
    /// The postcode.
    pub postcode: String,
    /// The city.
    pub city: String,
    /// The country, ISO 3166-1 alpha-2.
    pub country: String,
    /// Where invoices are sent: an e-mail address.
    pub email: String,
    /// The VAT identification number, if there is one.
    #[serde(default)]
    pub vat_id: Option<String>,
    /// The reference the customer asked for – the Leitweg-ID of a public authority; the invoice number when none.
    #[serde(default)]
    pub reference: Option<String>,
}

/// A line of a new invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLine {
    /// What was supplied.
    pub name: String,
    /// How much: hours, days, pieces.
    pub quantity: Decimal,
    /// The unit as a code of UN/ECE Recommendation 20: `HUR` hour, `DAY` day, `C62` piece.
    pub unit: String,
    /// The net price of one unit.
    pub price: Money,
    /// The tax code: `de-out-19`, `de-out-7`, `de-out-0`, `de-out-exempt`, `de-out-13b`, `de-out-eu-service`,
    /// `de-out-ig-supply`, `de-out-export`; a small business's lines carry none.
    pub tax: Option<TaxCode>,
}

/// A new invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInvoice {
    /// The customer, by its identity.
    pub customer: String,
    /// The invoice date.
    pub date: Date,
    /// When the supply was made; the invoice date when none.
    pub delivery: Option<Date>,
    /// The lines.
    pub lines: Vec<NewLine>,
    /// A note on the invoice.
    pub note: Option<String>,
    /// Why exempt lines are exempt – `Steuerfrei nach § 4 Nr. 14 UStG`; a general statement when none.
    pub exemption: Option<String>,
    /// The file the invoice is written as.
    pub format: InvoiceFormat,
}

/// The file an invoice is written as – the original that is sent and kept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InvoiceFormat {
    /// XRechnung 3.0 in UBL: XML only.
    #[default]
    XRechnung,
    /// ZUGFeRD, profile XRECHNUNG: a PDF/A-3 for people with the XRechnung in CII attached.
    Zugferd,
}

/// The tax codes an invoice can be written with, with the category of EN 16931 each falls into.
pub const INVOICE_TAX_CODES: [(&str, &str); 8] = [
    ("de-out-19", "S"),
    ("de-out-7", "S"),
    ("de-out-0", "Z"),
    ("de-out-exempt", "E"),
    ("de-out-13b", "AE"),
    ("de-out-eu-service", "AE"),
    ("de-out-ig-supply", "K"),
    ("de-out-export", "G"),
];

/// What an invoice states for amounts without VAT, after § 14 Abs. 4 Nr. 8 and § 14a UStG, with the code of the
/// VATEX list where one fits (BT-120, BT-121). The exemptions of § 4 UStG fall under several articles of the VAT
/// Directive, so they are stated in words only.
fn exemption(code: &str, exempt: Option<&str>) -> Option<(String, Option<&'static str>)> {
    Some(match code {
        "de-out-exempt" => (exempt.unwrap_or("Steuerfreie Leistung nach § 4 UStG.").to_owned(), None),
        "de-out-13b" | "de-out-eu-service" => {
            ("Steuerschuldnerschaft des Leistungsempfängers.".into(), Some("VATEX-EU-AE"))
        }
        "de-out-ig-supply" => ("Steuerfreie innergemeinschaftliche Lieferung.".into(), Some("VATEX-EU-IC")),
        "de-out-export" => ("Steuerfreie Ausfuhrlieferung.".into(), Some("VATEX-EU-G")),
        _ => return None,
    })
}

/// An invoice written: the document, its original and XML, and the booking it is recorded with – none on a cash
/// basis.
#[derive(Debug, Clone)]
pub struct WrittenInvoice {
    /// The document, with the invoice's data.
    pub document: Document,
    /// The XRechnung – in UBL, or in CII as a ZUGFeRD PDF carries it.
    pub xml: String,
    /// The original: the XML, or the ZUGFeRD PDF.
    pub original: Vec<u8>,
    /// The booking of the receivable, with double entry.
    pub booking: Option<librotenado::Booking>,
}

/// An amount times a decimal, rounded half away from zero to base units.
fn times(units: i128, factor: Decimal, divisor: i128) -> Option<i128> {
    let numerator = units.checked_mul(factor.units())?;
    let denominator = 10i128.checked_pow(u32::from(factor.scale()))?.checked_mul(divisor)?;
    let (quotient, remainder) = (numerator / denominator, numerator % denominator);
    Some(if remainder.abs() * 2 >= denominator { quotient + numerator.signum() } else { quotient })
}

fn money(units: i128, currency: &librotenado::Currency) -> Result<Money, AppError> {
    Money::new(units, currency.clone()).ok_or_else(|| AppError::Invoice("the amounts are too large".into()))
}

impl Workspace {
    /// The owner whose invoices are written: the first owner of the books.
    fn invoicing_owner(&self, books: &BooksId) -> Result<PartyId, AppError> {
        let owners = self.get(books)?.events().owners();
        owners.first().map(|o| (*o).clone()).ok_or_else(|| AppError::Invoice("the books have no owner".into()))
    }

    /// The name, tax numbers and contact of a party of the books.
    fn party_of(&self, books: &BooksId, id: &PartyId) -> Result<Option<PartyDetails>, AppError> {
        let (organisations, persons) = self.get(books)?.events().parties();
        Ok(match (organisations.get(id), persons.get(id)) {
            (Some(o), _) => Some((o.name.clone(), o.tax_ids.clone(), o.contact.clone())),
            (None, Some(p)) => Some((p.name.clone(), p.tax_ids.clone(), p.contact.clone())),
            (None, None) => None,
        })
    }

    /// The seller's details on the invoices of a set of books – the owner's contact and invoicing settings (spec
    /// 0008); none before they are given. Details kept on this device by spezado 0.1 are read where the books have
    /// none yet.
    pub fn invoice_profile(&self, books: &BooksId) -> Result<Option<InvoiceProfile>, AppError> {
        let owner = self.invoicing_owner(books)?;
        let events = self.get(books)?.events();
        let Some(invoicing) = events.invoicing(&owner) else {
            let Some(json) = self.store.setting(Some(books), PROFILE)? else { return Ok(None) };
            return serde_json::from_str(&json).map(Some).map_err(|e| AppError::Invoice(e.to_string()));
        };
        let (name, tax_ids, contact) = self.party_of(books, &owner)?.unwrap_or_default();
        let contact = contact.unwrap_or_default();
        let address = contact.address.clone();
        let tax = |scheme: &str| tax_ids.iter().find(|t| t.scheme == scheme).map(|t| t.number.clone());
        Ok(Some(InvoiceProfile {
            name,
            street: address.as_ref().and_then(|a| a.lines.first().cloned()).unwrap_or_default(),
            postcode: address.as_ref().and_then(|a| a.postcode.clone()).unwrap_or_default(),
            city: address.as_ref().and_then(|a| a.city.clone()).unwrap_or_default(),
            country: address.as_ref().map_or_else(|| "DE".to_owned(), |a| a.country.to_string()),
            vat_id: tax("vat"),
            tax_number: tax("de-steuernummer"),
            contact: contact.name.unwrap_or_default(),
            phone: contact.phone.unwrap_or_default(),
            email: contact.email.unwrap_or_default(),
            iban: invoicing.account.clone().unwrap_or_default(),
            payment_days: invoicing.payment_days.unwrap_or(14),
            prefix: invoicing.prefix.clone(),
        }))
    }

    /// Keeps the seller's details: as the owner's description and invoicing settings, which travel with the books.
    pub fn set_invoice_profile(&mut self, books: &BooksId, profile: &InvoiceProfile) -> Result<(), AppError> {
        let owner = self.invoicing_owner(books)?;
        let (organisations, persons) = self.get(books)?.events().parties();
        let (organisation, person) =
            (organisations.get(&owner).map(|o| (*o).clone()), persons.get(&owner).map(|p| (*p).clone()));
        let tax_ids = |mut ids: Vec<TaxId>| {
            ids.retain(|t| t.scheme != "vat" && t.scheme != "de-steuernummer");
            let given = |n: &Option<String>| n.as_ref().map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
            ids.extend(given(&profile.vat_id).map(|number| TaxId { scheme: "vat".into(), number }));
            ids.extend(given(&profile.tax_number).map(|number| TaxId { scheme: "de-steuernummer".into(), number }));
            ids
        };
        let text = |t: &str| Some(t.trim().to_owned()).filter(|t| !t.is_empty());
        let contact = Contact {
            address: Some(postal_address(&profile.street, &profile.postcode, &profile.city, &profile.country)?),
            name: text(&profile.contact),
            phone: text(&profile.phone),
            email: text(&profile.email),
            endpoint: text(&profile.email).map(|address| librotenado::Endpoint { scheme: "EM".into(), address }),
            buyer_reference: None,
        };
        let describe = match (organisation, person) {
            (Some(o), _) => Command::DescribeOrganisation(Organisation {
                name: profile.name.trim().to_owned(),
                tax_ids: tax_ids(o.tax_ids.clone()),
                contact: Some(contact),
                ..o
            }),
            (None, Some(p)) => Command::DescribePerson(Person {
                name: profile.name.trim().to_owned(),
                tax_ids: tax_ids(p.tax_ids.clone()),
                contact: Some(contact),
                ..p
            }),
            (None, None) => return Err(AppError::Invoice("the owner is not described".into())),
        };
        let invoicing = Invoicing {
            owner,
            prefix: profile.prefix.clone(),
            payment_days: Some(profile.payment_days),
            account: text(&profile.iban).map(|i| i.replace(' ', "")),
        };
        self.execute_all(vec![(books.clone(), describe), (books.clone(), Command::ConfigureInvoicing(invoicing))])?;
        Ok(())
    }

    /// The customers of a set of books, by name: the organisations and persons described in them who are not
    /// owners. Customers kept on this device by spezado 0.1 are read where the books have none yet.
    pub fn customers(&self, books: &BooksId) -> Result<Vec<Customer>, AppError> {
        let events = self.get(books)?.events();
        let owners = events.owners();
        let (organisations, persons) = events.parties();
        let parties = organisations
            .iter()
            .filter(|(id, _)| !owners.contains(id))
            .map(|(id, o)| (*id, o.name.clone(), &o.tax_ids, o.contact.as_ref()))
            .chain(
                persons
                    .iter()
                    .filter(|(id, _)| !owners.contains(id))
                    .map(|(id, p)| (*id, p.name.clone(), &p.tax_ids, p.contact.as_ref())),
            );
        let mut customers: Vec<Customer> = parties
            .map(|(id, name, tax_ids, contact)| {
                let address = contact.and_then(|c| c.address.as_ref());
                Customer {
                    id: id.to_string(),
                    name,
                    street: address.and_then(|a| a.lines.first().cloned()).unwrap_or_default(),
                    postcode: address.and_then(|a| a.postcode.clone()).unwrap_or_default(),
                    city: address.and_then(|a| a.city.clone()).unwrap_or_default(),
                    country: address.map_or_else(|| "DE".to_owned(), |a| a.country.to_string()),
                    email: contact
                        .and_then(|c| c.endpoint.as_ref().map(|e| e.address.clone()).or_else(|| c.email.clone()))
                        .unwrap_or_default(),
                    vat_id: tax_ids.iter().find(|t| t.scheme == "vat").map(|t| t.number.clone()),
                    reference: contact.and_then(|c| c.buyer_reference.clone()),
                }
            })
            .collect();
        if customers.is_empty()
            && let Some(json) = self.store.setting(Some(books), CUSTOMERS)?
        {
            customers = serde_json::from_str(&json).map_err(|e| AppError::Invoice(e.to_string()))?;
        }
        customers.sort_by_key(|c| c.name.to_lowercase());
        Ok(customers)
    }

    /// Keeps a customer as an organisation described in the books: a new one without an identity gets one, one with
    /// an identity is described anew. Returns the identity.
    pub fn save_customer(&mut self, books: &BooksId, customer: Customer) -> Result<String, AppError> {
        self.get(books)?;
        if customer.name.trim().is_empty() {
            return Err(AppError::Invoice("a customer needs a name".into()));
        }
        let id: PartyId = if customer.id.is_empty() {
            new_id()
        } else {
            customer.id.parse().map_err(|_| AppError::Invoice(format!("not a customer: {}", customer.id)))?
        };
        if self.get(books)?.events().owners().contains(&&id) {
            return Err(AppError::Invoice("an owner of the books is not a customer".into()));
        }
        let text = |t: &str| Some(t.trim().to_owned()).filter(|t| !t.is_empty());
        let country = customer.country.trim().to_uppercase();
        let organisation = Organisation {
            id: id.clone(),
            name: customer.name.trim().to_owned(),
            legal_form: String::new(),
            country: country.parse().map_err(|_| AppError::Invoice(format!("not a country: {country}")))?,
            registrations: vec![],
            tax_ids: customer
                .vat_id
                .as_deref()
                .and_then(text)
                .map(|number| vec![TaxId { scheme: "vat".into(), number }])
                .unwrap_or_default(),
            shares: vec![],
            contact: Some(Contact {
                address: Some(postal_address(&customer.street, &customer.postcode, &customer.city, &country)?),
                name: None,
                phone: None,
                email: text(&customer.email),
                endpoint: text(&customer.email).map(|address| librotenado::Endpoint { scheme: "EM".into(), address }),
                buyer_reference: customer.reference.as_deref().and_then(text),
            }),
        };
        self.execute(books, Command::DescribeOrganisation(organisation))?;
        Ok(id.to_string())
    }

    /// Brings the seller's details and customers kept on this device by spezado 0.1 into the books, so that they
    /// travel with them; does nothing when the books have them already.
    pub fn migrate_invoicing(&mut self, books: &BooksId) -> Result<(), AppError> {
        let owner = self.invoicing_owner(books)?;
        if self.get(books)?.events().invoicing(&owner).is_none()
            && let Some(profile) = self.invoice_profile(books)?
        {
            self.set_invoice_profile(books, &profile)?;
        }
        let (organisations, persons) = self.get(books)?.events().parties();
        let described = organisations.len() + persons.len();
        let owners = self.get(books)?.events().owners().len();
        if described <= owners
            && let Some(json) = self.store.setting(Some(books), CUSTOMERS)?
        {
            let legacy: Vec<Customer> = serde_json::from_str(&json).map_err(|e| AppError::Invoice(e.to_string()))?;
            for customer in legacy {
                self.save_customer(books, customer)?;
            }
        }
        Ok(())
    }

    /// The series of invoice numbers of this device for an owner, and whether it is still to be assigned: the one
    /// assigned, else none if no device has one yet, else the first letter no device has (spec 0008).
    fn device_series(&self, books: &BooksId, owner: &PartyId) -> Result<(String, bool), AppError> {
        let series = self.get(books)?.events().series(owner);
        if let Some(mine) = series.get(&self.device) {
            return Ok((mine.clone(), false));
        }
        let taken: Vec<&String> = series.values().collect();
        let free = std::iter::once(String::new())
            .chain(('B'..='Z').map(String::from))
            .chain(('A'..='Z').flat_map(|a| ('A'..='Z').map(move |b| format!("{a}{b}"))))
            .find(|s| !taken.contains(&s))
            .expect("a free series");
        Ok((free, true))
    }

    /// The next invoice number of a year on this device: the prefix, the year, `-`, the device's series and one more
    /// than the highest number of that series in that year – `RE-2026-001`, `RE-2026-B001`.
    pub fn next_invoice_number(&self, books: &BooksId, year: u16) -> Result<String, AppError> {
        let owner = self.invoicing_owner(books)?;
        let prefix = self.invoice_profile(books)?.map_or_else(|| "RE-".to_owned(), |p| p.prefix);
        let (series, _) = self.device_series(books, &owner)?;
        let start = format!("{prefix}{year}-{series}");
        let highest = self
            .documents(books)?
            .iter()
            .filter_map(|(_, d)| d.number.as_deref()?.strip_prefix(&start)?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(format!("{start}{:03}", highest + 1))
    }

    /// Writes an invoice as an XRechnung 3.0 in UBL, checked against what XRechnung requires; nothing is recorded.
    pub fn write_invoice(
        &self,
        books: &BooksId,
        owner: &PartyId,
        new: &NewInvoice,
    ) -> Result<WrittenInvoice, AppError> {
        let profile = self
            .invoice_profile(books)?
            .ok_or_else(|| AppError::Invoice("give the seller's details for invoices first".into()))?;
        let customer = self
            .customers(books)?
            .into_iter()
            .find(|c| c.id == new.customer)
            .ok_or_else(|| AppError::Invoice(format!("no such customer: {}", new.customer)))?;
        if new.lines.is_empty() {
            return Err(AppError::Invoice("an invoice needs a line".into()));
        }
        let (configuration, ..) = self.law_for(books, owner, new.date)?;
        let small_business = configuration.vat.as_ref().is_some_and(|v| v.scheme.as_str() == "de-small-business");
        let currency = configuration.currency.clone();
        let table = librotenado::TaxTable::de();
        let number = self.next_invoice_number(books, new.date.year())?;

        // The lines, and the VAT per category and rate.
        let mut lines = Vec::new();
        let mut breakdown: BTreeMap<(String, String), i128> = BTreeMap::new();
        // The statement per category without VAT.
        let mut stated: BTreeMap<String, (String, Option<&'static str>)> = BTreeMap::new();
        let has_vat_id = |id: &Option<String>| id.as_ref().is_some_and(|v| !v.trim().is_empty());
        let abroad = customer.country != "DE";
        for (i, line) in new.lines.iter().enumerate() {
            if line.price.currency() != &currency {
                return Err(AppError::Invoice(format!("line {}: not in {currency}", i + 1)));
            }
            let net = times(line.price.base_units(), line.quantity, 1)
                .ok_or_else(|| AppError::Invoice("the amounts are too large".into()))?;
            let (category, rate) = if small_business {
                ("E", "0".to_owned())
            } else {
                let code = line.tax.as_ref().ok_or_else(|| AppError::Invoice(format!("line {}: a tax code", i + 1)))?;
                let fail = |why: &str| AppError::Invoice(format!("line {}: {code} {why}", i + 1));
                let category = INVOICE_TAX_CODES
                    .iter()
                    .find(|(c, _)| *c == code.as_str())
                    .map(|(_, category)| *category)
                    .ok_or_else(|| fail("cannot be invoiced yet"))?;
                // What § 14a UStG and EN 16931 (BR-AE-02, BR-IC-02) ask for, and what tells the VAT return apart.
                match code.as_str() {
                    "de-out-13b" if abroad => return Err(fail("is for customers in Germany")),
                    "de-out-eu-service" | "de-out-ig-supply" if !abroad => {
                        return Err(fail("is for customers in other EU countries"));
                    }
                    "de-out-eu-service" | "de-out-ig-supply" if !has_vat_id(&profile.vat_id) => {
                        return Err(fail("needs the seller's VAT identification number"));
                    }
                    "de-out-13b" | "de-out-eu-service" | "de-out-ig-supply" if !has_vat_id(&customer.vat_id) => {
                        return Err(fail("needs the customer's VAT identification number"));
                    }
                    _ => {}
                }
                if let Some(statement) =
                    exemption(code.as_str(), new.exemption.as_deref().filter(|e| !e.trim().is_empty()))
                {
                    stated.insert(category.to_owned(), statement);
                }
                let rate = match category {
                    "S" => table
                        .codes
                        .iter()
                        .find(|c| c.code == *code)
                        .and_then(|d| d.rate)
                        .ok_or_else(|| fail("has no rate"))?
                        .to_string(),
                    _ => "0".to_owned(),
                };
                (category, rate)
            };
            *breakdown.entry((category.to_owned(), rate.clone())).or_insert(0) += net;
            lines.push(json!({
                "id": (i + 1).to_string(), "name": line.name, "quantity": line.quantity.to_string(), "unit": line.unit,
                "price": format_price(&line.price), "net": money(net, &currency)?, "category": category, "rate": rate,
            }));
        }
        let mut taxes = Vec::new();
        let (mut net_total, mut tax_total) = (0i128, 0i128);
        for ((category, rate), base) in &breakdown {
            let rate: Decimal = rate.parse().map_err(|_| AppError::Invoice("a rate".into()))?;
            let tax = times(*base, rate, 100).ok_or_else(|| AppError::Invoice("the amounts are too large".into()))?;
            net_total += base;
            tax_total += tax;
            let mut entry = json!({"category": category, "rate": rate.to_string(), "base": money(*base, &currency)?, "tax": money(tax, &currency)?});
            if small_business {
                entry["exemption"] = "Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.".into();
            } else if let Some((text, code)) = stated.get(category) {
                entry["exemption"] = text.as_str().into();
                if let Some(code) = code {
                    entry["exemption_code"] = (*code).into();
                }
            }
            taxes.push(entry);
        }
        let gross = net_total + tax_total;

        let address = |street: &str, postcode: &str, city: &str, country: &str| json!({"lines": [street], "postcode": postcode, "city": city, "country": country});
        let mut seller = json!({
            "name": profile.name,
            "address": address(&profile.street, &profile.postcode, &profile.city, &profile.country),
            "contact": {"name": profile.contact, "phone": profile.phone, "email": profile.email},
            "endpoint": {"scheme": "EM", "address": profile.email},
            "party": owner.as_str(),
        });
        if let Some(vat_id) = profile.vat_id.as_ref().filter(|v| !v.is_empty()) {
            seller["vat_id"] = vat_id.as_str().into();
        }
        if let Some(tax_number) = profile.tax_number.as_ref().filter(|v| !v.is_empty()) {
            seller["tax_number"] = tax_number.as_str().into();
            if seller.get("vat_id").is_none() {
                // Without a VAT number the seller is identified by its tax number (BR-CO-26, above).
                seller["identifiers"] = json!([{"id": tax_number}]);
            }
        }
        let mut buyer = json!({
            "name": customer.name,
            "address": address(&customer.street, &customer.postcode, &customer.city, &customer.country),
            "endpoint": {"scheme": "EM", "address": customer.email},
        });
        if let Some(vat_id) = customer.vat_id.as_ref().filter(|v| !v.is_empty()) {
            buyer["vat_id"] = vat_id.as_str().into();
        }
        let due = plus_days(new.date, profile.payment_days);
        let mut data = json!({
            "currency": currency.as_str(),
            "seller": seller,
            "buyer": buyer,
            "buyer_reference": customer.reference.clone().filter(|r| !r.is_empty()).unwrap_or_else(|| number.clone()),
            "delivery_date": new.delivery.unwrap_or(new.date).to_string(),
            "due_date": due.map(|d| d.to_string()),
            "payment_terms": format!("Zahlbar bis {} ohne Abzug.", due.map(|d| spezado_core::format_date_in(d, "de")).unwrap_or_default()),
            "payment": {"means": "58", "remittance": number, "transfers": [{"account": profile.iban.replace(' ', ""), "name": profile.name}]},
            "taxes": taxes,
            "lines": lines,
            "totals": {
                "net": money(net_total, &currency)?, "tax": money(tax_total, &currency)?,
                "gross": money(gross, &currency)?, "due": money(gross, &currency)?,
            },
        });
        if breakdown.keys().any(|(category, _)| category == "K") {
            // Where the goods went (BR-IC-12): to the customer.
            data["delivery"] = json!({"name": customer.name, "address": address(&customer.street, &customer.postcode, &customer.city, &customer.country)});
        }
        if let Some(note) = new.note.as_ref().filter(|n| !n.trim().is_empty()) {
            data["notes"] = json!([{"text": note}]);
        }
        let invoice: librotenado::InvoiceData =
            serde_json::from_value(data).map_err(|e| AppError::Invoice(e.to_string()))?;
        let translated = en16931::TranslatedInvoice {
            format: match new.format {
                InvoiceFormat::XRechnung => "en16931-ubl",
                InvoiceFormat::Zugferd => "en16931-cii",
            },
            kind: DocumentKind::Invoice,
            number: number.clone(),
            date: new.date,
            invoice,
            files: BTreeMap::new(),
        };
        en16931::check_xrechnung(&translated).map_err(|e| AppError::Invoice(e.to_string()))?;
        let currencies = CurrencyTable::v0_1();
        let failed = |e: en16931::WriteError| AppError::Invoice(e.to_string());
        let (xml, original, media_type, name) = match new.format {
            InvoiceFormat::XRechnung => {
                let xml = en16931::write_ubl(&translated, &currencies).map_err(failed)?;
                let bytes = xml.clone().into_bytes();
                (xml, bytes, "application/xml", format!("{number}.xml"))
            }
            InvoiceFormat::Zugferd => {
                let xml = en16931::write_cii(&translated, &currencies).map_err(failed)?;
                let pdf = crate::invoice_pdf::zugferd(&translated, &xml)?;
                (xml, pdf, "application/pdf", format!("{number}.pdf"))
            }
        };
        let hash = librotenado::ContentHash::of(&original);
        let document = Document {
            id: new_id(),
            kind: DocumentKind::Invoice,
            number: Some(number.clone()),
            date: new.date,
            received: new.date,
            original: Original {
                hash,
                size: original.len() as u64,
                media_type: media_type.into(),
                format: Some(translated.format.into()),
                name: Some(name),
            },
            invoice: Some(Box::new(translated.invoice)),
            statement: None,
        };
        // With double entry the receivable is booked when the invoice is issued; on a cash basis when it is paid.
        let booking = match configuration.accounting.as_ref().map(|a| a.as_str()) {
            Some("de-double-entry") => {
                let standard = self.standard_accounts(books, owner, new.date)?;
                let accounts = ProposalAccounts {
                    counter: standard.sales.clone(),
                    party: standard.receivables.clone(),
                    vat_output: standard
                        .vat
                        .get(&"de-out-19".parse().expect("a tax code"))
                        .cloned()
                        .unwrap_or(standard.vat_reverse_charge.clone()),
                    vat_input: standard.vat_reverse_charge_input.clone(),
                    vat_reverse_charge: standard.vat_reverse_charge.clone(),
                };
                let mut booking = propose_booking(&document, InvoiceSide::Sale, owner, &accounts)
                    .map_err(|e| AppError::Invoice(e.to_string()))?;
                booking.text = format!("{number} {}", customer.name);
                Some(booking)
            }
            _ => None,
        };
        Ok(WrittenInvoice { document, xml, original, booking })
    }

    /// Writes an invoice and records it – with the booking of its receivable under double entry –, all or none.
    pub fn issue_invoice(
        &mut self,
        books: &BooksId,
        owner: &PartyId,
        new: &NewInvoice,
    ) -> Result<WrittenInvoice, AppError> {
        let (series, assign) = self.device_series(books, owner)?;
        if assign {
            let device = self.device.clone();
            self.execute(books, Command::AssignSeries(Series { owner: owner.clone(), device, series }))?;
        }
        let written = self.write_invoice(books, owner, new)?;
        self.store.put_original(&written.original)?;
        self.record_document(books, written.document.clone(), written.booking.clone())?;
        Ok(written)
    }
}

/// The name, tax numbers and contact of a party.
type PartyDetails = (String, Vec<TaxId>, Option<Contact>);

/// A postal address of one line, a postcode, a city and a country.
fn postal_address(street: &str, postcode: &str, city: &str, country: &str) -> Result<PostalAddress, AppError> {
    let text = |t: &str| Some(t.trim().to_owned()).filter(|t| !t.is_empty());
    Ok(PostalAddress {
        lines: text(street).into_iter().collect(),
        city: text(city),
        postcode: text(postcode),
        subdivision: None,
        country: country
            .trim()
            .to_uppercase()
            .parse()
            .map_err(|_| AppError::Invoice(format!("not a country: {country}")))?,
    })
}

/// A unit price as a decimal with as many places as it needs: `95`, `12.5`.
fn format_price(price: &Money) -> String {
    let plain = spezado_core::format_amount(price);
    let places = plain.split_once('.').map_or(0, |(_, fraction)| fraction.len());
    Decimal::new(price.base_units(), places as u8).map_or(plain, |d| d.to_string())
}

/// A date some days later.
fn plus_days(date: Date, days: u16) -> Option<Date> {
    use chrono::Datelike;
    let later =
        chrono::NaiveDate::from_ymd_opt(i32::from(date.year()), u32::from(date.month()), u32::from(date.day()))?
            .checked_add_days(chrono::Days::new(u64::from(days)))?;
    Date::new(u16::try_from(later.year()).ok()?, later.month() as u8, later.day() as u8)
}
