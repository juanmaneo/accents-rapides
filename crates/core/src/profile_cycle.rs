//! Pure profile-cycle logic, ported from the C++ `profile_cycle.h`
//! (tests/testprofilecycle.cpp): the favorites-vs-all selection and the
//! wrapping next/previous step. No platform integration needed.

/// A profile entry for cycling: its name and whether it is a favorite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleEntry {
    pub name: String,
    pub favorite: bool,
}

impl CycleEntry {
    pub fn new(name: impl Into<String>, favorite: bool) -> Self {
        Self {
            name: name.into(),
            favorite,
        }
    }
}

/// The profile names the cycle steps through: only the favorites when any
/// are set (preserving their stored order), otherwise all profiles.
pub fn cycle_names(entries: &[CycleEntry]) -> Vec<String> {
    let favorites: Vec<String> = entries
        .iter()
        .filter(|e| e.favorite)
        .map(|e| e.name.clone())
        .collect();

    if favorites.is_empty() {
        entries.iter().map(|e| e.name.clone()).collect()
    } else {
        favorites
    }
}

/// The next profile name in the cycle, stepping `direction` (+1 forward,
/// -1 backward) with wrap-around.
///
/// When `active` is not part of the cycle list, +1 steps to the first entry
/// and -1 to the last. Returns `None` when the list is empty.
pub fn cycle_target(names: &[String], active: &str, direction: i32) -> Option<String> {
    if names.is_empty() {
        return None;
    }

    let position = names.iter().position(|n| n == active);

    let index: usize = match position {
        Some(pos) => {
            let len = names.len() as isize;
            let next = pos as isize + direction as isize;
            (((next % len) + len) % len) as usize // wrapping step, also for backward
        }
        None => {
            // Active is not in the list: step to first (forward) / last
            // (backward).
            if direction >= 0 {
                0
            } else {
                names.len() - 1
            }
        }
    };

    names.get(index).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from tests/testprofilecycle.cpp: no favorites — cycle through
    // all profiles, in order, wrapping.
    #[test]
    fn no_favorites_cycles_all_with_wrap() {
        let p = vec![
            CycleEntry::new("A", false),
            CycleEntry::new("B", false),
            CycleEntry::new("C", false),
        ];
        let names = cycle_names(&p);
        assert_eq!(names.len(), 3);
        assert_eq!(cycle_target(&names, "A", 1), Some("B".to_string()));
        assert_eq!(cycle_target(&names, "C", 1), Some("A".to_string())); // wrap forward
        assert_eq!(cycle_target(&names, "A", -1), Some("C".to_string())); // wrap backward
        assert_eq!(cycle_target(&names, "B", -1), Some("A".to_string()));
    }

    // Ported from tests/testprofilecycle.cpp: some favorites — cycle through
    // favorites only, preserving order.
    #[test]
    fn favorites_cycle_favorites_only() {
        let p = vec![
            CycleEntry::new("A", false),
            CycleEntry::new("B", true),
            CycleEntry::new("C", false),
            CycleEntry::new("D", true),
        ];
        let names = cycle_names(&p);
        assert_eq!(names, vec!["B".to_string(), "D".to_string()]);
        assert_eq!(cycle_target(&names, "B", 1), Some("D".to_string()));
        assert_eq!(cycle_target(&names, "D", 1), Some("B".to_string())); // wrap
                                                                         // Active is not a favorite: step to first (next) / last (previous).
        assert_eq!(cycle_target(&names, "A", 1), Some("B".to_string()));
        assert_eq!(cycle_target(&names, "A", -1), Some("D".to_string()));
    }

    // Ported from tests/testprofilecycle.cpp: empty profile list — nothing
    // to cycle to.
    #[test]
    fn empty_profile_list_cycles_nothing() {
        let p: Vec<CycleEntry> = vec![];
        assert_eq!(cycle_target(&cycle_names(&p), "X", 1), None);
    }
}
