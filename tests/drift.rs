//! Tests for `crate::drift` — RFC 7396 diff and apply, and the
//! drift journal's append + rotation + legacy-migration behaviour.

use hexis_cli::Error;
use hexis_cli::drift::{DriftEntry, DriftJournal, DriftPatch};
use serde_json::json;
use tempfile::tempdir;

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

#[test]
fn journal_empty_on_missing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.json");
    let journal = DriftJournal::from_path_or_empty(&path).expect("graceful first-run");
    assert!(journal.is_empty());
}

#[test]
fn journal_appends_and_round_trips_through_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("drift.json");

    let mut journal = DriftJournal::empty();
    journal.append(DriftEntry::new(
        "2026-04-28T12:00:00Z".to_string(),
        DriftPatch::from_value(json!({ "k": "v1" })),
    ));
    journal.append(DriftEntry::new(
        "2026-04-28T13:00:00Z".to_string(),
        DriftPatch::from_value(json!({ "k": "v2" })),
    ));
    journal.write_atomic(&path).expect("write");

    let read_back = DriftJournal::from_path_or_empty(&path).expect("read");
    assert_eq!(read_back.entries().len(), 2);
    assert_eq!(read_back.entries()[0].applied_at(), "2026-04-28T12:00:00Z");
    assert_eq!(read_back.entries()[1].applied_at(), "2026-04-28T13:00:00Z");
    assert_eq!(
        read_back.entries()[1].drift().as_value(),
        &json!({ "k": "v2" })
    );
}

#[test]
fn journal_rotates_oldest_off_at_max_entries() {
    let mut journal = DriftJournal::empty();
    for index in 0..(DriftJournal::MAX_ENTRIES + 5) {
        journal.append(DriftEntry::new(
            format!("2026-04-28T{index:02}:00:00Z"),
            DriftPatch::from_value(json!({ "i": index })),
        ));
    }
    assert_eq!(journal.entries().len(), DriftJournal::MAX_ENTRIES);
    // The oldest 5 entries rolled off — the first survivor is index 5.
    assert_eq!(journal.entries()[0].applied_at(), "2026-04-28T05:00:00Z");
}

#[test]
fn journal_migrates_legacy_single_entry_format() {
    // Old v0.1 drift files were a single { applied_at, drift } object.
    // Reading them must succeed and surface the entry as a single-element
    // journal so subsequent appends rotate cleanly.
    let legacy = r#"{
        "applied_at": "2026-04-28T10:00:00Z",
        "drift": { "editor": { "tabSize": 2 } }
    }"#;
    let journal = DriftJournal::from_text_for_test(legacy).expect("legacy migrates");
    assert_eq!(journal.entries().len(), 1);
    assert_eq!(journal.entries()[0].applied_at(), "2026-04-28T10:00:00Z");
}

#[test]
fn journal_rejects_unknown_shape() {
    let weird = r#"{ "neither_schema_nor_legacy": true }"#;
    let result = DriftJournal::from_text_for_test(weird);
    assert!(matches!(result, Err(Error::DriftParse { .. })));
}

#[test]
fn journal_appending_persists_through_disk_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("drift.json");

    // First write.
    let mut journal = DriftJournal::empty();
    journal.append(DriftEntry::new(
        "first".to_string(),
        DriftPatch::from_value(json!({ "first": true })),
    ));
    journal.write_atomic(&path).unwrap();

    // Read, append, write — simulating two consecutive applies.
    let mut journal = DriftJournal::from_path_or_empty(&path).unwrap();
    journal.append(DriftEntry::new(
        "second".to_string(),
        DriftPatch::from_value(json!({ "second": true })),
    ));
    journal.write_atomic(&path).unwrap();

    let final_state = DriftJournal::from_path_or_empty(&path).unwrap();
    assert_eq!(final_state.entries().len(), 2);
    assert_eq!(final_state.entries()[0].applied_at(), "first");
    assert_eq!(final_state.entries()[1].applied_at(), "second");
}
