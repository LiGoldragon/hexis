//! Tests for `crate::declared` — `$hexis` envelope extraction and
//! nearest-ancestor mode lookup.

use std::str::FromStr;

use hexis_cli::Error;
use hexis_cli::declared::Declared;
use hexis_cli::types::{JsonPointer, Mode};

#[test]
fn parse_strips_hexis_envelope_from_data() {
    let text = r#"{
        "$hexis": { "schema": 1, "modes": {} },
        "editor": { "tabSize": 4 }
    }"#;
    let declared = Declared::from_text_for_test(text).expect("parses");
    let object = declared.data().as_object().expect("root is object");
    assert!(!object.contains_key("$hexis"));
    assert!(object.contains_key("editor"));
}

#[test]
fn parse_with_no_envelope_succeeds_with_default_modes() {
    let text = r#"{ "editor": { "tabSize": 4 } }"#;
    let declared = Declared::from_text_for_test(text).expect("parses");
    let pointer = JsonPointer::from_str("/editor/tabSize").unwrap();
    assert_eq!(declared.mode_at(&pointer), Mode::Ensure);
}

#[test]
fn mode_at_returns_explicit_mode_for_listed_pointer() {
    let text = r#"{
        "$hexis": {
            "schema": 1,
            "modes": {
                "/devtools/autoConnect": "once",
                "/security/sandbox":     "always"
            }
        },
        "devtools": { "autoConnect": true },
        "security": { "sandbox": true }
    }"#;
    let declared = Declared::from_text_for_test(text).expect("parses");
    assert_eq!(
        declared.mode_at(&JsonPointer::from_str("/devtools/autoConnect").unwrap()),
        Mode::Once,
    );
    assert_eq!(
        declared.mode_at(&JsonPointer::from_str("/security/sandbox").unwrap()),
        Mode::Always,
    );
}

#[test]
fn mode_at_walks_to_nearest_ancestor() {
    let text = r#"{
        "$hexis": {
            "schema": 1,
            "modes": { "/security": "always" }
        },
        "security": { "sandbox": true, "policy": { "strict": true } }
    }"#;
    let declared = Declared::from_text_for_test(text).expect("parses");
    let deep = JsonPointer::from_str("/security/policy/strict").unwrap();
    assert_eq!(declared.mode_at(&deep), Mode::Always);
}

#[test]
fn mode_at_falls_back_to_ensure_when_no_ancestor_listed() {
    let text = r#"{
        "$hexis": {
            "schema": 1,
            "modes": { "/security": "always" }
        },
        "editor": { "tabSize": 4 }
    }"#;
    let declared = Declared::from_text_for_test(text).expect("parses");
    let pointer = JsonPointer::from_str("/editor/tabSize").unwrap();
    assert_eq!(declared.mode_at(&pointer), Mode::Ensure);
}

#[test]
fn parse_rejects_unknown_mode_name() {
    let text = r#"{
        "$hexis": { "schema": 1, "modes": { "/x": "weird" } }
    }"#;
    let result = Declared::from_text_for_test(text);
    assert!(matches!(result, Err(Error::DeclaredParse { .. })));
}

#[test]
fn parse_rejects_invalid_pointer() {
    let text = r#"{
        "$hexis": { "schema": 1, "modes": { "no-leading-slash": "once" } }
    }"#;
    let result = Declared::from_text_for_test(text);
    assert!(matches!(result, Err(Error::DeclaredParse { .. })));
}

#[test]
fn parse_rejects_unknown_schema_version() {
    let text = r#"{ "$hexis": { "schema": 99, "modes": {} } }"#;
    let result = Declared::from_text_for_test(text);
    assert!(matches!(result, Err(Error::DeclaredParse { .. })));
}

#[test]
fn parse_rejects_non_object_root() {
    let text = r#"[1, 2, 3]"#;
    let result = Declared::from_text_for_test(text);
    assert!(matches!(result, Err(Error::DeclaredParse { .. })));
}

#[test]
fn parse_rejects_malformed_json() {
    let text = r#"{ this is not json"#;
    let result = Declared::from_text_for_test(text);
    assert!(matches!(result, Err(Error::DeclaredParse { .. })));
}
