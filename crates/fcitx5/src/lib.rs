//! Fcitx5 integration for accents-rapides
//!
//! The native Fcitx5 addon (`cpp/addon.cpp`) is a thin C++ engine: it turns
//! each Fcitx5 key event into a call to the C ABI exported here (declared in
//! `cpp/accents_rapides.hpp`) and applies the returned action. All the
//! logic lives in Rust, in [`accents_rapides_core::InputHandler`].
//!
//! Each window (Fcitx5 input context) owns a [`Session`]. When a window
//! gains focus the addon reloads the configuration and mappings if their
//! files changed, while a key cycles through its variants it feeds the
//! overlay, and after each variant typed it saves the usage counts.

mod addon;

pub use addon::*;

use accents_rapides_core::{
    socket_path, Action, Config, CoreError, InputHandler, KeyEvent, Modifiers, OverlayMessage,
    OverlaySender, Session,
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::SystemTime;

/// Error type for Fcitx5 integration
#[derive(thiserror::Error, Debug)]
pub enum FcitxError {
    #[error("failed to load configuration: {0}")]
    Config(#[from] CoreError),

    #[error("invalid C string argument: {0}")]
    InvalidCString(String),

    #[error("null pointer argument")]
    NullPointer,
}

/// Key event flags of the C ABI, mirrored in `cpp/accents_rapides.hpp`.
pub mod key_flags {
    pub const RELEASE: u32 = 1 << 0;
    pub const SHIFT: u32 = 1 << 1;
    pub const CTRL: u32 = 1 << 2;
    pub const ALT: u32 = 1 << 3;
    pub const SUPER: u32 = 1 << 4;
    /// The key itself is a modifier.
    pub const MODIFIER: u32 = 1 << 5;
}

/// A key event from its C ABI form.
pub fn key_event(key: &str, code: u32, flags: u32) -> KeyEvent<'_> {
    let set = |flag| flags & flag != 0;
    KeyEvent {
        key,
        code,
        release: set(key_flags::RELEASE),
        is_modifier: set(key_flags::MODIFIER),
        modifiers: Modifiers {
            shift: set(key_flags::SHIFT),
            ctrl: set(key_flags::CTRL),
            alt: set(key_flags::ALT),
            super_key: set(key_flags::SUPER),
        },
    }
}

/// The state behind the C ABI: the input handler shared by every window,
/// the overlay feed, and the C strings of the last action, which stay valid
/// for the C side until the next call.
pub struct FcitxAddon {
    handler: InputHandler,
    /// When the configuration and mappings files were last modified, as of
    /// loading them.
    loaded: Stamps,
    overlay: Option<OverlaySender>,
    /// What the overlay was last told to show.
    shown: OverlayMessage,
    commit: Option<CString>,
    preedit: Option<CString>,
}

impl FcitxAddon {
    /// Load the configuration and mappings (creating them on first run),
    /// and the usage counts if there are any.
    pub fn new() -> Result<Self, FcitxError> {
        let (config, mut mapping_manager) = Config::load_or_create()?;
        log_usage_error(
            config
                .usage_path()
                .map(|path| mapping_manager.load_usage(&path)),
        );
        tracing::info!(
            "accents-rapides loaded, leader key: {:?}",
            config.leader_key
        );
        Ok(Self {
            loaded: modified(&config),
            handler: InputHandler::new(config, mapping_manager),
            overlay: socket_path().and_then(|path| OverlaySender::new(path).ok()),
            shown: OverlayMessage::Hide,
            commit: None,
            preedit: None,
        })
    }

    /// Reload the configuration, mappings and usage counts, e.g. after the
    /// editor saved them. On error nothing changes.
    pub fn reload(&mut self) -> Result<(), FcitxError> {
        *self = Self::new()?;
        Ok(())
    }

    /// Reload if the configuration or mappings file changed since they were
    /// loaded; whether they did.
    pub fn refresh(&mut self) -> Result<bool, FcitxError> {
        if modified(self.handler.config()) == self.loaded {
            return Ok(false);
        }
        self.reload()?;
        Ok(true)
    }

    /// Handle a key event sent to `program`, in the window of `session`; the
    /// app filter lets every key of filtered applications through untouched.
    pub fn key_event(&mut self, session: &mut Session, program: &str, event: &KeyEvent) -> Action {
        let action = if self.handler.is_enabled_for_app(program) {
            self.handler.handle_key(session, event)
        } else {
            Action::pass_through()
        };
        self.settle(session);
        action
    }

    /// See [`InputHandler::reset`].
    pub fn reset(&mut self, session: &mut Session) -> Action {
        let action = self.handler.reset(session);
        self.settle(session);
        action
    }

    /// After an event in `session`: tell the overlay what changed and save
    /// the usage counts if one was recorded.
    fn settle(&mut self, session: &Session) {
        self.update_overlay(session);
        let path = self.handler.config().usage_path();
        log_usage_error(path.map(|path| self.handler.mapping_manager_mut().save_usage(&path)));
    }

    /// Tell the overlay what `session` shows now, if that changed.
    fn update_overlay(&mut self, session: &Session) {
        if !self.handler.config().show_overlay {
            return;
        }
        let message = OverlayMessage::of(&self.handler, session);
        if message != self.shown {
            if let Some(overlay) = &self.overlay {
                overlay.send(&message);
            }
            self.shown = message;
        }
    }

    /// Get the input handler
    pub fn handler(&self) -> &InputHandler {
        &self.handler
    }

    /// Get a mutable reference to the input handler
    pub fn handler_mut(&mut self) -> &mut InputHandler {
        &mut self.handler
    }
}

/// When the configuration and mappings files were last modified; None for
/// a file that is gone. The usage file is not watched: the addon itself
/// writes it.
type Stamps = [Option<SystemTime>; 2];

fn modified(config: &Config) -> Stamps {
    let modified = |path: std::path::PathBuf| std::fs::metadata(path).ok()?.modified().ok();
    [config.config_path.clone(), config.mappings_path()].map(|path| path.and_then(modified))
}

/// The usage counts are a convenience: a failure to load or save them (at
/// the usage file, when the configuration has one) is logged and typing
/// goes on.
fn log_usage_error(result: Option<Result<(), CoreError>>) {
    if let Some(Err(e)) = result {
        tracing::warn!("usage counts: {e}");
    }
}

/// Borrow a C string argument as UTF-8.
///
/// # Safety
///
/// `ptr` must be NULL or a valid NUL-terminated C string that outlives the
/// returned reference.
unsafe fn str_arg<'a>(ptr: *const c_char) -> Result<&'a str, FcitxError> {
    if ptr.is_null() {
        return Err(FcitxError::NullPointer);
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map_err(|e| FcitxError::InvalidCString(e.to_string()))
}
