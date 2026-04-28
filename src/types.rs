//! Domain newtypes — the typed values hexis manipulates.
//!
//! Per the rust style guide: domain values are types, not primitives.
//! A file identifier is not a `String`; a JSON pointer is not a `String`;
//! a mode is not a `String`. Each gets a newtype.

use std::fmt;

/// Stable identifier for a managed file across runs.
///
/// Computed as `sha256(canonical(live_path))[..12]` rendered as lowercase
/// hex. Used as the directory key under `~/.local/state/hexis/` for the
/// snapshot and drift report.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FileId(String);

impl fmt::Display for FileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A JSON Pointer (RFC 6901) into a config document.
///
/// Wrapped to preserve the leading `/` and reject malformed inputs at
/// construction time when validation lands.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct JsonPointer(String);

impl fmt::Display for JsonPointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The lifecycle a key in the declared overlay follows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Assert at adoption, never re-touch.
    Once,
    /// Declared wins where it speaks; user drift survives where declared is silent.
    Ensure,
    /// Declared is asserted on every apply; user mutation is overwritten next pass.
    Always,
}

impl Mode {
    /// The mode applied to any key whose pointer has no enclosing entry in the mode map.
    pub const DEFAULT: Self = Self::Ensure;
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Once => "once",
            Self::Ensure => "ensure",
            Self::Always => "always",
        };
        formatter.write_str(name)
    }
}
