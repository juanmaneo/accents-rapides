//! Tests for configuration management, covering every error path and branch.

use accents_rapides_core::{
    mappings_path, usage_path, AppFilterMode, Config, CoreError, LeaderKey, MappingKey,
    MAPPINGS_FILE, USAGE_FILE,
};
use accents_rapides_test_support::{with_config_path, TempConfig, CONFIG_PATH_ENV};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn test_leader_key_names() {
    assert_eq!(LeaderKey::Space.name(), "space");
    assert_eq!(LeaderKey::LeftArrow.name(), "left");
    assert_eq!(LeaderKey::RightArrow.name(), "right");
    assert_eq!(LeaderKey::UpArrow.name(), "up");
    assert_eq!(LeaderKey::DownArrow.name(), "down");
    assert_eq!(LeaderKey::Alt.name(), "alt");
    assert_eq!(LeaderKey::AltGr.name(), "altgr");
    assert_eq!(LeaderKey::Custom("#".to_string()).name(), "#");
}

#[test]
fn test_load_from_file_success() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
leader_key = "AltGr"
time_window_ms = 250
show_overlay = false
overlay_position = [10, 20]
app_filter_mode = "Whitelist"
filtered_apps = ["vim"]
sort_by_usage = true
"#,
    )?;

    let config = Config::load_from_file(&path)?;

    assert_eq!(config.leader_key, LeaderKey::AltGr);
    assert_eq!(config.time_window_ms, 250);
    assert!(!config.show_overlay);
    assert_eq!(config.overlay_position, (10, 20));
    assert_eq!(config.app_filter_mode, AppFilterMode::Whitelist);
    assert_eq!(config.filtered_apps, vec!["vim".to_string()]);
    assert!(config.sort_by_usage);
    assert_eq!(config.config_path, Some(path.clone()));

    Ok(())
}

#[test]
fn test_load_from_file_missing() {
    let error =
        Config::load_from_file(Path::new("/nonexistent/accents-rapides/config.toml")).unwrap_err();
    match &error {
        CoreError::ReadConfig { path, .. } => {
            assert_eq!(
                path,
                &PathBuf::from("/nonexistent/accents-rapides/config.toml")
            )
        }
        other => panic!("expected ReadConfig, got {other:?}"),
    }
    // Error message includes the path and the source
    let msg = error.to_string();
    assert!(msg.contains("failed to read config file"), "got: {msg}");
    assert!(msg.contains("/nonexistent/accents-rapides/config.toml"));
}

#[test]
fn test_load_from_file_invalid_toml() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("config.toml");
    fs::write(&path, "this is not = = valid toml {{{")?;

    let error = Config::load_from_file(&path).unwrap_err();
    match &error {
        CoreError::ParseConfig { path: p, .. } => assert_eq!(p, &path),
        other => panic!("expected ParseConfig, got {other:?}"),
    }
    assert!(error.to_string().contains("failed to parse config file"));

    Ok(())
}

#[test]
fn test_load_from_file_missing_field() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("config.toml");
    // A config missing required fields fails with a parse error.
    fs::write(&path, "leader_key = \"Space\"\n")?;

    let error = Config::load_from_file(&path).unwrap_err();
    assert!(matches!(error, CoreError::ParseConfig { .. }));
    assert!(error.to_string().contains("missing field"));

    Ok(())
}

#[test]
fn test_save_to_file_success() -> Result<()> {
    let dir = tempdir()?;
    // Saving creates the parent directory when it does not exist yet.
    let path = dir.path().join("nested/dir/config.toml");
    let config = Config::default();
    config.save_to_file(&path)?;

    assert!(path.exists());

    // Round-trip: what we wrote loads back with the same values.
    let loaded = Config::load_from_file(&path)?;
    assert_eq!(loaded.leader_key, config.leader_key);
    assert_eq!(loaded.time_window_ms, config.time_window_ms);
    assert_eq!(loaded.overlay_position, config.overlay_position);
    assert_eq!(loaded.app_filter_mode, config.app_filter_mode);

    Ok(())
}

#[test]
fn test_save_to_file_create_dir_failure() -> Result<()> {
    // A regular file where a parent directory should be makes
    // create_dir_all fail.
    let config = TempConfig::unwritable();

    let error = Config::default().save_to_file(&config.path).unwrap_err();
    match &error {
        CoreError::CreateConfigDir { path: p, .. } => assert_eq!(p, config.path.parent().unwrap()),
        other => panic!("expected CreateConfigDir, got {other:?}"),
    }
    assert!(error
        .to_string()
        .contains("failed to create config directory"));

    Ok(())
}

#[test]
fn test_save_to_file_write_failure() -> Result<()> {
    let dir = tempdir()?;
    // Writing to a path that is a directory fails.
    let path = dir.path().join("config.toml");
    fs::create_dir(&path)?;

    let error = Config::default().save_to_file(&path).unwrap_err();
    match &error {
        CoreError::WriteConfig { path: p, .. } => assert_eq!(p, &path),
        other => panic!("expected WriteConfig, got {other:?}"),
    }
    assert!(error.to_string().contains("failed to write config file"));

    Ok(())
}

#[test]
fn test_default_config_path_env_override() {
    // Without the override the path lives under the platform config dir.
    with_config_path(None, || {
        let path = Config::default_config_path().expect("platform default path");
        assert!(path.to_string_lossy().contains("config.toml"));
        // The env var is not set in this closure.
        assert!(std::env::var(CONFIG_PATH_ENV).is_err());
    });

    // With the override the exact path is returned.
    with_config_path(Some("/tmp/accents-rapides-override/config.toml"), || {
        let path = Config::default_config_path().expect("overridden path");
        assert_eq!(
            path,
            PathBuf::from("/tmp/accents-rapides-override/config.toml")
        );
    });

    // An empty override is ignored and the platform default is used.
    with_config_path(Some(""), || {
        let path = Config::default_config_path().expect("platform default path");
        assert!(path.to_string_lossy().contains("config.toml"));
        assert!(!path.to_string_lossy().is_empty());
    });
}

#[test]
fn test_mappings_and_usage_files_are_next_to_the_config_file() {
    assert_eq!(MAPPINGS_FILE, "mappings.toml");
    assert_eq!(USAGE_FILE, "usage.toml");
    let config_file = Path::new("/etc/accents-rapides/config.toml");
    assert_eq!(
        mappings_path(config_file),
        PathBuf::from("/etc/accents-rapides/mappings.toml")
    );
    assert_eq!(
        usage_path(config_file),
        PathBuf::from("/etc/accents-rapides/usage.toml")
    );

    let mut config = Config::default();
    assert_eq!(config.mappings_path(), None);
    assert_eq!(config.usage_path(), None);
    config.config_path = Some(PathBuf::from("/home/me/.config/ar/config.toml"));
    assert_eq!(
        config.mappings_path(),
        Some(PathBuf::from("/home/me/.config/ar/mappings.toml"))
    );
    assert_eq!(
        config.usage_path(),
        Some(PathBuf::from("/home/me/.config/ar/usage.toml"))
    );
}

#[test]
fn test_load_or_create_uses_default_config_path() -> Result<()> {
    let temp = TempConfig::new();

    let (config, mappings) = temp.with(Config::load_or_create)?;

    assert!(temp.path.exists());
    assert_eq!(config.config_path, Some(temp.path.clone()));
    // The mappings file is created next to it, with the default profile.
    let mappings_file = mappings_path(&temp.path);
    assert!(mappings_file.exists());
    assert_eq!(config.mappings_path(), Some(mappings_file));
    assert_eq!(
        mappings.get_characters_for_key(&MappingKey::Char('a')),
        Some(["ä".to_string()].as_slice())
    );

    Ok(())
}

#[test]
fn test_load_or_create_at_loads_the_mappings_file() -> Result<()> {
    let temp = TempConfig::new();
    Config::default().save_to_file(&temp.path)?;
    fs::write(
        mappings_path(&temp.path),
        "current_profile = \"Mine\"\n[profiles.Mine.mappings.n]\ncharacters = [\"ñ\"]\n",
    )?;

    let (_, mappings) = Config::load_or_create_at(temp.path.clone())?;
    assert_eq!(
        mappings.get_characters_for_key(&MappingKey::Char('n')),
        Some(["ñ".to_string()].as_slice())
    );

    // A broken mappings file is reported like a broken config file.
    fs::write(mappings_path(&temp.path), "not = = toml")?;
    let error = Config::load_or_create_at(temp.path.clone()).unwrap_err();
    assert!(matches!(error, CoreError::ParseConfig { .. }), "{error:?}");

    Ok(())
}

#[test]
fn test_load_or_create_at_reports_unwritable_location() {
    let temp = TempConfig::unwritable();

    let error = Config::load_or_create_at(temp.path.clone()).unwrap_err();

    assert!(matches!(error, CoreError::CreateConfigDir { .. }));
}

#[test]
fn test_load_or_create_at_reports_corrupt_file() -> Result<()> {
    let temp = TempConfig::new();
    fs::write(&temp.path, "this is not = = valid toml {{{")?;

    let error = Config::load_or_create_at(temp.path.clone()).unwrap_err();

    assert!(matches!(error, CoreError::ParseConfig { .. }));
    Ok(())
}

#[test]
fn test_load_or_create_at_creates_then_loads() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("config.toml");
    assert!(!path.exists());

    let (config, mapping_manager) = Config::load_or_create_at(path.clone())?;

    // The config file was created and recorded as the active path.
    assert!(path.exists());
    assert_eq!(config.config_path, Some(path.clone()));
    // The created config is the default configuration (compared field by
    // field: config_path is not serialized and defaults to None).
    let expected = Config::default();
    assert_eq!(config.leader_key, expected.leader_key);
    assert_eq!(config.time_window_ms, expected.time_window_ms);
    assert_eq!(config.show_overlay, expected.show_overlay);
    assert_eq!(config.overlay_position, expected.overlay_position);
    assert_eq!(config.app_filter_mode, expected.app_filter_mode);
    assert_eq!(config.filtered_apps, expected.filtered_apps);
    assert_eq!(config.sort_by_usage, expected.sort_by_usage);

    // Default mappings are loaded alongside the config.
    assert!(mapping_manager.current_profile().is_some());

    // A second call loads the existing file instead of recreating it.
    fs::write(
        &path,
        r#"
leader_key = "Alt"
time_window_ms = 100
show_overlay = false
overlay_position = [1, 2]
app_filter_mode = "Whitelist"
filtered_apps = []
sort_by_usage = true
"#,
    )?;
    let (config, _) = Config::load_or_create_at(path.clone())?;
    assert_eq!(config.leader_key, LeaderKey::Alt);
    assert_eq!(config.time_window_ms, 100);

    Ok(())
}

#[test]
fn test_save_to_file_without_parent_directory() {
    // A root-level path has no parent component: saving skips directory
    // creation and fails at the write itself ("/" is a directory).
    let error = Config::default().save_to_file(Path::new("/")).unwrap_err();
    match &error {
        CoreError::WriteConfig { path, .. } => assert_eq!(path, &PathBuf::from("/")),
        other => panic!("expected WriteConfig, got {other:?}"),
    }
}

#[test]
fn test_config_new_matches_default() {
    assert_eq!(Config::new(), Config::default());
}

#[test]
fn test_is_app_allowed() {
    let mut config = Config {
        filtered_apps: vec!["Firefox".to_string()],
        ..Config::default()
    };

    // Blacklist (default): listed apps are blocked, everything else allowed.
    config.app_filter_mode = AppFilterMode::Blacklist;
    assert!(
        !config.is_app_allowed("firefox"),
        "listed app must be blocked"
    );
    assert!(
        !config.is_app_allowed("FIREFOX"),
        "match is case-insensitive"
    );
    assert!(config.is_app_allowed("kitty"), "unlisted app is allowed");

    // Whitelist: only listed apps are allowed.
    config.app_filter_mode = AppFilterMode::Whitelist;
    assert!(config.is_app_allowed("firefox"), "listed app is allowed");
    assert!(!config.is_app_allowed("kitty"), "unlisted app is blocked");
}
