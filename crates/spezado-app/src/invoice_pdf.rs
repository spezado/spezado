// SPDX-License-Identifier: AGPL-3.0-or-later
//! An invoice as ZUGFeRD: a PDF/A-3b for people – set in Domitian, the font of spezado's brand, embedded – with the
//! invoice in CII attached for machines (librotenado's `en16931::attach_to_pdf`, profile XRECHNUNG). What the page
//! shows is drawn from the same invoice data as the XML, in German.

use krilla::Document;
use krilla::color::rgb;
use krilla::configure::{Archival, ConfigurationBuilder, PdfVersion};
use krilla::geom::{PathBuilder, Point};
use krilla::metadata::{DateTime, Metadata};
use krilla::page::PageSettings;
use krilla::paint::Stroke;
use krilla::surface::Surface;
use krilla::text::{Font, TextDirection};
use librotenado::{Decimal, InvoiceData, Money, VatCategory, en16931};
use spezado_core::{format_amount_in, format_date_in};

use crate::AppError;

const ROMAN: &[u8] = include_bytes!("../../../brand/fonts/Domitian-Roman.otf");
const BOLD: &[u8] = include_bytes!("../../../brand/fonts/Domitian-Bold.otf");

/// A4 in points, and the margins: 20 mm left (DIN 5008), 18 mm right.
const WIDTH: f32 = 595.28;
const HEIGHT: f32 = 841.89;
const LEFT: f32 = 56.7;
const RIGHT: f32 = WIDTH - 51.0;
/// Where the content of a page ends, above the footer.
const BOTTOM: f32 = HEIGHT - 96.0;

/// Ink and copper of the brand.
fn ink() -> rgb::Color {
    rgb::Color::new(0x0A, 0x2C, 0x33)
}

fn copper() -> rgb::Color {
    rgb::Color::new(0x98, 0x5A, 0x34)
}

fn muted() -> rgb::Color {
    rgb::Color::new(0x4E, 0x62, 0x66)
}

/// The fonts, with what is needed to measure text.
struct Fonts {
    roman: Font,
    bold: Font,
}

impl Fonts {
    fn load() -> Result<Self, AppError> {
        let font = |bytes: &'static [u8]| {
            Font::new(bytes.to_vec().into(), 0).ok_or_else(|| AppError::Invoice("the font cannot be read".into()))
        };
        Ok(Fonts { roman: font(ROMAN)?, bold: font(BOLD)? })
    }
}

/// The width of a text in points.
fn width(bytes: &[u8], text: &str, size: f32) -> f32 {
    let Some(face) = rustybuzz::Face::from_slice(bytes, 0) else { return 0.0 };
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let units: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
    units as f32 * size / f32::from(face.units_per_em() as u16)
}

/// The pages being drawn: text left or right aligned, lines, and a new page when one is full.
struct Pages<'a> {
    document: Document,
    fonts: &'a Fonts,
    pages: Vec<Vec<Op>>,
    y: f32,
}

/// One thing drawn on a page.
enum Op {
    Text { x: f32, y: f32, bold: bool, size: f32, colour: rgb::Color, text: String },
    Rule { y: f32, colour: rgb::Color, width: f32 },
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

impl<'a> Pages<'a> {
    fn new(document: Document, fonts: &'a Fonts) -> Self {
        Pages { document, fonts, pages: vec![Vec::new()], y: 60.0 }
    }

    fn bytes(bold: bool) -> &'static [u8] {
        if bold { BOLD } else { ROMAN }
    }

    /// Text at `x` – its left or right end – on the current line.
    fn text(&mut self, x: f32, align: Align, text: &str, size: f32, bold: bool, colour: rgb::Color) {
        let x = match align {
            Align::Left => x,
            Align::Right => x - width(Self::bytes(bold), text, size),
        };
        let y = self.y;
        self.pages.last_mut().expect("a page").push(Op::Text { x, y, bold, size, colour, text: text.to_owned() });
    }

    /// A horizontal rule across the content.
    fn rule(&mut self, colour: rgb::Color, width: f32) {
        let y = self.y;
        self.pages.last_mut().expect("a page").push(Op::Rule { y, colour, width });
    }

    /// Moves down; a new page when the content would run into the footer.
    fn down(&mut self, by: f32) {
        self.y += by;
        if self.y > BOTTOM {
            self.pages.push(Vec::new());
            self.y = 60.0;
        }
    }

    /// Starts a new page unless `height` points still fit on this one; whether it did.
    fn keep(&mut self, height: f32) -> bool {
        if self.y + height <= BOTTOM {
            return false;
        }
        self.pages.push(Vec::new());
        self.y = 60.0;
        true
    }

    /// Lines of at most `room` points, broken between words.
    fn wrap(text: &str, room: f32, size: f32, bold: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && width(Self::bytes(bold), &candidate, size) > room {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// A paragraph, wrapped to the content's width.
    fn paragraph(&mut self, text: &str, size: f32, colour: rgb::Color) {
        for line in Self::wrap(text, RIGHT - LEFT, size, false) {
            self.text(LEFT, Align::Left, &line, size, false, colour);
            self.down(size * 1.45);
        }
    }

    /// Draws every page with the footer and returns the PDF/A-3b.
    fn finish(mut self, footer: &[String]) -> Result<Vec<u8>, AppError> {
        let count = self.pages.len();
        for (index, ops) in std::mem::take(&mut self.pages).into_iter().enumerate() {
            let mut page = self.document.start_page_with(PageSettings::from_wh(WIDTH, HEIGHT).expect("the size of A4"));
            let mut surface = page.surface();
            for op in ops {
                draw(&mut surface, self.fonts, &op);
            }
            let mut y = HEIGHT - 62.0;
            draw(&mut surface, self.fonts, &Op::Rule { y: y - 12.0, colour: copper(), width: 0.6 });
            for line in footer {
                let op = Op::Text { x: LEFT, y, bold: false, size: 7.5, colour: muted(), text: line.clone() };
                draw(&mut surface, self.fonts, &op);
                y += 10.5;
            }
            if count > 1 {
                let text = format!("Seite {} von {count}", index + 1);
                let x = RIGHT - width(ROMAN, &text, 7.5);
                let op = Op::Text { x, y: HEIGHT - 62.0, bold: false, size: 7.5, colour: muted(), text };
                draw(&mut surface, self.fonts, &op);
            }
            surface.finish();
            page.finish();
        }
        self.document.finish().map_err(|e| AppError::Invoice(format!("the PDF: {e:?}")))
    }
}

fn draw(surface: &mut Surface<'_>, fonts: &Fonts, op: &Op) {
    match op {
        Op::Text { x, y, bold, size, colour, text } => {
            surface.set_fill(Some(krilla::paint::Fill { paint: (*colour).into(), ..Default::default() }));
            let font = if *bold { fonts.bold.clone() } else { fonts.roman.clone() };
            surface.draw_text(Point::from_xy(*x, *y), font, *size, text, false, TextDirection::Auto);
        }
        Op::Rule { y, colour, width } => {
            let mut path = PathBuilder::new();
            path.move_to(LEFT, *y);
            path.line_to(RIGHT, *y);
            let Some(path) = path.finish() else { return };
            surface.set_fill(None);
            surface.set_stroke(Some(Stroke { paint: (*colour).into(), width: *width, ..Default::default() }));
            surface.draw_path(&path);
            surface.set_stroke(None);
        }
    }
}

/// An amount in German: `1.234,50 EUR`.
fn amount(money: &Money) -> String {
    format!("{} {}", format_amount_in(money, "de"), money.currency())
}

/// A decimal in German, with at least `places` decimal places: `2,5`, `95,00`.
fn decimal(value: Decimal, places: usize) -> String {
    let text = value.to_string();
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let fraction = format!("{fraction:0<places$}");
    if fraction.is_empty() { whole.to_owned() } else { format!("{whole},{fraction}") }
}

/// The name of a unit of UN/ECE Recommendation 20.
fn unit(code: &str) -> &str {
    match code {
        "HUR" => "Std.",
        "DAY" => "Tag(e)",
        "C62" | "H87" => "Stk.",
        "MON" => "Monat(e)",
        "LS" => "pauschal",
        other => other,
    }
}

/// The name of a VAT category and rate on the page.
fn vat(category: VatCategory, rate: Option<Decimal>) -> String {
    match (category, rate) {
        (VatCategory::Standard, Some(rate)) => format!("{} %", decimal(rate, 0)),
        (VatCategory::ZeroRated, _) => "0 %".into(),
        (VatCategory::ReverseCharge, _) => "§ 13b".into(),
        (VatCategory::IntraCommunity, _) => "ig. L.".into(),
        (VatCategory::Export, _) => "Ausfuhr".into(),
        _ => "frei".into(),
    }
}

/// Writes an invoice as ZUGFeRD: a PDF/A-3b showing it, with its CII attached.
pub(crate) fn zugferd(invoice: &en16931::TranslatedInvoice, cii: &str) -> Result<Vec<u8>, AppError> {
    let pdf = render(invoice)?;
    en16931::attach_to_pdf(&pdf, cii).map_err(|e| AppError::Invoice(e.to_string()))
}

/// The invoice as a PDF/A-3b, without the XML.
fn render(invoice: &en16931::TranslatedInvoice) -> Result<Vec<u8>, AppError> {
    let data: &InvoiceData = &invoice.invoice;
    let configuration = ConfigurationBuilder::new()
        .with_archival_validator(Archival::A3_B)
        .with_version(PdfVersion::Pdf17)
        .finish()
        .map_err(|e| AppError::Invoice(format!("the PDF: {e:?}")))?;
    let settings = krilla::SerializeSettings { configuration, ..Default::default() };
    let mut document = Document::new_with(settings);
    let credit = invoice.kind == librotenado::DocumentKind::CreditNote;
    let title = if credit { "Gutschrift" } else { "Rechnung" };
    document.set_metadata(
        Metadata::new()
            .title(format!("{title} {}", invoice.number))
            .authors(vec![data.seller.name.clone()])
            .language("de".into())
            .creator("spezado".into())
            // The invoice date, so that the same invoice is always the same file.
            .creation_date(DateTime::new(invoice.date.year()).month(invoice.date.month()).day(invoice.date.day())),
    );
    let fonts = Fonts::load()?;
    let mut pages = Pages::new(document, &fonts);
    let address = |party: &librotenado::InvoiceParty| -> Vec<String> {
        let mut lines = vec![party.name.clone()];
        if let Some(address) = &party.address {
            lines.extend(address.lines.iter().cloned());
            let city = [address.postcode.as_deref(), address.city.as_deref()].into_iter().flatten();
            lines.push(city.collect::<Vec<_>>().join(" "));
            if address.country.as_str() != "DE" {
                lines.push(address.country.to_string());
            }
        }
        lines
    };

    // The seller, and its address in one line above the window of the envelope.
    pages.text(LEFT, Align::Left, &data.seller.name, 17.0, true, ink());
    pages.down(18.0);
    let sender = address(&data.seller).join(" · ");
    pages.text(LEFT, Align::Left, &sender, 8.5, false, muted());
    pages.down(8.0);
    pages.rule(copper(), 0.8);

    // The buyer in the window (DIN 5008: 45 mm from the top), the invoice's details beside it.
    pages.y = 140.0;
    let top = pages.y;
    for line in address(&data.buyer) {
        pages.text(LEFT, Align::Left, &line, 10.5, false, ink());
        pages.down(14.0);
    }
    let below_address = pages.y;
    pages.y = top;
    let mut details =
        vec![(format!("{title}snummer"), invoice.number.clone()), ("Datum".into(), format_date_in(invoice.date, "de"))];
    if let Some(day) = data.delivery_date {
        details.push(("Leistungsdatum".into(), format_date_in(day, "de")));
    }
    if let Some(day) = data.due_date {
        details.push(("Fällig am".into(), format_date_in(day, "de")));
    }
    if let Some(reference) = data.buyer_reference.as_ref().filter(|r| **r != invoice.number) {
        details.push(("Ihre Referenz".into(), reference.clone()));
    }
    if let Some(vat_id) = &data.buyer.vat_id {
        details.push(("Ihre USt-IdNr.".into(), vat_id.clone()));
    }
    for (label, value) in &details {
        pages.text(370.0, Align::Left, label, 9.0, false, muted());
        pages.text(RIGHT, Align::Right, value, 9.0, false, ink());
        pages.down(13.0);
    }
    pages.y = below_address.max(pages.y) + 34.0;

    pages.text(LEFT, Align::Left, &format!("{title} {}", invoice.number), 15.0, true, ink());
    pages.down(26.0);
    for note in &data.notes {
        pages.paragraph(&note.text, 10.0, ink());
    }
    pages.down(6.0);

    // The lines.
    let columns = [(LEFT, "Pos."), (LEFT + 30.0, "Leistung")];
    let numbers = [(355.0, "Menge"), (430.0, "Einzelpreis"), (470.0, "USt"), (RIGHT, "Netto")];
    let header = |pages: &mut Pages<'_>| {
        for (x, label) in columns {
            pages.text(x, Align::Left, label, 8.5, true, muted());
        }
        for (x, label) in numbers {
            pages.text(x, Align::Right, label, 8.5, true, muted());
        }
        pages.down(5.0);
        pages.rule(muted(), 0.4);
        pages.down(14.0);
    };
    header(&mut pages);
    for (i, line) in data.lines.iter().enumerate() {
        let id = line.id.clone().unwrap_or_else(|| (i + 1).to_string());
        let mut text = vec![line.name.clone()];
        text.extend(line.description.iter().cloned());
        let wrapped: Vec<String> = text.iter().flat_map(|t| Pages::wrap(t, 225.0, 10.0, false)).collect();
        // A line stays on one page.
        if pages.keep(13.0 * wrapped.len().saturating_sub(1) as f32 + 17.0) {
            header(&mut pages);
        }
        pages.text(LEFT, Align::Left, &id, 10.0, false, ink());
        let quantity = match (line.quantity, line.unit.as_deref()) {
            (Some(q), Some(u)) => format!("{} {}", decimal(q, 0), unit(u)),
            (Some(q), None) => decimal(q, 0),
            _ => String::new(),
        };
        pages.text(355.0, Align::Right, &quantity, 10.0, false, ink());
        if let Some(price) = line.price {
            pages.text(430.0, Align::Right, &decimal(price, 2), 10.0, false, ink());
        }
        pages.text(470.0, Align::Right, &vat(line.category, line.rate), 10.0, false, ink());
        pages.text(RIGHT, Align::Right, &format_amount_in(&line.net, "de"), 10.0, false, ink());
        for (n, part) in wrapped.iter().enumerate() {
            if n > 0 {
                pages.down(13.0);
            }
            pages.text(LEFT + 30.0, Align::Left, part, 10.0, false, ink());
        }
        pages.down(17.0);
    }
    pages.down(-8.0);
    pages.rule(muted(), 0.4);
    pages.down(18.0);

    // The totals, with the VAT per category and rate.
    let total = |pages: &mut Pages<'_>, label: &str, value: &Money, bold: bool| {
        pages.text(330.0, Align::Left, label, 10.0, bold, ink());
        pages.text(RIGHT, Align::Right, &amount(value), 10.0, bold, ink());
        pages.down(15.0);
    };
    pages.keep(15.0 * (data.taxes.len() as f32 + 2.0));
    total(&mut pages, "Summe netto", &data.totals.net, false);
    for tax in &data.taxes {
        let label = format!("USt {} auf {}", vat(tax.category, tax.rate), amount(&tax.base));
        let label = if width(ROMAN, &label, 10.0) > RIGHT - 330.0 - 80.0 {
            format!("USt {}", vat(tax.category, tax.rate))
        } else {
            label
        };
        total(&mut pages, &label, &tax.tax, false);
    }
    let due_label = if data.totals.due == data.totals.gross { "Gesamtbetrag" } else { "Zu zahlen" };
    total(&mut pages, due_label, &data.totals.due, true);
    pages.down(10.0);

    // Why amounts carry no VAT, how to pay.
    let mut statements: Vec<String> = data.taxes.iter().filter_map(|t| t.exemption.clone()).collect();
    statements.dedup();
    for statement in statements {
        pages.paragraph(&statement, 10.0, ink());
    }
    if let Some(terms) = &data.payment_terms {
        pages.paragraph(terms, 10.0, ink());
    }
    if let Some(payment) = &data.payment {
        for transfer in &payment.transfers {
            let mut text = format!("Bitte überweisen Sie auf das Konto IBAN {}", group(&transfer.account));
            if let Some(name) = &transfer.name {
                text = format!("{text} ({name})");
            }
            if let Some(remittance) = &payment.remittance {
                text = format!("{text}, Verwendungszweck {remittance}");
            }
            pages.paragraph(&format!("{text}."), 10.0, ink());
        }
    }
    pages.down(6.0);
    pages.paragraph(
        "Diese Rechnung ist eine elektronische Rechnung (ZUGFeRD, Profil XRECHNUNG): die Rechnungsdaten liegen als \
         XML-Datei xrechnung.xml in diesem PDF.",
        8.5,
        muted(),
    );

    // The footer: the seller's contact and tax numbers.
    let seller = &data.seller;
    let mut footer = vec![address(seller).join(" · ")];
    if let Some(contact) = &seller.contact {
        let parts: Vec<&str> = [contact.name.as_deref(), contact.phone.as_deref(), contact.email.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        footer.push(parts.join(" · "));
    }
    let mut taxes = Vec::new();
    if let Some(vat_id) = &seller.vat_id {
        taxes.push(format!("USt-IdNr. {vat_id}"));
    }
    if let Some(number) = &seller.tax_number {
        taxes.push(format!("Steuernummer {number}"));
    }
    if !taxes.is_empty() {
        footer.push(taxes.join(" · "));
    }
    pages.finish(&footer)
}

/// An IBAN in groups of four.
fn group(iban: &str) -> String {
    let compact: Vec<char> = iban.chars().filter(|c| !c.is_whitespace()).collect();
    compact.chunks(4).map(|chunk| chunk.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}
