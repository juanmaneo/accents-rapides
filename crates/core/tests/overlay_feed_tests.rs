//! Tests for the overlay feed: what is shown for a session, where the socket
//! lives, and the datagrams between the input method and the overlay.

use accents_rapides_core::*;
use accents_rapides_test_support::with_env;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use tempfile::tempdir;

fn press(key: &str, code: u32) -> KeyEvent<'_> {
    KeyEvent {
        key,
        code,
        ..KeyEvent::default()
    }
}

fn show(variants: &[&str], index: usize) -> OverlayMessage {
    OverlayMessage::Show {
        variants: variants.iter().map(|v| v.to_string()).collect(),
        index,
    }
}

#[test]
fn test_message_shows_only_while_cycling() {
    let mut manager = MappingManager::new();
    manager.load_default_mappings();
    manager
        .current_profile_mut()
        .expect("default profile")
        .mappings
        .insert(
            MappingKey::Char('e'),
            KeyMapping {
                characters: vec!["é".to_string(), "è".to_string()],
                cycle_position: 0,
                reverse_cycle: false,
            },
        );
    let mut handler = InputHandler::new(Config::default(), manager);
    let mut session = Session::default();

    assert_eq!(OverlayMessage::of(&handler, &session), OverlayMessage::Hide);
    // Holding a mapped key alone shows nothing.
    handler.handle_key(&mut session, &press("e", 1));
    assert_eq!(OverlayMessage::of(&handler, &session), OverlayMessage::Hide);
    // The leader (Alt by default) starts the cycle.
    let alt = KeyEvent {
        is_modifier: true,
        ..press("alt", 64)
    };
    handler.handle_key(&mut session, &alt);
    assert_eq!(OverlayMessage::of(&handler, &session), show(&["é", "è"], 0));
}

#[test]
fn test_socket_path() {
    const XDG: &str = "XDG_RUNTIME_DIR";
    let path = |overlay, xdg| with_env(&[(OVERLAY_SOCKET_ENV, overlay), (XDG, xdg)], socket_path);

    // The override wins; an empty one is ignored.
    assert_eq!(
        path(Some("/tmp/o.sock"), Some("/run/user/1")),
        Some(PathBuf::from("/tmp/o.sock"))
    );
    assert_eq!(
        path(Some(""), Some("/run/user/1")),
        Some(PathBuf::from("/run/user/1/accents-rapides-overlay.sock"))
    );
    // Without a runtime directory there is nowhere to meet.
    assert_eq!(path(None, None), None);
    assert_eq!(path(None, Some("")), None);
}

#[test]
fn test_messages_travel_from_sender_to_receiver() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("overlay.sock");
    let receiver = OverlayReceiver::bind(&path)?;
    let sender = OverlaySender::new(path.clone())?;

    // Invalid datagrams are skipped.
    UnixDatagram::unbound()?.send_to(b"not json", &path)?;
    sender.send(&show(&["é", "è"], 1));
    sender.send(&OverlayMessage::Hide);

    let received: Vec<_> = receiver.messages().take(2).collect();
    assert_eq!(received, [show(&["é", "è"], 1), OverlayMessage::Hide]);

    // The overlay cleans up after itself.
    drop(receiver);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn test_sending_without_an_overlay_is_harmless() -> anyhow::Result<()> {
    let dir = tempdir()?;
    OverlaySender::new(dir.path().join("nobody.sock"))?.send(&OverlayMessage::Hide);
    Ok(())
}

#[test]
fn test_bind_replaces_a_stale_socket_only() -> anyhow::Result<()> {
    let dir = tempdir()?;

    // A previous overlay died and left its socket file behind.
    let path = dir.path().join("stale.sock");
    drop(UnixDatagram::bind(&path)?);
    assert!(path.exists());
    let receiver = OverlayReceiver::bind(&path)?;
    OverlaySender::new(path.clone())?.send(&OverlayMessage::Hide);
    assert_eq!(receiver.messages().next(), Some(OverlayMessage::Hide));

    // Any other file is left alone, and binding fails.
    let path = dir.path().join("file");
    std::fs::write(&path, "keep me")?;
    let error = OverlayReceiver::bind(&path).unwrap_err();
    assert!(matches!(error, CoreError::OverlaySocket { .. }));
    assert!(error.to_string().starts_with(&format!(
        "cannot bind the overlay socket {}",
        path.display()
    )));
    assert_eq!(std::fs::read_to_string(&path)?, "keep me");
    Ok(())
}
