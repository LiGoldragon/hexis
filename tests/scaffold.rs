//! Smoke test — confirms the crate links and the public surface
//! resolves. Real per-module integration tests land alongside their
//! subsystems.

use hexis_cli::{Mode, Error};

#[test]
fn modes_render_as_lowercase_words() {
    assert_eq!(Mode::Once.to_string(), "once");
    assert_eq!(Mode::Ensure.to_string(), "ensure");
    assert_eq!(Mode::Always.to_string(), "always");
}

#[test]
fn default_mode_is_ensure() {
    assert_eq!(Mode::DEFAULT, Mode::Ensure);
}

#[test]
fn not_yet_implemented_renders() {
    let error = Error::NotYetImplemented("apply");
    assert!(error.to_string().contains("apply"));
    assert!(error.to_string().contains("scaffold"));
}
