// SPDX-License-Identifier: AGPL-3.0-or-later
//! What the terminal interface says, in German and in English, and how it writes and reads amounts and dates.

use librotenado::{Currency, Date, Money};
use spezado_core::{AmountError, format_amount_in, format_date_in, parse_amount};

/// The language the interface speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// German.
    De,
    /// English.
    En,
}

impl Lang {
    /// From a code (`de`, `en`) or a locale (`de_DE.UTF-8`); English for anything else.
    pub fn from_code(code: &str) -> Self {
        if code.to_ascii_lowercase().starts_with("de") { Lang::De } else { Lang::En }
    }

    /// The code: `de`, `en`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }

    /// The other language.
    pub fn other(self) -> Self {
        match self {
            Lang::De => Lang::En,
            Lang::En => Lang::De,
        }
    }

    /// An amount as people of the language write it: `1.234,56` or `1,234.56`.
    pub fn amount(self, money: &Money) -> String {
        format_amount_in(money, self.code())
    }

    /// Reads an amount as people of the language write it – with or without separators of thousands.
    pub fn parse_amount(self, text: &str, currency: &Currency) -> Result<Money, AmountError> {
        let text = text.trim().replace([' ', '\u{a0}'], "");
        let plain = match self {
            Lang::De if text.contains(',') => text.replace('.', "").replace(',', "."),
            Lang::De => text,
            Lang::En => text.replace(',', ""),
        };
        parse_amount(&plain, currency)
    }

    /// A date as people of the language write it: `29.10.2026` or `2026-10-29`.
    pub fn date(self, date: Date) -> String {
        format_date_in(date, self.code())
    }

    /// Reads a date: `2026-10-29` always, `29.10.2026` and `29.10.` (in `year`) as well.
    pub fn parse_date(self, text: &str, year: u16) -> Option<Date> {
        let text = text.trim();
        if let Ok(date) = text.parse() {
            return Some(date);
        }
        let mut parts = text.split('.');
        let day = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let year = match parts.next() {
            None | Some("") => year,
            Some(year) => year.parse().ok()?,
        };
        Date::new(year, month, day)
    }

    /// A text in this language.
    pub fn t(self, text: Text) -> &'static str {
        text.get(self)
    }
}

/// Every text of the interface: in English, the source, then in German; a text without both does not build.
macro_rules! texts {
    ($($key:ident => $en:literal, $de:literal;)*) => {
        /// A text of the interface.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Text {
            $(
                #[allow(missing_docs)]
                $key,
            )*
        }

        impl Text {
            /// The text in a language.
            pub fn get(self, lang: Lang) -> &'static str {
                match (self, lang) {
                    $((Text::$key, Lang::De) => $de, (Text::$key, Lang::En) => $en,)*
                }
            }
        }
    };
}

texts! {
    // Tabs
    Overview => "Overview", "Übersicht";
    Journal => "Journal", "Journal";
    Documents => "Documents", "Belege";
    Accounts => "Accounts", "Konten";
    Reports => "Reports", "Berichte";
    Review => "Review", "Klärung";
    Closing => "Closing", "Abschluss";
    // Keys
    KeyNew => "n new", "n neu";
    KeyLines => "z lines", "z Zeilen";
    KeyReverse => "r reverse", "r stornieren";
    KeyImport => "i read document", "i Beleg einlesen";
    KeyOpen => "↵ open", "↵ öffnen";
    KeyExport => "e export", "e exportieren";
    KeyClose => "c close period", "c festschreiben";
    KeyGobd => "g GoBD export", "g GoBD-Export";
    KeyEarlier => "[ previous year", "[ Vorjahr";
    KeyLater => "] next year", "] Folgejahr";
    KeyLanguage => "s Deutsch", "s English";
    KeyQuit => "q quit", "q beenden";
    KeyBack => "Esc back", "Esc zurück";
    KeyNext => "Tab next", "Tab weiter";
    KeyConfirm => "↵ confirm", "↵ übernehmen";
    KeyCancel => "Esc cancel", "Esc abbrechen";
    KeyMouseOff => "m mouse off", "m Maus aus";
    KeyMouseOn => "m mouse on", "m Maus an";
    KeyRecordOnly => "a file only", "a nur ablegen";
    KeyOpening => "o opening balances", "o Anfangsbestände";
    KeyDeadlines => "d deadlines", "d Fristen";
    KeySync => "y synchronise", "y synchronisieren";
    KeySyncStart => "y set up synchronisation", "y Synchronisation einrichten";
    KeyInvite => "v invite a device", "v Gerät einladen";
    Synchronisation => "Synchronisation", "Synchronisation";
    NotSynchronised => "on this device only", "nur auf diesem Gerät";
    Synchronised => "Synchronised", "Synchronisiert";
    Added => "added", "hinzugefügt";
    SyncStart => "Set up synchronisation", "Synchronisation einrichten";
    SyncHelp => "Through a relay the books reach your other devices – end-to-end encrypted", "Über ein Relay kommen die Bücher auf deine anderen Geräte – Ende-zu-Ende verschlüsselt";
    Relay => "Relay", "Relay";
    DeviceName => "Name of this device", "Name dieses Geräts";
    RecoveryCode => "Recovery code", "Wiederherstellungscode";
    RecoveryHelp => "Write it down and keep it safe: it brings the books back if every device is lost. It is shown only now.", "Aufschreiben und sicher aufbewahren: Er holt die Bücher zurück, wenn alle Geräte verloren sind. Er wird nur jetzt gezeigt.";
    Invitation => "Invitation", "Einladung";
    InvitationHelp => "On the other device: spezado sync join <invitation>, or in the web app when setting up. Then y here.", "Auf dem anderen Gerät: spezado sync join <Einladung> oder in der Web-App beim Einrichten. Danach hier y.";
    Deadlines => "Change deadlines", "Fristen ändern";
    ValidFrom => "Applies from", "Gilt ab";
    DeadlinesChanged => "Deadlines changed", "Fristen geändert";
    // Overview
    Bank => "Bank", "Bank";
    Cash => "Cash", "Kasse";
    Income => "Income", "Einnahmen";
    Expenses => "Expenses", "Ausgaben";
    Result => "Result", "Ergebnis";
    NextVatReturn => "Next advance VAT return", "Nächste Umsatzsteuer-Voranmeldung";
    Due => "due on", "fällig am";
    NoVatReturns => "No advance VAT returns", "Keine Voranmeldungen";
    OpenIssues => "Items to review", "Klärungspunkte";
    ClosedUntil => "Closed up to", "Festgeschrieben bis";
    NotClosed => "nothing closed", "nichts festgeschrieben";
    // Journal and booking
    Date => "Date", "Datum";
    Text_ => "Text", "Text";
    Account => "Account", "Konto";
    Amount => "Amount", "Betrag";
    Gross => "Gross", "Brutto";
    TaxCode => "Tax code", "Steuerschlüssel";
    NoTax => "none", "ohne";
    Payment => "paid via", "bezahlt über";
    Reversal => "Reversal", "Storno";
    Reversed => "reversed", "storniert";
    NewBooking => "New booking", "Neue Buchung";
    NewLines => "New booking in lines", "Neue Buchung in Zeilen";
    LinesHelp => "Lines: account=amount[@code], separated by spaces", "Zeilen: Konto=Betrag[@Schlüssel], getrennt durch Leerzeichen";
    BookedBefore => "booked before:", "schon gebucht:";
    KeyInvoice => "r write invoice", "r Rechnung schreiben";
    KeyCustomer => "k new customer", "k neuer Kunde";
    KeyProfile => "p seller", "p Absender";
    SellerDetails => "Seller on invoices", "Absender auf Rechnungen";
    NewCustomer => "New customer", "Neuer Kunde";
    NewInvoice => "New invoice", "Neue Rechnung";
    Street => "Street and number", "Straße und Nr.";
    Postcode => "Postcode", "PLZ";
    City => "City", "Ort";
    Country => "Country", "Land";
    VatId => "VAT ID", "USt-IdNr.";
    TaxNumber => "Tax number", "Steuernummer";
    Contact => "Contact", "Ansprechpartner";
    Phone => "Phone", "Telefon";
    Email => "E-mail", "E-Mail";
    BankIban => "IBAN", "IBAN";
    PaymentDays => "Payment due (days)", "Zahlungsziel (Tage)";
    Prefix => "Numbers start with", "Nummern beginnen mit";
    Reference => "Leitweg-ID / reference", "Leitweg-ID / Referenz";
    Customer => "Customer", "Kunde";
    Service => "Service", "Leistung";
    Quantity => "Quantity", "Menge";
    Unit => "Unit", "Einheit";
    Hours => "hours", "Stunden";
    Days => "days", "Tage";
    Pieces => "pieces", "Stück";
    UnitPrice => "Net unit price", "Einzelpreis netto";
    NoCustomers => "Add a customer first (k)", "Erst einen Kunden anlegen (k)";
    InvoiceWritten => "Invoice written – it is under Documents", "Rechnung geschrieben – sie liegt unter Belege";
    InvoiceFile => "File", "Datei";
    Saved => "Saved", "Gespeichert";
    OpeningBalances => "Opening balances", "Anfangsbestände";
    OpeningHelp => "Per account account=amount, separated by spaces; holdings positive, debts negative; bank accounts by IBAN, too", "Je Konto Konto=Betrag, getrennt durch Leerzeichen; Guthaben positiv, Schulden negativ; Bankkonten auch per IBAN";
    OpeningBooked => "Opening balances booked", "Anfangsbestände gebucht";
    ReverseBooking => "Reverse booking", "Buchung stornieren";
    Reason => "Reason", "Grund";
    Booked => "Booked", "Gebucht";
    NothingYet => "Nothing booked yet – n for a new booking", "Noch nichts gebucht – n für eine neue Buchung";
    // Bank
    KeyImportStatement => "i read statement", "i Auszug einlesen";
    KeyBook => "↵ book", "↵ buchen";
    KeyAcceptAll => "a book all proposals", "a alle Vorschläge buchen";
    KeyFetch => "f fetch from the bank", "f von der Bank abrufen";
    ReadStatement => "Read bank statement (camt.053, MT940, CSV)", "Kontoauszug einlesen (camt.053, MT940, CSV)";
    Iban => "IBAN (CSV only)", "IBAN (nur für CSV)";
    BookEntry => "Book entry", "Umsatz buchen";
    OpenEntries => "Open bank entries", "Offene Bankumsätze";
    NoOpenEntries => "No open entries – i for a bank statement", "Keine offenen Umsätze – i für einen Kontoauszug";
    Imported => "New entries", "Neue Umsätze";
    ShownBefore => "known before", "schon bekannt";
    Accepted => "Proposals booked", "Vorschläge gebucht";
    FromInvoice => "Invoice", "Rechnung";
    AsBefore => "as before", "wie zuletzt";
    // Documents
    ReadDocument => "Read document", "Beleg einlesen";
    Path => "File", "Datei";
    Direction => "Direction", "Richtung";
    Purchase => "Purchase", "Eingangsrechnung";
    Sale => "Sale", "Ausgangsrechnung";
    Proposal => "Booking proposal", "Buchungsvorschlag";
    NoProposal => "No booking proposal – please book by hand", "Kein Buchungsvorschlag – bitte von Hand buchen";
    NotAnInvoice => "Not a readable e-invoice", "Keine lesbare E-Rechnung";
    Filed => "Filed", "Abgelegt";
    Number => "Number", "Nummer";
    Kind => "Kind", "Art";
    File => "File", "Datei";
    NoDocuments => "No documents yet – i to read one", "Noch keine Belege – i zum Einlesen";
    // Accounts
    Balance => "Balance", "Saldo";
    // Reports
    VatReturn => "Advance VAT return", "Umsatzsteuer-Voranmeldung";
    Draft => "Draft – not yet reviewed by tax advisers", "Entwurf – von Steuerberatern noch nicht geprüft";
    Preview => "Preview – the mappings are drafts not reviewed by tax advisers, not for a real tax return", "Vorschau – Zuordnungen sind Entwürfe ohne Prüfung durch Steuerberater, nicht für die echte Steuererklärung";
    Field => "Field", "Kennzahl";
    Exported => "Exported to", "Exportiert nach";
    NoReports => "No reports for this year", "Keine Berichte für dieses Jahr";
    KeyPrepare => "p prepare as a filing", "p als Meldung vorbereiten";
    Filing => "Filing", "Meldung";
    FilingPrepared => "prepared", "vorbereitet";
    FilingSubmitted => "submitted", "eingereicht";
    FilingAccepted => "accepted", "angenommen";
    FilingRejected => "rejected", "abgelehnt";
    Correction => "correction", "Berichtigung";
    Outdated => "outdated – prepare it again", "veraltet – neu vorbereiten";
    SubmitFiling => "Submit filing", "Meldung senden";
    Submission => "Submission", "Übermittlung";
    AsTest => "as a test (the authority does not process it)", "als Test (die Behörde verarbeitet sie nicht)";
    ForReal => "for real – to the authority", "echt – an die Behörde";
    Test => "Test", "Test";
    Received => "received", "erhalten";
    // Review
    NothingToReview => "Nothing to review", "Nichts zu klären";
    // Closing
    ClosePeriod => "Close period", "Zeitraum festschreiben";
    CloseUpTo => "close up to", "festschreiben bis";
    Closed => "Closed up to", "Festgeschrieben bis";
    GobdExport => "GoBD data transfer (Z3)", "GoBD-Datenüberlassung (Z3)";
    Directory => "Directory", "Ordner";
    From => "from", "von";
    To => "to", "bis";
    // Set-up
    SetUp => "Set up books", "Bücher einrichten";
    Name => "Name", "Name";
    StartDate => "Start", "Beginn";
    Accounting => "Profit determined by", "Gewinnermittlung";
    CashBasis => "Cash basis", "Einnahmenüberschussrechnung";
    DoubleEntry => "Double entry", "Bilanzierung";
    VatScheme => "VAT", "Umsatzsteuer";
    Regular => "Standard", "Regelbesteuerung";
    SmallBusiness => "Small business (§ 19 UStG)", "Kleinunternehmer (§ 19 UStG)";
    VatBasis => "Taxed on", "Besteuerung";
    Accrual => "Accrual (agreed)", "Soll (vereinbarte Entgelte)";
    CashVat => "Cash (received)", "Ist (vereinnahmte Entgelte)";
    VatPeriod => "Advance returns", "Voranmeldung";
    Monthly => "monthly", "monatlich";
    Quarterly => "quarterly", "vierteljährlich";
    Yearly => "none (annual only)", "keine (nur Jahreserklärung)";
    Region => "State (public holidays)", "Bundesland (Feiertage)";
    NoRegion => "not given", "nicht angegeben";
    Extension => "Permanent extension", "Dauerfristverlängerung";
    No => "no", "nein";
    Yes => "yes", "ja";
    Chart => "Chart of accounts", "Kontenrahmen";
    Choose => "Choose books", "Bücher wählen";
    // Messages
    Invalid => "Invalid input", "Ungültige Eingabe";
    InvalidDate => "Invalid date", "Datum ungültig";
    InvalidAmount => "Invalid amount", "Betrag ungültig";
    UnknownAccount => "Unknown account", "Konto unbekannt";
    Error => "Error", "Fehler";
}
