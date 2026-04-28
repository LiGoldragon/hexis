//! Root supervisor actor. Owns the hexis config, spawns one
//! `Reconciler` per managed file, spawns the (stub) `Proposer`, and
//! holds the `DriftJournal` handle.
//!
//! v0.1: placeholder.
