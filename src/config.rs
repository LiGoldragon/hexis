//! Hexis's own configuration — TOML at `/etc/hexis/config.toml` (or
//! `$XDG_CONFIG_HOME/hexis/config.toml`).
//!
//! Inside CriomOS, this file is rendered from the
//! `NodeProposal.hexis` substructure in horizon-rs. Outside CriomOS
//! it is written directly by the user (or by a home-manager option
//! shipping with this crate).
//!
//! See `ARCHITECTURE-DEFERRED.md` § "Horizon exposure" for the
//! NodeProposal shape.
//!
//! v0.1: placeholder. The proposal-loop knobs aren't consumed by v1.
