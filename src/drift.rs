//! User drift — the diff between snapshot (what hexis wrote) and live
//! (what the user has now).
//!
//! Represented as RFC 7396 JSON Merge Patch. Reads as a partial
//! config; symmetric with snapshot reproduction; trivially diff-able
//! across activations.
//!
//! v0.1: placeholder.
