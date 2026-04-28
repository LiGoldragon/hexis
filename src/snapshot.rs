//! The snapshot — what hexis last wrote, kept on disk as state.
//!
//! Holds the union of: per-pointer applied-markers (for `once` keys)
//! and the post-apply image of the live file (for `ensure` and
//! `always` regions). See `ARCHITECTURE.md` § "Snapshot evolution
//! under modes" for the load-bearing detail.
//!
//! On disk:
//!
//! ```jsonc
//! {
//!   "schema": 1,
//!   "applied_markers": {
//!     "/devtools/autoConnect": {
//!       "applied_at":         "2026-04-28T15:00:00Z",
//!       "value_when_applied": true
//!     }
//!   },
//!   "image": { ... post-apply image of ensure/always regions ... }
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::{Map, Value};
use tempfile::NamedTempFile;

use crate::error::Error;
use crate::types::JsonPointer;

const SCHEMA_VERSION: u64 = 1;

pub struct Snapshot {
    markers: HashMap<JsonPointer, Marker>,
    image: Value,
    source_path: PathBuf,
}

/// A "this `once` key has been applied" marker.
///
/// Records when we wrote the value and what value we wrote, so a
/// future apply can detect that the user has changed it (and we should
/// leave it alone) regardless of the current declared value.
pub struct Marker {
    applied_at: String,
    value_when_applied: Value,
}

impl Marker {
    pub fn new(applied_at: String, value_when_applied: Value) -> Self {
        Self {
            applied_at,
            value_when_applied,
        }
    }

    pub fn applied_at(&self) -> &str {
        &self.applied_at
    }

    pub fn value_when_applied(&self) -> &Value {
        &self.value_when_applied
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "applied_at".to_string(),
            Value::String(self.applied_at.clone()),
        );
        object.insert(
            "value_when_applied".to_string(),
            self.value_when_applied.clone(),
        );
        Value::Object(object)
    }

    fn from_json(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("marker must be an object")?;
        let applied_at = object
            .get("applied_at")
            .and_then(|v| v.as_str())
            .ok_or("marker.applied_at must be a string")?
            .to_string();
        let value_when_applied = object
            .get("value_when_applied")
            .ok_or("marker.value_when_applied required")?
            .clone();
        Ok(Self::new(applied_at, value_when_applied))
    }
}

impl Snapshot {
    /// Read and parse a snapshot file from disk.
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path)?;
        Self::from_text(&text, path.to_path_buf())
    }

    /// Read from disk; if the file does not exist, return an empty
    /// snapshot. Used for first-run adoption.
    pub fn from_path_or_empty(path: &Path) -> Result<Self, Error> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text, path.to_path_buf()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(Self::empty(path.to_path_buf()))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// An empty snapshot (no markers, null image).
    pub fn empty(source_path: PathBuf) -> Self {
        Self {
            markers: HashMap::new(),
            image: Value::Null,
            source_path,
        }
    }

    fn from_text(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        let value: Value = serde_json::from_str(text).map_err(|error| Error::SnapshotParse {
            source_path: source_path.clone(),
            reason: error.to_string(),
        })?;
        let object = value.as_object().ok_or_else(|| Error::SnapshotParse {
            source_path: source_path.clone(),
            reason: "root must be an object".to_string(),
        })?;

        let schema = object
            .get("schema")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::SnapshotParse {
                source_path: source_path.clone(),
                reason: "missing or non-integer schema field".to_string(),
            })?;
        if schema != SCHEMA_VERSION {
            return Err(Error::SnapshotParse {
                source_path,
                reason: format!("schema = {schema}, expected {SCHEMA_VERSION}"),
            });
        }

        let mut markers = HashMap::new();
        if let Some(markers_value) = object.get("applied_markers") {
            let markers_object =
                markers_value
                    .as_object()
                    .ok_or_else(|| Error::SnapshotParse {
                        source_path: source_path.clone(),
                        reason: "applied_markers must be an object".to_string(),
                    })?;
            for (pointer_text, marker_value) in markers_object {
                let pointer =
                    JsonPointer::from_str(pointer_text).map_err(|error| Error::SnapshotParse {
                        source_path: source_path.clone(),
                        reason: format!("applied_markers[{pointer_text:?}]: {error}"),
                    })?;
                let marker =
                    Marker::from_json(marker_value).map_err(|reason| Error::SnapshotParse {
                        source_path: source_path.clone(),
                        reason: format!("applied_markers[{pointer_text:?}]: {reason}"),
                    })?;
                markers.insert(pointer, marker);
            }
        }

        let image = object.get("image").cloned().unwrap_or(Value::Null);

        Ok(Self {
            markers,
            image,
            source_path,
        })
    }

    pub fn marker(&self, pointer: &JsonPointer) -> Option<&Marker> {
        self.markers.get(pointer)
    }

    pub fn set_marker(&mut self, pointer: JsonPointer, marker: Marker) {
        self.markers.insert(pointer, marker);
    }

    /// Drop all markers under (or at) the given pointer. Used when a
    /// declared overlay flips a pointer's mode away from `once`,
    /// invalidating the previous "applied" marker.
    pub fn clear_subtree(&mut self, root: &JsonPointer) {
        self.markers
            .retain(|pointer, _| !pointer.is_descendant_of(root));
    }

    pub fn image(&self) -> &Value {
        &self.image
    }

    pub fn set_image(&mut self, image: Value) {
        self.image = image;
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    fn to_value(&self) -> Value {
        let mut markers_obj = Map::new();
        for (pointer, marker) in &self.markers {
            markers_obj.insert(pointer.as_str().to_string(), marker.to_json());
        }
        let mut root = Map::new();
        root.insert("schema".to_string(), Value::from(SCHEMA_VERSION));
        root.insert("applied_markers".to_string(), Value::Object(markers_obj));
        root.insert("image".to_string(), self.image.clone());
        Value::Object(root)
    }

    /// Atomically write the snapshot file to disk.
    pub fn write_atomic(&self, path: &Path) -> Result<(), Error> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| Error::SnapshotWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create parent dir: {error}"),
        })?;
        let mut tempfile = NamedTempFile::new_in(parent).map_err(|error| Error::SnapshotWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create tempfile: {error}"),
        })?;
        let value = self.to_value();
        serde_json::to_writer_pretty(&mut tempfile, &value).map_err(|error| {
            Error::SnapshotWrite {
                destination_path: path.to_path_buf(),
                reason: format!("serialize: {error}"),
            }
        })?;
        writeln!(tempfile).map_err(|error| Error::SnapshotWrite {
            destination_path: path.to_path_buf(),
            reason: format!("write trailing newline: {error}"),
        })?;
        tempfile.persist(path).map_err(|error| Error::SnapshotWrite {
            destination_path: path.to_path_buf(),
            reason: format!("rename: {error}"),
        })?;
        Ok(())
    }

    #[doc(hidden)]
    pub fn from_text_for_test(text: &str) -> Result<Self, Error> {
        Self::from_text(text, PathBuf::from("<test>"))
    }
}
