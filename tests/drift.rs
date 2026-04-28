//! Tests for `crate::drift` — RFC 7396 diff and apply.

use hexis_cli::drift::DriftPatch;
use serde_json::json;

#[test]
fn empty_patch_is_empty() {
    let patch = DriftPatch::empty();
    assert!(patch.is_empty());
    assert_eq!(patch.as_value(), &json!({}));
}

#[test]
fn between_equal_objects_is_empty_patch() {
    let document = json!({ "editor": { "tabSize": 4 } });
    let patch = DriftPatch::between(&document, &document);
    assert!(patch.is_empty());
}

#[test]
fn between_detects_added_key() {
    let before = json!({ "editor": { "tabSize": 4 } });
    let after = json!({ "editor": { "tabSize": 4 }, "files": { "trim": true } });
    let patch = DriftPatch::between(&before, &after);
    assert_eq!(patch.as_value(), &json!({ "files": { "trim": true } }));
}

#[test]
fn between_detects_changed_value_via_recursion() {
    let before = json!({ "editor": { "tabSize": 4, "wordWrap": "on" } });
    let after = json!({ "editor": { "tabSize": 2, "wordWrap": "on" } });
    let patch = DriftPatch::between(&before, &after);
    // Only tabSize differs — wordWrap is unchanged and omitted.
    assert_eq!(patch.as_value(), &json!({ "editor": { "tabSize": 2 } }));
}

#[test]
fn between_emits_null_for_removed_key() {
    let before = json!({ "editor": { "tabSize": 4, "deprecated": true } });
    let after = json!({ "editor": { "tabSize": 4 } });
    let patch = DriftPatch::between(&before, &after);
    assert_eq!(patch.as_value(), &json!({ "editor": { "deprecated": null } }));
}

#[test]
fn between_replaces_wholesale_when_types_disagree() {
    let before = json!({ "k": [1, 2, 3] });
    let after = json!({ "k": "scalar-now" });
    let patch = DriftPatch::between(&before, &after);
    assert_eq!(patch.as_value(), &json!({ "k": "scalar-now" }));
}

#[test]
fn apply_to_round_trips_with_between() {
    let before = json!({
        "editor": { "tabSize": 4, "wordWrap": "on" },
        "files":  { "trim": true }
    });
    let after = json!({
        "editor": { "tabSize": 2, "wordWrap": "on" },
        "extras": ["one", "two"]
    });
    let patch = DriftPatch::between(&before, &after);
    let mut reconstructed = before.clone();
    patch.apply_to(&mut reconstructed);
    assert_eq!(reconstructed, after);
}

#[test]
fn apply_to_with_empty_patch_is_noop_for_object_targets() {
    let mut target = json!({ "editor": { "tabSize": 4 } });
    DriftPatch::empty().apply_to(&mut target);
    assert_eq!(target, json!({ "editor": { "tabSize": 4 } }));
}

#[test]
fn from_value_wraps_arbitrary_patch() {
    let patch = DriftPatch::from_value(json!({ "k": "v", "deleted": null }));
    let mut target = json!({ "deleted": "still here", "other": 1 });
    patch.apply_to(&mut target);
    assert_eq!(target, json!({ "k": "v", "other": 1 }));
}

#[test]
fn nested_diff_does_not_emit_empty_subtrees() {
    // When a nested object is unchanged, the entire key should be omitted —
    // not emitted as `{ "k": {} }`. Otherwise drift reports balloon with
    // empty noise.
    let before = json!({ "a": { "b": { "c": 1 } }, "x": 2 });
    let after = json!({ "a": { "b": { "c": 1 } }, "x": 99 });
    let patch = DriftPatch::between(&before, &after);
    assert_eq!(patch.as_value(), &json!({ "x": 99 }));
}
