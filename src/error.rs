//! Crate error type.
//!
//! One enum per crate, derived with `thiserror`. Variants carry the
//! data needed to render a useful message. New variants are added as
//! subsystems land — v0.1 carries only the placeholders the CLI scaffold
//! exercises.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0} is not yet implemented (v0.1 is scaffold-only)")]
    NotYetImplemented(&'static str),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
