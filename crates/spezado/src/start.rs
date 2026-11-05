// SPDX-License-Identifier: AGPL-3.0-or-later
//! Starting from nothing, shared by `spezado` and `spezado-tray`: the workspace directory, a workspace for this device
//! if there is none, and the pairing code an app may give.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use librotenado::PartyId;
use spezado_app::{Workspace, new_id};

/// The workspace directory: the one given, else `SPEZADO_DIR`, else `spezado` in the data directory of the system.
pub fn directory(given: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match given {
        Some(directory) => directory,
        None => match std::env::var_os("SPEZADO_DIR") {
            Some(directory) => directory.into(),
            None => dirs::data_dir().context("no data directory; give --dir")?.join("spezado"),
        },
    })
}

/// A workspace for this device in `directory`, if there is none yet.
pub fn ensure_workspace(directory: &Path) -> Result<()> {
    if !directory.join("spezado.sqlite").exists() {
        let device = format!("device-{}", uuid_suffix()).parse().expect("a valid device name");
        Workspace::create(directory, device)?;
    }
    Ok(())
}

/// A pairing code given in `SPEZADO_PAIRING_CODE` – by an app that starts `spezado serve` and pairs itself; at least
/// 16 letters, digits, `-` or `_`. In the environment rather than on the command line, where others could read it.
pub fn pairing_code_given() -> Result<Option<String>> {
    let Some(code) = std::env::var_os("SPEZADO_PAIRING_CODE") else { return Ok(None) };
    let code = code
        .into_string()
        .ok()
        .filter(|code| code.len() >= 16 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    code.map(Some).context("SPEZADO_PAIRING_CODE: at least 16 letters, digits, - or _")
}

/// Eight random hexadecimal digits, for a device name.
pub fn uuid_suffix() -> String {
    let id: PartyId = new_id();
    id.as_str()[id.as_str().len() - 8..].to_owned()
}
