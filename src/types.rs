//! Domain newtypes — the typed values hexis manipulates.
//!
//! Per the rust style guide: domain values are types, not primitives.
//! A file identifier is not a `String`; a JSON pointer is not a `String`;
//! a mode is not a `String`. Each gets a newtype.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::FromStr;

use crate::error::Error;

/// Stable identifier for a managed file across runs.
///
/// Computed as `sha256(canonical(live_path))[..12]` rendered as lowercase
/// hex. v0.1 substitutes the std `DefaultHasher` and truncates to 12 hex
/// chars; sha256 lands with the IO layer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FileId(String);

impl FileId {
    /// Compute a stable id from a live-file path.
    pub fn from_path(path: &Path) -> Self {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let raw = hasher.finish();
        let hex = format!("{raw:016x}");
        Self(hex[..12].to_string())
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A JSON Pointer (RFC 6901) into a config document.
///
/// The empty string is the root pointer (refers to the entire document).
/// Non-empty pointers must begin with `/`. Segment escaping (`~0` for `~`,
/// `~1` for `/`) is handled by `serde_json::Value::pointer`; we accept
/// what serde_json accepts.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct JsonPointer(String);

impl JsonPointer {
    /// The root pointer — refers to the entire document.
    pub fn root() -> Self {
        Self(String::new())
    }

    /// The raw pointer text, in RFC 6901 wire form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True iff this is the root pointer.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// The parent pointer, or `None` if `self` is the root.
    pub fn parent(&self) -> Option<Self> {
        if self.0.is_empty() {
            return None;
        }
        let last_slash = self.0.rfind('/')?;
        Some(Self(self.0[..last_slash].to_string()))
    }

    /// Iterator over `self`, then `self.parent()`, then its parent, ..., down
    /// to the root inclusive. Used for nearest-ancestor mode lookups.
    pub fn ancestors(&self) -> Ancestors {
        Ancestors {
            current: Some(self.clone()),
        }
    }

    /// Resolve the pointer against a JSON value. Returns `None` if any
    /// segment along the path is missing or has the wrong shape.
    pub fn resolve<'value>(
        &self,
        value: &'value serde_json::Value,
    ) -> Option<&'value serde_json::Value> {
        if self.0.is_empty() {
            return Some(value);
        }
        value.pointer(&self.0)
    }

    /// True iff `self` lies at-or-below `root` in the pointer tree.
    /// The root pointer is an ancestor of everything.
    pub fn is_descendant_of(&self, root: &JsonPointer) -> bool {
        if root.is_root() {
            return true;
        }
        let descendant_str = self.as_str();
        let root_str = root.as_str();
        descendant_str == root_str
            || (descendant_str.starts_with(root_str)
                && descendant_str.as_bytes().get(root_str.len()) == Some(&b'/'))
    }

    /// Construct a child pointer by appending a single object key as a
    /// segment. Per RFC 6901, the segment is escaped: `~` → `~0`,
    /// `/` → `~1`. Order matters — escape `~` first so the second pass
    /// can't double-escape it.
    pub fn append(&self, segment: &str) -> Self {
        let escaped = segment.replace('~', "~0").replace('/', "~1");
        let mut text = self.0.clone();
        text.push('/');
        text.push_str(&escaped);
        Self(text)
    }
}

impl FromStr for JsonPointer {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Ok(Self::root());
        }
        if !input.starts_with('/') {
            return Err(Error::InvalidJsonPointer(input.to_string()));
        }
        Ok(Self(input.to_string()))
    }
}

impl fmt::Display for JsonPointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Iterator yielding a pointer and each of its ancestors up to and
/// including the root.
pub struct Ancestors {
    current: Option<JsonPointer>,
}

impl Iterator for Ancestors {
    type Item = JsonPointer;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.take()?;
        self.current = current.parent();
        Some(current)
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

impl Default for Mode {
    /// The mode applied to any key whose pointer has no enclosing entry in the mode map.
    fn default() -> Self {
        Self::Ensure
    }
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "once" => Ok(Self::Once),
            "ensure" => Ok(Self::Ensure),
            "always" => Ok(Self::Always),
            other => Err(format!("unknown mode {other:?}; expected once|ensure|always")),
        }
    }
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
