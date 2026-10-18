//! Tests for the editor crate: loading, the iced UI (driven headlessly with
//! iced_test), saving and the binary entry point.

use accents_rapides_core::{
    mappings_path, AppFilterMode, Config, KeyMapping, LeaderKey, MappingKey, MappingManager,
    MappingProfile,
};
use accents_rapides_editor::app::{input_id, Editor, Message, Tab, NO_PROFILE, SAVED};
use accents_rapides_editor::load_editor;
use accents_rapides_test_support::TempConfig;
use anyhow::Result;
use iced_test::{selector, simulator};
use std::fs;

/// An editor with the default profile loaded, and nowhere to save.
fn default_editor() -> Editor {
    let mut mappings = MappingManager::new();
    mappings.load_default_mappings();
    Editor::new(Config::default(), mappings)
}

/// An editor with the default profile and a second one, `Second`, holding
/// an `e` mapping; nowhere to save.
fn two_profile_editor() -> Editor {
    let mut mappings = MappingManager::new();
    mappings.load_default_mappings();
    let mut second = MappingProfile::new("Second");
    second.mappings.insert(
        MappingKey::Char('e'),
        KeyMapping::new(vec!["é".to_string(), "è".to_string()]),
    );
    mappings.add_profile(second);
    Editor::new(Config::default(), mappings)
}

/// An editor loaded from (and saving to) `temp`.
fn saved_editor(temp: &TempConfig) -> Result<Editor> {
    temp.with(load_editor)
}

/// Whether the editor's current view shows `label` (a text or the content
/// of a text input).
fn shows(editor: &Editor, label: &str) -> bool {
    simulator(editor.view()).find(label).is_ok()
}

/// Click `label` in the editor's view and apply the resulting messages.
fn click(editor: &mut Editor, label: &str) {
    let mut ui = simulator(editor.view());
    ui.click(label).expect("clickable label");
    for message in ui.into_messages() {
        editor.update(message);
    }
}

/// Click the text input `id`, type `text` into it and apply the resulting
/// messages.
fn type_into(editor: &mut Editor, id: &'static str, text: &str) {
    let mut ui = simulator(editor.view());
    ui.click(selector::id(id)).expect("text input");
    ui.typewrite(text);
    for message in ui.into_messages() {
        editor.update(message);
    }
}

/// The text shown by the text input `id` (its placeholder when empty).
fn input_content(editor: &Editor, id: &'static str) -> String {
    match simulator(editor.view())
        .find(selector::id(id))
        .expect("text input")
    {
        selector::Target::TextInput { content, .. } => content,
        other => panic!("not a text input: {other:?}"),
    }
}

/// The variants of `key` in the editor's current profile.
fn variants(editor: &Editor, key: MappingKey) -> Option<Vec<String>> {
    editor
        .mappings()
        .get_characters_for_key(&key)
        .map(<[String]>::to_vec)
}

fn current_profile(editor: &Editor) -> Option<&str> {
    editor.mappings().current_profile.as_deref()
}

#[test]
fn test_load_editor_loads_or_creates_config() -> Result<()> {
    let temp = TempConfig::new();

    let editor = saved_editor(&temp)?;

    assert_eq!(editor.config.config_path, Some(temp.path.clone()));
    assert_eq!(current_profile(&editor), Some("Default"));
    assert_eq!(editor.tab, Tab::Profiles);
    assert_eq!(editor.status, "");
    // Both files were created on first use.
    assert!(temp.path.exists());
    assert!(mappings_path(&temp.path).exists());

    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_run_editor_under_coverage() -> Result<()> {
    // Under coverage measurement the window launch is skipped, so
    // `run_editor` runs the setup logic and returns normally.
    accents_rapides_test_support::with_temp_config(accents_rapides_editor::run_editor)
}

#[test]
fn test_tabs() {
    let mut editor = default_editor();
    assert!(shows(&editor, "Accents Rapides Editor"));
    assert!(shows(&editor, "Default (current)"));

    click(&mut editor, "Settings");
    assert_eq!(editor.tab, Tab::Settings);
    assert!(shows(&editor, "Leader key"));
    assert!(!shows(&editor, "Default (current)"));

    click(&mut editor, "Mappings");
    assert_eq!(editor.tab, Tab::Mappings);
    assert!(shows(&editor, "Mappings of Default"));

    click(&mut editor, "Profiles");
    assert_eq!(editor.tab, Tab::Profiles);
    assert!(shows(&editor, "Default (current)"));
    // Switching tabs changes nothing worth saving.
    assert_eq!(editor.status, "");
}

#[test]
fn test_profiles_tab_marks_only_the_current_profile() {
    let editor = two_profile_editor();

    assert!(shows(&editor, "Default (current)"));
    assert!(shows(&editor, "Second"));
    assert!(!shows(&editor, "Second (current)"));

    let mut editor = Editor::new(Config::default(), MappingManager::new());
    assert!(shows(&editor, "Add profile"));
    assert!(!shows(&editor, "Default (current)"));
    click(&mut editor, "Mappings");
    assert!(shows(&editor, NO_PROFILE));
}

#[test]
fn test_add_profile() {
    let mut editor = default_editor();

    // Nothing typed, or an existing name: the button does nothing (and
    // neither does a stale message).
    click(&mut editor, "Add profile");
    editor.update(Message::AddProfile);
    editor.update(Message::NewProfileName(" Default ".to_string()));
    click(&mut editor, "Add profile");
    assert_eq!(editor.mappings().profiles.len(), 1);
    assert_eq!(editor.status, "");
    editor.update(Message::NewProfileName(String::new()));

    type_into(&mut editor, input_id::PROFILE, "French");
    assert_eq!(input_content(&editor, input_id::PROFILE), "French");
    click(&mut editor, "Add profile");

    assert!(editor.mappings().profiles.contains_key("French"));
    // The new profile is listed but not current; the input is cleared.
    assert!(shows(&editor, "French"));
    assert!(shows(&editor, "Default (current)"));
    assert!(!shows(&editor, "French (current)"));
    assert_ne!(input_content(&editor, input_id::PROFILE), "French");
}

#[test]
fn test_use_and_delete_profiles() {
    let mut editor = two_profile_editor();

    // Use and Delete are disabled on the current profile (the first row).
    click(&mut editor, "Use");
    click(&mut editor, "Delete");
    assert_eq!(current_profile(&editor), Some("Default"));
    assert_eq!(editor.mappings().profiles.len(), 2);

    editor.update(Message::UseProfile("Second".to_string()));
    assert_eq!(current_profile(&editor), Some("Second"));
    assert!(shows(&editor, "Second (current)"));
    // The mappings tab follows the current profile.
    click(&mut editor, "Mappings");
    assert!(shows(&editor, "Mappings of Second"));
    assert!(shows(&editor, "é,è"));
    assert!(!shows(&editor, "ä"));

    // Now the first row (Default) can be used and deleted.
    click(&mut editor, "Profiles");
    click(&mut editor, "Use");
    assert_eq!(current_profile(&editor), Some("Default"));
    editor.update(Message::UseProfile("Second".to_string()));
    click(&mut editor, "Delete");
    assert_eq!(
        editor.mappings().profiles.keys().collect::<Vec<_>>(),
        ["Second"]
    );

    // Unknown names and the current profile change nothing.
    editor.update(Message::UseProfile("Nope".to_string()));
    editor.update(Message::DeleteProfile("Nope".to_string()));
    editor.update(Message::DeleteProfile("Second".to_string()));
    assert_eq!(current_profile(&editor), Some("Second"));
    assert_eq!(editor.mappings().profiles.len(), 1);
}

#[test]
fn test_mappings_tab_lists_current_profile_mappings() {
    let mut editor = default_editor();
    click(&mut editor, "Mappings");

    assert!(shows(&editor, "a"));
    assert!(shows(&editor, "ä"));
    assert!(shows(&editor, "Shift+A"));
    assert!(shows(&editor, "Ä"));
    assert!(shows(&editor, "Reverse"));
    assert!(shows(&editor, "Remove"));
}

#[test]
fn test_add_mapping() {
    let mut editor = default_editor();
    click(&mut editor, "Mappings");

    // No key, an invalid key or a mapped key: the button does nothing.
    for key in ["", "ab", "a", " shift+a "] {
        editor.update(Message::NewKey(key.to_string()));
        click(&mut editor, "Add mapping");
        assert_eq!(
            editor.mappings().current_profile().unwrap().mappings.len(),
            7
        );
    }
    editor.update(Message::NewKey(String::new()));

    type_into(&mut editor, input_id::KEY, "e");
    type_into(&mut editor, input_id::OUTPUT, "é,è");
    assert!(shows(&editor, "é,è"));
    click(&mut editor, "Add mapping");

    assert_eq!(
        variants(&editor, MappingKey::Char('e')).as_deref(),
        Some(["é".to_string(), "è".to_string()].as_slice())
    );
    // The new row shows what was typed and the add row is cleared.
    assert!(shows(&editor, "é,è"));
    assert_ne!(input_content(&editor, input_id::KEY), "e");
    assert_ne!(input_content(&editor, input_id::OUTPUT), "é,è");

    // The output may be filled in later.
    type_into(&mut editor, input_id::KEY, "shift+e");
    click(&mut editor, "Add mapping");
    assert_eq!(
        variants(&editor, MappingKey::ShiftChar('E')).as_deref(),
        Some([].as_slice())
    );
}

#[test]
fn test_edit_mappings() {
    let mut editor = two_profile_editor();
    editor.update(Message::UseProfile("Second".to_string()));
    click(&mut editor, "Mappings");

    // Typing into the output keeps it as typed while the variants follow.
    let mut ui = simulator(editor.view());
    ui.click("é,è").expect("output input");
    ui.typewrite(",");
    for message in ui.into_messages() {
        editor.update(message);
    }
    assert!(shows(&editor, "é,è,"));
    assert_eq!(
        variants(&editor, MappingKey::Char('e')).as_deref(),
        Some(["é".to_string(), "è".to_string()].as_slice())
    );
    editor.update(Message::Output(0, "ê".to_string()));
    assert!(shows(&editor, "ê"));
    assert_eq!(
        variants(&editor, MappingKey::Char('e')).as_deref(),
        Some(["ê".to_string()].as_slice())
    );

    click(&mut editor, "Reverse");
    assert!(
        editor
            .mappings()
            .get_mapping(&MappingKey::Char('e'))
            .unwrap()
            .reverse_cycle
    );

    // Stale rows change nothing.
    editor.update(Message::Output(9, "x".to_string()));
    editor.update(Message::Reverse(9, false));
    editor.update(Message::RemoveMapping(9));
    assert_eq!(
        variants(&editor, MappingKey::Char('e')).map(|v| v.len()),
        Some(1)
    );

    click(&mut editor, "Remove");
    assert_eq!(variants(&editor, MappingKey::Char('e')), None);
    assert!(!shows(&editor, "ê"));

    // Without a current profile there is nothing to edit or add to.
    let mut editor = Editor::new(Config::default(), MappingManager::new());
    editor.update(Message::Output(0, "x".to_string()));
    editor.update(Message::NewKey("e".to_string()));
    editor.update(Message::AddMapping);
    editor.update(Message::RemoveMapping(0));
    assert_eq!(editor.status, "");
}

#[test]
fn test_settings_leader_key() {
    let mut editor = default_editor();
    click(&mut editor, "Settings");

    click(&mut editor, "Alt");
    assert_eq!(editor.config.leader_key, LeaderKey::Alt);

    // Custom: the key is typed in the input, which is enabled only then.
    type_into(&mut editor, input_id::CUSTOM_LEADER, "#");
    assert_eq!(editor.config.leader_key, LeaderKey::Alt);
    click(&mut editor, "Custom");
    assert_eq!(editor.config.leader_key, LeaderKey::Custom(String::new()));
    type_into(&mut editor, input_id::CUSTOM_LEADER, "#");
    assert_eq!(editor.config.leader_key, LeaderKey::Custom("#".to_string()));
    assert!(shows(&editor, "#"));
    // Choosing Custom again keeps the key.
    click(&mut editor, "Custom");
    assert_eq!(editor.config.leader_key, LeaderKey::Custom("#".to_string()));

    click(&mut editor, "→");
    assert_eq!(editor.config.leader_key, LeaderKey::RightArrow);
}

#[test]
fn test_settings_time_window() {
    let mut editor = default_editor();
    click(&mut editor, "Settings");
    assert!(shows(&editor, "Time window: 500 ms"));

    click(&mut editor, "+");
    assert_eq!(editor.config.time_window_ms, 550);
    click(&mut editor, "−");
    click(&mut editor, "−");
    assert_eq!(editor.config.time_window_ms, 450);

    // The buttons stop at the bounds; messages are clamped to them.
    editor.update(Message::TimeWindow(5));
    assert_eq!(editor.config.time_window_ms, 50);
    click(&mut editor, "−");
    assert_eq!(editor.config.time_window_ms, 50);
    editor.update(Message::TimeWindow(9_999));
    assert_eq!(editor.config.time_window_ms, 2000);
    click(&mut editor, "+");
    assert_eq!(editor.config.time_window_ms, 2000);
}

#[test]
fn test_settings_toggles_and_filter() {
    let mut editor = default_editor();
    click(&mut editor, "Settings");

    click(&mut editor, "Show the cycle overlay");
    assert!(!editor.config.show_overlay);
    click(&mut editor, "Sort variants by usage");
    assert!(editor.config.sort_by_usage);

    click(&mut editor, "Whitelist");
    assert_eq!(editor.config.app_filter_mode, AppFilterMode::Whitelist);
    click(&mut editor, "Blacklist");
    assert_eq!(editor.config.app_filter_mode, AppFilterMode::Blacklist);
}

#[test]
fn test_settings_app_list() {
    let mut editor = default_editor();
    click(&mut editor, "Settings");

    // Nothing typed: the button does nothing, nor does a stale message.
    click(&mut editor, "Add application");
    editor.update(Message::AddApp);
    assert!(editor.config.filtered_apps.is_empty());

    type_into(&mut editor, input_id::APP, "KeePassXC");
    click(&mut editor, "Add application");
    assert_eq!(editor.config.filtered_apps, ["KeePassXC"]);
    assert!(shows(&editor, "KeePassXC"));

    // Already listed: the button does nothing.
    editor.update(Message::NewApp("KeePassXC".to_string()));
    click(&mut editor, "Add application");
    editor.update(Message::NewApp("Steam".to_string()));
    click(&mut editor, "Add application");
    assert_eq!(editor.config.filtered_apps, ["KeePassXC", "Steam"]);

    click(&mut editor, "Remove");
    assert_eq!(editor.config.filtered_apps, ["Steam"]);
    editor.update(Message::RemoveApp(5));
    assert_eq!(editor.config.filtered_apps, ["Steam"]);
}

#[test]
fn test_changes_are_saved() -> Result<()> {
    let temp = TempConfig::new();
    let mut editor = saved_editor(&temp)?;

    editor.update(Message::Leader(LeaderKey::AltGr));
    assert_eq!(editor.status, SAVED);
    assert!(shows(&editor, SAVED));
    assert_eq!(
        Config::load_from_file(&temp.path)?.leader_key,
        LeaderKey::AltGr
    );

    editor.update(Message::NewProfileName("French".to_string()));
    editor.update(Message::AddProfile);
    editor.update(Message::UseProfile("French".to_string()));
    editor.update(Message::NewKey("e".to_string()));
    editor.update(Message::NewOutput("é,è".to_string()));
    editor.update(Message::AddMapping);
    editor.update(Message::Reverse(0, true));
    assert_eq!(editor.status, SAVED);

    let reloaded = saved_editor(&temp)?;
    assert_eq!(reloaded.config.leader_key, LeaderKey::AltGr);
    assert_eq!(current_profile(&reloaded), Some("French"));
    let e = reloaded
        .mappings()
        .get_mapping(&MappingKey::Char('e'))
        .expect("saved mapping");
    assert_eq!(e.characters, ["é", "è"]);
    assert!(e.reverse_cycle);
    click(&mut editor, "Mappings");
    assert!(shows(&editor, "é,è"));

    Ok(())
}

#[test]
fn test_save_errors_are_reported() -> Result<()> {
    // Nowhere to save.
    let mut editor = default_editor();
    editor.update(Message::ShowOverlay(false));
    assert_eq!(editor.status, "could not determine the default config path");

    // The mappings file cannot be written.
    let temp = TempConfig::new();
    let mut editor = saved_editor(&temp)?;
    fs::remove_file(mappings_path(&temp.path))?;
    fs::create_dir(mappings_path(&temp.path))?;
    editor.update(Message::ShowOverlay(false));
    assert!(
        editor.status.starts_with("failed to write config file"),
        "{}",
        editor.status
    );
    // The configuration itself was saved before that.
    assert!(!Config::load_from_file(&temp.path)?.show_overlay);

    // The config file cannot be written.
    let blocked = TempConfig::unwritable();
    let config = Config {
        config_path: Some(blocked.path.clone()),
        ..Config::default()
    };
    let mut editor = Editor::new(config, MappingManager::new());
    editor.update(Message::ShowOverlay(false));
    assert!(
        editor
            .status
            .starts_with("failed to create config directory"),
        "{}",
        editor.status
    );
    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_editor_binary_entry_point() -> Result<()> {
    // Under coverage the editor never opens a window, so spawning it
    // exercises the entry point, the setup logic and the success path.
    use assert_cmd::prelude::*;
    use std::process::Command;

    let temp = TempConfig::new();

    let mut cmd = Command::cargo_bin("accents-rapides-editor")?;
    cmd.envs(temp.envs());
    cmd.assert().success();
    assert!(temp.path.exists(), "editor must load or create the config");

    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_editor_binary_reports_config_errors() -> Result<()> {
    // A config path whose parent is a regular file makes the editor fail:
    // the entry point must report the error and exit non-zero.
    use assert_cmd::prelude::*;
    use std::process::Command;

    let temp = TempConfig::unwritable();

    let mut cmd = Command::cargo_bin("accents-rapides-editor")?;
    cmd.envs(temp.envs());
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("Failed to run editor"));

    Ok(())
}
