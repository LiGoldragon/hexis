//! The live file — what the application or the user has the file as
//! right now. Owned by the application; hexis reads, plans, then
//! atomically rewrites under `flock(LOCK_EX)`.
//!
//! v0.1 covers JSON and TOML live files. v3 adds YAML.
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
    format: LiveFormat,
    source_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveFormat {
    Json,
    Toml,
}

impl LiveFormat {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("toml") => Self::Toml,
            _ => Self::Json,
        }
    }

    fn parse(self, text: &str, source_path: &Path) -> Result<Value, Error> {
        match self {
            Self::Json => serde_json::from_str(text).map_err(|error| Error::LiveParse {
                source_path: source_path.to_path_buf(),
                reason: error.to_string(),
            }),
            Self::Toml => {
                let value: toml::Value =
                    toml::from_str(text).map_err(|error| Error::LiveParse {
                        source_path: source_path.to_path_buf(),
                        reason: error.to_string(),
                    })?;
                serde_json::to_value(value).map_err(|error| Error::LiveParse {
                    source_path: source_path.to_path_buf(),
                    reason: format!("convert TOML to JSON value tree: {error}"),
                })
            }
        }
    }

    fn write(self, writer: &mut impl Write, data: &Value, path: &Path) -> Result<(), Error> {
        match self {
            Self::Json => {
                serde_json::to_writer_pretty(writer, data).map_err(|error| Error::LiveWrite {
                    destination_path: path.to_path_buf(),
                    reason: format!("serialize JSON: {error}"),
                })
            }
            Self::Toml => {
                let rendered = toml::to_string_pretty(data).map_err(|error| Error::LiveWrite {
                    destination_path: path.to_path_buf(),
                    reason: format!("serialize TOML: {error}"),
                })?;
                writer
                    .write_all(rendered.as_bytes())
                    .map_err(|error| Error::LiveWrite {
                        destination_path: path.to_path_buf(),
                        reason: format!("write TOML: {error}"),
                    })
            }
        }
    }
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
            format: LiveFormat::from_path(&source_path),
            source_path,
        }
    }

    /// Parse from a format inferred from the known source path.
    fn from_text(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        let format = LiveFormat::from_path(&source_path);
        let data = format.parse(text, &source_path)?;
        Ok(Self {
            data,
            format,
            source_path,
        })
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
        let format = if path == self.source_path {
            self.format
        } else {
            LiveFormat::from_path(path)
        };
        format.write(&mut tempfile, &self.data, path)?;
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

    /// Construct directly from a text fixture and source path. Used in
    /// tests that need extension-based format selection.
    #[doc(hidden)]
    pub fn from_text_for_test_at_path(text: &str, source_path: PathBuf) -> Result<Self, Error> {
        Self::from_text(text, source_path)
    }
}
