//! Tests for `crate::live` — read/parse/write round-trips and the
//! empty/missing-file path.

use hexis_cli::live::Live;
use hexis_cli::Error;
use tempfile::tempdir;

#[test]
fn parse_valid_json() {
    let live = Live::from_text_for_test(r#"{ "editor": { "tabSize": 2 } }"#).expect("parses");
    assert_eq!(
        live.data().pointer("/editor/tabSize"),
        Some(&serde_json::json!(2)),
    );
}

#[test]
fn parse_rejects_malformed_json() {
    let result = Live::from_text_for_test(r#"{ this is not json"#);
    assert!(matches!(result, Err(Error::LiveParse { .. })));
}

#[test]
fn parse_valid_toml() {
    let live = Live::from_text_for_test_at_path(
        r#"
        [build]
        jobs = 2
        "#,
        std::path::PathBuf::from("config.toml"),
    )
    .expect("parses");
    assert_eq!(
        live.data().pointer("/build/jobs"),
        Some(&serde_json::json!(2)),
    );
}

#[test]
fn parse_rejects_malformed_toml() {
    let result =
        Live::from_text_for_test_at_path(r#"[build"#, std::path::PathBuf::from("config.toml"));
    assert!(matches!(result, Err(Error::LiveParse { .. })));
}

#[test]
fn empty_carries_object_root() {
    let live = Live::empty(std::path::PathBuf::from("/tmp/example.json"));
    assert!(live.data().is_object());
    assert_eq!(live.data().as_object().unwrap().len(), 0);
}

#[test]
fn from_path_or_empty_yields_empty_when_missing() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("does-not-exist.json");
    let live = Live::from_path_or_empty(&path).expect("graceful first-run");
    assert!(live.data().is_object());
}

#[test]
fn from_path_or_empty_parses_when_present() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("present.json");
    std::fs::write(&path, r#"{ "k": "v" }"#).expect("seed");
    let live = Live::from_path_or_empty(&path).expect("parses");
    assert_eq!(live.data().pointer("/k"), Some(&serde_json::json!("v")));
}

#[test]
fn write_atomic_round_trips_through_from_path() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("settings.json");
    let mut live = Live::empty(path.clone());
    live.set_data(serde_json::json!({ "editor": { "tabSize": 4 } }));
    live.write_atomic(&path).expect("write");

    let read_back = Live::from_path(&path).expect("read");
    assert_eq!(
        read_back.data().pointer("/editor/tabSize"),
        Some(&serde_json::json!(4)),
    );
}

#[test]
fn write_atomic_preserves_toml_format_for_toml_paths() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    let mut live = Live::empty(path.clone());
    live.set_data(serde_json::json!({ "build": { "jobs": 2 } }));
    live.write_atomic(&path).expect("write");

    let text = std::fs::read_to_string(&path).expect("read text");
    assert!(text.contains("[build]"), "writes TOML tables");
    assert!(text.contains("jobs = 2"), "writes TOML values");

    let read_back = Live::from_path(&path).expect("read");
    assert_eq!(
        read_back.data().pointer("/build/jobs"),
        Some(&serde_json::json!(2)),
    );
}

#[test]
fn mutating_via_data_mut_persists_through_round_trip() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("settings.json");
    let mut live = Live::from_text_for_test(r#"{ "editor": { "tabSize": 2 } }"#).unwrap();
    if let Some(object) = live.data_mut().as_object_mut() {
        object.insert("trailing".to_string(), serde_json::json!(true));
    }
    live.write_atomic(&path).expect("write");

    let read_back = Live::from_path(&path).expect("read");
    assert_eq!(
        read_back.data().pointer("/trailing"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        read_back.data().pointer("/editor/tabSize"),
        Some(&serde_json::json!(2))
    );
}
