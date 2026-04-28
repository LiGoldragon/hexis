//! Tests for `crate::plan` — the leaf-walk dispatch from a declared
//! overlay + snapshot to a list of per-pointer actions.

use std::str::FromStr;

use hexis_cli::declared::Declared;
use hexis_cli::plan::{Action, Plan};
use hexis_cli::snapshot::{Marker, Snapshot};
use hexis_cli::types::JsonPointer;
use serde_json::json;

fn empty_snapshot() -> Snapshot {
    Snapshot::empty(std::path::PathBuf::from("<test-snap>"))
}

#[test]
fn empty_declared_yields_empty_plan() {
    let declared = Declared::from_text_for_test(r#"{}"#).expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert!(plan.is_empty());
}

#[test]
fn default_mode_emits_ensure_at_each_leaf() {
    let declared = Declared::from_text_for_test(
        r#"{ "editor": { "tabSize": 4, "wordWrap": "on" } }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 2);
    assert!(plan.actions().iter().all(|a| matches!(a, Action::Ensure { .. })));
}

#[test]
fn once_mode_with_no_marker_emits_write_once() {
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/devtools/autoConnect": "once" } },
            "devtools": { "autoConnect": true }
        }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 1);
    match &plan.actions()[0] {
        Action::WriteOnce { pointer, value } => {
            assert_eq!(pointer.as_str(), "/devtools/autoConnect");
            assert_eq!(value, &json!(true));
        }
        other => panic!("expected WriteOnce, got {:?}", other.pointer()),
    }
}

#[test]
fn once_mode_with_marker_emits_leave_alone() {
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/devtools/autoConnect": "once" } },
            "devtools": { "autoConnect": true }
        }"#,
    )
    .expect("parses");
    let mut snapshot = empty_snapshot();
    let pointer = JsonPointer::from_str("/devtools/autoConnect").unwrap();
    snapshot.set_marker(
        pointer.clone(),
        Marker::new("2026-04-28T12:00:00Z".to_string(), json!(true)),
    );
    let plan = Plan::build(&declared, &snapshot);
    assert_eq!(plan.actions().len(), 1);
    assert!(matches!(&plan.actions()[0], Action::LeaveAlone { .. }));
    assert_eq!(plan.actions()[0].pointer().as_str(), "/devtools/autoConnect");
}

#[test]
fn always_mode_emits_always_action() {
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/security/sandbox": "always" } },
            "security": { "sandbox": true }
        }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 1);
    assert!(matches!(&plan.actions()[0], Action::Always { .. }));
}

#[test]
fn mode_inherits_to_nested_leaves() {
    // mode_map["/security"] = Always — every leaf under /security gets Always.
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/security": "always" } },
            "security": { "sandbox": true, "policy": { "strict": true } }
        }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 2);
    assert!(plan.actions().iter().all(|a| matches!(a, Action::Always { .. })));
}

#[test]
fn explicit_modes_override_inherited_default_at_their_pointer() {
    // /a defaults to Ensure (no entry in mode_map). /a/b explicitly Once.
    // /a/c falls back to default Ensure.
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/a/b": "once" } },
            "a": { "b": true, "c": "drift-survives-here" }
        }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 2);

    let by_pointer: std::collections::HashMap<_, _> = plan
        .actions()
        .iter()
        .map(|action| (action.pointer().as_str().to_string(), action))
        .collect();

    assert!(matches!(
        by_pointer.get("/a/b").unwrap(),
        Action::WriteOnce { .. }
    ));
    assert!(matches!(
        by_pointer.get("/a/c").unwrap(),
        Action::Ensure { .. }
    ));
}

#[test]
fn declared_without_value_at_once_pointer_emits_no_action() {
    // Mode says /missing is once-mode, but declared has no value at /missing.
    // The leaf-walk only visits leaves of declared; it won't visit /missing.
    // Result: no action emitted for that pointer.
    let declared = Declared::from_text_for_test(
        r#"{
            "$hexis": { "schema": 1, "modes": { "/missing": "once" } },
            "other": 1
        }"#,
    )
    .expect("parses");
    let plan = Plan::build(&declared, &empty_snapshot());
    assert_eq!(plan.actions().len(), 1);
    assert_eq!(plan.actions()[0].pointer().as_str(), "/other");
}

