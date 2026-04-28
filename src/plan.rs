//! The plan — a list of per-pointer actions derived from declared and
//! snapshot. Computed in step 2 of the reconciliation flow, consumed
//! in step 3.
//!
//! v0.1 uses **leaf-walk dispatch**: walk the declared overlay's
//! leaves, look up the effective mode at each leaf via nearest-ancestor
//! lookup, emit one action per leaf. The Apply step folds the actions
//! into a fresh live image.
//!
//! ## Granularity simplification (v0.1)
//!
//! Strict subtree-replace semantics for `Always` and `Once` ("drop any
//! user-added neighbouring keys under this pointer") are deferred. v0.1
//! treats every mode at the leaf level: declared wins where declared
//! has a value, regardless of mode; the mode controls *whether* (Once,
//! Ensure, Always) and *when* (Once-with-marker → LeaveAlone). Keys
//! the user added that declared doesn't mention always survive.
//!
//! For most consumers (editor settings, MCP server lists, security
//! defaults at named keys) this is the desired behaviour. v2 may
//! introduce a `subtree-replace` flag for the rare case where the
//! caller really does want declared to own an entire subtree.

use serde_json::Value;

use crate::declared::Declared;
use crate::snapshot::Snapshot;
use crate::types::{JsonPointer, Mode};

/// One per-pointer decision produced by [`Plan::build`].
#[derive(Debug)]
pub enum Action {
    /// `Once` mode at this pointer with no existing snapshot marker:
    /// write the declared value, record an applied marker on commit.
    WriteOnce {
        pointer: JsonPointer,
        value: Value,
    },
    /// `Once` mode at this pointer with a snapshot marker already in
    /// place: do not touch the live value at this pointer; do not even
    /// diff it.
    LeaveAlone { pointer: JsonPointer },
    /// `Ensure` mode at this pointer (the default): declared wins where
    /// it speaks; user drift at sibling keys survives.
    Ensure {
        pointer: JsonPointer,
        value: Value,
    },
    /// `Always` mode at this pointer: declared is asserted on every
    /// apply; any user mutation here is overwritten.
    Always {
        pointer: JsonPointer,
        value: Value,
    },
}

impl Action {
    /// The pointer the action targets.
    pub fn pointer(&self) -> &JsonPointer {
        match self {
            Self::WriteOnce { pointer, .. }
            | Self::LeaveAlone { pointer }
            | Self::Ensure { pointer, .. }
            | Self::Always { pointer, .. } => pointer,
        }
    }
}

/// A list of [`Action`]s derived from a declared overlay against a
/// snapshot. Order is the depth-first walk order of the declared tree.
pub struct Plan {
    actions: Vec<Action>,
}

impl Plan {
    /// Build a plan by walking the declared overlay's leaves and
    /// dispatching by effective mode (nearest-ancestor lookup).
    pub fn build(declared: &Declared, snapshot: &Snapshot) -> Self {
        let mut actions = Vec::new();
        Self::walk(
            declared,
            snapshot,
            declared.data(),
            &JsonPointer::root(),
            &mut actions,
        );
        Self { actions }
    }

    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    fn walk(
        declared: &Declared,
        snapshot: &Snapshot,
        value: &Value,
        pointer: &JsonPointer,
        actions: &mut Vec<Action>,
    ) {
        if let Value::Object(map) = value {
            for (key, sub_value) in map {
                let sub_pointer = pointer.append(key);
                Self::walk(declared, snapshot, sub_value, &sub_pointer, actions);
            }
            return;
        }
        // Leaf: emit one action based on the effective mode.
        let mode = declared.mode_at(pointer);
        let action = match mode {
            Mode::Once => match snapshot.marker(pointer) {
                Some(_) => Action::LeaveAlone {
                    pointer: pointer.clone(),
                },
                None => Action::WriteOnce {
                    pointer: pointer.clone(),
                    value: value.clone(),
                },
            },
            Mode::Ensure => Action::Ensure {
                pointer: pointer.clone(),
                value: value.clone(),
            },
            Mode::Always => Action::Always {
                pointer: pointer.clone(),
                value: value.clone(),
            },
        };
        actions.push(action);
    }
}
