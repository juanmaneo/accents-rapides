//! Unit tests for `split_outputs` (double-comma escaping), ported from the
//! splitOutputs tests in tests/testschnelleumlaute.cpp (tests 14-16, 87-91,
//! 101-103).

use accents_rapides_core::split_outputs;

#[test]
fn single_comma_separates_variants() {
    assert_eq!(split_outputs("ä,ae"), vec!["ä", "ae"]);
}

// Test 14: "a,,b" → single output "a,b" (escaped comma, not cycling)
#[test]
fn double_comma_escape_single_output() {
    assert_eq!(split_outputs("a,,b"), vec!["a,b"]);
}

// Test 15: "x,,y,z" → ["x,y", "z"] (double-comma within cycling)
#[test]
fn double_comma_with_cycling() {
    assert_eq!(split_outputs("x,,y,z"), vec!["x,y", "z"]);
}

// Test 16 / 90: ",,," → [","] (escaped comma + separator)
#[test]
fn triple_comma_single_literal() {
    assert_eq!(split_outputs(",,,"), vec![","]);
}

// Test 87: "ä," → ["ä"] (trailing empty segment skipped)
#[test]
fn trailing_comma_ignored() {
    assert_eq!(split_outputs("ä,"), vec!["ä"]);
}

// Test 88: ",ä" → ["ä"] (leading empty segment skipped)
#[test]
fn leading_comma_ignored() {
    assert_eq!(split_outputs(",ä"), vec!["ä"]);
}

// Test 89: "x,,,y" → ["x,", "y"] (double-comma literal + separator)
#[test]
fn double_comma_then_separator() {
    assert_eq!(split_outputs("x,,,y"), vec!["x,", "y"]);
}

// Test 102: ",," is a double-comma escape → single output [","]
#[test]
fn only_double_comma_is_literal_comma() {
    assert_eq!(split_outputs(",,"), vec![","]);
}

// Test 101: "," produces an empty output vector; the mapping is skipped.
#[test]
fn single_comma_is_empty_output() {
    assert_eq!(split_outputs(","), Vec::<String>::new());
}

// Test 103 edge: the empty string produces no variants either.
#[test]
fn empty_spec_is_empty_output() {
    assert_eq!(split_outputs(""), Vec::<String>::new());
}
