//! User drift — the diff between snapshot (what hexis wrote) and live
//! (what the user has now).
//!
//! Represented as RFC 7396 JSON Merge Patch. Reads as a partial
//! config; symmetric with snapshot reproduction; trivially diff-able
//! across activations to track *evolution* of user drift over time.
//!
//! Apply uses `json-patch`'s RFC 7396 implementation. Diff is
//! hand-rolled — `json-patch` ships an RFC 6902 differ but no Merge
//! Patch differ. The recursion follows the RFC 7396 rules:
//! - Equal values → empty object (no change at this node).
//! - Both objects → recursive object diff; keys present in `before`
//!   but absent in `after` map to `null` (delete).
//! - Otherwise → wholesale replacement with `after`.
//!
//! **Limitation** (intrinsic to RFC 7396): JSON `null` leaves cannot
//! be expressed in the patch — null always means *delete*. Hexis
//! config trees do not use null leaves, so this does not bite.

use serde_json::{Map, Value};

pub struct DriftPatch {
    patch: Value,
}

impl DriftPatch {
    /// An empty patch (object with no keys). Applying this to a JSON
    /// object is a no-op.
    pub fn empty() -> Self {
        Self {
            patch: Value::Object(Map::new()),
        }
    }

    /// Compute the merge-patch that, when applied to `before`, yields `after`.
    pub fn between(before: &Value, after: &Value) -> Self {
        Self {
            patch: Self::diff_value(before, after),
        }
    }

    /// Wrap an existing JSON value as a patch (e.g. one read from disk).
    pub fn from_value(patch: Value) -> Self {
        Self { patch }
    }

    /// Apply the patch to `target` in place, per RFC 7396.
    pub fn apply_to(&self, target: &mut Value) {
        json_patch::merge(target, &self.patch);
    }

    /// True iff the patch is the no-op object `{}`.
    pub fn is_empty(&self) -> bool {
        matches!(&self.patch, Value::Object(map) if map.is_empty())
    }

    pub fn as_value(&self) -> &Value {
        &self.patch
    }

    pub fn into_value(self) -> Value {
        self.patch
    }

    fn diff_value(before: &Value, after: &Value) -> Value {
        if before == after {
            return Value::Object(Map::new());
        }
        match (before, after) {
            (Value::Object(before_obj), Value::Object(after_obj)) => {
                let mut patch = Map::new();
                for (key, after_value) in after_obj {
                    match before_obj.get(key) {
                        Some(before_value) if before_value == after_value => {}
                        Some(before_value) => {
                            patch.insert(key.clone(), Self::diff_value(before_value, after_value));
                        }
                        None => {
                            patch.insert(key.clone(), after_value.clone());
                        }
                    }
                }
                for key in before_obj.keys() {
                    if !after_obj.contains_key(key) {
                        patch.insert(key.clone(), Value::Null);
                    }
                }
                Value::Object(patch)
            }
            _ => after.clone(),
        }
    }
}
