// SPDX-License-Identifier: AGPL-3.0-or-later
//! Starting spezado when the person logs in: a launch agent on macOS, the Run key of the registry on Windows, an
//! XDG autostart entry on Linux. What starts is this program with the arguments given – `tray` where there is a
//! menu bar, else `serve`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The name of the entry, the same on every platform.
const NAME: &str = "app.spezado";

/// Whether spezado starts at login.
pub fn enabled() -> Result<bool> {
    platform::enabled()
}

/// Starts `program` with `arguments` at login from now on.
pub fn enable(program: &Path, arguments: &[String]) -> Result<()> {
    platform::enable(program, arguments)
}

/// Stops starting spezado at login; nothing happens if it did not.
pub fn disable() -> Result<()> {
    platform::disable()
}

/// The arguments that start spezado at login: `tray` where this build has a menu bar, else `serve`; with the
/// workspace directory if one was given.
pub fn arguments(directory: Option<&Path>, port: u16) -> Vec<String> {
    let mut arguments = Vec::new();
    if let Some(directory) = directory {
        arguments.extend(["--dir".to_owned(), directory.display().to_string()]);
    }
    let command = if cfg!(feature = "tray") { "tray" } else { "serve" };
    arguments.extend([command.to_owned(), "--port".to_owned(), port.to_string()]);
    arguments
}

#[cfg(not(target_os = "windows"))]
fn home() -> Result<PathBuf> {
    dirs::home_dir().context("no home directory")
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    fn agent() -> Result<PathBuf> {
        Ok(home()?.join("Library/LaunchAgents").join(format!("{NAME}.plist")))
    }

    fn xml(text: &str) -> String {
        text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn enabled() -> Result<bool> {
        Ok(agent()?.exists())
    }

    pub fn enable(program: &Path, arguments: &[String]) -> Result<()> {
        let mut words = vec![program.display().to_string()];
        words.extend(arguments.iter().cloned());
        let words: String = words.iter().map(|word| format!("\n    <string>{}</string>", xml(word))).collect();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{NAME}</string>
  <key>ProgramArguments</key>
  <array>{words}
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
</dict>
</plist>
"#
        );
        let path = agent()?;
        std::fs::create_dir_all(path.parent().expect("LaunchAgents"))?;
        std::fs::write(&path, plist).with_context(|| path.display().to_string())
    }

    pub fn disable() -> Result<()> {
        let path = agent()?;
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| path.display().to_string())?;
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::process::Command;

    const RUN: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

    fn reg(arguments: &[&str]) -> Result<bool> {
        Ok(Command::new("reg").args(arguments).output().context("reg.exe")?.status.success())
    }

    pub fn enabled() -> Result<bool> {
        reg(&["query", RUN, "/v", NAME])
    }

    pub fn enable(program: &Path, arguments: &[String]) -> Result<()> {
        let mut line = format!("\"{}\"", program.display());
        for argument in arguments {
            line.push_str(&format!(" \"{argument}\""));
        }
        anyhow::ensure!(reg(&["add", RUN, "/v", NAME, "/t", "REG_SZ", "/d", &line, "/f"])?, "reg add {RUN}");
        Ok(())
    }

    pub fn disable() -> Result<()> {
        if enabled()? {
            anyhow::ensure!(reg(&["delete", RUN, "/v", NAME, "/f"])?, "reg delete {RUN}");
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::*;

    fn entry() -> Result<PathBuf> {
        let config = dirs::config_dir().map(Ok).unwrap_or_else(|| home().map(|home| home.join(".config")))?;
        Ok(config.join("autostart").join(format!("{NAME}.desktop")))
    }

    /// A word of an Exec line, quoted after the Desktop Entry Specification.
    fn quote(word: &str) -> String {
        let escaped = word.replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$");
        format!("\"{escaped}\"")
    }

    pub fn enabled() -> Result<bool> {
        Ok(entry()?.exists())
    }

    pub fn enable(program: &Path, arguments: &[String]) -> Result<()> {
        let mut exec = quote(&program.display().to_string());
        for argument in arguments {
            exec.push(' ');
            exec.push_str(&quote(argument));
        }
        let desktop = format!(
            "[Desktop Entry]\nType=Application\nName=spezado\nComment=Free accounting for everyone\nExec={exec}\n\
             Terminal=false\nX-GNOME-Autostart-enabled=true\n"
        );
        let path = entry()?;
        std::fs::create_dir_all(path.parent().expect("autostart"))?;
        std::fs::write(&path, desktop).with_context(|| path.display().to_string())
    }

    pub fn disable() -> Result<()> {
        let path = entry()?;
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| path.display().to_string())?;
        }
        Ok(())
    }
}
