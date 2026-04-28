//! The declared overlay — what the consuming Nix module wants installed.
//!
//! Carries an optional `$hexis` envelope describing per-key modes. The
//! envelope is stripped before merge.
//!
//! v0.1: opaque stub; the actor message types reference this name. Real
//! parsing lands when the value layer is filled in.

pub struct Declared;
