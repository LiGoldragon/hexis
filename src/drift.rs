//! User drift — the diff between snapshot (what hexis wrote) and live
//! (what the user has now), and the on-disk journal that accumulates
//! drift across applies.
//!
//! [`DriftPatch`] represents a single diff in RFC 7396 JSON Merge Patch
//! form. Reads as a partial config, symmetric with snapshot
//! reproduction, trivially diff-able across activations.
//!
//! [`DriftJournal`] is the on-disk record at
//! `<drift_dir>/<file_id>.json` — an append-only list of (applied_at,
//! patch) entries with rotation to keep the last [`DriftJournal::MAX_ENTRIES`].
//! v2's proposal-loop reads from this journal to detect high-frequency
//! overrides.
//!
//! Apply uses `json-patch`'s RFC 7396 implementation. Diff is
//! hand-rolled — `json-patch` ships an RFC 6902 differ but no Merge
//! Patch differ. The recursion follows the RFC 7396 rules:
//! - Equal values → empty object (no change at this node).
//! - Both objects → recursive object diff; keys present in `before`
//!   but absent in `after` map to `null` (delete).
//! - Otherwise → wholesale replacement with `after`.
//!
//! **Limitation** (intrinsic to RFC 7396): JSON `null` leaves cannot
//! be expressed in the patch — null always means *delete*. Hexis
//! config trees do not use null leaves, so this does not bite.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use tempfile::NamedTempFile;

use crate::error::Error;

const SCHEMA_VERSION: u64 = 1;

pub struct DriftPatch {
    patch: Value,
}

impl DriftPatch {
    /// An empty patch (object with no keys). Applying this to a JSON
    /// object is a no-op.
    pub fn empty() -> Self {
        Self {
            patch: Value::Object(Map::new()),
        }
    }

    /// Compute the merge-patch that, when applied to `before`, yields `after`.
    pub fn between(before: &Value, after: &Value) -> Self {
        Self {
            patch: Self::diff_value(before, after),
        }
    }

    /// Wrap an existing JSON value as a patch (e.g. one read from disk).
    pub fn from_value(patch: Value) -> Self {
        Self { patch }
    }

    /// Apply the patch to `target` in place, per RFC 7396.
    pub fn apply_to(&self, target: &mut Value) {
        json_patch::merge(target, &self.patch);
    }

    /// True iff the patch is the no-op object `{}`.
    pub fn is_empty(&self) -> bool {
        matches!(&self.patch, Value::Object(map) if map.is_empty())
    }

    pub fn as_value(&self) -> &Value {
        &self.patch
    }

    pub fn into_value(self) -> Value {
        self.patch
    }

    fn diff_value(before: &Value, after: &Value) -> Value {
        if before == after {
            return Value::Object(Map::new());
        }
        match (before, after) {
            (Value::Object(before_obj), Value::Object(after_obj)) => {
                let mut patch = Map::new();
                for (key, after_value) in after_obj {
                    match before_obj.get(key) {
                        Some(before_value) if before_value == after_value => {}
                        Some(before_value) => {
                            patch.insert(key.clone(), Self::diff_value(before_value, after_value));
                        }
                        None => {
                            patch.insert(key.clone(), after_value.clone());
                        }
                    }
                }
                for key in before_obj.keys() {
                    if !after_obj.contains_key(key) {
                        patch.insert(key.clone(), Value::Null);
                    }
                }
                Value::Object(patch)
            }
            _ => after.clone(),
        }
    }
}

/// One entry in the drift journal — a timestamp paired with the
/// drift patch observed at that apply.
pub struct DriftEntry {
    applied_at: String,
    drift: DriftPatch,
}

impl DriftEntry {
    pub fn new(applied_at: String, drift: DriftPatch) -> Self {
        Self { applied_at, drift }
    }

    pub fn applied_at(&self) -> &str {
        &self.applied_at
    }

    pub fn drift(&self) -> &DriftPatch {
        &self.drift
    }

    fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "applied_at".to_string(),
            Value::String(self.applied_at.clone()),
        );
        object.insert("drift".to_string(), self.drift.as_value().clone());
        Value::Object(object)
    }

    fn from_value(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("entry must be an object")?;
        let applied_at = object
            .get("applied_at")
            .and_then(|v| v.as_str())
            .ok_or("entry.applied_at required")?
            .to_string();
        let drift_value = object
            .get("drift")
            .ok_or("entry.drift required")?
            .clone();
        Ok(Self::new(applied_at, DriftPatch::from_value(drift_value)))
    }
}

/// Per-target append-only record of (applied_at, drift_patch) entries.
/// On disk at `<drift_dir>/<file_id>.json`. Rotated to keep the last
/// [`Self::MAX_ENTRIES`].
pub struct DriftJournal {
    entries: Vec<DriftEntry>,
}

impl DriftJournal {
    /// Maximum number of entries retained per managed file. Older
    /// entries roll off when the journal grows past this count.
    pub const MAX_ENTRIES: usize = 30;

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Read the journal from disk; if missing or empty, return an empty
    /// journal. Migrates legacy single-entry drift files (the v0.1
    /// "latest-only" format that lacked an `entries` array).
    pub fn from_path_or_empty(path: &Path) -> Result<Self, Error> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::empty()),
            Err(error) => return Err(error.into()),
        };
        if text.trim().is_empty() {
            return Ok(Self::empty());
        }
        Self::from_text(&text, path.to_path_buf())
    }

    fn from_text(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        let value: Value = serde_json::from_str(text).map_err(|error| Error::DriftParse {
            source_path: source_path.clone(),
            reason: error.to_string(),
        })?;
        let object = value.as_object().ok_or_else(|| Error::DriftParse {
            source_path: source_path.clone(),
            reason: "root must be an object".to_string(),
        })?;

        // Current schema: { schema: 1, entries: [...] }.
        if let Some(entries_value) = object.get("entries") {
            let entries_array = entries_value.as_array().ok_or_else(|| Error::DriftParse {
                source_path: source_path.clone(),
                reason: "entries must be an array".to_string(),
            })?;
            let entries = entries_array
                .iter()
                .map(|entry| {
                    DriftEntry::from_value(entry).map_err(|reason| Error::DriftParse {
                        source_path: source_path.clone(),
                        reason,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Self { entries });
        }

        // Legacy v0.1 format: single { applied_at, drift } object.
        // Migrate it into a single-entry journal so subsequent appends
        // rotate cleanly.
        if object.contains_key("applied_at") {
            let entry = DriftEntry::from_value(&value).map_err(|reason| Error::DriftParse {
                source_path: source_path.clone(),
                reason: format!("legacy single-entry format: {reason}"),
            })?;
            return Ok(Self {
                entries: vec![entry],
            });
        }

        Err(Error::DriftParse {
            source_path,
            reason: "missing both `entries` array and legacy `applied_at` field".to_string(),
        })
    }

    pub fn entries(&self) -> &[DriftEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append an entry, rotating the oldest off if the journal exceeds
    /// [`Self::MAX_ENTRIES`].
    pub fn append(&mut self, entry: DriftEntry) {
        self.entries.push(entry);
        if self.entries.len() > Self::MAX_ENTRIES {
            let drop_count = self.entries.len() - Self::MAX_ENTRIES;
            self.entries.drain(0..drop_count);
        }
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), Error> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create parent dir: {error}"),
        })?;
        let mut tempfile = NamedTempFile::new_in(parent).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create tempfile: {error}"),
        })?;
        let value = self.to_value();
        serde_json::to_writer_pretty(&mut tempfile, &value).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("serialize: {error}"),
        })?;
        writeln!(tempfile).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("write trailing newline: {error}"),
        })?;
        tempfile.persist(path).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("rename: {error}"),
        })?;
        Ok(())
    }

    fn to_value(&self) -> Value {
        let mut entries_array = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            entries_array.push(entry.to_value());
        }
        let mut root = Map::new();
        root.insert("schema".to_string(), Value::from(SCHEMA_VERSION));
        root.insert("entries".to_string(), Value::Array(entries_array));
        Value::Object(root)
    }

    #[doc(hidden)]
    pub fn from_text_for_test(text: &str) -> Result<Self, Error> {
        Self::from_text(text, PathBuf::from("<test>"))
    }
}
