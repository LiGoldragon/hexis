//! Per-target reconciler actor. One instance per `(declared_path,
//! live_path)` pair in the hexis config.
//!
//! State machine: `Idle → Loaded → Planned → Applied → Committed`,
//! with `Failed(Error)` as the absorbing failure state. The four
//! reconcile steps map to four message variants.
//!
//! v0.1: placeholder. Ractor wire-up lands when the core types are in.
