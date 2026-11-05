// SPDX-License-Identifier: AGPL-3.0-or-later
//! `spezado-tray`: what `spezado tray` does, as a program of its own that Windows starts without a console window –
//! the subsystem `windows` instead of `console`, without `unsafe` code. It is what starts at login on Windows. Errors
//! go to `spezado-tray.log` in the workspace directory, as there is no console to show them.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[path = "../autostart.rs"]
mod autostart;
#[path = "../start.rs"]
mod start;
#[path = "../tray.rs"]
mod tray;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use spezado_app::Workspace;

/// The server on this device with an icon in the system tray or the menu bar.
#[derive(Parser)]
#[command(name = "spezado-tray", version)]
struct Cli {
    /// The workspace directory; default: SPEZADO_DIR, else spezado in the data directory of the system.
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    /// The port on this device.
    #[arg(long, default_value_t = 8048)]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    let directory = start::directory(cli.dir.clone());
    if let Err(error) = directory.as_ref().map_err(|e| anyhow::anyhow!("{e:#}")).and_then(|d| run(&cli, d.clone())) {
        let log = directory.map(|d| d.join("spezado-tray.log"));
        let written = log
            .as_ref()
            .ok()
            .and_then(|path| std::fs::create_dir_all(path.parent()?).ok().map(|_| path))
            .is_some_and(|path| std::fs::write(path, format!("{error:#}\n")).is_ok());
        if !written {
            eprintln!("{error:#}");
        }
        std::process::exit(1);
    }
}

fn run(cli: &Cli, directory: PathBuf) -> Result<()> {
    start::ensure_workspace(&directory)?;
    let workspace = Workspace::open(&directory).with_context(|| directory.display().to_string())?;
    let pairing = start::pairing_code_given()?;
    let again = autostart::launcher(cli.dir.as_deref(), cli.port)?;
    tray::run(workspace, std::net::SocketAddr::from(([127, 0, 0, 1], cli.port)), pairing, again)
}
