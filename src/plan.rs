//! The plan — a list of per-pointer actions derived from declared,
//! snapshot, and live. Computed in step 2 of the reconciliation flow,
//! consumed in step 3.
//!
//! Each action is one of: `WriteOnce`, `Ensure { user_drift }`,
//! `Always`, or `LeaveAlone`.
//!
//! v0.1: placeholder.
