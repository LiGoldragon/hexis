//! Tests for `crate::types` — `JsonPointer` ancestor walks, parsing,
//! and resolution against `serde_json::Value`.

use std::str::FromStr;

use hexis_cli::Error;
use hexis_cli::types::JsonPointer;

#[test]
fn root_pointer_is_empty() {
    let root = JsonPointer::root();
    assert!(root.is_root());
    assert_eq!(root.as_str(), "");
}

#[test]
fn parse_empty_string_yields_root() {
    let pointer = JsonPointer::from_str("").expect("empty string is valid");
    assert!(pointer.is_root());
}

#[test]
fn parse_rejects_pointer_without_leading_slash() {
    let result = JsonPointer::from_str("foo/bar");
    assert!(matches!(result, Err(Error::InvalidJsonPointer(_))));
}

#[test]
fn parse_accepts_well_formed_pointer() {
    let pointer = JsonPointer::from_str("/foo/bar").expect("valid");
    assert_eq!(pointer.as_str(), "/foo/bar");
}

#[test]
fn parent_of_root_is_none() {
    assert!(JsonPointer::root().parent().is_none());
}

#[test]
fn parent_of_top_level_pointer_is_root() {
    let parent = JsonPointer::from_str("/foo").unwrap().parent().unwrap();
    assert!(parent.is_root());
}

#[test]
fn parent_strips_one_segment() {
    let parent = JsonPointer::from_str("/foo/bar/baz")
        .unwrap()
        .parent()
        .unwrap();
    assert_eq!(parent.as_str(), "/foo/bar");
}

#[test]
fn ancestors_yields_self_through_root() {
    let pointer = JsonPointer::from_str("/foo/bar").unwrap();
    let chain: Vec<String> = pointer.ancestors().map(|p| p.to_string()).collect();
    assert_eq!(chain, vec!["/foo/bar", "/foo", ""]);
}

#[test]
fn ancestors_of_root_yields_just_root() {
    let chain: Vec<String> = JsonPointer::root().ancestors().map(|p| p.to_string()).collect();
    assert_eq!(chain, vec![""]);
}

#[test]
fn resolve_root_returns_whole_document() {
    let document = serde_json::json!({ "a": 1, "b": [2, 3] });
    let resolved = JsonPointer::root().resolve(&document).expect("root resolves");
    assert_eq!(resolved, &document);
}

#[test]
fn resolve_walks_into_nested_objects() {
    let document = serde_json::json!({ "outer": { "inner": "leaf" } });
    let pointer = JsonPointer::from_str("/outer/inner").unwrap();
    let resolved = pointer.resolve(&document).expect("resolves");
    assert_eq!(resolved, &serde_json::json!("leaf"));
}

#[test]
fn resolve_returns_none_when_segment_is_missing() {
    let document = serde_json::json!({ "outer": { } });
    let pointer = JsonPointer::from_str("/outer/missing").unwrap();
    assert!(pointer.resolve(&document).is_none());
}

#[test]
fn root_is_ancestor_of_everything_including_itself() {
    let root = JsonPointer::root();
    assert!(root.is_descendant_of(&root));
    let deep = JsonPointer::from_str("/a/b/c").unwrap();
    assert!(deep.is_descendant_of(&root));
}

#[test]
fn pointer_is_descendant_of_itself() {
    let pointer = JsonPointer::from_str("/a/b").unwrap();
    assert!(pointer.is_descendant_of(&pointer));
}

#[test]
fn child_is_descendant_of_parent() {
    let parent = JsonPointer::from_str("/a").unwrap();
    let child = JsonPointer::from_str("/a/b").unwrap();
    assert!(child.is_descendant_of(&parent));
}

#[test]
fn prefix_overlap_is_not_descendancy() {
    // "/aa" starts with "/a" but is not a child of "/a" — the segment
    // boundary check (next byte = '/') is what prevents the false match.
    let candidate = JsonPointer::from_str("/aa").unwrap();
    let root = JsonPointer::from_str("/a").unwrap();
    assert!(!candidate.is_descendant_of(&root));
}

#[test]
fn unrelated_pointer_is_not_descendant() {
    let a = JsonPointer::from_str("/foo").unwrap();
    let b = JsonPointer::from_str("/bar").unwrap();
    assert!(!a.is_descendant_of(&b));
    assert!(!b.is_descendant_of(&a));
}
