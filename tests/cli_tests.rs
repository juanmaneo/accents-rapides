//! Tests for the CLI application

use accents_rapides_test_support::{TempConfig, CONFIG_PATH_ENV};
use anyhow::Result;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;

/// The CLI binary with its config path and overlay socket in `config`.
fn cli(config: &TempConfig) -> Result<Command> {
    let mut cmd = Command::cargo_bin("accents-rapides")?;
    cmd.envs(config.envs());
    Ok(cmd)
}

#[test]
fn test_config_command_with_existing_config() -> Result<()> {
    let temp = TempConfig::new();
    fs::write(
        &temp.path,
        r#"
leader_key = "Space"
time_window_ms = 500
show_overlay = true
app_filter_mode = "Blacklist"
filtered_apps = ["app1", "app2"]
overlay_position = [0, 0]
sort_by_usage = false
    "#,
    )?;

    cli(&temp)?
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration path:"))
        .stdout(predicate::str::contains(temp.as_str()))
        .stdout(predicate::str::contains("Mappings path:"))
        .stdout(predicate::str::contains("mappings.toml"))
        .stdout(predicate::str::contains("Usage path:"))
        .stdout(predicate::str::contains("usage.toml"))
        .stdout(predicate::str::contains("Leader key: Space"))
        .stdout(predicate::str::contains("Time window: 500 ms"))
        .stdout(predicate::str::contains("Show overlay: true"))
        .stdout(predicate::str::contains("App filter mode: Blacklist"))
        .stdout(predicate::str::contains(
            "Filtered apps: [\"app1\", \"app2\"]",
        ));

    Ok(())
}

#[test]
fn test_config_command_with_no_config() -> Result<()> {
    let temp = TempConfig::new();

    cli(&temp)?
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration path:"))
        .stdout(predicate::str::contains("No configuration file found"));

    Ok(())
}

#[test]
fn test_config_command_uses_platform_default_path() -> Result<()> {
    // Without the override, the platform default config path is used.
    let mut cmd = Command::cargo_bin("accents-rapides")?;
    cmd.env_remove(CONFIG_PATH_ENV);
    cmd.arg("config");

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output)?;

    assert!(stdout.contains("Configuration path:"));
    // Either a config exists on this machine or the default is reported
    // missing; both outcomes exercise the platform-default branch.
    assert!(
        stdout.contains("Current configuration:") || stdout.contains("No configuration file found")
    );

    Ok(())
}

#[test]
fn test_config_command_with_invalid_config() -> Result<()> {
    // An invalid config file makes the config command fail: the error must
    // propagate out of print_config_info and exit non-zero.
    let temp = TempConfig::new();
    fs::write(&temp.path, "this is not = = valid toml {{{")?;

    cli(&temp)?
        .arg("config")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Configuration path:"))
        .stderr(predicate::str::contains("failed to parse config file"));

    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_editor_command() -> Result<()> {
    // Under coverage measurement the editor never opens a window, so the
    // command runs its setup logic and exits successfully.
    let temp = TempConfig::new();

    cli(&temp)?.arg("editor").assert().success();
    assert!(temp.path.exists(), "editor must load or create the config");

    Ok(())
}

#[test]
#[cfg(coverage)]
fn test_overlay_command() -> Result<()> {
    // Under coverage measurement the overlay never opens a window, so the
    // command binds the overlay socket and exits successfully.
    let temp = TempConfig::new();

    cli(&temp)?.arg("overlay").assert().success();

    Ok(())
}
