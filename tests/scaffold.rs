//! Smoke tests — confirm the crate links, types render, and the actor
//! topology spawns and tears down cleanly. Real per-module integration
//! tests land alongside their subsystems.

use hexis_cli::{Error, Mode, supervisor};

#[test]
fn modes_render_as_lowercase_words() {
    assert_eq!(Mode::Once.to_string(), "once");
    assert_eq!(Mode::Ensure.to_string(), "ensure");
    assert_eq!(Mode::Always.to_string(), "always");
}

#[test]
fn default_mode_is_ensure() {
    assert_eq!(Mode::default(), Mode::Ensure);
}

#[test]
fn not_yet_implemented_renders() {
    let error = Error::NotYetImplemented("apply");
    let rendered = error.to_string();
    assert!(rendered.contains("apply"));
    assert!(rendered.contains("scaffold"));
}

#[tokio::test]
async fn supervisor_starts_and_shuts_down_cleanly_with_no_targets() {
    let supervisor = supervisor::SupervisorHandle::start(supervisor::Arguments {
        reconciler_targets: vec![],
    })
    .await
    .expect("supervisor should spawn with empty targets");

    supervisor
        .shutdown()
        .await
        .expect("clean shutdown via SupervisorHandle::shutdown");
}
