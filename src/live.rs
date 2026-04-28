//! The live file — what the application or the user has the file as
//! right now. Owned by the application; hexis reads, plans, then
//! atomically rewrites under `flock(LOCK_EX)`.
//!
//! v0.1: opaque stub. JSON load/store lands with the value layer.

pub struct Live;
