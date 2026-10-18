//! Tests for the gesture engine: every path of the key state machine (press,
//! leader, release, repeats, modifiers, time window), the cycling and the
//! accessors.

use accents_rapides_core::*;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

const A: u32 = 38;
const O: u32 = 32;
const X: u32 = 53;
const SPACE: u32 = 65;
const ALT: u32 = 64;
const SHIFT: u32 = 50;

/// A handler and the session of one window, driven together.
struct Harness {
    handler: InputHandler,
    session: Session,
}

impl Harness {
    fn handle_key(&mut self, event: &KeyEvent) -> Action {
        self.handler.handle_key(&mut self.session, event)
    }

    fn reset(&mut self) -> Action {
        self.handler.reset(&mut self.session)
    }

    fn current_pressed_key(&self) -> Option<&MappingKey> {
        self.session.pressed_key()
    }

    /// The variants being cycled through, owned, and the index shown.
    fn cycle(&self) -> Option<(Vec<&str>, usize)> {
        let (variants, index) = self.handler.cycle(&self.session)?;
        Some((variants.iter().map(String::as_str).collect(), index))
    }
}

impl Deref for Harness {
    type Target = InputHandler;

    fn deref(&self) -> &InputHandler {
        &self.handler
    }
}

impl DerefMut for Harness {
    fn deref_mut(&mut self) -> &mut InputHandler {
        &mut self.handler
    }
}

/// A handler with a generous time window and the given mappings.
fn handler_with(config: Config, mappings: &[(MappingKey, &[&str], bool)]) -> Harness {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Test");
    for (key, variants, reverse_cycle) in mappings {
        profile.mappings.insert(
            key.clone(),
            KeyMapping {
                characters: variants.iter().map(|s| s.to_string()).collect(),
                cycle_position: 0,
                reverse_cycle: *reverse_cycle,
            },
        );
    }
    manager.add_profile(profile);
    manager.set_current_profile("Test");
    Harness {
        handler: InputHandler::new(config, manager),
        session: Session::default(),
    }
}

fn config(leader_key: LeaderKey, time_window_ms: u32) -> Config {
    Config {
        leader_key,
        time_window_ms,
        ..Config::default()
    }
}

/// "a" cycles through `variants`, "o" commits "ö"; Space leads.
fn handler(variants: &[&str]) -> Harness {
    handler_with(
        config(LeaderKey::Space, 10_000),
        &[
            (MappingKey::Char('a'), variants, false),
            (MappingKey::Char('o'), &["ö"], false),
        ],
    )
}

fn press(key: &str, code: u32) -> KeyEvent<'_> {
    KeyEvent {
        key,
        code,
        ..KeyEvent::default()
    }
}

fn release(key: &str, code: u32) -> KeyEvent<'_> {
    KeyEvent {
        release: true,
        ..press(key, code)
    }
}

fn with(event: KeyEvent<'_>, modifiers: Modifiers) -> KeyEvent<'_> {
    KeyEvent { modifiers, ..event }
}

const CTRL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
    super_key: false,
};
const ALT_HELD: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: true,
    super_key: false,
};

fn preedit(text: &str) -> Action {
    Action {
        preedit: Some(text.to_string()),
        ..Action::default()
    }
}

fn commit(text: &str) -> Action {
    Action {
        commit: Some(text.to_string()),
        ..Action::default()
    }
}

fn commit_and_forward(text: &str) -> Action {
    Action {
        forward: true,
        ..commit(text)
    }
}

/// Swallow the key, doing nothing else.
fn swallow() -> Action {
    Action::default()
}

#[test]
fn test_mapped_key_types_itself_on_release() {
    let mut h = handler(&["ä", "à"]);

    assert_eq!(h.handle_key(&press("a", A)), preedit("a"));
    assert_eq!(h.current_pressed_key(), Some(&MappingKey::Char('a')));
    assert_eq!(h.handle_key(&release("a", A)), commit("a"));
    assert_eq!(h.current_pressed_key(), None);
}

#[test]
fn test_unmapped_keys_pass_through() {
    let mut h = handler(&["ä"]);

    assert_eq!(h.handle_key(&press("z", X)), Action::pass_through());
    assert_eq!(h.handle_key(&press("", X)), Action::pass_through());
    assert_eq!(h.handle_key(&release("z", X)), Action::pass_through());
}

#[test]
fn test_leader_commits_single_variant_and_swallows_the_rest() {
    let mut h = handler(&["ä"]);

    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("space", SPACE)), commit("ä"));
    assert_eq!(
        h.mapping_manager().usage_stats[&MappingKey::Char('a')]["ä"],
        1
    );

    // The key is still held: its repeats and release must not type "a".
    assert_eq!(h.handle_key(&press("a", A)), swallow());
    assert_eq!(h.handle_key(&release("a", A)), swallow());
    assert_eq!(
        h.handle_key(&release("space", SPACE)),
        Action::pass_through()
    );
}

#[test]
fn test_space_after_a_commit_is_committed_too() {
    let mut h = handler(&["ä"]);

    h.handle_key(&press("a", A));
    h.handle_key(&release("a", A));
    // Committed as text, so it cannot overtake the committed "a".
    assert_eq!(h.handle_key(&press("space", SPACE)), commit(" "));
    // Only once: the next leader without a gesture is an ordinary key.
    assert_eq!(h.handle_key(&press("space", SPACE)), Action::pass_through());

    // Any other key ends the guard too.
    h.handle_key(&press("a", A));
    h.handle_key(&release("a", A));
    assert_eq!(h.handle_key(&press("z", X)), Action::pass_through());
    assert_eq!(h.handle_key(&press("space", SPACE)), Action::pass_through());
}

#[test]
fn test_leader_cycles_through_variants_while_held() {
    let mut h = handler(&["ä", "à", "â"]);

    // Holding the key is not cycling yet.
    h.handle_key(&press("a", A));
    assert_eq!(h.cycle(), None);
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("ä"));
    assert_eq!(h.cycle(), Some((vec!["ä", "à", "â"], 0)));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("à"));
    assert_eq!(h.cycle(), Some((vec!["ä", "à", "â"], 1)));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("â"));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("ä"));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("à"));

    // Releasing the key commits the variant shown.
    assert_eq!(h.handle_key(&release("a", A)), commit("à"));
    assert_eq!(
        h.mapping_manager().usage_stats[&MappingKey::Char('a')]["à"],
        1
    );
    assert_eq!(h.cycle(), None);
}

#[test]
fn test_reverse_cycle() {
    let mut h = handler_with(
        config(LeaderKey::Space, 10_000),
        &[(MappingKey::Char('a'), &["ä", "à", "â"], true)],
    );

    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("ä"));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("â"));
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("à"));
}

#[test]
fn test_sort_by_usage_reorders_after_commit() {
    let mut h = handler(&["ä", "à"]);
    h.config_mut().sort_by_usage = true;

    h.handle_key(&press("a", A));
    h.handle_key(&press("space", SPACE));
    h.handle_key(&press("space", SPACE));
    assert_eq!(h.handle_key(&release("a", A)), commit("à"));

    assert_eq!(
        h.mapping_manager()
            .get_characters_for_key(&MappingKey::Char('a')),
        Some(&["à".to_string(), "ä".to_string()][..])
    );
}

#[test]
fn test_handler_starts_sorted_by_usage() {
    let mut h = handler(&["ä", "à"]);
    h.mapping_manager_mut()
        .record_usage(&MappingKey::Char('a'), "à");

    // With the setting on, the usage counts loaded with the mappings order
    // the variants from the start; off, the stored order stays.
    for (sort_by_usage, expected) in [(true, "à,ä"), (false, "ä,à")] {
        let config = Config {
            sort_by_usage,
            ..Config::default()
        };
        let handler = InputHandler::new(config, h.mapping_manager().clone());
        let variants = handler
            .mapping_manager()
            .get_characters_for_key(&MappingKey::Char('a'))
            .map(|variants| variants.join(","));
        assert_eq!(variants.as_deref(), Some(expected));
    }
}

#[test]
fn test_other_keys_commit_the_held_key_first() {
    let mut h = handler(&["ä", "à"]);

    // An unmapped key: the held one is committed, then the key goes on.
    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("z", X)), commit_and_forward("a"));

    // A mapped key: the held one is committed, the new one is shown.
    h.handle_key(&press("a", A));
    h.handle_key(&press("space", SPACE));
    let action = h.handle_key(&press("o", O));
    assert_eq!(
        action,
        Action {
            commit: Some("ä".to_string()),
            preedit: Some("o".to_string()),
            forward: false,
        }
    );

    // The Space guard waits for the end of a gesture: here it leads.
    assert_eq!(h.handle_key(&press("space", SPACE)), commit("ö"));
}

#[test]
fn test_repeats_during_a_gesture_are_swallowed() {
    let mut h = handler(&["ä", "à"]);

    // "z" is held when the gesture starts: its repeats are swallowed...
    h.handle_key(&press("z", X));
    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("z", X)), swallow());
    // ...as are the held key's own.
    assert_eq!(h.handle_key(&press("a", A)), swallow());
    assert_eq!(h.current_pressed_key(), Some(&MappingKey::Char('a')));
}

#[test]
fn test_repeat_starts_a_new_gesture_once_the_previous_one_ended() {
    let mut h = handler(&["ä", "à"]);

    h.handle_key(&press("a", A));
    h.handle_key(&press("z", X));
    // "a" is still held: its next repeat starts over.
    assert_eq!(h.handle_key(&press("a", A)), preedit("a"));
}

#[test]
fn test_time_window_expiry() {
    let mut h = handler_with(
        config(LeaderKey::Space, 1),
        &[(MappingKey::Char('a'), &["ä"], false)],
    );
    let wait = || std::thread::sleep(Duration::from_millis(5));

    // Too late for the leader: the key and the space are typed together.
    h.handle_key(&press("a", A));
    wait();
    assert_eq!(h.handle_key(&press("space", SPACE)), commit("a "));

    // Any other key: the held key is committed first.
    h.handle_key(&press("a", A));
    wait();
    assert_eq!(h.handle_key(&press("z", X)), commit_and_forward("a"));

    // Released late: it types itself.
    h.handle_key(&press("a", A));
    wait();
    assert_eq!(h.handle_key(&release("a", A)), commit("a"));

    // Cycling has no time limit.
    let mut h = handler_with(
        config(LeaderKey::Space, 1),
        &[(MappingKey::Char('a'), &["ä", "à"], false)],
    );
    h.handle_key(&press("a", A));
    h.handle_key(&press("space", SPACE));
    wait();
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("à"));
}

#[test]
fn test_shortcuts_commit_the_held_key_and_pass() {
    let mut h = handler(&["ä"]);

    h.handle_key(&press("a", A));
    assert_eq!(
        h.handle_key(&with(press("c", X), CTRL)),
        commit_and_forward("a")
    );

    // Ctrl+Space is a shortcut (e.g. the input method toggle), not Space.
    h.handle_key(&press("a", A));
    h.handle_key(&release("a", A));
    assert_eq!(
        h.handle_key(&with(press("space", SPACE), CTRL)),
        Action::pass_through()
    );

    let super_key = Modifiers {
        super_key: true,
        ..Modifiers::default()
    };
    assert!(super_key.shortcut());
    assert_eq!(
        h.handle_key(&with(press("x", X), super_key)),
        Action::pass_through()
    );
}

#[test]
fn test_modifier_keys_pass_without_ending_the_gesture() {
    let mut h = handler(&["ä"]);

    h.handle_key(&press("a", A));
    let shift = KeyEvent {
        is_modifier: true,
        ..press("", SHIFT)
    };
    assert_eq!(h.handle_key(&shift), Action::pass_through());
    assert_eq!(h.current_pressed_key(), Some(&MappingKey::Char('a')));
}

#[test]
fn test_shifted_keys_use_their_own_mapping() {
    let mut h = handler_with(
        config(LeaderKey::Space, 10_000),
        &[(MappingKey::ShiftChar('A'), &["Ä"], false)],
    );
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };

    assert_eq!(h.handle_key(&with(press("A", A), shift)), preedit("A"));
    assert_eq!(
        h.handle_key(&with(press("space", SPACE), shift)),
        commit("Ä")
    );
}

#[test]
fn test_alt_leader() {
    let mut h = handler_with(
        config(LeaderKey::Alt, 10_000),
        &[
            (MappingKey::Char('a'), &["ä", "à"], false),
            (MappingKey::Char('o'), &["ö"], false),
        ],
    );
    let alt = KeyEvent {
        is_modifier: true,
        ..press("alt", ALT)
    };

    // Without a gesture Alt is just a modifier.
    assert_eq!(h.handle_key(&alt), Action::pass_through());
    assert_eq!(h.handle_key(&release("alt", ALT)), Action::pass_through());
    // And Alt+x is a shortcut.
    assert_eq!(
        h.handle_key(&with(press("x", X), ALT_HELD)),
        Action::pass_through()
    );

    // During a gesture Alt leads, and repeats of the held key with Alt
    // down belong to the gesture instead of being shortcuts.
    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&alt), preedit("ä"));
    assert_eq!(h.handle_key(&with(press("a", A), ALT_HELD)), swallow());
    // Ctrl or Super with Alt are still shortcuts.
    let ctrl_alt = Modifiers {
        ctrl: true,
        ..ALT_HELD
    };
    let super_alt = Modifiers {
        super_key: true,
        ..ALT_HELD
    };
    assert!(h.handle_key(&with(press("x", X), super_alt)).forward);
    h.handle_key(&press("a", A));
    h.handle_key(&alt);
    assert!(h.handle_key(&with(press("x", X), ctrl_alt)).forward);

    // The consumed Alt's release is swallowed; releasing the key commits.
    h.handle_key(&press("a", A));
    h.handle_key(&alt);
    assert_eq!(h.handle_key(&release("alt", ALT)), swallow());
    assert_eq!(h.handle_key(&release("a", A)), commit("ä"));

    // After a direct commit Alt is still held: Alt+x ends the session.
    h.handle_key(&press("o", O));
    assert_eq!(h.handle_key(&alt), commit("ö"));
    assert_eq!(
        h.handle_key(&with(press("x", X), ALT_HELD)),
        Action::pass_through()
    );
}

#[test]
fn test_altgr_leader() {
    let mut h = handler_with(
        config(LeaderKey::AltGr, 10_000),
        &[(MappingKey::Char('a'), &["ä"], false)],
    );

    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("altgr", ALT)), commit("ä"));
    assert_eq!(h.handle_key(&release("altgr", ALT)), swallow());
}

#[test]
fn test_custom_leaders() {
    // Letters match case-insensitively (Shift may be held for capitals).
    let mut h = handler_with(
        config(LeaderKey::Custom("f".to_string()), 10_000),
        &[(MappingKey::Char('a'), &["ä"], false)],
    );
    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("F", X)), commit("ä"));

    // An empty custom leader never matches.
    let mut h = handler_with(
        config(LeaderKey::Custom(String::new()), 10_000),
        &[(MappingKey::Char('a'), &["ä"], false)],
    );
    h.handle_key(&press("a", A));
    assert_eq!(h.handle_key(&press("", X)), commit_and_forward("a"));
}

#[test]
fn test_leader_without_a_gesture_passes() {
    let mut h = handler(&["ä"]);
    assert_eq!(h.handle_key(&press("space", SPACE)), Action::pass_through());
}

#[test]
fn test_mapping_changes_during_a_gesture() {
    // The mapping disappears: the leader is an ordinary key again and the
    // held key is committed as itself.
    let mut h = handler(&["ä", "à"]);
    h.handle_key(&press("a", A));
    h.mapping_manager_mut().profiles.clear();
    h.mapping_manager_mut().current_profile = None;
    assert_eq!(
        h.handle_key(&press("space", SPACE)),
        commit_and_forward("a")
    );

    // The mapping loses its variants.
    let mut h = handler(&["ä", "à"]);
    h.handle_key(&press("a", A));
    h.mapping_manager_mut()
        .current_profile_mut()
        .unwrap()
        .mappings
        .get_mut(&MappingKey::Char('a'))
        .unwrap()
        .characters
        .clear();
    assert_eq!(
        h.handle_key(&press("space", SPACE)),
        commit_and_forward("a")
    );

    // While cycling: releasing commits the key itself.
    let mut h = handler(&["ä", "à"]);
    h.handle_key(&press("a", A));
    h.handle_key(&press("space", SPACE));
    h.mapping_manager_mut().profiles.clear();
    // Nothing left to cycle through.
    assert_eq!(h.cycle(), None);
    assert_eq!(h.handle_key(&release("a", A)), commit("a"));
}

#[test]
fn test_reset() {
    let mut h = handler(&["ä"]);

    h.handle_key(&press("a", A));
    assert_eq!(h.reset(), commit("a"));
    assert_eq!(h.reset(), swallow());
    // Nothing is held or pending afterwards.
    assert_eq!(h.handle_key(&press("space", SPACE)), Action::pass_through());
    assert_eq!(h.handle_key(&press("a", A)), preedit("a"));
}

#[test]
fn test_sessions_are_independent() {
    let mut h = handler(&["ä", "à"]);
    let mut other = Session::default();

    // A key held in one window...
    h.handle_key(&press("a", A));
    h.handle_key(&press("space", SPACE));
    // ...is not held in another, where the leader is an ordinary key.
    assert_eq!(
        h.handler.handle_key(&mut other, &press("space", SPACE)),
        Action::pass_through()
    );
    assert_eq!(other.pressed_key(), None);
    assert_eq!(h.handler.cycle(&other), None);

    // The first window carries on where it was.
    assert_eq!(h.handle_key(&press("space", SPACE)), preedit("à"));
}

#[test]
fn test_enable_toggles_and_app_filter() {
    let mut h = handler(&["ä"]);
    h.config_mut().filtered_apps = vec!["vim".to_string()];

    // Enabled by default; the blacklist lets unlisted apps through.
    assert!(h.is_enabled());
    assert!(h.is_enabled_for_app("kitty"));
    assert!(!h.is_enabled_for_app("vim"));

    // Disabling blocks every app.
    h.set_enabled(false);
    assert!(!h.is_enabled());
    assert!(!h.is_enabled_for_app("kitty"));

    h.set_enabled(true);
    assert!(h.is_enabled_for_app("kitty"));
}

#[test]
fn test_accessors_expose_config_and_mappings() {
    let mut h = handler(&["ä"]);

    assert_eq!(h.config().time_window_ms, 10_000);
    assert!(h.mapping_manager().current_profile().is_some());

    h.config_mut().time_window_ms = 42;
    assert_eq!(h.config().time_window_ms, 42);
}
