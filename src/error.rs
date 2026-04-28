//! Crate error type.
//!
//! One enum per crate, derived with `thiserror`. Variants carry the
//! data needed to render a useful message.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0} is not yet implemented (v0.1 is scaffold-only)")]
    NotYetImplemented(&'static str),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("actor spawn failed: {0}")]
    ActorSpawn(String),

    #[error("actor call failed ({label}): {reason}")]
    ActorCall { label: &'static str, reason: String },
}
