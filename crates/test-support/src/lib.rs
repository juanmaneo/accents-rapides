//! Shared test helpers for the accents-rapides workspace.
//!
//! Every binary and library reads its configuration from the path named by
//! the `ACCENTS_RAPIDES_CONFIG_PATH` environment variable (see
//! [`accents_rapides_core::CONFIG_PATH_ENV`]), and finds the overlay through
//! `ACCENTS_RAPIDES_OVERLAY_SOCKET`. Tests must never touch the developer's
//! real configuration or overlay, so they point both variables at a
//! throw-away location with the helpers below.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

pub use accents_rapides_core::{CONFIG_PATH_ENV, OVERLAY_SOCKET_ENV};
use tempfile::TempDir;

/// `std::env::set_var` is process-global: every in-process change to the
/// environment serializes on this lock (one per test binary).
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` with each variable of `vars` set to its value (unset for `None`),
/// restoring the previous values afterwards.
///
/// Calls are serialized within the process; do not nest them.
pub fn with_env<T>(vars: &[(&str, Option<&str>)], f: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let saved = set_vars(vars.iter().copied());
    let result = f();
    restore(saved);
    result
}

/// Run `f` with `ACCENTS_RAPIDES_CONFIG_PATH` set to `path` (unset for
/// `None`); see [`with_env`].
pub fn with_config_path<T>(path: Option<&str>, f: impl FnOnce() -> T) -> T {
    with_env(&[(CONFIG_PATH_ENV, path)], f)
}

/// Run `f` with the config path and overlay socket in a fresh temporary
/// directory.
pub fn with_temp_config<T>(f: impl FnOnce() -> T) -> T {
    TempConfig::new().with(f)
}

/// Variables and their values before [`set_vars`].
type Saved = Vec<(String, Option<String>)>;

fn set_vars<'a>(vars: impl Iterator<Item = (&'a str, Option<&'a str>)>) -> Saved {
    vars.map(|(name, value)| {
        let previous = std::env::var(name).ok();
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        (name.to_string(), previous)
    })
    .collect()
}

fn restore(saved: Saved) {
    set_vars(
        saved
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_deref())),
    );
}

fn utf8(path: &Path) -> &str {
    path.to_str().expect("temp paths are valid UTF-8")
}

/// A configuration file and overlay socket location inside a temporary
/// directory that is removed on drop.
pub struct TempConfig {
    _dir: TempDir,
    /// The config file path (does not exist until something saves it).
    pub path: PathBuf,
    /// The overlay socket path (does not exist until an overlay binds it).
    pub overlay_socket: PathBuf,
}

impl TempConfig {
    /// A writable location: `<tempdir>/config.toml` and `<tempdir>/overlay.sock`.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        Self::inside(dir.path().to_path_buf(), dir)
    }

    /// A location that can never be created: its parent is a regular file,
    /// so creating the config or binding the overlay socket there fails.
    pub fn unwritable() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "not a directory").expect("write blocker file");
        Self::inside(blocker, dir)
    }

    fn inside(parent: PathBuf, dir: TempDir) -> Self {
        Self {
            _dir: dir,
            path: parent.join("config.toml"),
            overlay_socket: parent.join("overlay.sock"),
        }
    }

    /// The config path as a `&str`.
    pub fn as_str(&self) -> &str {
        utf8(&self.path)
    }

    /// The environment pointing at this location, e.g. for `Command::envs`.
    pub fn envs(&self) -> [(&'static str, &str); 2] {
        [
            (CONFIG_PATH_ENV, self.as_str()),
            (OVERLAY_SOCKET_ENV, utf8(&self.overlay_socket)),
        ]
    }

    /// Run `f` with the environment pointing at this location (see
    /// [`with_env`]).
    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        with_env(&self.envs().map(|(name, value)| (name, Some(value))), f)
    }
}

impl Default for TempConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_env_sets_and_restores_the_variables() {
        const OTHER: &str = "ACCENTS_RAPIDES_TEST_SUPPORT_OTHER";
        with_config_path(Some("/outer/config.toml"), || {
            assert_eq!(
                std::env::var(CONFIG_PATH_ENV).unwrap(),
                "/outer/config.toml"
            );

            // The lock is not reentrant, so this nested change bypasses it.
            let saved = set_vars([(CONFIG_PATH_ENV, None), (OTHER, Some("set"))].into_iter());
            assert!(std::env::var(CONFIG_PATH_ENV).is_err());
            assert_eq!(std::env::var(OTHER).unwrap(), "set");

            // A previous value is restored, and so is "unset".
            restore(saved);
            assert_eq!(
                std::env::var(CONFIG_PATH_ENV).unwrap(),
                "/outer/config.toml"
            );
            assert!(std::env::var(OTHER).is_err());
        });
    }

    #[test]
    fn temp_config_locations() {
        let writable = TempConfig::new();
        assert!(!writable.path.exists());
        assert!(writable.path.parent().unwrap().is_dir());
        assert!(writable.as_str().ends_with("config.toml"));
        assert_eq!(writable.overlay_socket.parent(), writable.path.parent());

        let unwritable = TempConfig::unwritable();
        assert!(unwritable.path.parent().unwrap().is_file());
        assert!(std::fs::create_dir_all(unwritable.path.parent().unwrap()).is_err());

        let temp = TempConfig::default();
        let [(_, config), (_, socket)] = temp.envs();
        assert_eq!(
            temp.with(|| (
                std::env::var(CONFIG_PATH_ENV),
                std::env::var(OVERLAY_SOCKET_ENV)
            )),
            (Ok(config.to_string()), Ok(socket.to_string()))
        );
    }
}
