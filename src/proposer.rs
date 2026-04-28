//! Proposer actor — *stub in v0.1.*
//!
//! Subscribes to drift events from the Reconcilers and (in v2)
//! batches them, calls out to the agent subprocess, and opens a PR
//! against the consuming repo. v1 spawns the actor with a body that
//! drops every message on the floor; the supervision tree is real
//! even when the body is empty.
//!
//! See `ARCHITECTURE-DEFERRED.md` § "Auto-PR proposal loop" for the
//! v2 design.
//!
//! v0.1: placeholder.
