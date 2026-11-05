// SPDX-License-Identifier: AGPL-3.0-or-later
//! Contacts: the customers and suppliers of an owner, with their numbers, contact persons and bank accounts – the
//! organisations and persons of the books with their relation to the owner (librotenado spec part 0008). A document
//! read from a file is offered the contacts its seller or buyer may be.

use librotenado::{
    BankAccount, Contact, Endpoint, InvoiceParty, MatchBy, Organisation, PartyId, Person, Relation, RelationKind, Role,
    RoleKind, TaxId,
};
use serde::{Deserialize, Serialize};
use spezado_core::Command;

use crate::{AppError, BooksId, Workspace, new_id};

/// Where customer numbers begin; supplier numbers begin at [`FIRST_SUPPLIER`].
const FIRST_CUSTOMER: u64 = 10001;
/// Where supplier numbers begin.
const FIRST_SUPPLIER: u64 = 70001;

/// A customer or a supplier of an owner, or both, as a contact list shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactCard {
    /// Its identity; empty for a new one.
    #[serde(default)]
    pub id: String,
    /// The name.
    pub name: String,
    /// The owner supplies it.
    #[serde(default)]
    pub customer: bool,
    /// It supplies the owner.
    #[serde(default)]
    pub supplier: bool,
    /// The owner's number for it; a new one gets the next free one when left out.
    #[serde(default)]
    pub number: Option<String>,
    /// Street and number.
    #[serde(default)]
    pub street: String,
    /// The postcode.
    #[serde(default)]
    pub postcode: String,
    /// The city.
    #[serde(default)]
    pub city: String,
    /// The country, ISO 3166-1 alpha-2.
    #[serde(default)]
    pub country: String,
    /// The e-mail address – where invoices go.
    #[serde(default)]
    pub email: String,
    /// The telephone number.
    #[serde(default)]
    pub phone: String,
    /// The VAT identification number.
    #[serde(default)]
    pub vat_id: Option<String>,
    /// Another tax number, e.g. a German Steuernummer.
    #[serde(default)]
    pub tax_number: Option<String>,
    /// The bank account, an IBAN.
    #[serde(default)]
    pub iban: Option<String>,
    /// The BIC of the bank account.
    #[serde(default)]
    pub bic: Option<String>,
    /// What it asks to be named on invoices – the Leitweg-ID of a public authority.
    #[serde(default)]
    pub reference: Option<String>,
    /// Whom to contact there.
    #[serde(default)]
    pub persons: Vec<ContactPerson>,
}

/// Whom to contact at a customer or a supplier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactPerson {
    /// Their identity; empty for a new one.
    #[serde(default)]
    pub id: String,
    /// The name.
    pub name: String,
    /// The e-mail address.
    #[serde(default)]
    pub email: String,
    /// The telephone number.
    #[serde(default)]
    pub phone: String,
}

/// A contact a party of a document may be, and what it matched by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactSuggestion {
    /// The contact.
    pub party: PartyId,
    /// Its name.
    pub name: String,
    /// What matched.
    pub by: MatchBy,
}

/// The tax scheme of a German tax number.
const TAX_NUMBER: &str = "de-steuernummer";

fn text(value: &str) -> Option<String> {
    Some(value.trim().to_owned()).filter(|t| !t.is_empty())
}

impl Workspace {
    /// The contacts of an owner by name: the organisations and persons of the books that are neither owners nor
    /// contact persons, with their relation to the owner. One without a relation is a customer – as spezado 0.3
    /// kept every party but the owners.
    pub fn contacts(&self, books: &BooksId, owner: &PartyId) -> Result<Vec<ContactCard>, AppError> {
        let events = self.get(books)?.events();
        let owners = events.owners();
        let relations = events.relations(owner);
        let (organisations, persons) = events.parties();
        let contact_persons = |organisation: &PartyId| -> Vec<ContactPerson> {
            persons
                .values()
                .filter(|p| p.roles.iter().any(|r| r.kind == RoleKind::Contact && r.organisation == *organisation))
                .map(|p| ContactPerson {
                    id: p.id.to_string(),
                    name: p.name.clone(),
                    email: p.contact.as_ref().and_then(|c| c.email.clone()).unwrap_or_default(),
                    phone: p.contact.as_ref().and_then(|c| c.phone.clone()).unwrap_or_default(),
                })
                .collect()
        };
        let card = |id: &PartyId,
                    name: &str,
                    country: Option<String>,
                    tax_ids: &[TaxId],
                    contact: Option<&Contact>,
                    banks: &[BankAccount]|
         -> ContactCard {
            let address = contact.and_then(|c| c.address.as_ref());
            let relation = relations.get(id);
            let kinds = relation.map_or(&[RelationKind::Customer][..], |r| r.kinds.as_slice());
            ContactCard {
                id: id.to_string(),
                name: name.to_owned(),
                customer: kinds.contains(&RelationKind::Customer),
                supplier: kinds.contains(&RelationKind::Supplier),
                number: relation.and_then(|r| r.number.clone()),
                street: address.and_then(|a| a.lines.first().cloned()).unwrap_or_default(),
                postcode: address.and_then(|a| a.postcode.clone()).unwrap_or_default(),
                city: address.and_then(|a| a.city.clone()).unwrap_or_default(),
                country: address.map(|a| a.country.to_string()).or(country).unwrap_or_else(|| "DE".into()),
                email: contact
                    .and_then(|c| c.email.clone().or_else(|| c.endpoint.as_ref().map(|e| e.address.clone())))
                    .unwrap_or_default(),
                phone: contact.and_then(|c| c.phone.clone()).unwrap_or_default(),
                vat_id: tax_ids.iter().find(|t| t.scheme == "vat").map(|t| t.number.clone()),
                tax_number: tax_ids.iter().find(|t| t.scheme != "vat").map(|t| t.number.clone()),
                iban: banks.first().map(|b| b.iban.clone()),
                bic: banks.first().and_then(|b| b.bic.clone()),
                reference: contact.and_then(|c| c.buyer_reference.clone()),
                persons: contact_persons(id),
            }
        };
        let is_contact_person = |p: &Person| p.roles.iter().any(|r| r.kind == RoleKind::Contact);
        let mut cards: Vec<ContactCard> = organisations
            .values()
            .filter(|o| !owners.contains(&&o.id))
            .map(|o| {
                card(&o.id, &o.name, Some(o.country.to_string()), &o.tax_ids, o.contact.as_ref(), &o.bank_accounts)
            })
            .chain(
                persons
                    .values()
                    .filter(|p| !owners.contains(&&p.id) && !is_contact_person(p))
                    .map(|p| card(&p.id, &p.name, None, &p.tax_ids, p.contact.as_ref(), &p.bank_accounts)),
            )
            .collect();
        cards.sort_by_key(|c| c.name.to_lowercase());
        Ok(cards)
    }

    /// The next free number of an owner for a customer – from 10001 – or, for a supplier only, from 70001.
    pub fn next_contact_number(
        &self,
        books: &BooksId,
        owner: &PartyId,
        supplier_only: bool,
    ) -> Result<String, AppError> {
        let first = if supplier_only { FIRST_SUPPLIER } else { FIRST_CUSTOMER };
        let taken = self
            .get(books)?
            .events()
            .relations(owner)
            .values()
            .filter_map(|r| r.number.as_deref()?.parse::<u64>().ok())
            .filter(|n| (first..first + 60000).contains(n))
            .max();
        Ok(taken.map_or(first, |n| n + 1).to_string())
    }

    /// Keeps a contact of an owner: describes it as an organisation – a new one without an identity gets one –, its
    /// contact persons as persons with the role `contact`, and its relation to the owner. Returns its identity.
    pub fn save_contact(&mut self, books: &BooksId, owner: &PartyId, card: ContactCard) -> Result<String, AppError> {
        let events = self.get(books)?.events();
        if card.name.trim().is_empty() {
            return Err(AppError::Invoice("a contact needs a name".into()));
        }
        if !card.customer && !card.supplier {
            return Err(AppError::Invoice("a contact is a customer, a supplier, or both".into()));
        }
        let id: PartyId = if card.id.is_empty() {
            new_id()
        } else {
            card.id.parse().map_err(|_| AppError::Invoice(format!("not a contact: {}", card.id)))?
        };
        if events.owners().contains(&&id) {
            return Err(AppError::Invoice("an owner of the books is not a contact".into()));
        }
        let (organisations, persons) = events.parties();
        let earlier = organisations.get(&id).map(|o| (*o).clone());
        let earlier_persons: Vec<Person> = persons
            .values()
            .filter(|p| p.roles.iter().any(|r| r.kind == RoleKind::Contact && r.organisation == id))
            .map(|p| (*p).clone())
            .collect();
        let country = card.country.trim().to_uppercase();
        let country = if country.is_empty() { "DE".to_owned() } else { country };
        let mut tax_ids = Vec::new();
        if let Some(number) = card.vat_id.as_deref().and_then(text) {
            tax_ids.push(TaxId { scheme: "vat".into(), number });
        }
        if let Some(number) = card.tax_number.as_deref().and_then(text) {
            tax_ids.push(TaxId { scheme: TAX_NUMBER.into(), number });
        }
        let bank_accounts = card
            .iban
            .as_deref()
            .and_then(text)
            .map(|iban| vec![BankAccount { iban, bic: card.bic.as_deref().and_then(text) }])
            .unwrap_or_default();
        let address = if [&card.street, &card.postcode, &card.city].iter().all(|t| t.trim().is_empty()) {
            None
        } else {
            Some(crate::invoicing::postal_address(&card.street, &card.postcode, &card.city, &country)?)
        };
        let email = text(&card.email);
        let organisation = Organisation {
            id: id.clone(),
            name: card.name.trim().to_owned(),
            legal_form: earlier.as_ref().map(|o| o.legal_form.clone()).unwrap_or_default(),
            country: country.parse().map_err(|_| AppError::Invoice(format!("not a country: {country}")))?,
            registrations: earlier.as_ref().map(|o| o.registrations.clone()).unwrap_or_default(),
            tax_ids,
            shares: earlier.as_ref().map(|o| o.shares.clone()).unwrap_or_default(),
            contact: Some(Contact {
                address,
                name: card.persons.first().map(|p| p.name.trim().to_owned()).filter(|n| !n.is_empty()),
                phone: text(&card.phone),
                email: email.clone(),
                endpoint: email.map(|address| Endpoint { scheme: "EM".into(), address }),
                buyer_reference: card.reference.as_deref().and_then(text),
            }),
            bank_accounts,
        };
        let mut commands = vec![(books.clone(), Command::DescribeOrganisation(organisation))];
        // Contact persons: kept or new ones with the role, those taken off the card without it.
        let mut kept = Vec::new();
        for person in card.persons.iter().filter(|p| !p.name.trim().is_empty()) {
            let person_id: PartyId = if person.id.is_empty() {
                new_id()
            } else {
                person.id.parse().map_err(|_| AppError::Invoice(format!("not a person: {}", person.id)))?
            };
            let before = persons.get(&person_id);
            let mut roles: Vec<Role> = before.map(|p| p.roles.clone()).unwrap_or_default();
            if !roles.iter().any(|r| r.kind == RoleKind::Contact && r.organisation == id) {
                roles.push(Role { organisation: id.clone(), kind: RoleKind::Contact, shares: None });
            }
            commands.push((
                books.clone(),
                Command::DescribePerson(Person {
                    id: person_id.clone(),
                    name: person.name.trim().to_owned(),
                    tax_ids: before.map(|p| p.tax_ids.clone()).unwrap_or_default(),
                    roles,
                    contact: Some(Contact {
                        phone: text(&person.phone),
                        email: text(&person.email),
                        ..Contact::default()
                    }),
                    bank_accounts: before.map(|p| p.bank_accounts.clone()).unwrap_or_default(),
                }),
            ));
            kept.push(person_id);
        }
        for gone in earlier_persons.into_iter().filter(|p| !kept.contains(&p.id)) {
            let roles = gone.roles.iter().filter(|r| !(r.kind == RoleKind::Contact && r.organisation == id)).cloned();
            commands.push((books.clone(), Command::DescribePerson(Person { roles: roles.collect(), ..gone })));
        }
        let number = match card.number.as_deref().and_then(text) {
            Some(number) => number,
            None => match events.relations(owner).get(&id).and_then(|r| r.number.clone()) {
                Some(number) => number,
                None => self.next_contact_number(books, owner, card.supplier && !card.customer)?,
            },
        };
        let kinds = [(card.customer, RelationKind::Customer), (card.supplier, RelationKind::Supplier)]
            .into_iter()
            .filter_map(|(is, kind)| is.then_some(kind))
            .collect();
        let relation = Relation { owner: owner.clone(), party: id.clone(), kinds, number: Some(number) };
        commands.push((books.clone(), Command::DescribeRelation(relation)));
        self.execute_all(commands)?;
        Ok(id.to_string())
    }

    /// A new contact from a party of a document – what the document says of it, ready to be kept.
    pub fn contact_from(&self, party: &InvoiceParty, supplier: bool, accounts: &[String]) -> ContactCard {
        let address = party.address.as_ref();
        let contact = party.contact.as_ref();
        ContactCard {
            name: party.name.clone(),
            customer: !supplier,
            supplier,
            street: address.and_then(|a| a.lines.first().cloned()).unwrap_or_default(),
            postcode: address.and_then(|a| a.postcode.clone()).unwrap_or_default(),
            city: address.and_then(|a| a.city.clone()).unwrap_or_default(),
            country: address.map_or_else(|| "DE".to_owned(), |a| a.country.to_string()),
            email: contact.and_then(|c| c.email.clone()).unwrap_or_default(),
            phone: contact.and_then(|c| c.phone.clone()).unwrap_or_default(),
            vat_id: party.vat_id.clone(),
            tax_number: party.tax_number.clone(),
            iban: accounts.first().cloned(),
            persons: contact
                .and_then(|c| c.name.clone())
                .map(|name| ContactPerson { name, ..ContactPerson::default() })
                .into_iter()
                .collect(),
            ..ContactCard::default()
        }
    }

    /// The contacts a party of a document may be, the most certain first; `accounts` are the IBANs the document
    /// names for it. Owners are left out.
    pub fn suggest_contacts(
        &self,
        books: &BooksId,
        party: &InvoiceParty,
        accounts: &[String],
    ) -> Result<Vec<ContactSuggestion>, AppError> {
        let events = self.get(books)?.events();
        let owners = events.owners();
        let (organisations, persons) = events.parties();
        let accounts: Vec<&str> = accounts.iter().map(String::as_str).collect();
        Ok(events
            .match_parties(party, &accounts)
            .into_iter()
            .filter(|m| !owners.contains(&&m.party))
            .filter_map(|m| {
                let name = organisations
                    .get(&m.party)
                    .map(|o| o.name.clone())
                    .or_else(|| persons.get(&m.party).map(|p| p.name.clone()))?;
                Some(ContactSuggestion { party: m.party, name, by: m.by })
            })
            .collect())
    }
}
