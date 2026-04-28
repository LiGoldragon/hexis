//! Agent subprocess client — *absent in v0.1.*
//!
//! v2 contract: subprocess JSON-on-stdin / JSON-on-stdout. The
//! `ProposalRequest` struct is written to stdin, a `ProposalResponse`
//! is read from stdout. Hexis does not link against any LLM library;
//! the agent can be replaced (claude-code, codex, gh-cli script, no-op
//! for testing) without touching reconciler code.
//!
//! See `ARCHITECTURE-DEFERRED.md` § "Agent interface (black box)".
//!
//! v0.1: placeholder.
