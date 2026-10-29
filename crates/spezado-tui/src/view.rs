// SPDX-License-Identifier: AGPL-3.0-or-later
//! Drawing the interface – and remembering what was drawn where, so that a click can be told what it meant.

use librotenado::Money;
use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs},
};

use crate::{
    app::{App, Mode, Tab},
    form::{Form, Kind},
    i18n::Text,
};

/// Copper bronze, light – the accent of the brand on a dark terminal.
const ACCENT: Color = Color::Rgb(0xD5, 0x9A, 0x70);
/// Red for errors.
const ERROR: Color = Color::Rgb(0xE0, 0x6C, 0x6C);

fn accent() -> Style {
    Style::new().fg(ACCENT)
}

fn selected() -> Style {
    Style::new().fg(Color::Black).bg(ACCENT)
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// What a place on the screen stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A tab.
    Tab(Tab),
    /// A row of the tab shown.
    Row(usize),
    /// A key, as a button.
    Key(KeyCode),
    /// A field of the form shown.
    Field(usize),
    /// A choice of the form shown, to turn.
    Choice(usize),
    /// An account offered by the search.
    Match(usize),
}

/// A place on the screen and what it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// Where.
    pub area: Rect,
    /// What.
    pub target: Target,
}

/// What was drawn where, in the order drawn – what is drawn later lies on top.
#[derive(Debug, Default, Clone)]
pub struct Hits(Vec<Hit>);

impl Hits {
    fn push(&mut self, area: Rect, target: Target) {
        self.0.push(Hit { area, target });
    }

    /// What lies on top at a place.
    pub fn at(&self, column: u16, row: u16) -> Option<Target> {
        self.0.iter().rev().find(|hit| hit.area.contains(Position::new(column, row))).map(|hit| hit.target)
    }
}

/// Draws the whole interface; returns what was drawn where.
pub fn draw(frame: &mut Frame, app: &App) -> Hits {
    let mut hits = Hits::default();
    let [head, body, status, keys] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5), Constraint::Length(1), Constraint::Length(1)])
            .areas(frame.area());
    draw_head(frame, app, head, &mut hits);
    match &app.mode {
        Mode::Figures(figures) => draw_figures(frame, app, figures, body),
        Mode::Ledger(account) => draw_ledger(frame, app, account, body),
        _ => draw_tab(frame, app, body, &mut hits),
    }
    if let Some((error, message)) = &app.status {
        let style = if *error { Style::new().fg(ERROR) } else { accent() };
        frame.render_widget(Paragraph::new(message.as_str()).style(style), status);
    }
    draw_keys(frame, app, keys, &mut hits);
    match &app.mode {
        Mode::Form(_, form) => draw_form(frame, app, form, &mut hits),
        Mode::Proposal(proposal) => draw_proposal(frame, app, proposal, &mut hits),
        Mode::Notice(title, lines) => {
            let lines = lines.iter().map(|line| (Line::from(line.as_str()), None)).collect();
            draw_box(frame, format!(" {} ", app.lang.t(*title)), lines, 76, &mut hits);
        }
        _ => {}
    }
    hits
}

fn draw_head(frame: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let t = |text| app.lang.t(text);
    let mut title = format!(" spezado · {} · {}", app.view.owner_name, app.year);
    if let Some(chart) = &app.view.chart {
        title.push_str(&format!(" · {}", chart.replace("de-skr0", "SKR 0")));
    }
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let mut label = format!("{} {}", i + 1, t(tab.title()));
            if *tab == Tab::Review && !app.view.issues.is_empty() {
                label.push_str(&format!(" ({})", app.view.issues.len()));
            }
            Line::from(label)
        })
        .collect();
    // While a form is open, the tabs wait – dimmed, and not to be clicked.
    let open = matches!(app.mode, Mode::Form(..) | Mode::Proposal(_));
    // Each title stands between a space on either side, the titles between dividers of one column.
    let mut x = area.x + 1;
    for (tab, title) in Tab::ALL.iter().zip(&titles) {
        let width = title.width() as u16 + 2;
        if !open {
            hits.push(Rect::new(x, area.y + 1, width, 1), Target::Tab(*tab));
        }
        x += width + 1;
    }
    let highlight = if open { dim() } else { accent().add_modifier(Modifier::BOLD) };
    let tabs = Tabs::new(titles)
        .style(if open { dim() } else { Style::new() })
        .select(Tab::ALL.iter().position(|tab| *tab == app.tab))
        .highlight_style(highlight)
        .block(Block::new().borders(Borders::ALL).title(Span::styled(title + " ", accent())));
    frame.render_widget(tabs, area);
}

/// The keys of the mode and tab shown, each a button.
fn keys(app: &App) -> Vec<(Text, KeyCode)> {
    let char = KeyCode::Char;
    let mut keys = match (&app.mode, app.tab) {
        (Mode::Form(..), _) => {
            vec![(Text::KeyNext, KeyCode::Tab), (Text::KeyConfirm, KeyCode::Enter), (Text::KeyCancel, KeyCode::Esc)]
        }
        (Mode::Proposal(proposal), _) => {
            let mut keys = vec![(Text::KeyNext, KeyCode::Tab), (Text::KeyConfirm, KeyCode::Enter)];
            if proposal.document.is_some() {
                keys.push((Text::KeyRecordOnly, char('a')));
            }
            keys.push((Text::KeyCancel, KeyCode::Esc));
            keys
        }
        (Mode::Figures(_), _) => vec![(Text::KeyExport, char('e')), (Text::KeyBack, KeyCode::Esc)],
        (Mode::Ledger(_), _) | (Mode::Notice(..), _) => vec![(Text::KeyBack, KeyCode::Esc)],
        (Mode::Normal, Tab::Journal) => {
            vec![(Text::KeyNew, char('n')), (Text::KeyLines, char('z')), (Text::KeyReverse, char('r'))]
        }
        (Mode::Normal, Tab::Documents) => vec![
            (Text::KeyImport, char('i')),
            (Text::KeyNew, char('n')),
            (Text::KeyInvoice, char('r')),
            (Text::KeyCustomer, char('k')),
            (Text::KeyProfile, char('p')),
        ],
        (Mode::Normal, Tab::Bank) => vec![
            (Text::KeyImportStatement, char('i')),
            (Text::KeyBook, KeyCode::Enter),
            (Text::KeyAcceptAll, char('a')),
            (Text::KeyFetch, char('f')),
        ],
        (Mode::Normal, Tab::Overview) => {
            let mut keys =
                vec![(Text::KeyNew, char('n')), (Text::KeyImport, char('i')), (Text::KeyDeadlines, char('d'))];
            keys.push((if app.view.relay.is_some() { Text::KeySync } else { Text::KeySyncStart }, char('y')));
            if app.view.relay.is_some() {
                keys.push((Text::KeyInvite, char('v')));
            }
            keys
        }
        (Mode::Normal, Tab::Accounts) => vec![(Text::KeyOpen, KeyCode::Enter), (Text::KeyOpening, char('o'))],
        (Mode::Normal, Tab::Reports) => vec![(Text::KeyOpen, KeyCode::Enter)],
        (Mode::Normal, Tab::Closing) => vec![(Text::KeyClose, char('c')), (Text::KeyGobd, char('g'))],
        (Mode::Normal, _) => vec![(Text::KeyNew, char('n')), (Text::KeyImport, char('i'))],
    };
    if matches!(app.mode, Mode::Normal) {
        if !matches!(app.tab, Tab::Closing | Tab::Review | Tab::Bank) {
            keys.push((Text::KeyEarlier, char('[')));
            keys.push((Text::KeyLater, char(']')));
        }
        keys.push((Text::KeyLanguage, char('s')));
        keys.push((if app.mouse { Text::KeyMouseOff } else { Text::KeyMouseOn }, char('m')));
        keys.push((Text::KeyQuit, char('q')));
    }
    keys
}

fn draw_keys(frame: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let separator = "  ·  ";
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x + 1;
    for (i, (text, code)) in keys(app).into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(separator));
            x += separator.chars().count() as u16;
        }
        let span = Span::raw(app.lang.t(text));
        let width = span.width() as u16;
        hits.push(Rect::new(x, area.y, width, 1), Target::Key(code));
        x += width;
        spans.push(span);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(dim()), area);
}

fn money(app: &App, money: &Money) -> String {
    app.lang.amount(money)
}

/// The rows of a table or list, after drawing: where each visible one is.
fn row_hits(hits: &mut Hits, area: Rect, header: u16, offset: usize, len: usize) {
    let visible = area.height.saturating_sub(header) as usize;
    for (line, index) in (offset..len).take(visible).enumerate() {
        hits.push(Rect::new(area.x, area.y + header + line as u16, area.width, 1), Target::Row(index));
    }
}

fn draw_tab(frame: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let t = |text| app.lang.t(text);
    let block = Block::new().borders(Borders::ALL).title(format!(" {} ", t(app.tab.title())));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let selected_row = app.selected[Tab::ALL.iter().position(|tab| *tab == app.tab).expect("a tab")];
    match app.tab {
        Tab::Overview => draw_overview(frame, app, inner),
        Tab::Journal => draw_journal(frame, app, inner, selected_row, hits),
        Tab::Bank => {
            if app.view.open_entries.is_empty() {
                frame.render_widget(Paragraph::new(t(Text::NoOpenEntries)).style(dim()), inner);
                return;
            }
            let rows = app.view.open_entries.iter().map(|item| {
                let proposal = match item.suggestion.as_ref().map(|s| &s.source) {
                    Some(spezado_app::Source::Invoice { number, .. }) => {
                        format!("→ {} {}", t(Text::FromInvoice), number.as_deref().unwrap_or(""))
                    }
                    Some(spezado_app::Source::History(_)) => {
                        let booking = &item.suggestion.as_ref().expect("a suggestion").booking;
                        let counter = booking.postings.iter().find(|p| Some(&p.account) != item.account.as_ref());
                        let label = counter.map(|p| app.account_label(&p.account)).unwrap_or_default();
                        format!("→ {} {}", t(Text::AsBefore), label.trim())
                    }
                    Some(spezado_app::Source::Booked(_)) => {
                        let booking = &item.suggestion.as_ref().expect("a suggestion").booking;
                        format!("→ {} {}", t(Text::BookedBefore), booking.text)
                    }
                    None => String::new(),
                };
                Row::new(vec![
                    Cell::from(app.lang.date(item.entry.booked)),
                    Cell::from(item.text()),
                    Cell::from(Line::from(money(app, &item.entry.amount)).right_aligned()),
                    Cell::from(Span::styled(proposal, accent())),
                ])
            });
            let header =
                Row::new(vec![t(Text::Date), t(Text::Text_), t(Text::Amount), t(Text::Proposal)]).style(accent());
            let widths = [Constraint::Length(10), Constraint::Min(30), Constraint::Length(14), Constraint::Length(36)];
            let table = Table::new(rows, widths).header(header).row_highlight_style(selected());
            let mut state = TableState::new().with_selected(Some(selected_row));
            frame.render_stateful_widget(table, inner, &mut state);
            row_hits(hits, inner, 1, state.offset(), app.view.open_entries.len());
        }
        Tab::Documents => {
            if app.view.documents.is_empty() {
                frame.render_widget(Paragraph::new(t(Text::NoDocuments)).style(dim()), inner);
                return;
            }
            let rows = app.view.documents.iter().map(|(_, d)| {
                Row::new(vec![
                    app.lang.date(d.date),
                    d.number.clone().unwrap_or_default(),
                    d.original.format.clone().unwrap_or_else(|| d.original.media_type.clone()),
                    d.original.name.clone().unwrap_or_default(),
                ])
            });
            let header = Row::new(vec![t(Text::Date), t(Text::Number), t(Text::Kind), t(Text::File)]).style(accent());
            let widths = [Constraint::Length(10), Constraint::Length(16), Constraint::Length(20), Constraint::Min(10)];
            let table = Table::new(rows, widths).header(header).row_highlight_style(selected());
            let mut state = TableState::new().with_selected(Some(selected_row));
            frame.render_stateful_widget(table, inner, &mut state);
            row_hits(hits, inner, 1, state.offset(), app.view.documents.len());
        }
        Tab::Accounts => {
            let rows = app.view.balances.iter().map(|(_, number, name, balance)| {
                Row::new(vec![
                    Cell::from(number.as_str()),
                    Cell::from(name.as_str()),
                    Cell::from(Line::from(money(app, balance)).right_aligned()),
                ])
            });
            let header = Row::new(vec![t(Text::Number), t(Text::Account), t(Text::Balance)]).style(accent());
            let widths = [Constraint::Length(7), Constraint::Min(20), Constraint::Length(16)];
            let table = Table::new(rows, widths).header(header).row_highlight_style(selected());
            let mut state = TableState::new().with_selected(Some(selected_row));
            frame.render_stateful_widget(table, inner, &mut state);
            row_hits(hits, inner, 1, state.offset(), app.view.balances.len());
        }
        Tab::Reports => {
            if app.view.reports.is_empty() {
                frame.render_widget(Paragraph::new(t(Text::NoReports)).style(dim()), inner);
                return;
            }
            let items: Vec<ListItem> = app
                .view
                .reports
                .iter()
                .map(|r| {
                    let due =
                        r.due.map(|due| format!("  ({} {})", t(Text::Due), app.lang.date(due))).unwrap_or_default();
                    ListItem::new(format!("{}  {} – {}{due}", r.name, app.lang.date(r.from), app.lang.date(r.to)))
                })
                .collect();
            let list = List::new(items).highlight_style(selected());
            let mut state = ListState::default().with_selected(Some(selected_row));
            frame.render_stateful_widget(list, inner, &mut state);
            row_hits(hits, inner, 0, state.offset(), app.view.reports.len());
        }
        Tab::Review => {
            if app.view.issues.is_empty() {
                frame.render_widget(Paragraph::new(t(Text::NothingToReview)).style(accent()), inner);
                return;
            }
            let items: Vec<ListItem> = app.view.issues.iter().map(|issue| ListItem::new(issue.as_str())).collect();
            let list = List::new(items).highlight_style(selected());
            let mut state = ListState::default().with_selected(Some(selected_row));
            frame.render_stateful_widget(list, inner, &mut state);
            row_hits(hits, inner, 0, state.offset(), app.view.issues.len());
        }
        Tab::Closing => {
            let closed = match app.view.closed {
                Some(date) => format!("{} {}", t(Text::ClosedUntil), app.lang.date(date)),
                None => t(Text::NotClosed).to_owned(),
            };
            let lines = vec![
                Line::from(closed),
                Line::from(""),
                Line::from(Span::styled(t(Text::KeyClose), accent())),
                Line::from(Span::styled(t(Text::KeyGobd), accent())),
            ];
            for (line, key) in [(2, 'c'), (3, 'g')] {
                let width = lines[line].width() as u16;
                hits.push(Rect::new(inner.x, inner.y + line as u16, width, 1), Target::Key(KeyCode::Char(key)));
            }
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

fn draw_overview(frame: &mut Frame, app: &App, area: Rect) {
    let t = |text| app.lang.t(text);
    let mut lines = Vec::new();
    if let Some((bank, cash, income, expenses)) = &app.view.money {
        let row = |label: Text, amount: &Money| {
            Line::from(vec![
                Span::raw(format!("{:<14}", t(label))),
                Span::styled(format!("{:>16}", money(app, amount)), accent()),
            ])
        };
        lines.push(row(Text::Bank, bank));
        lines.push(row(Text::Cash, cash));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("{}", app.year), dim())));
        lines.push(row(Text::Income, income));
        lines.push(row(Text::Expenses, expenses));
        let result =
            Money::new(income.base_units() - expenses.base_units(), income.currency().clone()).expect("an amount");
        lines.push(row(Text::Result, &result));
        lines.push(Line::from(""));
    }
    match &app.view.next_return {
        Some(next) => lines.push(Line::from(format!(
            "{}: {} – {}, {} {}",
            t(Text::NextVatReturn),
            app.lang.date(next.from),
            app.lang.date(next.to),
            t(Text::Due),
            app.lang.date(next.due.expect("a due date")),
        ))),
        None => lines.push(Line::from(Span::styled(t(Text::NoVatReturns), dim()))),
    }
    lines.push(Line::from(format!("{}: {}", t(Text::OpenEntries), app.view.open_entries.len())));
    lines.push(Line::from(format!("{}: {}", t(Text::OpenIssues), app.view.issues.len())));
    let closed = app.view.closed.map_or_else(|| t(Text::NotClosed).to_owned(), |date| app.lang.date(date));
    lines.push(Line::from(format!("{}: {closed}", t(Text::ClosedUntil))));
    let relay = app.view.relay.as_deref().map_or_else(|| t(Text::NotSynchronised).to_owned(), str::to_owned);
    lines.push(Line::from(format!("{}: {relay}", t(Text::Synchronisation))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(t(Text::Preview), dim())));
    frame.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: true }), area);
}

fn draw_journal(frame: &mut Frame, app: &App, area: Rect, selected_row: usize, hits: &mut Hits) {
    let t = |text| app.lang.t(text);
    if app.view.journal.is_empty() {
        frame.render_widget(Paragraph::new(t(Text::NothingYet)).style(dim()), area);
        return;
    }
    let [list, detail] = Layout::vertical([Constraint::Min(3), Constraint::Length(8)]).areas(area);
    let rows = app.view.journal.iter().map(|entry| {
        let mark = match (entry.reverses, entry.reversed_by) {
            (Some(_), _) => t(Text::Reversal),
            (None, Some(_)) => t(Text::Reversed),
            (None, None) => "",
        };
        // The amount of a booking: what moved, the sum of its debits.
        let debit: i128 = entry.booking.postings.iter().map(|p| p.amount.base_units().max(0)).sum();
        let currency = entry.booking.postings[0].amount.currency().clone();
        let amount = Money::new(debit, currency).expect("an amount");
        Row::new(vec![
            Cell::from(app.lang.date(entry.booking.date)),
            Cell::from(entry.booking.text.as_str()),
            Cell::from(Span::styled(mark, dim())),
            Cell::from(Line::from(money(app, &amount)).right_aligned()),
        ])
    });
    let header = Row::new(vec![t(Text::Date), t(Text::Text_), "", t(Text::Amount)]).style(accent());
    let widths = [Constraint::Length(10), Constraint::Min(20), Constraint::Length(10), Constraint::Length(14)];
    let table = Table::new(rows, widths).header(header).row_highlight_style(selected());
    let mut state = TableState::new().with_selected(Some(selected_row));
    frame.render_stateful_widget(table, list, &mut state);
    row_hits(hits, list, 1, state.offset(), app.view.journal.len());
    if let Some(entry) = app.view.journal.get(selected_row) {
        let mut lines: Vec<Line> = entry
            .booking
            .postings
            .iter()
            .map(|p| {
                let tax = p.tax.as_ref().map(|code| format!("  {code}")).unwrap_or_default();
                Line::from(format!("{:<50} {:>14}{tax}", app.account_label(&p.account), money(app, &p.amount)))
            })
            .collect();
        for document in &entry.booking.documents {
            let label = format!("{}: {}", t(Text::Documents), app.document_label(document));
            lines.push(Line::from(Span::styled(label, dim())));
        }
        frame.render_widget(Paragraph::new(lines).block(Block::new().borders(Borders::TOP)), detail);
    }
}

fn draw_figures(frame: &mut Frame, app: &App, figures: &crate::app::Figures, area: Rect) {
    let t = |text| app.lang.t(text);
    let block = Block::new().borders(Borders::ALL).title(format!(" {} ", figures.title));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [note, table_area] = Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(inner);
    let draft = format!("{} · {}", t(Text::Draft), figures.law);
    frame.render_widget(Paragraph::new(draft).style(Style::new().fg(ERROR)), note);
    let rows = figures.rows.iter().map(|(field, name, amount)| {
        Row::new(vec![
            Cell::from(field.as_str()),
            Cell::from(name.as_str()),
            Cell::from(Line::from(money(app, amount)).right_aligned()),
        ])
    });
    let header = Row::new(vec![t(Text::Field), "", t(Text::Amount)]).style(accent());
    let widths = [Constraint::Length(8), Constraint::Min(20), Constraint::Length(16)];
    frame.render_widget(Table::new(rows, widths).header(header), table_area);
}

fn draw_ledger(frame: &mut Frame, app: &App, account: &librotenado::AccountId, area: Rect) {
    let t = |text| app.lang.t(text);
    let block = Block::new().borders(Borders::ALL).title(format!(" {} ", app.account_label(account).trim()));
    let rows = app.ledger(account).into_iter().map(|(date, text, amount, balance)| {
        Row::new(vec![
            Cell::from(app.lang.date(date)),
            Cell::from(text),
            Cell::from(Line::from(money(app, &amount)).right_aligned()),
            Cell::from(Line::from(money(app, &balance)).right_aligned()),
        ])
    });
    let header = Row::new(vec![t(Text::Date), t(Text::Text_), t(Text::Amount), t(Text::Balance)]).style(accent());
    let widths = [Constraint::Length(10), Constraint::Min(20), Constraint::Length(14), Constraint::Length(14)];
    frame.render_widget(Table::new(rows, widths).header(header).block(block), area);
}

/// A rectangle in the middle of `area`.
fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

/// The lines of a form, each with what clicking it means.
fn form_lines<'a>(app: &App, form: &'a Form) -> Vec<(Line<'a>, Option<Target>)> {
    let t = |text| app.lang.t(text);
    let mut lines = Vec::new();
    for (i, field) in form.fields.iter().enumerate() {
        let focus = i == form.focus;
        let (value, target) = match &field.kind {
            Kind::Choice(options) => {
                let shown = options.get(field.chosen).map_or("", |(shown, _)| shown.as_str());
                (format!("‹ {shown} ›"), Target::Choice(i))
            }
            Kind::Account => {
                let name = crate::form::resolve(&field.value, &app.view.accounts)
                    .map(|a| format!("  {}", a.name))
                    .unwrap_or_default();
                (format!("{}{}{name}", field.value, if focus { "▏" } else { "" }), Target::Field(i))
            }
            Kind::Text => (format!("{}{}", field.value, if focus { "▏" } else { "" }), Target::Field(i)),
        };
        let label = Span::styled(format!("{:<18}", t(field.label)), if focus { accent() } else { dim() });
        lines.push((Line::from(vec![label, Span::raw(value)]), Some(target)));
        if focus && !form.matches.is_empty() {
            for (j, index) in form.matches.iter().enumerate() {
                let account = &app.view.accounts[*index];
                let text = format!("{:<18}{:>6} {}", "", account.number.as_deref().unwrap_or(""), account.name);
                let style = if j == form.selected { selected() } else { Style::new() };
                lines.push((Line::from(Span::styled(text, style)), Some(Target::Match(j))));
            }
        }
    }
    if let Some(help) = form.help {
        lines.push((Line::from(""), None));
        lines.push((Line::from(Span::styled(t(help), dim())), None));
    }
    lines
}

/// Draws lines in a framed box in the middle of the screen; remembers where the clickable ones are.
fn draw_box(frame: &mut Frame, title: String, lines: Vec<(Line, Option<Target>)>, width: u16, hits: &mut Hits) {
    let area = centred(frame.area(), width, lines.len() as u16 + 2);
    frame.render_widget(Clear, area);
    // The whole box catches clicks, so that nothing behind it is hit.
    hits.push(area, Target::Key(KeyCode::Null));
    for (i, (_, target)) in lines.iter().enumerate() {
        if let Some(target) = target
            && (i as u16) < area.height.saturating_sub(2)
        {
            hits.push(Rect::new(area.x + 1, area.y + 1 + i as u16, area.width.saturating_sub(2), 1), *target);
        }
    }
    let block = Block::new().borders(Borders::ALL).border_style(accent()).title(title);
    // Long names are cut, not wrapped, so that every field stays inside the frame.
    let lines: Vec<Line> = lines.into_iter().map(|(line, _)| line).collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_form(frame: &mut Frame, app: &App, form: &Form, hits: &mut Hits) {
    let title = format!(" {} ", app.lang.t(form.title));
    draw_box(frame, title, form_lines(app, form), 76, hits);
}

fn draw_proposal(frame: &mut Frame, app: &App, proposal: &crate::app::Proposal, hits: &mut Hits) {
    let mut lines = form_lines(app, &proposal.form);
    lines.push((Line::from(""), None));
    for posting in &proposal.booking.postings {
        let tax = posting.tax.as_ref().map(|code| format!("  {code}")).unwrap_or_default();
        let text = format!("{:<46} {:>14}{tax}", app.account_label(&posting.account), money(app, &posting.amount));
        lines.push((Line::from(text), None));
    }
    let title = format!(
        " {} · {} ",
        app.lang.t(Text::Proposal),
        proposal.document.as_ref().and_then(|d| d.number.as_deref()).unwrap_or("")
    );
    draw_box(frame, title, lines, 84, hits);
}
