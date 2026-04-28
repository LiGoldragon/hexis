//! The snapshot — what hexis last wrote, kept on disk as state.
//!
//! Holds the union of: per-pointer applied-markers (for `once` keys)
//! and the post-apply image of the live file (for `ensure` and
//! `always` regions). See `ARCHITECTURE.md` § "Snapshot evolution
//! under modes" for the load-bearing detail.
//!
//! v0.1: opaque stub.

pub struct Snapshot;
