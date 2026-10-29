// SPDX-License-Identifier: AGPL-3.0-or-later
//! Forms: fields to type into or choose from, and the search for an account by number or name.

use librotenado::AccountKind;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spezado_app::AccountChoice;

use crate::i18n::Text;

/// How a field is filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Free text.
    Text,
    /// An account, searched by number or name.
    Account,
    /// One of a list: (what is shown, what it stands for).
    Choice(Vec<(String, String)>),
}

/// A field of a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// What it is called.
    pub label: Text,
    /// How it is filled.
    pub kind: Kind,
    /// What was typed; for a choice, the index of the option.
    pub value: String,
    /// For a choice: the option chosen.
    pub chosen: usize,
    /// Whether someone changed it – a default may follow other fields until then.
    pub touched: bool,
}

impl Field {
    /// A text field with a value to start with.
    pub fn text(label: Text, value: impl Into<String>) -> Self {
        Field { label, kind: Kind::Text, value: value.into(), chosen: 0, touched: false }
    }

    /// An account field.
    pub fn account(label: Text, value: impl Into<String>) -> Self {
        Field { label, kind: Kind::Account, value: value.into(), chosen: 0, touched: false }
    }

    /// A choice, starting at `chosen`.
    pub fn choice(label: Text, options: Vec<(String, String)>, chosen: usize) -> Self {
        Field { label, kind: Kind::Choice(options), value: String::new(), chosen, touched: false }
    }

    /// What the field stands for: the text typed, or the value of the option chosen.
    pub fn get(&self) -> &str {
        match &self.kind {
            Kind::Choice(options) => options.get(self.chosen).map_or("", |(_, value)| value.as_str()),
            _ => &self.value,
        }
    }

    /// Chooses the option standing for `value`, if there is one.
    pub fn choose(&mut self, value: &str) {
        if let Kind::Choice(options) = &self.kind
            && let Some(index) = options.iter().position(|(_, v)| v == value)
        {
            self.chosen = index;
        }
    }
}

/// What a key did to a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It is still being filled.
    Editing,
    /// Confirmed.
    Submit,
    /// Given up.
    Cancel,
    /// An account was taken into the field with this index.
    AccountTaken(usize),
}

/// A form: fields, the one in focus, and – in an account field – the accounts matching what was typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// Its title.
    pub title: Text,
    /// Its fields.
    pub fields: Vec<Field>,
    /// The field in focus.
    pub focus: usize,
    /// The matching accounts, by index into the list searched, and the one selected.
    pub matches: Vec<usize>,
    /// The match selected.
    pub selected: usize,
    /// A line of help below the fields.
    pub help: Option<Text>,
}

/// At most this many accounts are offered.
const MATCHES: usize = 8;

impl Form {
    /// A form of fields.
    pub fn new(title: Text, fields: Vec<Field>) -> Self {
        Form { title, fields, focus: 0, matches: Vec::new(), selected: 0, help: None }
    }

    /// The field labelled `label`.
    pub fn field(&self, label: Text) -> &Field {
        self.fields.iter().find(|field| field.label == label).expect("a field of the form")
    }

    /// The field labelled `label`, to change.
    pub fn field_mut(&mut self, label: Text) -> &mut Field {
        self.fields.iter_mut().find(|field| field.label == label).expect("a field of the form")
    }

    /// Handles a key; `accounts` are searched in an account field.
    pub fn handle(&mut self, key: KeyEvent, accounts: &[AccountChoice]) -> Outcome {
        let picking = !self.matches.is_empty();
        match key.code {
            KeyCode::Esc if picking => self.matches.clear(),
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Up if picking => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down if picking => self.selected = (self.selected + 1).min(self.matches.len() - 1),
            KeyCode::Enter if picking => {
                let account = &accounts[self.matches[self.selected]];
                let field = &mut self.fields[self.focus];
                field.value = account.number.clone().unwrap_or_else(|| account.id.to_string());
                field.touched = true;
                self.matches.clear();
                return Outcome::AccountTaken(self.focus);
            }
            KeyCode::Enter if self.focus + 1 == self.fields.len() => return Outcome::Submit,
            KeyCode::Enter | KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(-1),
            KeyCode::Left | KeyCode::Right => {
                let field = &mut self.fields[self.focus];
                if let Kind::Choice(options) = &field.kind {
                    let n = options.len();
                    field.chosen =
                        if key.code == KeyCode::Left { (field.chosen + n - 1) % n } else { (field.chosen + 1) % n };
                    field.touched = true;
                }
            }
            KeyCode::Backspace => {
                let field = &mut self.fields[self.focus];
                if field.kind != Kind::Text && field.kind != Kind::Account {
                    return Outcome::Editing;
                }
                field.value.pop();
                field.touched = true;
                self.search(accounts);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let field = &mut self.fields[self.focus];
                if matches!(field.kind, Kind::Choice(_)) {
                    return Outcome::Editing;
                }
                field.value.push(c);
                field.touched = true;
                self.search(accounts);
            }
            _ => {}
        }
        Outcome::Editing
    }

    fn move_focus(&mut self, by: isize) {
        self.matches.clear();
        let n = self.fields.len() as isize;
        self.focus = ((self.focus as isize + by).rem_euclid(n)) as usize;
    }

    /// Finds the accounts matching what was typed in an account field: by the start of the number, else by a part of
    /// the name, case aside – names that start with it first, then accounts of income and expense, which are what
    /// one books on.
    fn search(&mut self, accounts: &[AccountChoice]) {
        self.matches.clear();
        self.selected = 0;
        let field = &self.fields[self.focus];
        let typed = field.value.trim().to_lowercase();
        if field.kind != Kind::Account || typed.is_empty() {
            return;
        }
        let by_number = |a: &AccountChoice| a.number.as_deref().is_some_and(|n| n.starts_with(&typed));
        let by_name = |a: &AccountChoice| a.name.to_lowercase().contains(&typed);
        let numeric = typed.chars().all(|c| c.is_ascii_digit());
        let mut found: Vec<usize> = accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| if numeric { by_number(a) } else { by_name(a) })
            .map(|(i, _)| i)
            .collect();
        if !numeric {
            let rank = |a: &AccountChoice| {
                let profit_and_loss = matches!(a.kind, AccountKind::Income | AccountKind::Expense);
                (!a.name.to_lowercase().starts_with(&typed), !profit_and_loss)
            };
            found.sort_by_key(|i| rank(&accounts[*i]));
        }
        found.truncate(MATCHES);
        self.matches = found;
        // An exact number needs no list.
        if self.matches.len() == 1 && accounts[self.matches[0]].number.as_deref() == Some(typed.as_str()) {
            self.matches.clear();
        }
    }
}

/// The account a field names: by its number, its identity, or a name that only one account has.
pub fn resolve<'a>(value: &str, accounts: &'a [AccountChoice]) -> Option<&'a AccountChoice> {
    let value = value.trim();
    // A bank account opened for an IBAN is named after it.
    let iban: String = value.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let by_iban = |a: &&AccountChoice| iban.len() >= 15 && a.number.is_none() && a.name.ends_with(&format!(" {iban}"));
    accounts
        .iter()
        .find(|a| a.number.as_deref() == Some(value) || a.id.as_str() == value)
        .or_else(|| accounts.iter().find(by_iban))
        .or_else(|| {
            let lower = value.to_lowercase();
            let mut named = accounts.iter().filter(|a| a.name.to_lowercase() == lower);
            match (named.next(), named.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        })
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;

    use super::*;

    fn accounts() -> Vec<AccountChoice> {
        [("4920", "Telefon"), ("4925", "Internetkosten"), ("4930", "Bürobedarf"), ("1200", "Bank")]
            .iter()
            .enumerate()
            .map(|(i, (number, name))| AccountChoice {
                id: format!("00000000-0000-7000-8000-{i:012x}").parse().unwrap(),
                number: Some((*number).into()),
                name: (*name).into(),
                kind: AccountKind::Expense,
            })
            .collect()
    }

    fn type_in(form: &mut Form, text: &str, accounts: &[AccountChoice]) {
        for c in text.chars() {
            form.handle(KeyEvent::from(KeyCode::Char(c)), accounts);
        }
    }

    #[test]
    fn accounts_are_found_by_number_or_name() {
        let accounts = accounts();
        let mut form =
            Form::new(Text::NewBooking, vec![Field::account(Text::Account, ""), Field::text(Text::Gross, "")]);
        type_in(&mut form, "492", &accounts);
        assert_eq!(form.matches, [0, 1]);
        form.handle(KeyEvent::from(KeyCode::Down), &accounts);
        assert_eq!(form.handle(KeyEvent::from(KeyCode::Enter), &accounts), Outcome::AccountTaken(0));
        assert_eq!(form.fields[0].value, "4925");
        form.fields[0].value.clear();
        type_in(&mut form, "büro", &accounts);
        assert_eq!(form.matches, [2]);
        form.handle(KeyEvent::from(KeyCode::Esc), &accounts);
        assert!(form.matches.is_empty());
        assert_eq!(resolve("4930", &accounts).unwrap().name, "Bürobedarf");
        assert_eq!(resolve("bank", &accounts).unwrap().number.as_deref(), Some("1200"));
        assert!(resolve("49", &accounts).is_none());
    }

    #[test]
    fn choices_turn_and_the_last_field_submits() {
        let accounts = accounts();
        let options = vec![("a".to_owned(), "1".to_owned()), ("b".to_owned(), "2".to_owned())];
        let mut form =
            Form::new(Text::SetUp, vec![Field::text(Text::Name, "Ada"), Field::choice(Text::Chart, options, 0)]);
        assert_eq!(form.handle(KeyEvent::from(KeyCode::Enter), &accounts), Outcome::Editing);
        form.handle(KeyEvent::from(KeyCode::Left), &accounts);
        assert_eq!(form.field(Text::Chart).get(), "2");
        assert_eq!(form.handle(KeyEvent::from(KeyCode::Enter), &accounts), Outcome::Submit);
        assert_eq!(form.handle(KeyEvent::from(KeyCode::Esc), &accounts), Outcome::Cancel);
    }
}
