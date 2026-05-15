//! End-to-end tests for the four-step reconcile flow.
//!
//! Tests construct a [`reconciler::Arguments`] pointing at a tempdir,
//! call [`State::apply`], and verify the resulting on-disk state
//! (live, snapshot, drift). The actor harness is exercised separately
//! by `scaffold.rs`'s spawn/shutdown smoke test.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use hexis_cli::live::Live;
use hexis_cli::reconciler::{Arguments, State};
use hexis_cli::snapshot::Snapshot;
use hexis_cli::types::{FileId, JsonPointer};
use serde_json::json;
use tempfile::tempdir;

struct Fixture {
    _dir: tempfile::TempDir,
    arguments: Arguments,
}

impl Fixture {
    fn apply(&self) -> Result<(), hexis_cli::Error> {
        State::new(self.arguments.clone()).apply()
    }

    fn new() -> Self {
        Self::new_with_live_name("live.json")
    }

    fn new_with_live_name(live_name: &str) -> Self {
        let dir = tempdir().expect("tempdir");
        let live_path = dir.path().join(live_name);
        let declared_path = dir.path().join("declared.json");
        let snapshot_dir = dir.path().join("snapshot");
        let drift_dir = dir.path().join("drift");
        let arguments = Arguments {
            file_id: FileId::from_path(&live_path),
            declared_path,
            live_path,
            snapshot_dir,
            drift_dir,
            dry_run: false,
        };
        Self {
            _dir: dir,
            arguments,
        }
    }

    fn write_declared(&self, json: &str) {
        fs::write(&self.arguments.declared_path, json).expect("write declared");
    }

    fn write_live(&self, json: &str) {
        fs::write(&self.arguments.live_path, json).expect("write live");
    }

    fn snapshot_path(&self) -> PathBuf {
        self.arguments
            .snapshot_dir
            .join(format!("{}.json", self.arguments.file_id))
    }

    fn drift_path(&self) -> PathBuf {
        self.arguments
            .drift_dir
            .join(format!("{}.json", self.arguments.file_id))
    }
}

#[test]
fn apply_writes_live_and_snapshot_when_neither_exists() {
    let fixture = Fixture::new();
    fixture.write_declared(r#"{ "editor": { "tabSize": 4 } }"#);

    fixture.apply().expect("apply");

    let live = Live::from_path(&fixture.arguments.live_path).expect("live written");
    assert_eq!(live.data().pointer("/editor/tabSize"), Some(&json!(4)));

    let snapshot_path = fixture.snapshot_path();
    assert!(snapshot_path.exists(), "snapshot file written");
    let snapshot = Snapshot::from_path(&snapshot_path).expect("snapshot parses");
    assert_eq!(snapshot.image().pointer("/editor/tabSize"), Some(&json!(4)));
}

#[test]
fn apply_first_run_does_not_emit_drift_report() {
    // First run has no prior snapshot to diff against — drift would be
    // "the entire live wholesale," which isn't meaningful.
    let fixture = Fixture::new();
    fixture.write_declared(r#"{ "editor": { "tabSize": 4 } }"#);

    fixture.apply().expect("apply");

    assert!(
        !fixture.drift_path().exists(),
        "no drift report on first run"
    );
}

#[test]
fn apply_preserves_user_keys_declared_does_not_mention() {
    // Live has /editor/lineNumbers; declared mentions only /editor/tabSize.
    // Default mode is Ensure → user keys survive.
    let fixture = Fixture::new();
    fixture.write_declared(r#"{ "editor": { "tabSize": 4 } }"#);
    fixture.write_live(r#"{ "editor": { "tabSize": 2, "lineNumbers": true } }"#);

    fixture.apply().expect("apply");

    let live = Live::from_path(&fixture.arguments.live_path).expect("read live");
    assert_eq!(live.data().pointer("/editor/tabSize"), Some(&json!(4)));
    assert_eq!(
        live.data().pointer("/editor/lineNumbers"),
        Some(&json!(true))
    );
}

#[test]
fn apply_dry_run_skips_writes() {
    let fixture = Fixture::new();
    fixture.write_declared(r#"{ "editor": { "tabSize": 4 } }"#);
    let mut arguments = fixture.arguments;
    arguments.dry_run = true;

    State::new(arguments.clone())
        .apply()
        .expect("apply dry-run");

    assert!(!arguments.live_path.exists(), "live not written in dry-run");
    let snapshot_path = arguments
        .snapshot_dir
        .join(format!("{}.json", arguments.file_id));
    assert!(!snapshot_path.exists(), "snapshot not written in dry-run");
}

#[test]
fn once_mode_writes_marker_then_leaves_alone_on_second_pass() {
    let fixture = Fixture::new();
    fixture.write_declared(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/devtools/autoConnect": "once" } },
            "devtools": { "autoConnect": true }
        }"#,
    );

    // First pass: writes the value, records the marker.
    fixture.apply().expect("first apply");
    let live_after_first = Live::from_path(&fixture.arguments.live_path).expect("first live");
    assert_eq!(
        live_after_first.data().pointer("/devtools/autoConnect"),
        Some(&json!(true)),
    );
    let snapshot = Snapshot::from_path(&fixture.snapshot_path()).expect("first snapshot");
    let pointer = JsonPointer::from_str("/devtools/autoConnect").unwrap();
    assert!(snapshot.marker(&pointer).is_some(), "marker recorded");

    // User toggles the value off.
    fixture.write_live(r#"{ "devtools": { "autoConnect": false } }"#);

    // Second pass: marker exists → LeaveAlone. User's value survives.
    fixture.apply().expect("second apply");
    let live_after_second = Live::from_path(&fixture.arguments.live_path).expect("second live");
    assert_eq!(
        live_after_second.data().pointer("/devtools/autoConnect"),
        Some(&json!(false)),
        "once-mode + marker → user value survives",
    );
}

#[test]
fn second_apply_with_drift_emits_drift_report() {
    let fixture = Fixture::new();
    fixture.write_declared(r#"{ "editor": { "tabSize": 4 } }"#);

    // First pass establishes the snapshot baseline.
    fixture.apply().expect("first apply");

    // User changes a key declared doesn't override.
    fixture.write_live(r#"{ "editor": { "tabSize": 4, "wordWrap": "on" } }"#);

    // Second pass: drift detected, drift report written.
    fixture.apply().expect("second apply");

    assert!(
        fixture.drift_path().exists(),
        "drift report exists after second apply"
    );
    let drift_text = fs::read_to_string(fixture.drift_path()).expect("read drift");
    let journal: serde_json::Value =
        serde_json::from_str(&drift_text).expect("parse drift journal");
    assert_eq!(
        journal.pointer("/schema"),
        Some(&json!(1)),
        "drift journal carries schema version",
    );
    assert!(
        journal.pointer("/entries/0/applied_at").is_some(),
        "first entry has applied_at",
    );
    assert_eq!(
        journal.pointer("/entries/0/drift/editor/wordWrap"),
        Some(&json!("on")),
        "drift records the user's added key",
    );
}

#[test]
fn always_mode_overwrites_user_drift() {
    let fixture = Fixture::new();
    fixture.write_declared(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/security/sandbox": "always" } },
            "security": { "sandbox": true }
        }"#,
    );
    fixture.write_live(r#"{ "security": { "sandbox": false } }"#);

    fixture.apply().expect("apply");

    let live = Live::from_path(&fixture.arguments.live_path).expect("read live");
    assert_eq!(
        live.data().pointer("/security/sandbox"),
        Some(&json!(true)),
        "always-mode overrides user value",
    );
}

#[test]
fn apply_updates_toml_live_file() {
    let fixture = Fixture::new_with_live_name("config.toml");
    fixture.write_declared(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/build/jobs": "always" } },
            "build": { "jobs": 2 }
        }"#,
    );
    fixture.write_live(
        r#"
        [build]
        jobs = 8

        [term]
        verbose = true
        "#,
    );

    fixture.apply().expect("apply");

    let live = Live::from_path(&fixture.arguments.live_path).expect("read live");
    assert_eq!(live.data().pointer("/build/jobs"), Some(&json!(2)));
    assert_eq!(live.data().pointer("/term/verbose"), Some(&json!(true)));

    let live_text = fs::read_to_string(&fixture.arguments.live_path).expect("read text");
    assert!(live_text.contains("[build]"), "TOML table survives");
    assert!(
        live_text.contains("jobs = 2"),
        "always-mode rewrites Cargo jobs cap"
    );
}
