//! Character mapping system for accents-rapides

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use crate::config::{read_toml, write_toml, CoreError};
use crate::input::key_name;

/// Split an output spec into its cycling variants.
///
/// A single comma separates variants; a double comma (",,") is an escaped
/// literal comma inside a variant; empty variants are skipped. Ported from
/// the C++ `splitOutputs` (tests 14-16, 87-91, 101-103).
///
/// Examples:
/// - `"ä,ae"` → `["ä", "ae"]`
/// - `"a,,b"` → `["a,b"]` (escaped comma, one variant)
/// - `"x,,y,z"` → `["x,y", "z"]`
/// - `","` → `[]` (empty output, the mapping is skipped)
pub fn split_outputs(spec: &str) -> Vec<String> {
    let mut variants: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = spec.chars().peekable();

    while let Some(c) = chars.next() {
        if c == ',' {
            if chars.peek() == Some(&',') {
                // Double comma: escaped literal comma inside the variant.
                chars.next();
                current.push(',');
            } else if !current.is_empty() {
                // Single comma: end of the current variant.
                variants.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        variants.push(current);
    }

    variants
}

/// Join cycling variants into an output spec, the inverse of
/// [`split_outputs`]: one comma between variants, and a comma inside a
/// variant doubled.
pub fn join_outputs(variants: &[String]) -> String {
    variants
        .iter()
        .map(|variant| variant.replace(',', ",,"))
        .collect::<Vec<_>>()
        .join(",")
}

/// How many times each variant of a key was typed.
pub type UsageCounts = BTreeMap<String, u64>;

/// The [`UsageCounts`] of every key, persisted in the usage file (see
/// [`MappingManager::save_usage`]):
///
/// ```toml
/// [e]
/// "é" = 5
/// "è" = 1
/// ```
///
/// Ordered maps, so that the file is written the same way every time.
pub type UsageStats = BTreeMap<MappingKey, UsageCounts>;

/// Sort cycling variants by usage count, descending.
///
/// The three properties the feature leans on (ported from the C++
/// `sortVariantsByUsage`, see tests/testusagesort.cpp):
/// - descending by count
/// - stable tie-break on the stored order
/// - a no-op when all counts are zero (fresh stats look exactly like
///   toggle-off), because variants with no recorded usage count as zero
///   and a stable sort keeps their stored relative order.
pub fn sort_variants_by_usage(stored: &[String], counts: &UsageCounts) -> Vec<String> {
    let mut sorted = stored.to_vec();
    sorted.sort_by(|a, b| {
        let count_a = counts.get(a).copied().unwrap_or(0);
        let count_b = counts.get(b).copied().unwrap_or(0);
        count_b.cmp(&count_a)
    });
    sorted
}

/// Represents a key that can trigger accent mappings
///
/// Written (in the mappings file and the editor) as `a`, `Shift+A` or
/// `Space`: see the [`Display`](std::fmt::Display) and [`FromStr`] impls.
/// Ordered by kind (characters, then shifted characters, then named keys)
/// and then by content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum MappingKey {
    /// A single character key (e.g., 'a')
    Char(char),
    /// A character with shift modifier (e.g., Shift+'A')
    ShiftChar(char),
    /// A special key (e.g., Space, Arrow keys)
    Special(String),
}

/// Named keys that can be mapped: their [`KeyEvent::key`](crate::KeyEvent::key)
/// name and their [`MappingKey::Special`] name.
const SPECIAL_KEYS: [(&str, &str); 5] = [
    (key_name::SPACE, "Space"),
    (key_name::LEFT, "LeftArrow"),
    (key_name::RIGHT, "RightArrow"),
    (key_name::UP, "UpArrow"),
    (key_name::DOWN, "DownArrow"),
];

/// Prefix of a [`MappingKey::ShiftChar`] in its written form.
const SHIFT_PREFIX: &str = "Shift+";

/// The only character of `s`, if it has exactly one.
fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// The named key called `name` (either of its names, case-insensitive).
fn special(name: &str) -> Option<MappingKey> {
    SPECIAL_KEYS
        .iter()
        .find(|(key, special)| name.eq_ignore_ascii_case(key) || name.eq_ignore_ascii_case(special))
        .map(|(_, special)| MappingKey::Special((*special).to_string()))
}

/// `s` without its `Shift+` prefix (any case), if it has one.
fn strip_shift(s: &str) -> Option<&str> {
    let (prefix, rest) = s.split_at_checked(SHIFT_PREFIX.len())?;
    prefix.eq_ignore_ascii_case(SHIFT_PREFIX).then_some(rest)
}

impl MappingKey {
    /// The mapping key for a [`KeyEvent::key`](crate::KeyEvent::key): a
    /// single character (`ShiftChar` of its uppercase form when Shift is
    /// held) or a named key. `None` for anything else.
    pub fn from_key(key: &str, shift: bool) -> Option<Self> {
        match single_char(key) {
            Some(c) if shift => Some(Self::ShiftChar(c.to_ascii_uppercase())),
            Some(c) => Some(Self::Char(c)),
            None => special(key),
        }
    }
}

/// The key as users write it: `a`, `Shift+A`, `Space`.
impl std::fmt::Display for MappingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingKey::Char(c) => write!(f, "{c}"),
            MappingKey::ShiftChar(c) => write!(f, "{SHIFT_PREFIX}{c}"),
            MappingKey::Special(name) => f.write_str(name),
        }
    }
}

/// The inverse of [`Display`](std::fmt::Display): a single character,
/// `Shift+` and a character (uppercased), or a named key, case-insensitive.
impl FromStr for MappingKey {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, CoreError> {
        if let Some(c) = single_char(s) {
            Ok(Self::Char(c))
        } else if let Some(c) = strip_shift(s).and_then(single_char) {
            Ok(Self::ShiftChar(c.to_ascii_uppercase()))
        } else {
            special(s).ok_or_else(|| CoreError::InvalidMappingKey(s.to_string()))
        }
    }
}

impl TryFrom<String> for MappingKey {
    type Error = CoreError;

    fn try_from(s: String) -> Result<Self, CoreError> {
        s.parse()
    }
}

impl From<MappingKey> for String {
    fn from(key: MappingKey) -> Self {
        key.to_string()
    }
}

/// Represents a mapping from a key to a sequence of characters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    /// The characters that this key maps to
    pub characters: Vec<String>,
    /// Current position in the cycle (for stateful cycling)
    #[serde(skip)]
    pub cycle_position: usize,
    /// Whether to cycle in reverse order
    #[serde(default)]
    pub reverse_cycle: bool,
}

impl KeyMapping {
    /// A mapping cycling forward through `characters`
    pub fn new(characters: Vec<String>) -> Self {
        Self {
            characters,
            cycle_position: 0,
            reverse_cycle: false,
        }
    }
}

/// Profile containing multiple key mappings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MappingProfile {
    /// Name of the profile: its key in [`MappingManager::profiles`], restored
    /// from there when loading
    #[serde(skip)]
    pub name: String,
    /// Mappings for this profile
    #[serde(default)]
    pub mappings: IndexMap<MappingKey, KeyMapping>,
}

impl MappingProfile {
    /// An empty profile called `name`
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mappings: IndexMap::new(),
        }
    }
}

/// Name of the profile created on first run.
pub const DEFAULT_PROFILE: &str = "Default";

/// The mappings of the profile created on first run: each key and its
/// variants, in cycle order. Lowercase only: capitals take no accent.
const DEFAULT_MAPPINGS: [(char, &[&str]); 8] = [
    ('s', &["ß"]),
    ('a', &["ä", "á", "à", "â"]),
    ('o', &["ö", "ó", "ò", "ô"]),
    ('c', &["ç", "ć", "č"]),
    ('u', &["ü", "ú", "ù", "û"]),
    ('e', &["é", "è", "ê", "ë", "€"]),
    ('n', &["ñ"]),
    ('i', &["í", "ï", "î", "ì"]),
];

/// Main mapping manager that handles all profiles
///
/// Persisted as the mappings file (see [`Self::save_to_file`]):
///
/// ```toml
/// current_profile = "Default"
///
/// [profiles.Default.mappings.a]
/// characters = ["ä"]
/// reverse_cycle = false
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MappingManager {
    /// Current active profile
    #[serde(default)]
    pub current_profile: Option<String>,
    /// All available profiles
    #[serde(default)]
    pub profiles: IndexMap<String, MappingProfile>,
    /// Usage statistics for sorting characters by frequency, kept in the
    /// usage file (see [`Self::save_usage`]) rather than in the mappings file
    #[serde(skip)]
    pub usage_stats: UsageStats,
    /// Whether usage was recorded since the usage file was loaded or saved
    #[serde(skip)]
    usage_changed: bool,
}

impl MappingManager {
    /// Create a new empty MappingManager
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the profiles from the mappings file `path`
    pub fn load_from_file(path: &Path) -> Result<Self, CoreError> {
        let mut manager: Self = read_toml(path)?;
        for (name, profile) in &mut manager.profiles {
            profile.name = name.clone();
        }
        Ok(manager)
    }

    /// Save the profiles to the mappings file `path`
    pub fn save_to_file(&self, path: &Path) -> Result<(), CoreError> {
        write_toml(path, self)
    }

    /// Load the profiles from the mappings file `path`, creating it with
    /// the default mappings if it does not exist yet
    pub fn load_or_create_at(path: &Path) -> Result<Self, CoreError> {
        if path.exists() {
            return Self::load_from_file(path);
        }
        let mut manager = Self::new();
        manager.load_default_mappings();
        manager.save_to_file(path)?;
        Ok(manager)
    }

    /// Add a new profile
    pub fn add_profile(&mut self, profile: MappingProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get the current active profile
    pub fn current_profile(&self) -> Option<&MappingProfile> {
        self.current_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }

    /// Get the current active profile mutably
    pub fn current_profile_mut(&mut self) -> Option<&mut MappingProfile> {
        self.current_profile
            .as_ref()
            .and_then(|name| self.profiles.get_mut(name))
    }

    /// Set the current active profile
    pub fn set_current_profile(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(name) {
            self.current_profile = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// Get the mapping of a key in the current profile
    pub fn get_mapping(&self, key: &MappingKey) -> Option<&KeyMapping> {
        self.current_profile()
            .and_then(|profile| profile.mappings.get(key))
    }

    /// Get characters for a specific key
    pub fn get_characters_for_key(&self, key: &MappingKey) -> Option<&[String]> {
        self.get_mapping(key).map(|m| m.characters.as_slice())
    }

    /// Load the usage counts from the usage file `path`; a missing file
    /// means none were recorded yet
    pub fn load_usage(&mut self, path: &Path) -> Result<(), CoreError> {
        self.usage_stats = if path.exists() {
            read_toml(path)?
        } else {
            UsageStats::new()
        };
        self.usage_changed = false;
        Ok(())
    }

    /// Save the usage counts to the usage file `path`, if any were recorded
    /// since they were loaded or last saved. A failed save is reported once:
    /// the counts stay in memory and go out with the next one.
    pub fn save_usage(&mut self, path: &Path) -> Result<(), CoreError> {
        if !std::mem::take(&mut self.usage_changed) {
            return Ok(());
        }
        write_toml(path, &self.usage_stats)
    }

    /// Count one use of `character` for `key` (see [`Self::sort_by_usage`])
    pub fn record_usage(&mut self, key: &MappingKey, character: &str) {
        *self
            .usage_stats
            .entry(key.clone())
            .or_default()
            .entry(character.to_string())
            .or_insert(0) += 1;
        self.usage_changed = true;
    }

    /// Cycle to the next character for a key
    pub fn cycle_next(&mut self, key: &MappingKey) -> Option<String> {
        // Get the current profile mutably
        if let Some(profile) = self.current_profile_mut() {
            if let Some(mapping) = profile.mappings.get_mut(key) {
                if mapping.characters.is_empty() {
                    return None;
                }

                // Store the current character before cycling
                let current_char = mapping.characters[mapping.cycle_position].clone();

                // Update the cycle position
                if mapping.reverse_cycle {
                    // For reverse cycling, go backward
                    if mapping.cycle_position == 0 {
                        mapping.cycle_position = mapping.characters.len() - 1;
                    } else {
                        mapping.cycle_position -= 1;
                    }
                } else {
                    // For normal cycling, go forward
                    mapping.cycle_position =
                        (mapping.cycle_position + 1) % mapping.characters.len();
                }

                // Update usage statistics (after releasing the profile borrow)
                self.record_usage(key, &current_char);

                // Return the character that was at the old position
                Some(current_char)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get the current character for a key without cycling
    pub fn get_current_character(&self, key: &MappingKey) -> Option<String> {
        self.current_profile().and_then(|profile| {
            profile.mappings.get(key).and_then(|m| {
                if m.characters.is_empty() {
                    None
                } else {
                    Some(m.characters[m.cycle_position].clone())
                }
            })
        })
    }

    /// Sort the characters of every mapping of the current profile by usage
    /// frequency (see [`Self::sort_by_usage`]), e.g. once the usage counts
    /// are loaded
    pub fn sort_all_by_usage(&mut self) {
        for key in self.usage_stats.keys().cloned().collect::<Vec<_>>() {
            self.sort_by_usage(&key);
        }
    }

    /// Sort characters by usage frequency
    pub fn sort_by_usage(&mut self, key: &MappingKey) {
        // Get usage stats first (before borrowing profile)
        let stats = self.usage_stats.get(key).cloned();

        if let Some(profile) = self.current_profile_mut() {
            if let Some(mapping) = profile.mappings.get_mut(key) {
                if let Some(stats) = stats {
                    // Stable sort, descending by count; variants without
                    // recorded usage count as zero and keep their stored
                    // relative order.
                    mapping.characters = sort_variants_by_usage(&mapping.characters, &stats);
                    mapping.cycle_position = 0; // Reset to first position
                }
            }
        }
    }

    /// Add the [`DEFAULT_PROFILE`] and make it current
    pub fn load_default_mappings(&mut self) {
        let mut profile = MappingProfile::new(DEFAULT_PROFILE);
        for (key, variants) in DEFAULT_MAPPINGS {
            let variants = variants.iter().map(|v| v.to_string()).collect();
            profile
                .mappings
                .insert(MappingKey::Char(key), KeyMapping::new(variants));
        }
        self.add_profile(profile);
        self.set_current_profile(DEFAULT_PROFILE);
    }
}
