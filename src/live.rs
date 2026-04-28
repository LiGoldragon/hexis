//! The live file — what the application or the user has the file as
//! right now. Owned by the application; hexis reads, plans, then
//! atomically rewrites under `flock(LOCK_EX)`.
//!
//! v0.1 covers JSON only. v2 adds TOML; v3 adds YAML.
//!
//! v0.1: placeholder.
