//! Core functionality for accents-rapides
//!
//! This crate contains the main logic for character mapping, accent cycling,
//! and configuration management.

pub mod config;
pub mod input;
pub mod mappings;
pub mod overlay_feed;
pub mod profile_cycle;

pub use config::{
    mappings_path, usage_path, AppFilterMode, Config, CoreError, LeaderKey, CONFIG_PATH_ENV,
    MAPPINGS_FILE, USAGE_FILE,
};
pub use input::{key_name, Action, InputHandler, KeyEvent, Modifiers, Session};
pub use mappings::{
    join_outputs, sort_variants_by_usage, split_outputs, KeyMapping, MappingKey, MappingManager,
    MappingProfile, UsageCounts, UsageStats, DEFAULT_PROFILE,
};
pub use overlay_feed::{
    socket_path, OverlayMessage, OverlayReceiver, OverlaySender, OVERLAY_SOCKET_ENV,
};
pub use profile_cycle::{cycle_names, cycle_target, CycleEntry};
