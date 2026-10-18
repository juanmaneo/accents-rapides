//! Cycle overlay for accents-rapides.
//!
//! A small iced surface on the Wayland `wlr-layer-shell` protocol: always on
//! top, never takes keyboard focus. Like the original, it needs a compositor
//! with layer-shell (KDE Plasma, sway, Hyprland, COSMIC, ...); GNOME/Mutter
//! and X11 cannot host it.
//!
//! The Fcitx5 addon feeds it over the overlay socket (see
//! [`accents_rapides_core::overlay_feed`]): it shows the variants while a key
//! cycles through them, and nothing otherwise.

use accents_rapides_core::{socket_path, OverlayReceiver};
use anyhow::{Context, Result};

pub mod app;

/// Bind the overlay socket, then run the overlay UI.
pub fn run_overlay() -> Result<()> {
    let path =
        socket_path().context("nowhere to put the overlay socket: XDG_RUNTIME_DIR is not set")?;
    launch_overlay(OverlayReceiver::bind(&path)?)
}

/// Open the layer-shell surface, fed by `receiver`.
///
/// Not measured by coverage: it needs a running Wayland compositor
/// (cargo-llvm-cov sets `cfg(coverage)`, so coverage builds skip the launch
/// and only the setup logic above is measured).
#[cfg(not(coverage))]
fn launch_overlay(receiver: OverlayReceiver) -> Result<()> {
    use iced::futures::channel::mpsc;
    use iced::{Color, Task};
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    use std::cell::Cell;

    // The socket blocks: read it on its own thread, and hand the messages
    // to the UI as a stream.
    let (tx, rx) = mpsc::unbounded();
    std::thread::spawn(move || {
        for message in receiver.messages() {
            if tx.unbounded_send(message).is_err() {
                break;
            }
        }
    });
    let feed = Cell::new(Some(rx));

    // No text input: skip the clipboard worker thread.
    iced_layershell::disable_clipboard();
    iced_layershell::application(
        move || {
            let task = feed
                .take()
                .map_or_else(Task::none, |rx| Task::run(rx, app::Message::Feed));
            (app::Overlay::default(), task)
        },
        "accents-rapides-overlay",
        |overlay: &mut app::Overlay, message: app::Message| {
            if let app::Message::Feed(message) = message {
                overlay.apply(message);
            }
            Task::none()
        },
        app::view,
    )
    .style(|_, theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor: Anchor::Top,
            layer: Layer::Overlay,
            size: Some((480, 72)),
            margin: (48, 0, 0, 0),
            exclusive_zone: 0,
            keyboard_interactivity: KeyboardInteractivity::None,
            events_transparent: true,
            ..LayerShellSettings::default()
        },
        ..Settings::default()
    })
    .run()?;
    Ok(())
}

/// Coverage builds cannot open a surface; `run_overlay` still exercises the
/// setup logic and returns normally.
#[cfg(coverage)]
fn launch_overlay(_receiver: OverlayReceiver) -> Result<()> {
    Ok(())
}
