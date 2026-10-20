//! Tests for the Fcitx5 integration: the addon state, the decoding of C ABI
//! key events and the error type. (The raw C ABI is tested inside the lib's
//! own unit tests so the #[no_mangle] symbols stay referenced in every
//! compiled copy — see crates/fcitx5/src/addon.rs.)

use accents_rapides_core::{
    mappings_path, usage_path, Action, AppFilterMode, Config, CoreError, KeyEvent, KeyMapping,
    MappingKey, MappingManager, MappingProfile, Modifiers, OverlayMessage, OverlayReceiver,
    OverlaySender, Session,
};
use accents_rapides_fcitx5::{key_event, key_flags, FcitxAddon, FcitxError};
use accents_rapides_test_support::{with_temp_config, TempConfig};
use anyhow::Result;
use std::fs::{self, File};
use std::path::Path;
use std::time::SystemTime;

fn press(key: &str) -> KeyEvent<'_> {
    key_event(key, 30, 0)
}

fn release(key: &str) -> KeyEvent<'_> {
    key_event(key, 30, key_flags::RELEASE)
}

/// A press of the default leader, Alt: a modifier, on a key of its own.
fn leader() -> KeyEvent<'static> {
    key_event("alt", 64, key_flags::MODIFIER)
}

/// Hold "a", press the leader and release "a" in `session`, from kitty:
/// what the leader committed.
fn type_umlaut_a(addon: &mut FcitxAddon, session: &mut Session) -> Option<String> {
    let mut actions =
        [press("a"), leader(), release("a")].map(|event| addon.key_event(session, "kitty", &event));
    actions[1].commit.take()
}

#[test]
fn test_key_event_decodes_flags() {
    assert_eq!(
        key_event("a", 7, 0),
        KeyEvent {
            key: "a",
            code: 7,
            ..KeyEvent::default()
        }
    );

    let all = key_flags::RELEASE
        | key_flags::SHIFT
        | key_flags::CTRL
        | key_flags::ALT
        | key_flags::SUPER
        | key_flags::MODIFIER;
    assert_eq!(
        key_event("alt", 64, all),
        KeyEvent {
            key: "alt",
            code: 64,
            release: true,
            is_modifier: true,
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                alt: true,
                super_key: true,
            },
        }
    );
}

#[test]
fn test_addon_runs_the_gesture_engine() -> Result<()> {
    let mut addon = with_temp_config(FcitxAddon::new)?;
    let mut session = Session::default();
    assert_eq!(addon.handler().config().time_window_ms, 500);

    let action = addon.key_event(&mut session, "kitty", &press("a"));
    assert_eq!(action.preedit.as_deref(), Some("a"));
    assert!(!action.forward);

    // Reset hands back the pending key.
    assert_eq!(addon.reset(&mut session).commit.as_deref(), Some("a"));
    Ok(())
}

#[test]
fn test_addon_app_filter_passes_keys_through() -> Result<()> {
    let mut addon = with_temp_config(FcitxAddon::new)?;
    let mut session = Session::default();
    let config = addon.handler_mut().config_mut();
    config.app_filter_mode = AppFilterMode::Blacklist;
    config.filtered_apps = vec!["KeePassXC".to_string()];

    assert_eq!(
        addon.key_event(&mut session, "keepassxc", &press("a")),
        Action::pass_through()
    );
    assert!(!addon.key_event(&mut session, "kitty", &press("a")).forward);
    Ok(())
}

#[test]
fn test_addon_reload() -> Result<()> {
    let temp = TempConfig::new();
    let mut addon = temp.with(FcitxAddon::new)?;
    let mut session = Session::default();
    addon.handler_mut().config_mut().time_window_ms = 7;
    assert_eq!(
        type_umlaut_a(&mut addon, &mut session).as_deref(),
        Some("ä")
    );

    // Reloading starts over with the files on disk: the configuration, and
    // the usage counts saved by the commit above.
    temp.with(|| addon.reload())?;
    assert_eq!(addon.handler().config().time_window_ms, 500);
    let stats = &addon.handler().mapping_manager().usage_stats;
    assert_eq!(stats[&MappingKey::Char('a')]["ä"], 1);

    // A failed reload keeps the current state.
    let result = TempConfig::unwritable().with(|| addon.reload());
    assert!(matches!(result, Err(FcitxError::Config(_))));
    Ok(())
}

/// Save `config` at `temp`, with a modification time unlike any so far.
fn save(temp: &TempConfig, config: &Config) -> Result<()> {
    config.save_to_file(&temp.path)?;
    touch(&temp.path)
}

/// Give the file at `path` a modification time unlike any so far: writes
/// in quick succession may share one.
fn touch(path: &Path) -> Result<()> {
    static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let secs = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    File::options().write(true).open(path)?.set_modified(time)?;
    Ok(())
}

#[test]
fn test_addon_refresh_reloads_changed_configuration_only() -> Result<()> {
    let temp = TempConfig::new();
    let mut addon = temp.with(FcitxAddon::new)?;
    let time_window = |addon: &FcitxAddon| addon.handler().config().time_window_ms;

    // Unchanged on disk: nothing is reloaded, not even in-memory changes.
    addon.handler_mut().config_mut().time_window_ms = 7;
    assert!(!temp.with(|| addon.refresh())?);
    assert_eq!(time_window(&addon), 7);

    // Saved (by the editor, or by hand): reloaded.
    let config = Config {
        time_window_ms: 42,
        ..Config::default()
    };
    save(&temp, &config)?;
    assert!(temp.with(|| addon.refresh())?);
    assert_eq!(time_window(&addon), 42);
    assert!(!temp.with(|| addon.refresh())?);

    // Deleted: recreated with the defaults.
    fs::remove_file(&temp.path)?;
    assert!(temp.with(|| addon.refresh())?);
    assert_eq!(time_window(&addon), 500);
    assert!(temp.path.exists());

    // Broken: the current configuration stays.
    fs::write(&temp.path, "not = = toml")?;
    touch(&temp.path)?;
    let result = temp.with(|| addon.refresh());
    assert!(matches!(
        result,
        Err(FcitxError::Config(CoreError::ParseConfig { .. }))
    ));
    assert_eq!(time_window(&addon), 500);
    Ok(())
}

#[test]
fn test_addon_refresh_reloads_changed_mappings() -> Result<()> {
    let temp = TempConfig::new();
    let mut addon = temp.with(FcitxAddon::new)?;
    let mappings_file = mappings_path(&temp.path);
    assert!(!temp.with(|| addon.refresh())?);

    // The editor saved a new mapping: picked up on the next focus.
    let mut mappings = MappingManager::new();
    let mut profile = MappingProfile::new("Mine");
    profile.mappings.insert(
        MappingKey::Char('n'),
        KeyMapping::new(vec!["ñ".to_string()]),
    );
    mappings.add_profile(profile);
    mappings.set_current_profile("Mine");
    mappings.save_to_file(&mappings_file)?;
    touch(&mappings_file)?;
    assert!(temp.with(|| addon.refresh())?);
    assert_eq!(
        addon
            .handler()
            .mapping_manager()
            .get_characters_for_key(&MappingKey::Char('n')),
        Some(["ñ".to_string()].as_slice())
    );
    assert!(!temp.with(|| addon.refresh())?);

    // Deleted: recreated with the default profile.
    fs::remove_file(&mappings_file)?;
    assert!(temp.with(|| addon.refresh())?);
    assert!(mappings_file.exists());
    assert_eq!(
        addon
            .handler()
            .mapping_manager()
            .get_characters_for_key(&MappingKey::Char('n')),
        None
    );
    Ok(())
}

/// The messages `addon` sends the overlay at `temp` while `f` runs.
fn overlay_messages(temp: &TempConfig, f: impl FnOnce()) -> Result<Vec<OverlayMessage>> {
    let receiver = OverlayReceiver::bind(&temp.overlay_socket)?;
    f();
    // Everything before this marker came from the addon.
    let end = OverlayMessage::Show {
        variants: Vec::new(),
        index: usize::MAX,
    };
    OverlaySender::new(temp.overlay_socket.clone())?.send(&end);
    Ok(receiver.messages().take_while(|m| *m != end).collect())
}

/// Map "e" to "é" and "è", so that it cycles.
fn map_cycling_e(addon: &mut FcitxAddon) {
    addon
        .handler_mut()
        .mapping_manager_mut()
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
}

#[test]
fn test_addon_feeds_the_overlay_while_cycling() -> Result<()> {
    let temp = TempConfig::new();
    let mut addon = temp.with(FcitxAddon::new)?;
    map_cycling_e(&mut addon);
    let mut session = Session::default();
    let show = |index| OverlayMessage::Show {
        variants: vec!["é".to_string(), "è".to_string()],
        index,
    };

    // Only changes are sent: holding "e", or typing "z" once hidden, sends
    // nothing.
    let messages = overlay_messages(&temp, || {
        for event in [press("e"), leader(), leader(), release("e"), press("z")] {
            addon.key_event(&mut session, "kitty", &event);
        }
    })?;
    assert_eq!(messages, [show(0), show(1), OverlayMessage::Hide]);

    // Resetting the window hides the overlay too.
    let messages = overlay_messages(&temp, || {
        addon.key_event(&mut session, "kitty", &press("e"));
        addon.key_event(&mut session, "kitty", &leader());
        addon.reset(&mut session);
    })?;
    assert_eq!(messages, [show(0), OverlayMessage::Hide]);

    // Turned off in the configuration: nothing at all.
    addon.handler_mut().config_mut().show_overlay = false;
    let messages = overlay_messages(&temp, || {
        addon.key_event(&mut session, "kitty", &press("e"));
        addon.key_event(&mut session, "kitty", &leader());
    })?;
    assert!(messages.is_empty());
    Ok(())
}

#[test]
fn test_addon_persists_usage_counts() -> Result<()> {
    let temp = TempConfig::new();
    let usage_file = usage_path(&temp.path);
    let mut addon = temp.with(FcitxAddon::new)?;
    let mut session = Session::default();

    // Typing the plain key records nothing.
    addon.key_event(&mut session, "kitty", &press("a"));
    addon.key_event(&mut session, "kitty", &release("a"));
    assert!(!usage_file.exists());

    // A variant is counted as soon as it is committed, by the leader...
    type_umlaut_a(&mut addon, &mut session);
    assert_eq!(fs::read_to_string(&usage_file)?, "[a]\n\"ä\" = 1\n");

    // ...or by a reset while cycling.
    map_cycling_e(&mut addon);
    addon.key_event(&mut session, "kitty", &press("e"));
    addon.key_event(&mut session, "kitty", &leader());
    addon.reset(&mut session);
    assert_eq!(
        fs::read_to_string(&usage_file)?,
        "[a]\n\"ä\" = 1\n\n[e]\n\"é\" = 1\n"
    );

    // The usage file is the addon's own: writing it is not a change to
    // pick up.
    assert!(!temp.with(|| addon.refresh())?);

    // The counts survive a restart.
    let addon = temp.with(FcitxAddon::new)?;
    let stats = &addon.handler().mapping_manager().usage_stats;
    assert_eq!(stats[&MappingKey::Char('a')]["ä"], 1);
    assert_eq!(stats[&MappingKey::Char('e')]["é"], 1);
    Ok(())
}

#[test]
fn test_addon_starts_with_variants_sorted_by_usage() -> Result<()> {
    let temp = TempConfig::new();
    let config = Config {
        sort_by_usage: true,
        ..Config::default()
    };
    config.save_to_file(&temp.path)?;
    fs::write(
        mappings_path(&temp.path),
        "current_profile = \"Mine\"\n[profiles.Mine.mappings.e]\ncharacters = [\"é\", \"è\"]\n",
    )?;
    fs::write(usage_path(&temp.path), "[e]\n\"è\" = 3\n\"é\" = 1\n")?;

    // The most used variant comes first from the start, not only after
    // the first commit.
    let mut addon = temp.with(FcitxAddon::new)?;
    let mut session = Session::default();
    addon.key_event(&mut session, "kitty", &press("e"));
    let action = addon.key_event(&mut session, "kitty", &leader());
    assert_eq!(action.preedit.as_deref(), Some("è"));
    Ok(())
}

#[test]
fn test_addon_tolerates_usage_file_failures() -> Result<()> {
    let temp = TempConfig::new();
    let usage_file = usage_path(&temp.path);
    let mut session = Session::default();

    // A broken usage file: the addon starts without counts, and the next
    // variant typed replaces the file.
    fs::write(&usage_file, "not = = toml")?;
    let mut addon = temp.with(FcitxAddon::new)?;
    assert!(addon.handler().mapping_manager().usage_stats.is_empty());
    assert_eq!(
        type_umlaut_a(&mut addon, &mut session).as_deref(),
        Some("ä")
    );
    assert_eq!(fs::read_to_string(&usage_file)?, "[a]\n\"ä\" = 1\n");

    // A usage file that can be neither read nor written: typing goes on,
    // with the counts in memory.
    fs::remove_file(&usage_file)?;
    fs::create_dir(&usage_file)?;
    let mut addon = temp.with(FcitxAddon::new)?;
    assert_eq!(
        type_umlaut_a(&mut addon, &mut session).as_deref(),
        Some("ä")
    );
    let stats = &addon.handler().mapping_manager().usage_stats;
    assert_eq!(stats[&MappingKey::Char('a')]["ä"], 1);
    assert!(usage_file.is_dir());
    Ok(())
}

#[test]
fn test_addon_without_an_overlay_socket() -> Result<()> {
    // Nowhere to send to: the addon works the same.
    let temp = TempConfig::new();
    let mut addon = accents_rapides_test_support::with_env(
        &[
            (
                accents_rapides_test_support::CONFIG_PATH_ENV,
                Some(temp.as_str()),
            ),
            (accents_rapides_test_support::OVERLAY_SOCKET_ENV, None),
            ("XDG_RUNTIME_DIR", None),
        ],
        FcitxAddon::new,
    )?;
    map_cycling_e(&mut addon);
    let mut session = Session::default();
    addon.key_event(&mut session, "kitty", &press("e"));
    let action = addon.key_event(&mut session, "kitty", &leader());
    assert_eq!(action.preedit.as_deref(), Some("é"));
    Ok(())
}

#[test]
fn test_addon_creation_propagates_config_errors() {
    let result = TempConfig::unwritable().with(FcitxAddon::new);
    assert!(matches!(result, Err(FcitxError::Config(_))));
}

#[test]
fn test_fcitx_error_messages() {
    let core_err =
        accents_rapides_core::Config::load_from_file(std::path::Path::new("/nonexistent/x.toml"))
            .expect_err("missing file must fail");
    let err = FcitxError::from(core_err);
    assert!(err.to_string().contains("failed to load configuration"));

    assert_eq!(FcitxError::NullPointer.to_string(), "null pointer argument");
    assert_eq!(
        FcitxError::InvalidCString("bad".to_string()).to_string(),
        "invalid C string argument: bad"
    );
}
