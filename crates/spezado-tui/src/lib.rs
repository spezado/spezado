// SPDX-License-Identifier: AGPL-3.0-or-later
//! The terminal interface of spezado: a freelancer's year – setting up books, booking from gross amounts, reading
//! e-invoices into booking proposals, balances, the advance VAT returns and the Anlage EÜR, the review list, closing
//! periods and the data transfer of the GoBD – in German and in English. It works through the workspace of
//! `spezado-app` only, like every other interface.

use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
};
use spezado_app::Workspace;

mod app;
mod form;
mod i18n;
mod view;

pub use app::App;
pub use i18n::Lang;

/// Why the interface ended with an error.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    /// The terminal.
    #[error("terminal: {0}")]
    Terminal(#[from] std::io::Error),
}

/// The language to speak: the one asked for, else the workspace's, else the system's.
pub fn language(workspace: &Workspace, asked: Option<&str>) -> Lang {
    let system = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).unwrap_or_default();
    let code = asked.map(str::to_owned).or_else(|| workspace.language().ok().flatten()).unwrap_or(system);
    Lang::from_code(&code)
}

/// Runs the terminal interface on a workspace until it is left.
pub fn run(workspace: &mut Workspace, lang: Lang) -> Result<(), TuiError> {
    let mut app = App::new(workspace, lang);
    let mut terminal = ratatui::try_init()?;
    let mut mouse = false;
    let result = (|| -> Result<(), TuiError> {
        while !app.quit {
            if app.mouse != mouse {
                mouse = app.mouse;
                if mouse {
                    execute!(std::io::stdout(), EnableMouseCapture)?;
                } else {
                    execute!(std::io::stdout(), DisableMouseCapture)?;
                }
            }
            let mut hits = view::Hits::default();
            terminal.draw(|frame| hits = view::draw(frame, &app))?;
            app.hits = hits;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
        Ok(())
    })();
    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests;
