//! Tests for the overlay crate: the overlay state and view, binding the
//! overlay socket, and the binary entry point.

use accents_rapides_core::OverlayMessage;
use accents_rapides_overlay::app::{cell_style, panel_style, view, Overlay, ACTIVE_COLOR};
use accents_rapides_overlay::run_overlay;
use accents_rapides_test_support::{colored_pixels, with_env, TempConfig, OVERLAY_SOCKET_ENV};
use iced_test::simulator;

fn show(variants: &[&str], index: usize) -> OverlayMessage {
    OverlayMessage::Show {
        variants: variants.iter().map(|v| v.to_string()).collect(),
        index,
    }
}

/// The cells of `overlay`, as owned values; the view is built too, so every
/// code path of the view runs.
fn cells(overlay: &Overlay) -> Vec<(String, bool)> {
    let _ = view(overlay);
    overlay
        .cells()
        .map(|(variant, active)| (variant.to_string(), active))
        .collect()
}

#[test]
fn test_overlay_is_empty_until_shown() {
    let overlay = Overlay::default();
    assert!(cells(&overlay).is_empty());
}

#[test]
fn test_overlay_shows_variants_with_current_marked() {
    let mut overlay = Overlay::default();

    overlay.apply(show(&["é", "è"], 1));
    assert_eq!(
        cells(&overlay),
        [("é".to_string(), false), ("è".to_string(), true)]
    );

    overlay.apply(show(&["é", "è"], 0));
    assert_eq!(
        cells(&overlay),
        [("é".to_string(), true), ("è".to_string(), false)]
    );
}

#[test]
fn test_overlay_hides() {
    let mut overlay = Overlay::default();
    overlay.apply(show(&["ä"], 0));
    overlay.apply(OverlayMessage::Hide);

    assert_eq!(overlay, Overlay::default());
    assert!(cells(&overlay).is_empty());
}

/// How many coloured pixels the overlay shows for `variant` alone, with no
/// current one (that one is coloured by design).
fn colored_pixels_for(variant: &str) -> usize {
    let overlay = Overlay {
        variants: vec![variant.to_string()],
        current: None,
    };
    // The CPU renderer, as in the editor tests: no GPU needed.
    with_env(&[("ICED_TEST_BACKEND", Some("tiny-skia"))], || {
        let snapshot = simulator(view(&overlay))
            .snapshot(&iced::Theme::Dark)
            .expect("render the overlay");
        colored_pixels(&snapshot)
    })
}

/// Emoji must show in colour, with the colour emoji font: iced's own font
/// fallback renders most of them as the grey glyphs of a text font.
/// Needs a colour emoji font installed (Noto Color Emoji, say).
#[test]
fn test_emoji_show_in_color() {
    assert_eq!(colored_pixels_for("a"), 0, "a letter is white on grey");
    assert!(
        colored_pixels_for("😀") > 0,
        "the grinning face is yellow in every colour emoji font"
    );
}

#[test]
fn test_panel_style_is_translucent_dark() {
    let style = panel_style(&iced::Theme::Dark);

    assert_eq!(style.text_color, Some(iced::Color::WHITE));
    assert_eq!(
        style.background,
        Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7).into())
    );
}

#[test]
fn test_only_the_current_cell_is_tinted() {
    assert_eq!(
        cell_style(true).background,
        Some(ACTIVE_COLOR.scale_alpha(0.25).into())
    );
    assert_eq!(cell_style(false).background, None);
}

#[test]
fn test_run_overlay_needs_a_socket_location() {
    let result = with_env(
        &[(OVERLAY_SOCKET_ENV, None), ("XDG_RUNTIME_DIR", None)],
        run_overlay,
    );
    assert!(result.unwrap_err().to_string().contains("XDG_RUNTIME_DIR"));
}

#[test]
fn test_run_overlay_reports_bind_errors() {
    let result = TempConfig::unwritable().with(run_overlay);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("cannot bind the overlay socket"));
}

#[test]
#[cfg(coverage)]
fn test_run_overlay_under_coverage() -> anyhow::Result<()> {
    // Under coverage measurement the surface is not opened, so
    // `run_overlay` binds the socket and returns normally.
    let temp = TempConfig::new();
    temp.with(run_overlay)?;
    // The socket is removed on the way out.
    assert!(!temp.overlay_socket.exists());
    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_overlay_binary_entry_point() -> anyhow::Result<()> {
    // Under coverage the overlay never opens a surface, so spawning it
    // exercises the entry point, the setup logic and the success path.
    use assert_cmd::prelude::*;
    use std::process::Command;

    let temp = TempConfig::new();
    Command::cargo_bin("accents-rapides-overlay")?
        .envs(temp.envs())
        .assert()
        .success();
    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_overlay_binary_reports_errors() -> anyhow::Result<()> {
    // A socket path whose parent is a regular file makes the overlay fail:
    // the entry point must report the error and exit non-zero.
    use assert_cmd::prelude::*;
    use std::process::Command;

    let temp = TempConfig::unwritable();
    Command::cargo_bin("accents-rapides-overlay")?
        .envs(temp.envs())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Failed to run overlay"));
    Ok(())
}
