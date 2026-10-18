//! The gesture engine: hold a mapped key, press the leader, get a variant.
//!
//! Ported from the key handling of the original Fcitx5 addon
//! (`SchnelleUmlauteEngine::keyEvent`). Fcitx5 itself stays in the native
//! addon (`crates/fcitx5/cpp`), which turns every key event into a
//! [`KeyEvent`] and applies the returned [`Action`]:
//!
//! 1. Pressing a mapped key shows it as preedit instead of typing it.
//! 2. Releasing it types it, unless the leader was pressed meanwhile: then
//!    the first variant replaces it (committed at once when it is the only
//!    one, otherwise shown, and every further leader press shows the next
//!    variant until the key is released).
//! 3. Held longer than the time window, the key just types itself.
//!
//! The configuration and mappings ([`InputHandler`]) are shared, while each
//! window (Fcitx5 input context) has its own gesture state ([`Session`]), so
//! that a key held in one window cannot commit in another.
//!
//! Not ported: the original's workarounds for Alt-led gestures under KWin
//! Wayland auto-repeat (deferred commits), its dual custom leaders, and its
//! timer that commits an expired key while still held (here it commits on
//! the next key event instead).

use crate::config::{Config, LeaderKey};
use crate::mappings::{MappingKey, MappingManager};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Names of the keys that a [`KeyEvent`] reports by name, because what they
/// type (if anything) is not what identifies them.
pub mod key_name {
    pub const SPACE: &str = "space";
    pub const LEFT: &str = "left";
    pub const RIGHT: &str = "right";
    pub const UP: &str = "up";
    pub const DOWN: &str = "down";
    /// Left or right Alt.
    pub const ALT: &str = "alt";
    /// AltGr (ISO Level 3 Shift).
    pub const ALTGR: &str = "altgr";
}

/// Modifier keys held during a key event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl Modifiers {
    /// Whether the event is a shortcut (Ctrl+C, Alt+F4...). Shift is not:
    /// it types uppercase letters.
    pub fn shortcut(self) -> bool {
        self.ctrl || self.alt || self.super_key
    }
}

/// A key event, as delivered by the input method framework.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyEvent<'a> {
    /// What the key types (`a`, `A`, `é`), or its [`key_name`]. Empty for
    /// any other key.
    pub key: &'a str,
    /// Physical key code: pairs a release with its press, and tells
    /// auto-repeats from fresh presses.
    pub code: u32,
    pub release: bool,
    /// The key is itself a modifier (Shift, Ctrl, Alt, Super...).
    pub is_modifier: bool,
    pub modifiers: Modifiers,
}

/// What to do with a key event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Action {
    /// Text to commit to the application, replacing the preedit.
    pub commit: Option<String>,
    /// Preedit text to show after the commit.
    pub preedit: Option<String>,
    /// Whether the key event continues to the application.
    pub forward: bool,
}

impl Action {
    /// Let the key event through, doing nothing else.
    pub fn pass_through() -> Self {
        Self {
            forward: true,
            ..Self::default()
        }
    }

    fn commit(&mut self, text: &str) {
        self.commit.get_or_insert_with(String::new).push_str(text);
    }
}

/// The mapped key being held, from its press until it commits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Gesture {
    key: MappingKey,
    /// What the key types by itself.
    text: String,
    code: u32,
    since: Instant,
    /// Index of the variant shown, once the leader was pressed.
    cycle: Option<usize>,
}

/// The gesture state of one window: what is held, shown and pending there.
#[derive(Debug, Default)]
pub struct Session {
    gesture: Option<Gesture>,
    /// Physical keys currently held: a press of one of them is a repeat.
    held: HashSet<u32>,
    /// Key whose single variant was committed by the leader while it is
    /// still held: its repeats and release are swallowed.
    committed_code: Option<u32>,
    /// Alt/AltGr leader press that was consumed: its release is too.
    leader_code: Option<u32>,
    /// A commit just happened: the next Space is committed as text too, so
    /// that applications cannot receive both out of order.
    recently_committed: bool,
}

impl Session {
    /// The mapped key being held, if any.
    pub fn pressed_key(&self) -> Option<&MappingKey> {
        self.gesture.as_ref().map(|g| &g.key)
    }
}

/// Input handler for accents-rapides: the gesture engine plus its
/// configuration and mappings, shared by every [`Session`].
pub struct InputHandler {
    config: Config,
    mapping_manager: MappingManager,
    enabled: bool,
}

impl InputHandler {
    /// Create a new InputHandler. When variants are sorted by usage, the
    /// usage counts loaded so far order them from the start.
    pub fn new(config: Config, mut mapping_manager: MappingManager) -> Self {
        if config.sort_by_usage {
            mapping_manager.sort_all_by_usage();
        }
        Self {
            config,
            mapping_manager,
            enabled: true,
        }
    }

    /// Handle a key event of the window whose state is `session`.
    pub fn handle_key(&mut self, session: &mut Session, event: &KeyEvent) -> Action {
        let fresh = if event.release {
            session.held.remove(&event.code);
            true
        } else {
            session.held.insert(event.code)
        };
        if event.release {
            return self.release(session, event.code);
        }

        let leader = self.is_leader(event.key);
        if event.is_modifier && !(leader && session.gesture.is_some()) {
            return Action::pass_through();
        }

        let mut action = Action::default();
        let space = event.key == key_name::SPACE && !event.modifiers.shortcut();
        if std::mem::take(&mut session.recently_committed) && space && session.gesture.is_none() {
            action.commit(" ");
            return action;
        }

        if self.expired(session) {
            self.flush(session, &mut action);
            if space {
                action.commit(" ");
                return action;
            }
        }

        if event.modifiers.shortcut() && !self.alt_leader_bypass(session, event.modifiers) {
            self.flush(session, &mut action);
            action.forward = true;
            return action;
        }

        if leader {
            if self.leader(session, event.code, &mut action) {
                return action;
            }
            self.flush(session, &mut action);
            session.leader_code = None;
            action.forward = true;
            return action;
        }

        // During a gesture, repeats (of the held key, or of another key held
        // meanwhile) are swallowed.
        if session.gesture.is_some() && !fresh {
            return action;
        }

        let mapped = MappingKey::from_key(event.key, event.modifiers.shift)
            .filter(|key| !self.variants(key).is_empty());
        if let Some(key) = mapped {
            if !fresh && session.committed_code == Some(event.code) {
                return action;
            }
            self.flush(session, &mut action);
            session.gesture = Some(Gesture {
                key,
                text: event.key.to_string(),
                code: event.code,
                since: Instant::now(),
                cycle: None,
            });
            action.preedit = Some(event.key.to_string());
            return action;
        }

        self.flush(session, &mut action);
        session.leader_code = None;
        action.forward = true;
        action
    }

    /// Commit what is pending in `session` and forget it, e.g. when its
    /// window loses focus. The caller decides whether to apply the commit.
    pub fn reset(&mut self, session: &mut Session) -> Action {
        let mut action = Action::default();
        self.flush(session, &mut action);
        *session = Session::default();
        action
    }

    fn release(&mut self, session: &mut Session, code: u32) -> Action {
        if session.leader_code == Some(code) {
            session.leader_code = None;
            return Action::default();
        }
        if session.committed_code == Some(code) {
            session.committed_code = None;
            return Action::default();
        }
        if session.gesture.as_ref().is_some_and(|g| g.code == code) {
            let mut action = Action::default();
            self.flush(session, &mut action);
            return action;
        }
        Action::pass_through()
    }

    /// Handle the leader during a gesture; `false` when there is none (or
    /// its mapping is gone), so the leader is an ordinary key.
    fn leader(&mut self, session: &mut Session, code: u32, action: &mut Action) -> bool {
        let Some(gesture) = &mut session.gesture else {
            return false;
        };
        let Some(mapping) = self.mapping_manager.get_mapping(&gesture.key) else {
            return false;
        };
        let variants = &mapping.characters;
        if variants.is_empty() {
            return false;
        }

        if variants.len() == 1 && gesture.cycle.is_none() {
            let variant = variants[0].clone();
            session.committed_code = Some(gesture.code);
            let key = gesture.key.clone();
            session.gesture = None;
            self.commit_variant(session, &key, &variant, action);
        } else {
            let next = gesture
                .cycle
                .map_or(0, |i| step(i, variants.len(), mapping.reverse_cycle));
            gesture.cycle = Some(next);
            action.preedit = Some(variants[next].clone());
        }

        if self.is_alt_leader() {
            session.leader_code = Some(code);
        }
        true
    }

    /// Commit what the held key shows and end its gesture.
    fn flush(&mut self, session: &mut Session, action: &mut Action) {
        let Some(gesture) = session.gesture.take() else {
            return;
        };
        let variant = gesture
            .cycle
            .and_then(|i| self.variants(&gesture.key).get(i).cloned());
        match variant {
            Some(variant) => self.commit_variant(session, &gesture.key, &variant, action),
            None => {
                action.commit(&gesture.text);
                session.recently_committed = true;
            }
        }
    }

    fn commit_variant(
        &mut self,
        session: &mut Session,
        key: &MappingKey,
        variant: &str,
        action: &mut Action,
    ) {
        action.commit(variant);
        session.recently_committed = true;
        self.mapping_manager.record_usage(key, variant);
        if self.config.sort_by_usage {
            self.mapping_manager.sort_by_usage(key);
        }
    }

    /// Whether a waiting key was held past the time window.
    fn expired(&self, session: &Session) -> bool {
        let window = Duration::from_millis(u64::from(self.config.time_window_ms));
        session
            .gesture
            .as_ref()
            .is_some_and(|g| g.cycle.is_none() && g.since.elapsed() > window)
    }

    fn variants(&self, key: &MappingKey) -> &[String] {
        self.mapping_manager
            .get_characters_for_key(key)
            .unwrap_or_default()
    }

    fn is_leader(&self, key: &str) -> bool {
        let name = self.config.leader_key.name();
        !name.is_empty() && key.eq_ignore_ascii_case(name)
    }

    fn is_alt_leader(&self) -> bool {
        matches!(self.config.leader_key, LeaderKey::Alt | LeaderKey::AltGr)
    }

    /// With an Alt leader, keys pressed while Alt is held (and only Alt)
    /// belong to the gesture rather than being shortcuts.
    fn alt_leader_bypass(&self, session: &Session, modifiers: Modifiers) -> bool {
        self.is_alt_leader()
            && (session.gesture.is_some() || session.leader_code.is_some())
            && modifiers.alt
            && !modifiers.ctrl
            && !modifiers.super_key
    }

    /// While `session` cycles through the variants of its held key: those
    /// variants and the index of the one shown.
    pub fn cycle<'a>(&'a self, session: &Session) -> Option<(&'a [String], usize)> {
        let gesture = session.gesture.as_ref()?;
        let index = gesture.cycle?;
        let variants = self.variants(&gesture.key);
        (index < variants.len()).then_some((variants, index))
    }

    /// Check if the input method is enabled for a specific application
    pub fn is_enabled_for_app(&self, app_name: &str) -> bool {
        self.enabled && self.config.is_app_allowed(app_name)
    }

    /// Enable or disable the input method
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the input method is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Get the mapping manager
    pub fn mapping_manager(&self) -> &MappingManager {
        &self.mapping_manager
    }

    /// Get a mutable reference to the mapping manager
    pub fn mapping_manager_mut(&mut self) -> &mut MappingManager {
        &mut self.mapping_manager
    }
}

/// The next index when cycling through `len` variants.
fn step(index: usize, len: usize, reverse: bool) -> usize {
    if reverse {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    }
}
