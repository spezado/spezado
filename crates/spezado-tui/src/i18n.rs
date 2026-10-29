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

/// Every text of the interface in both languages; a text without both does not build.
macro_rules! texts {
    ($($key:ident => $de:literal, $en:literal;)*) => {
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
    Overview => "Übersicht", "Overview";
    Journal => "Journal", "Journal";
    Documents => "Belege", "Documents";
    Accounts => "Konten", "Accounts";
    Reports => "Berichte", "Reports";
    Review => "Klärung", "Review";
    Closing => "Abschluss", "Closing";
    // Keys
    KeyNew => "n neu", "n new";
    KeyLines => "z Zeilen", "z lines";
    KeyReverse => "r stornieren", "r reverse";
    KeyImport => "i Beleg einlesen", "i read document";
    KeyOpen => "↵ öffnen", "↵ open";
    KeyExport => "e exportieren", "e export";
    KeyClose => "c festschreiben", "c close period";
    KeyGobd => "g GoBD-Export", "g GoBD export";
    KeyEarlier => "[ Vorjahr", "[ previous year";
    KeyLater => "] Folgejahr", "] next year";
    KeyLanguage => "s English", "s Deutsch";
    KeyQuit => "q beenden", "q quit";
    KeyBack => "Esc zurück", "Esc back";
    KeyNext => "Tab weiter", "Tab next";
    KeyConfirm => "↵ übernehmen", "↵ confirm";
    KeyCancel => "Esc abbrechen", "Esc cancel";
    KeyMouseOff => "m Maus aus", "m mouse off";
    KeyMouseOn => "m Maus an", "m mouse on";
    KeyRecordOnly => "a nur ablegen", "a file only";
    KeyOpening => "o Anfangsbestände", "o opening balances";
    KeyDeadlines => "d Fristen", "d deadlines";
    KeySync => "y synchronisieren", "y synchronise";
    KeySyncStart => "y Synchronisation einrichten", "y set up synchronisation";
    KeyInvite => "v Gerät einladen", "v invite a device";
    Synchronisation => "Synchronisation", "Synchronisation";
    NotSynchronised => "nur auf diesem Gerät", "on this device only";
    Synchronised => "Synchronisiert", "Synchronised";
    Added => "hinzugefügt", "added";
    SyncStart => "Synchronisation einrichten", "Set up synchronisation";
    SyncHelp => "Über ein Relay kommen die Bücher auf deine anderen Geräte – Ende-zu-Ende verschlüsselt", "Through a relay the books reach your other devices – end-to-end encrypted";
    Relay => "Relay", "Relay";
    DeviceName => "Name dieses Geräts", "Name of this device";
    RecoveryCode => "Wiederherstellungscode", "Recovery code";
    RecoveryHelp => "Aufschreiben und sicher aufbewahren: Er holt die Bücher zurück, wenn alle Geräte verloren sind. Er wird nur jetzt gezeigt.", "Write it down and keep it safe: it brings the books back if every device is lost. It is shown only now.";
    Invitation => "Einladung", "Invitation";
    InvitationHelp => "Auf dem anderen Gerät: spezado sync join <Einladung> oder in der Web-App beim Einrichten. Danach hier y.", "On the other device: spezado sync join <invitation>, or in the web app when setting up. Then y here.";
    Deadlines => "Fristen ändern", "Change deadlines";
    ValidFrom => "Gilt ab", "Applies from";
    DeadlinesChanged => "Fristen geändert", "Deadlines changed";
    // Overview
    Bank => "Bank", "Bank";
    Cash => "Kasse", "Cash";
    Income => "Einnahmen", "Income";
    Expenses => "Ausgaben", "Expenses";
    Result => "Ergebnis", "Result";
    NextVatReturn => "Nächste Umsatzsteuer-Voranmeldung", "Next advance VAT return";
    Due => "fällig am", "due on";
    NoVatReturns => "Keine Voranmeldungen", "No advance VAT returns";
    OpenIssues => "Klärungspunkte", "Items to review";
    ClosedUntil => "Festgeschrieben bis", "Closed up to";
    NotClosed => "nichts festgeschrieben", "nothing closed";
    // Journal and booking
    Date => "Datum", "Date";
    Text_ => "Text", "Text";
    Account => "Konto", "Account";
    Amount => "Betrag", "Amount";
    Gross => "Brutto", "Gross";
    TaxCode => "Steuerschlüssel", "Tax code";
    NoTax => "ohne", "none";
    Payment => "bezahlt über", "paid via";
    Reversal => "Storno", "Reversal";
    Reversed => "storniert", "reversed";
    NewBooking => "Neue Buchung", "New booking";
    NewLines => "Neue Buchung in Zeilen", "New booking in lines";
    LinesHelp => "Zeilen: Konto=Betrag[@Schlüssel], getrennt durch Leerzeichen", "Lines: account=amount[@code], separated by spaces";
    BookedBefore => "schon gebucht:", "booked before:";
    KeyInvoice => "r Rechnung schreiben", "r write invoice";
    KeyCustomer => "k neuer Kunde", "k new customer";
    KeyProfile => "p Absender", "p seller";
    SellerDetails => "Absender auf Rechnungen", "Seller on invoices";
    NewCustomer => "Neuer Kunde", "New customer";
    NewInvoice => "Neue Rechnung", "New invoice";
    Street => "Straße und Nr.", "Street and number";
    Postcode => "PLZ", "Postcode";
    City => "Ort", "City";
    Country => "Land", "Country";
    VatId => "USt-IdNr.", "VAT ID";
    TaxNumber => "Steuernummer", "Tax number";
    Contact => "Ansprechpartner", "Contact";
    Phone => "Telefon", "Phone";
    Email => "E-Mail", "E-mail";
    BankIban => "IBAN", "IBAN";
    PaymentDays => "Zahlungsziel (Tage)", "Payment due (days)";
    Prefix => "Nummern beginnen mit", "Numbers start with";
    Reference => "Leitweg-ID / Referenz", "Leitweg-ID / reference";
    Customer => "Kunde", "Customer";
    Service => "Leistung", "Service";
    Quantity => "Menge", "Quantity";
    Unit => "Einheit", "Unit";
    Hours => "Stunden", "hours";
    Days => "Tage", "days";
    Pieces => "Stück", "pieces";
    UnitPrice => "Einzelpreis netto", "Net unit price";
    NoCustomers => "Erst einen Kunden anlegen (k)", "Add a customer first (k)";
    InvoiceWritten => "Rechnung geschrieben – sie liegt unter Belege", "Invoice written – it is under Documents";
    InvoiceFile => "Datei", "File";
    Saved => "Gespeichert", "Saved";
    OpeningBalances => "Anfangsbestände", "Opening balances";
    OpeningHelp => "Je Konto Konto=Betrag, getrennt durch Leerzeichen; Guthaben positiv, Schulden negativ; Bankkonten auch per IBAN", "Per account account=amount, separated by spaces; holdings positive, debts negative; bank accounts by IBAN, too";
    OpeningBooked => "Anfangsbestände gebucht", "Opening balances booked";
    ReverseBooking => "Buchung stornieren", "Reverse booking";
    Reason => "Grund", "Reason";
    Booked => "Gebucht", "Booked";
    NothingYet => "Noch nichts gebucht – n für eine neue Buchung", "Nothing booked yet – n for a new booking";
    // Bank
    KeyImportStatement => "i Auszug einlesen", "i read statement";
    KeyBook => "↵ buchen", "↵ book";
    KeyAcceptAll => "a alle Vorschläge buchen", "a book all proposals";
    KeyFetch => "f von der Bank abrufen", "f fetch from the bank";
    ReadStatement => "Kontoauszug einlesen (camt.053, MT940, CSV)", "Read bank statement (camt.053, MT940, CSV)";
    Iban => "IBAN (nur für CSV)", "IBAN (CSV only)";
    BookEntry => "Umsatz buchen", "Book entry";
    OpenEntries => "Offene Bankumsätze", "Open bank entries";
    NoOpenEntries => "Keine offenen Umsätze – i für einen Kontoauszug", "No open entries – i for a bank statement";
    Imported => "Neue Umsätze", "New entries";
    ShownBefore => "schon bekannt", "known before";
    Accepted => "Vorschläge gebucht", "Proposals booked";
    FromInvoice => "Rechnung", "Invoice";
    AsBefore => "wie zuletzt", "as before";
    // Documents
    ReadDocument => "Beleg einlesen", "Read document";
    Path => "Datei", "File";
    Direction => "Richtung", "Direction";
    Purchase => "Eingangsrechnung", "Purchase";
    Sale => "Ausgangsrechnung", "Sale";
    Proposal => "Buchungsvorschlag", "Booking proposal";
    NoProposal => "Kein Buchungsvorschlag – bitte von Hand buchen", "No booking proposal – please book by hand";
    NotAnInvoice => "Keine lesbare E-Rechnung", "Not a readable e-invoice";
    Filed => "Abgelegt", "Filed";
    Number => "Nummer", "Number";
    Kind => "Art", "Kind";
    File => "Datei", "File";
    NoDocuments => "Noch keine Belege – i zum Einlesen", "No documents yet – i to read one";
    // Accounts
    Balance => "Saldo", "Balance";
    // Reports
    VatReturn => "Umsatzsteuer-Voranmeldung", "Advance VAT return";
    Draft => "Entwurf – von Steuerberatern noch nicht geprüft", "Draft – not yet reviewed by tax advisers";
    Preview => "Vorschau – Zuordnungen sind Entwürfe ohne Prüfung durch Steuerberater, nicht für die echte Steuererklärung", "Preview – the mappings are drafts not reviewed by tax advisers, not for a real tax return";
    Field => "Kennzahl", "Field";
    Exported => "Exportiert nach", "Exported to";
    NoReports => "Keine Berichte für dieses Jahr", "No reports for this year";
    KeyPrepare => "p als Meldung vorbereiten", "p prepare as a filing";
    Filing => "Meldung", "Filing";
    FilingPrepared => "vorbereitet", "prepared";
    FilingSubmitted => "eingereicht", "submitted";
    FilingAccepted => "angenommen", "accepted";
    FilingRejected => "abgelehnt", "rejected";
    Correction => "Berichtigung", "correction";
    Outdated => "veraltet – neu vorbereiten", "outdated – prepare it again";
    SubmitFiling => "Meldung senden", "Submit filing";
    Submission => "Übermittlung", "Submission";
    AsTest => "als Test (die Behörde verarbeitet sie nicht)", "as a test (the authority does not process it)";
    ForReal => "echt – an die Behörde", "for real – to the authority";
    Test => "Test", "Test";
    Received => "erhalten", "received";
    // Review
    NothingToReview => "Nichts zu klären", "Nothing to review";
    // Closing
    ClosePeriod => "Zeitraum festschreiben", "Close period";
    CloseUpTo => "festschreiben bis", "close up to";
    Closed => "Festgeschrieben bis", "Closed up to";
    GobdExport => "GoBD-Datenüberlassung (Z3)", "GoBD data transfer (Z3)";
    Directory => "Ordner", "Directory";
    From => "von", "from";
    To => "bis", "to";
    // Set-up
    SetUp => "Bücher einrichten", "Set up books";
    Name => "Name", "Name";
    StartDate => "Beginn", "Start";
    Accounting => "Gewinnermittlung", "Profit determined by";
    CashBasis => "Einnahmenüberschussrechnung", "Cash basis";
    DoubleEntry => "Bilanzierung", "Double entry";
    VatScheme => "Umsatzsteuer", "VAT";
    Regular => "Regelbesteuerung", "Standard";
    SmallBusiness => "Kleinunternehmer (§ 19 UStG)", "Small business (§ 19 UStG)";
    VatBasis => "Besteuerung", "Taxed on";
    Accrual => "Soll (vereinbarte Entgelte)", "Accrual (agreed)";
    CashVat => "Ist (vereinnahmte Entgelte)", "Cash (received)";
    VatPeriod => "Voranmeldung", "Advance returns";
    Monthly => "monatlich", "monthly";
    Quarterly => "vierteljährlich", "quarterly";
    Yearly => "keine (nur Jahreserklärung)", "none (annual only)";
    Region => "Bundesland (Feiertage)", "State (public holidays)";
    NoRegion => "nicht angegeben", "not given";
    Extension => "Dauerfristverlängerung", "Permanent extension";
    No => "nein", "no";
    Yes => "ja", "yes";
    Chart => "Kontenrahmen", "Chart of accounts";
    Choose => "Bücher wählen", "Choose books";
    // Messages
    Invalid => "Ungültige Eingabe", "Invalid input";
    InvalidDate => "Datum ungültig", "Invalid date";
    InvalidAmount => "Betrag ungültig", "Invalid amount";
    UnknownAccount => "Konto unbekannt", "Unknown account";
    Error => "Fehler", "Error";
}
