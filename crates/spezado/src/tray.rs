// SPDX-License-Identifier: AGPL-3.0-or-later
//! `spezado tray`: the server on this device with an icon in the menu bar (macOS) or the system tray (Windows,
//! Linux) – to open the web app, pair a device, start at login and quit. The server runs on a thread of its own; the
//! icon needs the main thread's event loop.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use spezado_api::Pairing;
use spezado_app::Workspace;
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::autostart;

/// What the server and the menu tell the event loop.
enum Message {
    Ready { address: SocketAddr, pairing: Pairing },
    Failed(String),
    Stopped,
    Menu(MenuEvent),
}

/// Runs the server and the icon until "Quit" is chosen. `again` are the arguments that start this again at login.
pub fn run(workspace: Workspace, listen: SocketAddr, pairing: Option<String>, again: Vec<String>) -> Result<()> {
    let program = std::env::current_exe().context("the path of this program")?;
    let event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    // On macOS an accessory: the icon in the menu bar, nothing in the Dock.
    #[cfg(target_os = "macos")]
    let event_loop = {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        let mut event_loop = event_loop;
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop
    };
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(Message::Menu(event));
    }));

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let proxy = event_loop.create_proxy();
    let server = std::thread::spawn(move || {
        let host = gethostname::gethostname().to_string_lossy().into_owned();
        let ready = proxy.clone();
        let result = spezado_api::serve(
            workspace,
            listen,
            &host,
            pairing,
            move |serving| {
                let address = serving.link.trim_start_matches("http://").split('/').next().unwrap_or_default();
                let address = address.parse().unwrap_or(listen);
                let _ = ready.send_event(Message::Ready { address, pairing: serving.pairing.clone() });
            },
            // "Quit" in the menu, Ctrl-C, or the system ending the session.
            async {
                tokio::select! {
                    _ = stopped => {}
                    _ = terminated() => {}
                }
            },
        );
        let _ = proxy.send_event(match result {
            Ok(()) => Message::Stopped,
            Err(error) => Message::Failed(error.to_string()),
        });
    });

    let status = MenuItem::new("spezado – starting …", false, None);
    let open = MenuItem::new("Open spezado", false, None);
    let pair = MenuItem::new("Pair a device …", false, None);
    let code = MenuItem::new("", false, None);
    let login = CheckMenuItem::new("Start at login", true, autostart::enabled().unwrap_or(false), None);
    let quit = MenuItem::new("Quit spezado", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &PredefinedMenuItem::separator(),
        &open,
        &pair,
        &code,
        &PredefinedMenuItem::separator(),
        &login,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let mut tray: Option<TrayIcon> = None;
    let mut serving: Option<(SocketAddr, Pairing)> = None;
    let mut stop = Some(stop);
    let mut server = Some(server);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // The icon is made once the event loop runs – on macOS it needs the application.
            Event::NewEvents(StartCause::Init) => match icon() {
                Ok(image) => {
                    let builder = TrayIconBuilder::new().with_menu(Box::new(menu.clone())).with_tooltip("spezado");
                    // On macOS a template, which the menu bar colours itself.
                    #[cfg(target_os = "macos")]
                    let builder = builder.with_icon_templated(image);
                    #[cfg(not(target_os = "macos"))]
                    let builder = builder.with_icon(image);
                    tray = builder.build().map_err(|error| eprintln!("spezado tray: {error}")).ok();
                }
                Err(error) => eprintln!("spezado tray: {error}"),
            },
            Event::UserEvent(Message::Ready { address, pairing }) => {
                status.set_text(format!("spezado at {address}"));
                open.set_enabled(true);
                pair.set_enabled(true);
                serving = Some((address, pairing));
            }
            Event::UserEvent(Message::Failed(error)) => {
                status.set_text(format!("spezado could not start: {error}"));
                eprintln!("spezado tray: {error}");
                open.set_enabled(false);
                pair.set_enabled(false);
            }
            Event::UserEvent(Message::Stopped) => *control_flow = ControlFlow::Exit,
            Event::UserEvent(Message::Menu(event)) => {
                if event.id == open.id() {
                    if let Some((address, _)) = &serving {
                        browse(&format!("http://{address}/"));
                    }
                } else if event.id == pair.id() {
                    if let Some((address, pairing)) = &serving {
                        let new = pairing.new_code();
                        code.set_text(format!("Code {new} – valid for 15 minutes"));
                        browse(&format!("http://{address}/#pair={new}"));
                    }
                } else if event.id == login.id() {
                    let wanted = login.is_checked();
                    let done = if wanted { autostart::enable(&program, &again) } else { autostart::disable() };
                    if let Err(error) = done {
                        eprintln!("spezado tray: {error}");
                        login.set_checked(!wanted);
                    }
                } else if event.id == quit.id() {
                    tray.take();
                    match stop.take() {
                        // The server stops gracefully, then says so.
                        Some(stop) if serving.is_some() => {
                            let _ = stop.send(());
                        }
                        _ => *control_flow = ControlFlow::Exit,
                    }
                }
            }
            Event::LoopDestroyed => {
                if let Some(server) = server.take() {
                    let _ = server.join();
                }
            }
            _ => {}
        }
    })
}

/// Completes when the process is asked to end: Ctrl-C, and SIGTERM on Unix – as at logout.
async fn terminated() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// The icon: on macOS the mark in black, a template the menu bar colours itself; elsewhere the small icon in colour.
fn icon() -> Result<Icon> {
    #[cfg(target_os = "macos")]
    let bytes = &include_bytes!("../../../brand/png/spezado-mark-black-256.png")[..];
    #[cfg(not(target_os = "macos"))]
    let bytes = &include_bytes!("../../../brand/png/spezado-icon-small-32.png")[..];
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().context("the size of the icon")?];
    let info = reader.next_frame(&mut rgba)?;
    anyhow::ensure!(info.color_type == png::ColorType::Rgba, "the icon is not RGBA");
    rgba.truncate(info.buffer_size());
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}

/// Opens an address in the default browser.
fn browse(address: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(address).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd").args(["/C", "start", "", address]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(address).spawn();
    if let Err(error) = result {
        eprintln!("spezado tray: {address}: {error}");
    }
}
