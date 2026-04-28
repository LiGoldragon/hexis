//! hexis — managed-mutable config reconciliation with per-key modes.
//!
//! See [`ARCHITECTURE.md`](https://github.com/LiGoldragon/hexis/blob/main/ARCHITECTURE.md)
//! for the v1 contract.

pub mod error;
pub mod types;

pub mod declared;
pub mod snapshot;
pub mod live;
pub mod drift;
pub mod plan;

pub mod reconciler;
pub mod supervisor;
pub mod proposer;
pub mod agent;
pub mod config;

pub use error::Error;
pub use types::{FileId, JsonPointer, Mode};
