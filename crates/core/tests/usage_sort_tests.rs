//! Unit tests for the frequency-sort comparator, ported from
//! tests/testusagesort.cpp.
//!
//! The three properties the whole feature leans on: descending by count,
//! stable tie-break on the stored order, and a no-op when all counts are
//! zero (fresh stats look exactly like toggle-off).

use accents_rapides_core::{sort_variants_by_usage, UsageCounts};

#[test]
fn descending_by_count() {
    let stored: Vec<String> = ["ä", "â", "à"].iter().map(|s| s.to_string()).collect();
    let mut counts = UsageCounts::new();
    counts.insert("à".to_string(), 40);
    counts.insert("ä".to_string(), 12);
    counts.insert("â".to_string(), 3);

    let out = sort_variants_by_usage(&stored, &counts);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], "à"); // à, 40
    assert_eq!(out[1], "ä"); // ä, 12
    assert_eq!(out[2], "â"); // â, 3
}

#[test]
fn all_zero_is_stored_order() {
    let stored: Vec<String> = ["ä", "â", "à"].iter().map(|s| s.to_string()).collect();
    let counts = UsageCounts::new(); // empty → all zero

    let out = sort_variants_by_usage(&stored, &counts);
    assert_eq!(out, stored); // nothing jumps around
}

#[test]
fn ties_keep_stored_order() {
    let stored: Vec<String> = ["ä", "â", "à"].iter().map(|s| s.to_string()).collect();
    // ä and à tie at 5, â is 0.
    let mut counts = UsageCounts::new();
    counts.insert("ä".to_string(), 5);
    counts.insert("à".to_string(), 5);

    let out = sort_variants_by_usage(&stored, &counts);
    assert_eq!(out[0], "ä"); // ä before à — stored relative order kept
    assert_eq!(out[1], "à"); // à
    assert_eq!(out[2], "â"); // â, 0, last
}
