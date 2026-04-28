//! Crate error type.
//!
//! One enum per crate, derived with `thiserror`. Variants carry the
//! data needed to render a useful message.

use std::path::PathBuf;
use thiserror::Error;

use crate::types::JsonPointer;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0} is not yet implemented (v0.1 is scaffold-only)")]
    NotYetImplemented(&'static str),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("actor spawn failed: {0}")]
    ActorSpawn(String),

    #[error("actor call failed ({label}): {reason}")]
    ActorCall {
        label: &'static str,
        reason: String,
    },

    #[error("invalid JSON pointer {0:?}: must be empty or start with '/'")]
    InvalidJsonPointer(String),

    #[error("declared parse failed at {source_path:?}: {reason}")]
    DeclaredParse {
        source_path: PathBuf,
        reason: String,
    },

    #[error("live parse failed at {source_path:?}: {reason}")]
    LiveParse {
        source_path: PathBuf,
        reason: String,
    },

    #[error("live write failed at {destination_path:?}: {reason}")]
    LiveWrite {
        destination_path: PathBuf,
        reason: String,
    },

    #[error("snapshot parse failed at {source_path:?}: {reason}")]
    SnapshotParse {
        source_path: PathBuf,
        reason: String,
    },

    #[error("snapshot write failed at {destination_path:?}: {reason}")]
    SnapshotWrite {
        destination_path: PathBuf,
        reason: String,
    },

    #[error("drift write failed at {destination_path:?}: {reason}")]
    DriftWrite {
        destination_path: PathBuf,
        reason: String,
    },

    #[error("drift parse failed at {source_path:?}: {reason}")]
    DriftParse {
        source_path: PathBuf,
        reason: String,
    },

    #[error("cannot apply at pointer {pointer}: {reason}")]
    ApplyAtPointer {
        pointer: JsonPointer,
        reason: String,
    },

    #[error("could not acquire apply-window lock at {path:?}: {reason}")]
    Lock { path: PathBuf, reason: String },
}
