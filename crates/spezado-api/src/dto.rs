// SPDX-License-Identifier: AGPL-3.0-or-later
//! What the API reads and writes: plain JSON for any client – identities, dates and amounts as strings, amounts as
//! decimal numbers with a point (`"12.40"`) in their currency.

use librotenado::Money;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// An amount of money.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Amount {
    /// The amount with a decimal point, as many places as the currency has: `"-12.40"`.
    pub value: String,
    /// The currency, ISO 4217: `"EUR"`.
    pub currency: String,
}

impl From<&Money> for Amount {
    fn from(money: &Money) -> Self {
        Amount { value: spezado_core::format_amount(money), currency: money.currency().to_string() }
    }
}

/// An error: what was refused or went wrong.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Problem {
    /// What happened, in English.
    pub error: String,
}

/// Exchanging a pairing code for a token.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PairRequest {
    /// The code `spezado serve` shows.
    pub code: String,
    /// What the device is called, to recognise it later: `"Laptop browser"`.
    pub name: String,
}

/// A token for a device, to send as `Authorization: Bearer <token>`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Paired {
    /// The token; shown once, stored only as its hash.
    pub token: String,
    /// The device's identity, to revoke it.
    pub device: String,
}

/// A device paired with the API.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Device {
    /// Its identity.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// When it was paired, `YYYY-MM-DD`.
    pub paired: String,
}

/// A set of books of the workspace.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BooksSummary {
    /// The identity of the books.
    pub id: String,
    /// The owner shown – the first.
    pub owner: Option<String>,
    /// The owner's name.
    pub owner_name: Option<String>,
    /// The national chart of accounts: `"de-skr03"`.
    pub chart: Option<String>,
}

/// Setting up books for a person.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetUp {
    /// The person's name.
    pub name: String,
    /// The first day, `YYYY-MM-DD`.
    pub from: String,
    /// `de-cash-basis` or `de-double-entry`.
    pub accounting: String,
    /// `de-regular` or `de-small-business`.
    pub vat_scheme: String,
    /// `accrual` or `cash`.
    pub vat_basis: String,
    /// `month`, `quarter` or `year`.
    pub vat_period: String,
    /// `de-skr03` or `de-skr04`.
    pub chart: String,
    /// Whether the deadlines of advance returns are extended for good (§ 46 UStDV).
    #[serde(default)]
    pub vat_extended: bool,
    /// The state whose public holidays move deadlines, ISO 3166-2 (`DE-BY`); none when not given.
    #[serde(default)]
    pub region: Option<String>,
}

/// A state whose public holidays move deadlines.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegionView {
    /// Its code, ISO 3166-2: `DE-BY`.
    pub code: String,
    /// Its name in the language asked for.
    pub name: String,
}

/// The facts that move an owner's deadlines.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Deadlines {
    /// From when they apply, `YYYY-MM-DD`: when changing, today if none.
    #[serde(default)]
    pub from: Option<String>,
    /// The state whose public holidays move deadlines, ISO 3166-2 (`DE-BY`); none for holidays of all states only.
    #[serde(default)]
    pub region: Option<String>,
    /// Whether advance VAT returns are due a month later for good (§ 46 UStDV).
    #[serde(default)]
    pub vat_extended: bool,
}

/// An advance VAT return and when it is due.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VatReturnDue {
    /// The first day of its period.
    pub from: String,
    /// The last day of its period.
    pub to: String,
    /// The day it is due.
    pub due: String,
}

/// The books at a glance.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Overview {
    /// The year.
    pub year: u16,
    /// The bank balance today.
    pub bank: Option<Amount>,
    /// The cash balance today.
    pub cash: Option<Amount>,
    /// Income of the year.
    pub income: Option<Amount>,
    /// Expenses of the year.
    pub expenses: Option<Amount>,
    /// The next advance VAT return from today.
    pub next_vat_return: Option<VatReturnDue>,
    /// Entries of bank statements not booked yet.
    pub open_entries: usize,
    /// Items on the review list.
    pub issues: usize,
    /// The last day closed.
    pub closed_until: Option<String>,
}

/// A line of a booking.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostingView {
    /// The account's identity.
    pub account: String,
    /// Its national number.
    pub number: Option<String>,
    /// Its name.
    pub name: String,
    /// Debit positive, credit negative.
    pub amount: Amount,
    /// The tax code.
    pub tax: Option<String>,
}

/// A booking as the journal shows it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JournalEntry {
    /// The event that recorded it.
    pub event: String,
    /// Its date.
    pub date: String,
    /// Its text.
    pub text: String,
    /// What moved: the sum of its debits.
    pub amount: Amount,
    /// For a reversal, the event it reverses.
    pub reverses: Option<String>,
    /// The event that reversed it.
    pub reversed_by: Option<String>,
    /// Its lines.
    pub postings: Vec<PostingView>,
    /// The documents it rests on.
    pub documents: Vec<String>,
}

/// An account to choose from.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccountView {
    /// Its identity.
    pub id: String,
    /// Its national number.
    pub number: Option<String>,
    /// Its name.
    pub name: String,
    /// `asset`, `liability`, `equity`, `income`, `expense`, `memo`.
    pub kind: String,
}

/// The balance of an account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BalanceView {
    /// The account.
    pub account: String,
    /// Its national number.
    pub number: Option<String>,
    /// Its name.
    pub name: String,
    /// Debit positive.
    pub balance: Amount,
}

/// A tax code to choose from.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaxCodeView {
    /// The code: `de-in-19`.
    pub code: String,
    /// Its name.
    pub name: String,
}

/// A booking from a gross amount.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SimpleBooking {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// What it is about.
    pub text: String,
    /// The account: its national number or identity.
    pub account: String,
    /// The gross amount, VAT included, with a decimal point; negative for a refund.
    pub gross: String,
    /// The tax code; none for what VAT does not touch.
    pub tax: Option<String>,
    /// `bank` or `cash`.
    pub payment: String,
    /// The documents it rests on: identities of documents recorded.
    #[serde(default)]
    pub documents: Vec<String>,
}

/// A line of a booking to write.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Line {
    /// The account: its national number or identity.
    pub account: String,
    /// Debit positive, credit negative, with a decimal point.
    pub amount: String,
    /// The tax code.
    pub tax: Option<String>,
}

/// A booking in lines.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LinesBooking {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// What it is about.
    pub text: String,
    /// The lines; they balance.
    pub lines: Vec<Line>,
}

/// A reversal of a booking.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Reversal {
    /// The event that recorded the booking.
    pub event: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Why.
    pub text: String,
}

/// What was written.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Written {
    /// The events, `sha256:…`.
    pub events: Vec<String>,
}

/// A report of the year.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReportItem {
    /// Its identity: `de-ustva`.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The first day of its period.
    pub from: String,
    /// The last day of its period.
    pub to: String,
    /// When an advance VAT return is due.
    pub due: Option<String>,
}

/// A field of a report.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Field {
    /// The field: `81`, `1.112`.
    pub field: String,
    /// Its name on the form.
    pub name: Option<String>,
    /// Its amount.
    pub amount: Amount,
}

/// The figures of a report – a draft until tax advisers have reviewed its mapping.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Figures {
    /// Its identity.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The law module it was derived with, `name/version`.
    pub law: String,
    /// The fields with an amount.
    pub fields: Vec<Field>,
}

/// Closing the books up to a date.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Close {
    /// The last day closed, `YYYY-MM-DD`.
    pub to: String,
}

/// Something changed in a set of books – read again what is shown.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Change {
    /// The books.
    pub books: String,
}

/// A document recorded, or read and waiting to be recorded.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DocumentView {
    /// Its identity.
    pub id: String,
    /// `invoice`, `credit_note`, `receipt`, `bank_statement` …
    pub kind: String,
    /// Its number, e.g. the invoice number.
    pub number: Option<String>,
    /// The date it bears.
    pub date: String,
    /// The format of its original: `en16931-ubl`, `iso20022-camt.053` …
    pub format: Option<String>,
    /// The media type of its original.
    pub media_type: String,
    /// The file name it came with.
    pub name: Option<String>,
    /// The size of its original in bytes.
    pub size: u64,
    /// The amount due, for an invoice.
    pub due: Option<Amount>,
    /// Whether a booking rests on it.
    pub booked: bool,
}

/// A booking proposed, not written.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BookingView {
    /// Its date.
    pub date: String,
    /// Its text.
    pub text: String,
    /// Its lines.
    pub postings: Vec<PostingView>,
}

/// A file read as a document: waiting to be recorded, with a proposal if it is an e-invoice.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReadDocument {
    /// What to name when recording it, within the hour.
    pub pending: String,
    /// The document.
    pub document: DocumentView,
    /// The booking proposed from an e-invoice.
    pub proposal: Option<BookingView>,
    /// Why an XML file was not read as an e-invoice.
    pub not_an_invoice: Option<String>,
}

/// Recording a document read: alone, or with its proposal – dated and worded as given.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Record {
    /// Whether to book the proposal with it.
    pub book: bool,
    /// The booking's date, `YYYY-MM-DD` – on a cash basis the day paid.
    pub date: Option<String>,
    /// The booking's text.
    pub text: Option<String>,
}

/// What reading a file of statements did.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Imported {
    /// The statements recorded.
    pub documents: Vec<String>,
    /// Statements read before, left out.
    pub known: usize,
    /// Entries new to the books.
    pub entries: usize,
    /// Entries an earlier statement showed already.
    pub shown_before: usize,
}

/// A proposal for an entry.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SuggestionView {
    /// `booked` – the payment booked before the statement came, booked again naming the entry –, `invoice` – an
    /// unbooked invoice it pays – or `history` – as the same party's last entry.
    pub source: String,
    /// The invoice's number, for `invoice`.
    pub number: Option<String>,
    /// The booking proposed.
    pub booking: BookingView,
}

/// An entry of a bank statement not booked yet.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenEntry {
    /// The statement.
    pub document: String,
    /// The entry's position in it, from 1.
    pub entry: u32,
    /// The IBAN of the account.
    pub iban: String,
    /// The day booked.
    pub date: String,
    /// The other party and the purpose.
    pub text: String,
    /// Positive money in, negative money out.
    pub amount: Amount,
    /// A proposal.
    pub suggestion: Option<SuggestionView>,
}

/// An entry of a bank statement.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EntryRef {
    /// The statement.
    pub document: String,
    /// The entry's position in it, from 1.
    pub entry: u32,
}

/// Booking an entry on an account, its amount as the gross amount.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BookEntry {
    /// The statement.
    pub document: String,
    /// The entry's position in it, from 1.
    pub entry: u32,
    /// The account: national number or identity.
    pub account: String,
    /// The tax code.
    pub tax: Option<String>,
    /// The text.
    pub text: String,
}

/// How many proposals were booked.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Accepted {
    /// Their number.
    pub count: usize,
}

/// An opening balance to book.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpeningLine {
    /// The account: its national number or identity.
    pub account: String,
    /// What it held on the first day, with a decimal point: holdings positive, debts negative.
    pub amount: String,
}

/// Opening balances to book, once, against the opening balance sheet.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Opening {
    /// `YYYY-MM-DD`; the first day of the books when none.
    pub date: Option<String>,
    /// The text of the booking; "Anfangsbestände" when none.
    pub text: Option<String>,
    /// One per account.
    pub lines: Vec<OpeningLine>,
}

/// An opening balance booked.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpeningBalance {
    /// The account's identity.
    pub account: String,
    /// Its national number, if it has one.
    pub number: Option<String>,
    /// Its name.
    pub name: String,
    /// What it held, debit positive.
    pub amount: Amount,
}

/// The opening balances of the books.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpeningView {
    /// The first day of the books, `YYYY-MM-DD`.
    pub start: Option<String>,
    /// The balances booked, by account.
    pub balances: Vec<OpeningBalance>,
}

/// The seller's details on invoices.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvoiceProfile {
    /// The seller's name.
    pub name: String,
    /// Street and number.
    pub street: String,
    /// The postcode.
    pub postcode: String,
    /// The city.
    pub city: String,
    /// The country, ISO 3166-1 alpha-2: `DE`.
    pub country: String,
    /// The VAT identification number, if there is one.
    pub vat_id: Option<String>,
    /// The tax number at the tax office.
    pub tax_number: Option<String>,
    /// Whom to contact.
    pub contact: String,
    /// The phone number.
    pub phone: String,
    /// The e-mail address.
    pub email: String,
    /// The account to pay to.
    pub iban: String,
    /// The days until payment is due.
    pub payment_days: u16,
    /// What invoice numbers start with: `RE-`.
    pub prefix: String,
}

impl From<spezado_app::InvoiceProfile> for InvoiceProfile {
    fn from(p: spezado_app::InvoiceProfile) -> Self {
        let spezado_app::InvoiceProfile {
            name,
            street,
            postcode,
            city,
            country,
            vat_id,
            tax_number,
            contact,
            phone,
            email,
            iban,
            payment_days,
            prefix,
        } = p;
        InvoiceProfile {
            name,
            street,
            postcode,
            city,
            country,
            vat_id,
            tax_number,
            contact,
            phone,
            email,
            iban,
            payment_days,
            prefix,
        }
    }
}

impl From<InvoiceProfile> for spezado_app::InvoiceProfile {
    fn from(p: InvoiceProfile) -> Self {
        let InvoiceProfile {
            name,
            street,
            postcode,
            city,
            country,
            vat_id,
            tax_number,
            contact,
            phone,
            email,
            iban,
            payment_days,
            prefix,
        } = p;
        spezado_app::InvoiceProfile {
            name,
            street,
            postcode,
            city,
            country,
            vat_id,
            tax_number,
            contact,
            phone,
            email,
            iban,
            payment_days,
            prefix,
        }
    }
}

/// A customer.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Customer {
    /// Its identity; empty for a new one.
    #[serde(default)]
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
    pub vat_id: Option<String>,
    /// The reference the customer asked for – a Leitweg-ID; the invoice number when none.
    pub reference: Option<String>,
}

impl From<spezado_app::Customer> for Customer {
    fn from(c: spezado_app::Customer) -> Self {
        let spezado_app::Customer { id, name, street, postcode, city, country, email, vat_id, reference } = c;
        Customer { id, name, street, postcode, city, country, email, vat_id, reference }
    }
}

impl From<Customer> for spezado_app::Customer {
    fn from(c: Customer) -> Self {
        let Customer { id, name, street, postcode, city, country, email, vat_id, reference } = c;
        spezado_app::Customer { id, name, street, postcode, city, country, email, vat_id, reference }
    }
}

/// A customer kept.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Saved {
    /// Its identity.
    pub id: String,
}

/// A line of a new invoice.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvoiceLineInput {
    /// What was supplied.
    pub name: String,
    /// How much, with a decimal point: `2.5`.
    pub quantity: String,
    /// The unit, UN/ECE Recommendation 20: `HUR` hour, `DAY` day, `C62` piece.
    pub unit: String,
    /// The net price of one unit, with a decimal point.
    pub price: String,
    /// `de-out-19`, `de-out-7`, `de-out-0`, `de-out-exempt`, `de-out-13b`, `de-out-eu-service`, `de-out-ig-supply`
    /// or `de-out-export`; none for a small business.
    pub tax: Option<String>,
}

/// A new invoice.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewInvoice {
    /// The customer's identity.
    pub customer: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// When the supply was made, `YYYY-MM-DD`; the invoice date when none.
    pub delivery: Option<String>,
    /// A note on the invoice.
    pub note: Option<String>,
    /// Why exempt lines (`de-out-exempt`) are exempt; a general statement when none.
    #[serde(default)]
    pub exemption: Option<String>,
    /// `xrechnung` – XML – or `zugferd` – a PDF with the XRechnung attached; `xrechnung` when none.
    #[serde(default)]
    pub format: Option<String>,
    /// The lines.
    pub lines: Vec<InvoiceLineInput>,
}

/// An invoice issued.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Issued {
    /// The document, whose original is the XRechnung or the ZUGFeRD PDF.
    pub document: String,
    /// The invoice number.
    pub number: String,
    /// The file name of its original: `RE-2026-001.xml`, `RE-2026-001.pdf`.
    pub name: String,
    /// The amount due.
    pub due: Amount,
}

/// The number the next invoice gets.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NextNumber {
    /// `RE-2026-004`.
    pub number: String,
}

/// Whether books are synchronised.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncStatus {
    /// The relay they are synchronised through; none when they are not.
    pub relay: Option<String>,
}

/// What a synchronisation did.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncReport {
    /// Events and originals received.
    pub received: usize,
    /// Events and originals sent.
    pub sent: usize,
    /// Devices added by invitation.
    pub added: Vec<String>,
    /// Whether this device still waits to be added.
    pub waiting: bool,
}

/// Starting to synchronise books.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncStart {
    /// The address of the relay: `https://relay.example.org`.
    pub relay: String,
    /// What to call this device: `Laptop`.
    #[serde(default)]
    pub name: Option<String>,
}

/// The recovery code of books, shown once.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecoveryCode {
    /// 53 characters in groups of four.
    pub code: String,
}

/// An invitation for another device, good for a day.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncInvitation {
    /// The text to carry to the other device.
    pub invitation: String,
}

/// What an invited device may do.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncInvite {
    /// `read`, `write` or `manage` – the person's own devices; `manage` when none.
    #[serde(default)]
    pub rights: Option<String>,
}

/// A member removed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Removed {
    /// Its name.
    pub name: String,
}

/// A device of synchronised books.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncDevice {
    /// Its identity, to remove it by.
    pub id: String,
    /// What it may do: `read`, `write` or `manage`.
    pub rights: String,
    /// What people call it.
    pub name: String,
    /// The device name it writes under; none for the recovery code.
    pub device: Option<String>,
    /// Whether it is the recovery code.
    pub recovery: bool,
    /// Whether it is this device.
    pub me: bool,
}

/// Joining books with an invitation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncJoin {
    /// The invitation from the other device.
    pub invitation: String,
    /// What to call this device.
    #[serde(default)]
    pub name: Option<String>,
}

/// Recovering books with their recovery code.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncRecover {
    /// The address of the relay.
    pub relay: String,
    /// The identity of the books.
    pub books: String,
    /// The recovery code.
    pub code: String,
    /// What to call this device.
    #[serde(default)]
    pub name: Option<String>,
}

/// A set of books, by identity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BooksRef {
    /// The identity of the books.
    pub books: String,
}
