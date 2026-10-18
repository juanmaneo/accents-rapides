//! The overlay feed: the input method tells the overlay what to show.
//!
//! One JSON [`OverlayMessage`] per datagram, over a Unix datagram socket in
//! `$XDG_RUNTIME_DIR` (see [`socket_path`]). The overlay binds it; the input
//! method sends without waiting, and without caring whether an overlay runs.

use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use crate::config::CoreError;
use crate::input::{InputHandler, Session};

/// Environment variable that overrides the default overlay socket path.
pub const OVERLAY_SOCKET_ENV: &str = "ACCENTS_RAPIDES_OVERLAY_SOCKET";

/// Name of the socket in `$XDG_RUNTIME_DIR`.
const SOCKET_NAME: &str = "accents-rapides-overlay.sock";

/// Larger datagrams are truncated, hence dropped as invalid.
const MAX_DATAGRAM: usize = 64 * 1024;

/// What the overlay shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayMessage {
    /// The variants being cycled through, and the index of the one shown.
    Show { variants: Vec<String>, index: usize },
    /// Nothing.
    Hide,
}

impl OverlayMessage {
    /// What to show for `session`: its variants while it cycles, else nothing.
    /// Merely holding a mapped key shows nothing, or every letter would flash.
    pub fn of(handler: &InputHandler, session: &Session) -> Self {
        match handler.cycle(session) {
            Some((variants, index)) => Self::Show {
                variants: variants.to_vec(),
                index,
            },
            None => Self::Hide,
        }
    }
}

/// The overlay socket: `$ACCENTS_RAPIDES_OVERLAY_SOCKET`, else
/// `$XDG_RUNTIME_DIR/accents-rapides-overlay.sock`, else none.
pub fn socket_path() -> Option<PathBuf> {
    env_path(OVERLAY_SOCKET_ENV)
        .or_else(|| env_path("XDG_RUNTIME_DIR").map(|dir| dir.join(SOCKET_NAME)))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The input method's end: fire and forget.
#[derive(Debug)]
pub struct OverlaySender {
    socket: UnixDatagram,
    path: PathBuf,
}

impl OverlaySender {
    /// A sender to the overlay socket at `path`.
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        // Non-blocking: a busy overlay must never stall the keyboard.
        UnixDatagram::unbound()
            .and_then(|socket| socket.set_nonblocking(true).map(|()| Self { socket, path }))
    }

    /// Send `message`, ignoring failures: no overlay running, or a busy one.
    pub fn send(&self, message: &OverlayMessage) {
        let json = serde_json::to_vec(message).expect("OverlayMessage always serializes");
        let _ = self.socket.send_to(&json, &self.path);
    }
}

/// The overlay's end: owns the socket file, removed on drop.
#[derive(Debug)]
pub struct OverlayReceiver {
    socket: UnixDatagram,
    path: PathBuf,
}

impl OverlayReceiver {
    /// Bind the overlay socket at `path`, replacing the socket file a
    /// previous overlay left behind (but never another kind of file).
    pub fn bind(path: &Path) -> Result<Self, CoreError> {
        if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_socket()) {
            let _ = fs::remove_file(path);
        }
        let socket = UnixDatagram::bind(path).map_err(|source| CoreError::OverlaySocket {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            socket,
            path: path.to_path_buf(),
        })
    }

    /// The messages as they arrive, blocking; invalid datagrams are skipped.
    /// Ends if the socket fails.
    pub fn messages(&self) -> impl Iterator<Item = OverlayMessage> + '_ {
        let mut buf = vec![0; MAX_DATAGRAM];
        std::iter::repeat_with(move || {
            self.socket
                .recv(&mut buf)
                .map(|len| serde_json::from_slice(&buf[..len]))
        })
        .map_while(Result::ok)
        .filter_map(Result::ok)
    }
}

impl Drop for OverlayReceiver {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
