//! Configuration management for accents-rapides

use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::input::key_name;
use crate::mappings::MappingManager;

/// Environment variable that overrides the default config path.
pub const CONFIG_PATH_ENV: &str = "ACCENTS_RAPIDES_CONFIG_PATH";

/// Name of the mappings file, which lives next to the config file.
pub const MAPPINGS_FILE: &str = "mappings.toml";

/// Name of the usage file (see [`MappingManager::save_usage`]), which lives
/// next to the config file.
pub const USAGE_FILE: &str = "usage.toml";

/// The mappings file next to the config file `config_path`.
pub fn mappings_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name(MAPPINGS_FILE)
}

/// The usage file next to the config file `config_path`.
pub fn usage_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name(USAGE_FILE)
}

/// Read and parse the TOML file at `path`.
pub(crate) fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let content = fs::read_to_string(path).map_err(|source| CoreError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&content).map_err(|source| CoreError::ParseConfig {
        path: path.to_path_buf(),
        source,
    })
}

/// Write `value` as TOML to `path`, creating the parent directory if it
/// does not exist.
pub(crate) fn write_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    // Every field we persist is a TOML-representable type, so this cannot fail.
    let content = toml::to_string_pretty(value).expect("always serializes to TOML");

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CoreError::CreateConfigDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, content).map_err(|source| CoreError::WriteConfig {
        path: path.to_path_buf(),
        source,
    })
}

/// Error type for the accents-rapides core library
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("failed to read config file {path}: {source}")]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ParseConfig {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("failed to write config file {path}: {source}")]
    WriteConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to create config directory {path}: {source}")]
    CreateConfigDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not determine the default config path")]
    NoConfigPath,

    #[error("invalid mapping key {0:?}: expected a single character, Shift+<character> or a named key (Space, LeftArrow, RightArrow, UpArrow, DownArrow)")]
    InvalidMappingKey(String),

    #[error("cannot bind the overlay socket {path}: {source}")]
    OverlaySocket {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Leader key options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaderKey {
    Space,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    Alt,
    AltGr,
    Custom(String),
}

impl LeaderKey {
    /// The [`KeyEvent::key`](crate::KeyEvent::key) that triggers this
    /// leader: a [`key_name`], or the custom key itself.
    pub fn name(&self) -> &str {
        match self {
            Self::Space => key_name::SPACE,
            Self::LeftArrow => key_name::LEFT,
            Self::RightArrow => key_name::RIGHT,
            Self::UpArrow => key_name::UP,
            Self::DownArrow => key_name::DOWN,
            Self::Alt => key_name::ALT,
            Self::AltGr => key_name::ALTGR,
            Self::Custom(key) => key,
        }
    }
}

/// Application filter mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppFilterMode {
    Blacklist,
    Whitelist,
}

/// Configuration for the application
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Leader key configuration
    pub leader_key: LeaderKey,

    /// Time window for key press (in milliseconds)
    pub time_window_ms: u32,

    /// Whether to show the overlay
    pub show_overlay: bool,

    /// Overlay position (x, y)
    pub overlay_position: (i32, i32),

    /// Application filter mode
    pub app_filter_mode: AppFilterMode,

    /// List of applications to filter
    pub filtered_apps: Vec<String>,

    /// Whether to sort characters by usage frequency
    pub sort_by_usage: bool,

    /// Path to the configuration file (not serialized)
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            leader_key: LeaderKey::Alt,
            time_window_ms: 500,
            show_overlay: true,
            overlay_position: (0, 0),
            app_filter_mode: AppFilterMode::Blacklist,
            filtered_apps: Vec::new(),
            sort_by_usage: false,
            config_path: None,
        }
    }
}

impl Config {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a file
    pub fn load_from_file(path: &Path) -> Result<Self, CoreError> {
        let mut config: Config = read_toml(path)?;
        config.config_path = Some(path.to_path_buf());
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &Path) -> Result<(), CoreError> {
        write_toml(path, self)
    }

    /// The mappings file next to this configuration's file, if it has one
    pub fn mappings_path(&self) -> Option<PathBuf> {
        self.config_path.as_deref().map(mappings_path)
    }

    /// The usage file next to this configuration's file, if it has one
    pub fn usage_path(&self) -> Option<PathBuf> {
        self.config_path.as_deref().map(usage_path)
    }

    /// Get the default configuration path
    ///
    /// The `ACCENTS_RAPIDES_CONFIG_PATH` environment variable overrides the
    /// platform default (also useful for tests and packaging).
    pub fn default_config_path() -> Option<PathBuf> {
        if let Ok(path) = std::env::var(CONFIG_PATH_ENV) {
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }

        ProjectDirs::from("com", "accents-rapides", "accents-rapides")
            .map(|proj_dirs| proj_dirs.config_dir().join("config.toml"))
    }

    /// Load the configuration and the mappings from the default path,
    /// creating them with defaults if they do not exist yet
    pub fn load_or_create() -> Result<(Self, MappingManager), CoreError> {
        Self::default_config_path()
            .ok_or(CoreError::NoConfigPath)
            .and_then(Self::load_or_create_at)
    }

    /// Load the configuration from `config_path` and the mappings from the
    /// file next to it, creating them with defaults if they do not exist yet
    pub fn load_or_create_at(config_path: PathBuf) -> Result<(Self, MappingManager), CoreError> {
        let mut config = if config_path.exists() {
            Self::load_from_file(&config_path)?
        } else {
            let config = Self::new();
            config.save_to_file(&config_path)?;
            config
        };

        let mapping_manager = MappingManager::load_or_create_at(&mappings_path(&config_path))?;
        config.config_path = Some(config_path);

        Ok((config, mapping_manager))
    }

    /// Check if an application is allowed based on the filter
    ///
    /// Matching is case-insensitive on both sides.
    pub fn is_app_allowed(&self, app_name: &str) -> bool {
        let app_name = app_name.to_lowercase();
        let listed = self
            .filtered_apps
            .iter()
            .any(|app| app.to_lowercase() == app_name);
        match self.app_filter_mode {
            AppFilterMode::Blacklist => !listed,
            AppFilterMode::Whitelist => listed,
        }
    }
}
