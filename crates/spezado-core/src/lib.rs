// SPDX-License-Identifier: AGPL-3.0-or-later
//! The core of spezado: a set of books built from librotenado events, the commands that add to it and the queries
//! that read it.
//!
//! The core is pure – no I/O, no clock, no randomness. A command is checked against the books and becomes the
//! payload of one event; stamping it (time, device, chain) and storing it is the application's job. The events are
//! the truth: everything here is derived from them and can be derived again. Whatever does not fit is not guessed
//! away but put on the review list.

mod amount;
mod books;
mod command;

pub use amount::{AmountError, format_amount, format_amount_in, format_date_in, parse_amount, split_gross};
pub use books::{Books, Issue, JournalEntry};
pub use command::{Command, CommandError};
