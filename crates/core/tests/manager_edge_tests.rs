//! Edge-case tests for the mapping manager: every branch that the happy-path
//! mapping tests do not reach (missing profiles, missing keys, empty
//! character lists, unknown profile names, sorting fallbacks).

use accents_rapides_core::*;

/// A manager with one profile that has mappings for the "a" key.
fn manager_with_profile() -> MappingManager {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Test");
    profile.mappings.insert(
        MappingKey::Char('a'),
        KeyMapping::new(vec!["ä".to_string()]),
    );
    manager.add_profile(profile);
    manager.set_current_profile("Test");
    manager
}

#[test]
fn test_set_current_profile_unknown_name() {
    let mut manager = manager_with_profile();

    // Switching to an existing profile succeeds and updates the current.
    assert!(manager.set_current_profile("Test"));
    assert_eq!(manager.current_profile.clone(), Some("Test".to_string()));

    // Switching to an unknown profile fails and keeps the previous one.
    assert!(!manager.set_current_profile("DoesNotExist"));
    assert_eq!(manager.current_profile.clone(), Some("Test".to_string()));
}

#[test]
fn test_no_current_profile() {
    let mut manager = MappingManager::new();
    let key = MappingKey::Char('a');

    // With no profile selected, every lookup is a miss.
    assert!(manager.current_profile().is_none());
    assert!(manager.current_profile_mut().is_none());
    assert_eq!(manager.get_characters_for_key(&key), None);
    assert_eq!(manager.cycle_next(&key), None);
    assert_eq!(manager.get_current_character(&key), None);

    // Sorting is a no-op without a current profile.
    manager.sort_by_usage(&key);

    // A profile exists but is not selected: still nothing current.
    let mut profile = MappingProfile::new("Unselected");
    profile.mappings.insert(
        MappingKey::Char('a'),
        KeyMapping::new(vec!["ä".to_string()]),
    );
    manager.add_profile(profile);
    assert!(manager.current_profile().is_none());
    assert_eq!(manager.get_characters_for_key(&key), None);
}

#[test]
fn test_unknown_key_in_current_profile() {
    let mut manager = manager_with_profile();
    let missing = MappingKey::Char('z');

    // The key has no mapping in the current profile.
    assert_eq!(manager.get_characters_for_key(&missing), None);
    assert_eq!(manager.cycle_next(&missing), None);
    assert_eq!(manager.get_current_character(&missing), None);

    // Sorting an unmapped key is a no-op.
    manager.sort_by_usage(&missing);
}

#[test]
fn test_empty_character_list() {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Empty");
    profile
        .mappings
        .insert(MappingKey::Char('a'), KeyMapping::new(Vec::new()));
    manager.add_profile(profile);
    manager.set_current_profile("Empty");

    let key = MappingKey::Char('a');

    // A mapping exists but holds no characters: nothing to cycle or show.
    assert!(manager.get_characters_for_key(&key).is_some());
    assert_eq!(manager.cycle_next(&key), None);
    assert_eq!(manager.get_current_character(&key), None);
}

#[test]
fn test_reverse_cycle_wraps_from_zero() {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Wrap");
    profile.mappings.insert(
        MappingKey::Char('a'),
        KeyMapping {
            characters: vec!["ä".to_string(), "à".to_string()],
            cycle_position: 0,
            reverse_cycle: true,
        },
    );
    manager.add_profile(profile);
    manager.set_current_profile("Wrap");

    let key = MappingKey::Char('a');

    // At position 0, a reverse cycle wraps to the last variant.
    assert_eq!(manager.cycle_next(&key), Some("ä".to_string()));
    // The position moved to the end, so the next reverse step goes backward.
    assert_eq!(manager.cycle_next(&key), Some("à".to_string()));
}

#[test]
fn test_cycle_next_records_usage() {
    let mut manager = manager_with_profile();
    let key = MappingKey::Char('a');

    assert_eq!(manager.cycle_next(&key), Some("ä".to_string()));
    assert_eq!(manager.cycle_next(&key), Some("ä".to_string()));

    // Each commit bumps the usage counter for the committed variant.
    let stats = manager
        .usage_stats
        .get(&key)
        .expect("usage stats recorded for the cycled key");
    assert_eq!(stats.get("ä"), Some(&2));
}

#[test]
fn test_sort_by_usage_without_stats_is_noop() {
    let mut manager = manager_with_profile();
    let key = MappingKey::Char('a');

    manager.sort_by_usage(&key);

    // No recorded usage: the stored order is untouched.
    assert_eq!(manager.get_current_character(&key), Some("ä".to_string()));
    let profile = manager.current_profile().unwrap();
    assert_eq!(profile.mappings[&key].cycle_position, 0);
}

#[test]
fn test_sort_by_usage_reorders_and_resets_position() {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Sort");
    profile.mappings.insert(
        MappingKey::Char('a'),
        KeyMapping {
            characters: vec!["ä".to_string(), "à".to_string(), "â".to_string()],
            cycle_position: 2,
            reverse_cycle: false,
        },
    );
    manager.add_profile(profile);
    manager.set_current_profile("Sort");

    let key = MappingKey::Char('a');

    // Record usage: "à" leads, "ä" second, "â" unused (counts as zero).
    let mut stats = UsageCounts::new();
    stats.insert("ä".to_string(), 1);
    stats.insert("à".to_string(), 5);
    manager.usage_stats.insert(key.clone(), stats);

    manager.sort_by_usage(&key);

    let profile = manager.current_profile().unwrap();
    let mapping = &profile.mappings[&key];
    assert_eq!(
        mapping.characters,
        vec!["à".to_string(), "ä".to_string(), "â".to_string()]
    );
    // Sorting resets the cycle to the first (most used) variant.
    assert_eq!(mapping.cycle_position, 0);
    assert_eq!(manager.get_current_character(&key), Some("à".to_string()));
}

#[test]
fn test_add_profile_overwrites_same_name() {
    let mut manager = MappingManager::new();

    let mut first = MappingProfile::new("P");
    first.mappings.insert(
        MappingKey::Char('a'),
        KeyMapping::new(vec!["ä".to_string()]),
    );
    manager.add_profile(first);
    manager.add_profile(MappingProfile::new("P"));

    // Adding a profile with an existing name replaces it.
    assert_eq!(manager.profiles.len(), 1);
    assert!(manager.profiles["P"].mappings.is_empty());
}
