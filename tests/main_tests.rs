//! Integration tests for the main application's command line surface

use anyhow::Result;
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn test_help_command() -> Result<()> {
    let mut cmd = Command::cargo_bin("accents-rapides")?;
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Rust port of schnelle-umlaute"))
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("editor"))
        .stdout(predicate::str::contains("overlay"))
        .stdout(predicate::str::contains("config"));

    Ok(())
}

#[test]
fn test_version_command() -> Result<()> {
    let mut cmd = Command::cargo_bin("accents-rapides")?;
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("accents-rapides 0.1.0"));

    Ok(())
}
