//! The live file — what the application or the user has the file as
//! right now. Owned by the application; hexis reads, plans, then
//! atomically rewrites under `flock(LOCK_EX)`.
//!
//! v0.1 covers JSON only. v2 adds TOML; v3 adds YAML.
//!
//! The flock half of the read-merge-write contract lives in the
//! reconciler — Live exposes only the read/write primitives.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::NamedTempFile;

use crate::error::Error;

pub struct Live {
    data: Value,
    source_path: PathBuf,
}

impl Live {
    /// Read and parse the live file from disk.
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path)?;
        Self::from_text(&text, path.to_path_buf())
    }

    /// Read from disk; if the file does not exist, return an empty live.
    /// Used for first-run adoption when the application has not yet
    /// created its config file.
    pub fn from_path_or_empty(path: &Path) -> Result<Self, Error> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_text(&text, path.to_path_buf()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(Self::empty(path.to_path_buf()))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// An empty live (root = empty object) carrying the given source path.
    pub fn empty(source_path: PathBuf) -> Self {
        Self {
            data: Value::Object(serde_json::Map::new()),
            source_path,
        }
    }

    /// Parse from a JSON string with a known source path.
    fn from_text(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        let data: Value = serde_json::from_str(text).map_err(|error| Error::LiveParse {
            source_path: source_path.clone(),
            reason: error.to_string(),
        })?;
        Ok(Self { data, source_path })
    }

    pub fn data(&self) -> &Value {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut Value {
        &mut self.data
    }

    /// Replace the data tree wholesale. Used by the reconciler when
    /// committing the result of a plan.
    pub fn set_data(&mut self, data: Value) {
        self.data = data;
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Atomically write the live file to disk: tempfile + persist on the
    /// same filesystem. The tempfile-rename is atomic on POSIX.
    pub fn write_atomic(&self, path: &Path) -> Result<(), Error> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| Error::LiveWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create parent dir: {error}"),
        })?;
        let mut tempfile = NamedTempFile::new_in(parent).map_err(|error| Error::LiveWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create tempfile: {error}"),
        })?;
        serde_json::to_writer_pretty(&mut tempfile, &self.data).map_err(|error| {
            Error::LiveWrite {
                destination_path: path.to_path_buf(),
                reason: format!("serialize: {error}"),
            }
        })?;
        writeln!(tempfile).map_err(|error| Error::LiveWrite {
            destination_path: path.to_path_buf(),
            reason: format!("write trailing newline: {error}"),
        })?;
        tempfile.persist(path).map_err(|error| Error::LiveWrite {
            destination_path: path.to_path_buf(),
            reason: format!("rename: {error}"),
        })?;
        Ok(())
    }

    /// Construct directly from a JSON string. Used in tests.
    #[doc(hidden)]
    pub fn from_text_for_test(text: &str) -> Result<Self, Error> {
        Self::from_text(text, PathBuf::from("<test>"))
    }
}
