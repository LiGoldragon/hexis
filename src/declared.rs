//! The declared overlay — what the consuming Nix module wants installed.
//!
//! On disk it's a JSON document. The optional `$hexis` envelope describes
//! per-key modes:
//!
//! ```jsonc
//! {
//!   "$hexis": {
//!     "schema": 1,
//!     "modes": {
//!       "/devtools/autoConnect": "once",
//!       "/security/sandbox":     "always"
//!     }
//!   },
//!   "editor":   { "tabSize": 4 },
//!   "devtools": { "autoConnect": true },
//!   "security": { "sandbox": true }
//! }
//! ```
//!
//! After parsing, the envelope is stripped from the data tree and the
//! mode map is exposed via [`Declared::mode_at`], which performs a
//! nearest-ancestor lookup keyed by [`JsonPointer`].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::Value;

use crate::error::Error;
use crate::types::{JsonPointer, Mode};

const ENVELOPE_KEY: &str = "$hexis";
const SCHEMA_VERSION: u64 = 1;

/// A parsed declared overlay: the data tree plus the per-pointer mode map.
pub struct Declared {
    data: Value,
    modes: HashMap<JsonPointer, Mode>,
    source_path: PathBuf,
}

impl Declared {
    /// Read and parse a declared-overlay JSON file from disk.
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path)?;
        Self::from_text(&text, path.to_path_buf())
    }

    /// Parse declared text with a known source path (used in error messages).
    fn from_text(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        let mut data: Value = serde_json::from_str(text).map_err(|error| Error::DeclaredParse {
            source_path: source_path.clone(),
            reason: error.to_string(),
        })?;

        let object = data.as_object_mut().ok_or_else(|| Error::DeclaredParse {
            source_path: source_path.clone(),
            reason: "root must be an object".to_string(),
        })?;

        let modes = Self::extract_envelope(object, &source_path)?;

        Ok(Self {
            data,
            modes,
            source_path,
        })
    }

    /// The data tree with the `$hexis` envelope stripped.
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// The path the overlay was read from.
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// The effective mode at the given pointer.
    ///
    /// Walks the ancestor chain; if no ancestor has an explicit mode,
    /// returns [`Mode::default`] (`Ensure`).
    pub fn mode_at(&self, pointer: &JsonPointer) -> Mode {
        for ancestor in pointer.ancestors() {
            if let Some(mode) = self.modes.get(&ancestor) {
                return *mode;
            }
        }
        Mode::default()
    }

    /// Strip the `$hexis` envelope (if present) from the root object,
    /// validating its shape and extracting the mode map.
    fn extract_envelope(
        object: &mut serde_json::Map<String, Value>,
        source_path: &Path,
    ) -> Result<HashMap<JsonPointer, Mode>, Error> {
        let envelope = match object.shift_remove(ENVELOPE_KEY) {
            Some(envelope) => envelope,
            None => return Ok(HashMap::new()),
        };

        let envelope_obj = envelope.as_object().ok_or_else(|| Error::DeclaredParse {
            source_path: source_path.to_path_buf(),
            reason: format!("{ENVELOPE_KEY} must be an object"),
        })?;

        if let Some(schema) = envelope_obj.get("schema") {
            let version = schema.as_u64().ok_or_else(|| Error::DeclaredParse {
                source_path: source_path.to_path_buf(),
                reason: format!("{ENVELOPE_KEY}.schema must be a non-negative integer"),
            })?;
            if version != SCHEMA_VERSION {
                return Err(Error::DeclaredParse {
                    source_path: source_path.to_path_buf(),
                    reason: format!(
                        "{ENVELOPE_KEY}.schema = {version}, expected {SCHEMA_VERSION}"
                    ),
                });
            }
        }

        let modes_value = match envelope_obj.get("modes") {
            Some(value) => value,
            None => return Ok(HashMap::new()),
        };

        let modes_object = modes_value.as_object().ok_or_else(|| Error::DeclaredParse {
            source_path: source_path.to_path_buf(),
            reason: format!("{ENVELOPE_KEY}.modes must be an object"),
        })?;

        let mut modes = HashMap::new();
        for (pointer_text, mode_value) in modes_object {
            let pointer = JsonPointer::from_str(pointer_text).map_err(|error| {
                Error::DeclaredParse {
                    source_path: source_path.to_path_buf(),
                    reason: format!(
                        "{ENVELOPE_KEY}.modes[{pointer_text:?}]: {error}"
                    ),
                }
            })?;
            let mode_text = mode_value.as_str().ok_or_else(|| Error::DeclaredParse {
                source_path: source_path.to_path_buf(),
                reason: format!("{ENVELOPE_KEY}.modes[{pointer_text:?}] must be a string"),
            })?;
            let mode = Mode::from_str(mode_text).map_err(|reason| Error::DeclaredParse {
                source_path: source_path.to_path_buf(),
                reason: format!("{ENVELOPE_KEY}.modes[{pointer_text:?}]: {reason}"),
            })?;
            modes.insert(pointer, mode);
        }

        Ok(modes)
    }
}

impl Declared {
    /// Construct directly from a JSON string. Used in tests; production
    /// code goes through [`from_path`](Self::from_path).
    #[doc(hidden)]
    pub fn from_text_for_test(text: &str) -> Result<Self, Error> {
        Self::from_text(text, PathBuf::from("<test>"))
    }
}
