//! Tests for `crate::snapshot` — markers + image schema, round-trip,
//! and `clear_subtree` semantics.

use std::str::FromStr;

use hexis_cli::Error;
use hexis_cli::snapshot::{Marker, Snapshot};
use hexis_cli::types::JsonPointer;
use tempfile::tempdir;

#[test]
fn empty_snapshot_has_no_markers_and_null_image() {
    let snapshot = Snapshot::empty(std::path::PathBuf::from("/tmp/snap.json"));
    let any_pointer = JsonPointer::from_str("/anything").unwrap();
    assert!(snapshot.marker(&any_pointer).is_none());
    assert!(snapshot.image().is_null());
}

#[test]
fn set_and_retrieve_marker() {
    let mut snapshot = Snapshot::empty(std::path::PathBuf::from("/tmp/snap.json"));
    let pointer = JsonPointer::from_str("/devtools/autoConnect").unwrap();
    snapshot.set_marker(
        pointer.clone(),
        Marker::new("2026-04-28T15:00:00Z".to_string(), serde_json::json!(true)),
    );
    let marker = snapshot.marker(&pointer).expect("set marker is retrievable");
    assert_eq!(marker.applied_at(), "2026-04-28T15:00:00Z");
    assert_eq!(marker.value_when_applied(), &serde_json::json!(true));
}

#[test]
fn write_atomic_round_trips_markers_and_image() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("snap.json");

    let mut snapshot = Snapshot::empty(path.clone());
    let pointer = JsonPointer::from_str("/devtools/autoConnect").unwrap();
    snapshot.set_marker(
        pointer.clone(),
        Marker::new("2026-04-28T15:00:00Z".to_string(), serde_json::json!(true)),
    );
    snapshot.set_image(serde_json::json!({ "editor": { "tabSize": 4 } }));
    snapshot.write_atomic(&path).expect("write");

    let read_back = Snapshot::from_path(&path).expect("read");
    let read_marker = read_back.marker(&pointer).expect("marker survives round-trip");
    assert_eq!(read_marker.applied_at(), "2026-04-28T15:00:00Z");
    assert_eq!(read_marker.value_when_applied(), &serde_json::json!(true));
    assert_eq!(
        read_back.image().pointer("/editor/tabSize"),
        Some(&serde_json::json!(4)),
    );
}

#[test]
fn from_path_or_empty_yields_empty_when_missing() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("does-not-exist.json");
    let snapshot = Snapshot::from_path_or_empty(&path).expect("graceful first-run");
    assert!(snapshot.image().is_null());
}

#[test]
fn parse_rejects_unknown_schema() {
    let text = r#"{ "schema": 99, "applied_markers": {}, "image": null }"#;
    let result = Snapshot::from_text_for_test(text);
    assert!(matches!(result, Err(Error::SnapshotParse { .. })));
}

#[test]
fn parse_rejects_missing_schema() {
    let text = r#"{ "applied_markers": {}, "image": null }"#;
    let result = Snapshot::from_text_for_test(text);
    assert!(matches!(result, Err(Error::SnapshotParse { .. })));
}

#[test]
fn parse_rejects_non_object_root() {
    let text = r#"[1, 2, 3]"#;
    let result = Snapshot::from_text_for_test(text);
    assert!(matches!(result, Err(Error::SnapshotParse { .. })));
}

#[test]
fn parse_accepts_minimal_snapshot() {
    let text = r#"{ "schema": 1, "applied_markers": {}, "image": null }"#;
    let snapshot = Snapshot::from_text_for_test(text).expect("minimal schema");
    assert!(snapshot.image().is_null());
}

#[test]
fn parse_accepts_snapshot_without_applied_markers_field() {
    let text = r#"{ "schema": 1, "image": { "editor": { "tabSize": 4 } } }"#;
    let snapshot = Snapshot::from_text_for_test(text).expect("missing markers field is fine");
    assert_eq!(
        snapshot.image().pointer("/editor/tabSize"),
        Some(&serde_json::json!(4)),
    );
}

#[test]
fn clear_subtree_drops_descendants_of_root() {
    let mut snapshot = Snapshot::empty(std::path::PathBuf::from("/tmp/snap.json"));
    let inside = JsonPointer::from_str("/security/sandbox").unwrap();
    let outside = JsonPointer::from_str("/editor/tabSize").unwrap();
    let marker = Marker::new("now".to_string(), serde_json::json!(true));
    snapshot.set_marker(inside.clone(), Marker::new("now".to_string(), serde_json::json!(true)));
    snapshot.set_marker(outside.clone(), marker);

    let security = JsonPointer::from_str("/security").unwrap();
    snapshot.clear_subtree(&security);

    assert!(snapshot.marker(&inside).is_none(), "descendant marker dropped");
    assert!(snapshot.marker(&outside).is_some(), "unrelated marker survives");
}

#[test]
fn clear_subtree_at_root_drops_everything() {
    let mut snapshot = Snapshot::empty(std::path::PathBuf::from("/tmp/snap.json"));
    let pointer = JsonPointer::from_str("/anywhere").unwrap();
    snapshot.set_marker(
        pointer.clone(),
        Marker::new("now".to_string(), serde_json::json!(0)),
    );
    snapshot.clear_subtree(&JsonPointer::root());
    assert!(snapshot.marker(&pointer).is_none());
}
