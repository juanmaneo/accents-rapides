//! Tests for the mapping system

use accents_rapides_core::*;
use anyhow::Result;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_mapping_key_display() {
    assert_eq!(MappingKey::Char('a').to_string(), "a");
    assert_eq!(MappingKey::ShiftChar('A').to_string(), "Shift+A");
    assert_eq!(MappingKey::Special("Space".into()).to_string(), "Space");
}

#[test]
fn test_mapping_key_parse() -> Result<()> {
    assert_eq!("a".parse::<MappingKey>()?, MappingKey::Char('a'));
    assert_eq!("é".parse::<MappingKey>()?, MappingKey::Char('é'));
    assert_eq!("+".parse::<MappingKey>()?, MappingKey::Char('+'));
    // Shift+ in any case, the character uppercased.
    assert_eq!("Shift+a".parse::<MappingKey>()?, MappingKey::ShiftChar('A'));
    assert_eq!("shift+A".parse::<MappingKey>()?, MappingKey::ShiftChar('A'));
    assert_eq!("SHIFT+é".parse::<MappingKey>()?, MappingKey::ShiftChar('é'));
    // Named keys by either name, in any case.
    for (written, special) in [
        ("Space", "Space"),
        ("space", "Space"),
        ("LEFTARROW", "LeftArrow"),
        ("right", "RightArrow"),
        ("UpArrow", "UpArrow"),
        ("DownArrow", "DownArrow"),
    ] {
        assert_eq!(
            written.parse::<MappingKey>()?,
            MappingKey::Special(special.to_string())
        );
    }

    // Every key parses back from its written form.
    for key in [
        MappingKey::Char('a'),
        MappingKey::ShiftChar('A'),
        MappingKey::Special("LeftArrow".to_string()),
    ] {
        assert_eq!(key.to_string().parse::<MappingKey>()?, key);
        assert_eq!(MappingKey::try_from(String::from(key.clone()))?, key);
    }

    for invalid in ["", "ab", "éé", "Shift+", "Shift+ab", "Shift+Space", "alt"] {
        let error = invalid.parse::<MappingKey>().unwrap_err();
        assert!(
            matches!(&error, CoreError::InvalidMappingKey(s) if s == invalid),
            "{invalid:?} gave {error:?}"
        );
        assert!(error.to_string().contains("invalid mapping key"));
    }
    Ok(())
}

#[test]
fn test_join_outputs_is_the_inverse_of_split_outputs() {
    let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();

    assert_eq!(join_outputs(&[]), "");
    assert_eq!(join_outputs(&strings(&["ä"])), "ä");
    assert_eq!(join_outputs(&strings(&["ä", "ae"])), "ä,ae");
    // A comma inside a variant is escaped by doubling it.
    assert_eq!(join_outputs(&strings(&["a,b", "c"])), "a,,b,c");

    for variants in [
        strings(&["ä"]),
        strings(&["é", "è", "ê"]),
        strings(&["a,b", "c"]),
        strings(&["a,", "b"]),
        strings(&[","]),
    ] {
        assert_eq!(split_outputs(&join_outputs(&variants)), variants);
    }
}

#[test]
fn test_mappings_file_round_trip() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("nested/mappings.toml");
    let mut manager = MappingManager::new();
    manager.load_default_mappings();
    let mut french = MappingProfile::new("French");
    french.mappings.insert(
        MappingKey::Char('e'),
        KeyMapping {
            characters: vec!["é".to_string(), "è".to_string()],
            cycle_position: 1,
            reverse_cycle: true,
        },
    );
    french.mappings.insert(
        MappingKey::Special("Space".to_string()),
        KeyMapping::new(vec![" ".to_string()]),
    );
    manager.add_profile(french);
    manager.add_profile(MappingProfile::new("Empty"));
    manager.record_usage(&MappingKey::Char('a'), "ä");

    manager.save_to_file(&path)?;

    let text = fs::read_to_string(&path)?;
    assert!(
        text.starts_with("current_profile = \"Default\"\n"),
        "{text}"
    );
    assert!(text
        .contains("[profiles.Default.mappings.a]\ncharacters = [\"ä\"]\nreverse_cycle = false\n"));
    assert!(text.contains("[profiles.Default.mappings.\"Shift+A\"]\n"));
    assert!(text.contains("[profiles.French.mappings.e]\ncharacters = ["));
    assert!(text.contains("reverse_cycle = true\n"));
    assert!(text.contains("[profiles.Empty.mappings]\n"));
    // Neither the cycle position nor the usage counts (which have their own
    // file) are persisted here.
    assert!(!text.contains("cycle_position"));
    assert!(!text.contains("usage"));

    let loaded = MappingManager::load_from_file(&path)?;
    assert_eq!(loaded.current_profile.as_deref(), Some(DEFAULT_PROFILE));
    assert_eq!(
        loaded.profiles.keys().collect::<Vec<_>>(),
        ["Default", "French", "Empty"]
    );
    // Profile names come back from the table keys.
    for (name, profile) in &loaded.profiles {
        assert_eq!(&profile.name, name);
    }
    let e = &loaded.profiles["French"].mappings[&MappingKey::Char('e')];
    assert_eq!(e.characters, ["é", "è"]);
    assert!(e.reverse_cycle);
    assert_eq!(e.cycle_position, 0);
    assert!(loaded.profiles["French"]
        .mappings
        .contains_key(&MappingKey::Special("Space".to_string())));
    assert!(loaded.profiles["Empty"].mappings.is_empty());
    assert!(loaded.usage_stats.is_empty());
    Ok(())
}

#[test]
fn test_mappings_file_defaults() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("mappings.toml");

    // Everything is optional: an empty file is an empty manager.
    fs::write(&path, "")?;
    let loaded = MappingManager::load_from_file(&path)?;
    assert_eq!(loaded.current_profile, None);
    assert!(loaded.profiles.is_empty());

    // reverse_cycle defaults to false and a profile without mappings is
    // allowed.
    fs::write(
        &path,
        "current_profile = \"Hand\"\n[profiles.Hand.mappings.n]\ncharacters = [\"ñ\"]\n[profiles.Other]\n",
    )?;
    let loaded = MappingManager::load_from_file(&path)?;
    assert_eq!(
        loaded.get_characters_for_key(&MappingKey::Char('n')),
        Some(["ñ".to_string()].as_slice())
    );
    assert!(
        !loaded
            .get_mapping(&MappingKey::Char('n'))
            .unwrap()
            .reverse_cycle
    );
    assert_eq!(loaded.profiles["Other"].name, "Other");
    Ok(())
}

#[test]
fn test_mappings_file_errors() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("mappings.toml");

    let error = MappingManager::load_from_file(&path).unwrap_err();
    assert!(matches!(error, CoreError::ReadConfig { .. }), "{error:?}");

    // A key that is not a mapping key is a parse error naming the key.
    fs::write(&path, "[profiles.P.mappings.ab]\ncharacters = [\"x\"]\n")?;
    let error = MappingManager::load_from_file(&path).unwrap_err();
    assert!(matches!(error, CoreError::ParseConfig { .. }), "{error:?}");
    assert!(error.to_string().contains("invalid mapping key \"ab\""));

    // Saving where the file cannot be written.
    let error = MappingManager::new()
        .save_to_file(Path::new("/"))
        .unwrap_err();
    assert!(matches!(error, CoreError::WriteConfig { .. }), "{error:?}");
    Ok(())
}

#[test]
fn test_mappings_load_or_create_at() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("mappings.toml");

    // Missing: created with the default profile.
    let created = MappingManager::load_or_create_at(&path)?;
    assert!(path.exists());
    assert_eq!(created.current_profile.as_deref(), Some(DEFAULT_PROFILE));
    assert_eq!(created.current_profile().unwrap().mappings.len(), 7);

    // Existing: loaded as is.
    fs::write(&path, "[profiles.Mine.mappings.n]\ncharacters = [\"ñ\"]\n")?;
    let loaded = MappingManager::load_or_create_at(&path)?;
    assert_eq!(loaded.profiles.keys().collect::<Vec<_>>(), ["Mine"]);

    // Cannot be created: the error is reported.
    let blocked = dir.path().join("blocker");
    fs::write(&blocked, "not a directory")?;
    let error = MappingManager::load_or_create_at(&blocked.join("mappings.toml")).unwrap_err();
    assert!(
        matches!(error, CoreError::CreateConfigDir { .. }),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn test_usage_file_round_trip() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("nested/usage.toml");
    let mut manager = MappingManager::new();

    // Nothing recorded: nothing to save.
    manager.save_usage(&path)?;
    assert!(!path.exists());

    for (key, variant) in [
        (MappingKey::Char('e'), "è"),
        (MappingKey::Char('e'), "é"),
        (MappingKey::Char('e'), "é"),
        (MappingKey::ShiftChar('A'), "Ä"),
        (MappingKey::Char('a'), "ae"),
        (MappingKey::Special("Space".to_string()), "x,y"),
    ] {
        manager.record_usage(&key, variant);
    }
    manager.save_usage(&path)?;

    // Keys by kind then content, variants in order: the same file every
    // time.
    assert_eq!(
        fs::read_to_string(&path)?,
        "[a]\nae = 1\n\n[e]\n\"è\" = 1\n\"é\" = 2\n\n[\"Shift+A\"]\n\"Ä\" = 1\n\n[Space]\n\"x,y\" = 1\n"
    );

    // Saved once: saving again writes nothing until more is recorded.
    fs::remove_file(&path)?;
    manager.save_usage(&path)?;
    assert!(!path.exists());
    manager.record_usage(&MappingKey::Char('a'), "ae");
    manager.save_usage(&path)?;
    assert!(path.exists());

    let mut loaded = MappingManager::new();
    loaded.load_usage(&path)?;
    assert_eq!(loaded.usage_stats, manager.usage_stats);
    assert_eq!(loaded.usage_stats[&MappingKey::Char('a')]["ae"], 2);
    // Loading counts as saved.
    fs::remove_file(&path)?;
    loaded.save_usage(&path)?;
    assert!(!path.exists());
    Ok(())
}

#[test]
fn test_usage_file_errors() -> Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("usage.toml");
    let mut manager = MappingManager::new();
    manager.record_usage(&MappingKey::Char('a'), "ä");

    // Missing: no counts yet, and what was recorded is dropped.
    manager.load_usage(&path)?;
    assert!(manager.usage_stats.is_empty());

    // Broken: reported like any other file.
    fs::write(&path, "not = = toml")?;
    let error = manager.load_usage(&path).unwrap_err();
    assert!(matches!(error, CoreError::ParseConfig { .. }), "{error:?}");
    fs::write(&path, "[a]\n\"ä\" = -1\n")?;
    let error = manager.load_usage(&path).unwrap_err();
    assert!(matches!(error, CoreError::ParseConfig { .. }), "{error:?}");

    // Cannot be written: reported once, the counts stay in memory.
    manager.record_usage(&MappingKey::Char('a'), "ä");
    let error = manager.save_usage(Path::new("/")).unwrap_err();
    assert!(matches!(error, CoreError::WriteConfig { .. }), "{error:?}");
    manager.save_usage(Path::new("/"))?;
    assert_eq!(manager.usage_stats[&MappingKey::Char('a')]["ä"], 1);
    Ok(())
}

#[test]
fn test_sort_all_by_usage() {
    let mut manager = MappingManager::new();
    let mut profile = MappingProfile::new("Sort");
    for (key, variants) in [('a', ["ä", "à"]), ('e', ["é", "è"]), ('o', ["ö", "ò"])] {
        profile.mappings.insert(
            MappingKey::Char(key),
            KeyMapping::new(variants.map(String::from).to_vec()),
        );
    }
    manager.add_profile(profile);
    manager.set_current_profile("Sort");
    for (key, variant) in [('a', "à"), ('e', "é"), ('z', "ž")] {
        manager.record_usage(&MappingKey::Char(key), variant);
    }

    // Every key with counts is sorted (an unmapped one is ignored); the
    // others keep their stored order.
    manager.sort_all_by_usage();
    let characters = |key| {
        manager
            .get_characters_for_key(&MappingKey::Char(key))
            .map(|variants| variants.join(","))
    };
    assert_eq!(characters('a').as_deref(), Some("à,ä"));
    assert_eq!(characters('e').as_deref(), Some("é,è"));
    assert_eq!(characters('o').as_deref(), Some("ö,ò"));
    assert_eq!(characters('z'), None);
}

#[test]
fn test_mapping_key_from_key() {
    assert_eq!(
        MappingKey::from_key("a", false),
        Some(MappingKey::Char('a'))
    );
    assert_eq!(
        MappingKey::from_key("é", false),
        Some(MappingKey::Char('é'))
    );
    assert_eq!(
        MappingKey::from_key("a", true),
        Some(MappingKey::ShiftChar('A'))
    );
    for (name, special) in [
        ("space", "Space"),
        ("LEFT", "LeftArrow"),
        ("right", "RightArrow"),
        ("up", "UpArrow"),
        ("down", "DownArrow"),
    ] {
        assert_eq!(
            MappingKey::from_key(name, false),
            Some(MappingKey::Special(special.to_string()))
        );
    }
    assert_eq!(MappingKey::from_key("", false), None);
    assert_eq!(MappingKey::from_key("alt", false), None);
}

#[test]
fn test_mapping_manager() {
    let mut manager = MappingManager::new();
    manager.load_default_mappings();

    // Test that default profile is loaded
    assert!(manager.current_profile().is_some());
    assert_eq!(manager.current_profile().unwrap().name, "Default");

    // Test that default mappings are loaded
    let profile = manager.current_profile().unwrap();
    assert!(profile.mappings.contains_key(&MappingKey::Char('a')));
    assert!(profile.mappings.contains_key(&MappingKey::Char('o')));
    assert!(profile.mappings.contains_key(&MappingKey::Char('u')));
    assert!(profile.mappings.contains_key(&MappingKey::ShiftChar('A')));

    // Test character retrieval
    assert_eq!(
        manager.get_characters_for_key(&MappingKey::Char('a')),
        Some(vec!["ä".to_string()].as_slice())
    );
    assert_eq!(
        manager.get_characters_for_key(&MappingKey::Char('s')),
        Some(vec!["ß".to_string()].as_slice())
    );
}

#[test]
fn test_forward_cycling() {
    let mut manager = MappingManager::new();

    // Create a profile with multiple characters for a key
    let mut profile = MappingProfile::new("Test");

    profile.mappings.insert(
        MappingKey::Char('e'),
        KeyMapping::new(vec!["é".to_string(), "è".to_string(), "ê".to_string()]),
    );

    manager.add_profile(profile);
    manager.set_current_profile("Test");

    // Test forward cycling
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("é".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("è".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("ê".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("é".to_string())
    ); // Should wrap around
}

#[test]
fn test_reverse_cycling() {
    let mut manager = MappingManager::new();

    // Create a profile with multiple characters for a key
    let mut profile = MappingProfile::new("TestReverse");

    profile.mappings.insert(
        MappingKey::Char('e'),
        KeyMapping {
            characters: vec!["é".to_string(), "è".to_string(), "ê".to_string()],
            cycle_position: 2, // Start at the last position
            reverse_cycle: true,
        },
    );

    manager.add_profile(profile);
    manager.set_current_profile("TestReverse");

    // Test reverse cycling (going backward)
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("ê".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("è".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("é".to_string())
    );
    assert_eq!(
        manager.cycle_next(&MappingKey::Char('e')),
        Some("ê".to_string())
    ); // Should wrap around
}
