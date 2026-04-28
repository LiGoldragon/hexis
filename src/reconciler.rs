//! Per-target reconciler actor.
//!
//! Owns the four-step apply for one (declared_path, live_path) pair:
//! `Read → Plan → Apply → Commit`. v0.1 runs the four steps
//! synchronously inside a single `Run` handler; the actor's external
//! phase is `Idle | Committed | Failed(Error)`. Intermediate phase
//! tracking via self-cast (Loaded / Planned / Applied) is reserved for
//! v2 when watcher integration and parallel-friendly file IO make
//! observability across steps useful.
//!
//! `Reconciler::apply` is also callable synchronously (without an
//! actor harness) — the v0.1 CLI uses this path for the single-shot
//! `hexis apply` invocation. The actor wrapping comes into its own
//! once the supervised, multi-target, watcher-driven flow lands in v2.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use serde_json::{Map, Value};
use tempfile::NamedTempFile;

use crate::declared::Declared;
use crate::drift::DriftPatch;
use crate::error::Error;
use crate::live::Live;
use crate::plan::{Action, Plan};
use crate::snapshot::{Marker, Snapshot};
use crate::types::FileId;

pub struct Reconciler;

pub struct State {
    arguments: Arguments,
    phase: Phase,
}

pub struct Arguments {
    pub file_id: FileId,
    pub declared_path: PathBuf,
    pub live_path: PathBuf,
    pub snapshot_dir: PathBuf,
    pub drift_dir: PathBuf,
    pub dry_run: bool,
}

pub enum Message {
    Run,
    GetPhase {
        reply_port: RpcReplyPort<PhaseReport>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhaseReport {
    Idle,
    Committed,
    Failed(String),
}

enum Phase {
    Idle,
    Committed,
    Failed(Error),
}

impl Phase {
    fn report(&self) -> PhaseReport {
        match self {
            Self::Idle => PhaseReport::Idle,
            Self::Committed => PhaseReport::Committed,
            Self::Failed(error) => PhaseReport::Failed(error.to_string()),
        }
    }
}

impl Reconciler {
    /// Run the four-step apply for one target. Synchronous — IO is
    /// std::fs (millisecond-scale); no need for tokio's async file IO
    /// at v0.1's single-shot CLI scale.
    ///
    /// On success, writes (atomically): the new live file, the new
    /// snapshot, and (if drift was observed) the drift report. On
    /// `dry_run = true`, runs steps 1–3 and skips the writes.
    ///
    /// An advisory exclusive flock is held for the entire read-merge-
    /// write window, on a sibling lock file at
    /// `<snapshot_dir>/<file_id>.lock`. The same lock is what the v0.3
    /// `wrapWithHexis` Nix wrapper will acquire before launching apps
    /// (Chrome, Firefox) that own their config at runtime.
    pub fn apply(arguments: &Arguments) -> Result<(), Error> {
        // Acquire the apply-window lock first; held until function return
        // via the implicit drop of `_lock_file` (closing the fd releases
        // POSIX advisory locks).
        let _lock_file = Self::acquire_lock(arguments)?;

        let declared = Declared::from_path(&arguments.declared_path)?;
        let live = Live::from_path_or_empty(&arguments.live_path)?;
        let snapshot_path = arguments
            .snapshot_dir
            .join(format!("{}.json", arguments.file_id));
        let mut snapshot = Snapshot::from_path_or_empty(&snapshot_path)?;

        let plan = Plan::build(&declared, &snapshot);

        let mut new_live_data = live.data().clone();
        if !new_live_data.is_object() {
            // Ensure the live root is an object so the per-pointer set_in
            // can descend. An empty live or freshly-adopted live is
            // already an object; this guard catches the rare case of
            // pre-existing scalar/array roots.
            new_live_data = Value::Object(Map::new());
        }
        let now = chrono::Utc::now().to_rfc3339();

        for action in plan.actions() {
            match action {
                Action::WriteOnce { pointer, value } => {
                    pointer.set_in(&mut new_live_data, value.clone())?;
                    snapshot.set_marker(
                        pointer.clone(),
                        Marker::new(now.clone(), value.clone()),
                    );
                }
                Action::Ensure { pointer, value } | Action::Always { pointer, value } => {
                    pointer.set_in(&mut new_live_data, value.clone())?;
                }
                Action::LeaveAlone { .. } => {}
            }
        }

        // First-run snapshots have a Null image; any "diff" against null
        // would be the entire live wholesale, which isn't meaningful drift.
        // Skip drift on first run; subsequent runs compute against the
        // previous post-apply image.
        let drift = if snapshot.image().is_null() {
            DriftPatch::empty()
        } else {
            DriftPatch::between(snapshot.image(), live.data())
        };
        snapshot.set_image(new_live_data.clone());

        if arguments.dry_run {
            return Ok(());
        }

        let mut updated_live = live;
        updated_live.set_data(new_live_data);
        updated_live.write_atomic(&arguments.live_path)?;
        snapshot.write_atomic(&snapshot_path)?;

        if !drift.is_empty() {
            let drift_path = arguments
                .drift_dir
                .join(format!("{}.json", arguments.file_id));
            Self::write_drift(&drift_path, &drift, &now)?;
        }

        Ok(())
    }

    /// Acquire an advisory exclusive POSIX flock on the per-target
    /// lock file. Returns the open `File` handle; the caller holds it
    /// in scope and the lock releases automatically when it drops
    /// (POSIX flock is fd-bound).
    fn acquire_lock(arguments: &Arguments) -> Result<std::fs::File, Error> {
        fs::create_dir_all(&arguments.snapshot_dir).map_err(|error| Error::Lock {
            path: arguments.snapshot_dir.clone(),
            reason: format!("create snapshot dir: {error}"),
        })?;
        let lock_path = arguments
            .snapshot_dir
            .join(format!("{}.lock", arguments.file_id));
        let lock_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| Error::Lock {
                path: lock_path.clone(),
                reason: format!("open: {error}"),
            })?;
        FileExt::lock_exclusive(&lock_file).map_err(|error| Error::Lock {
            path: lock_path,
            reason: format!("acquire: {error}"),
        })?;
        Ok(lock_file)
    }

    /// Write the drift report atomically. v0.1 stores latest-only —
    /// the journal/rotation pattern lands in v2 once the proposer
    /// actor consumes drift across runs.
    fn write_drift(path: &Path, drift: &DriftPatch, applied_at: &str) -> Result<(), Error> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create parent dir: {error}"),
        })?;
        let mut tempfile = NamedTempFile::new_in(parent).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("create tempfile: {error}"),
        })?;
        let mut entry = Map::new();
        entry.insert("applied_at".to_string(), Value::String(applied_at.to_string()));
        entry.insert("drift".to_string(), drift.as_value().clone());
        let document = Value::Object(entry);
        serde_json::to_writer_pretty(&mut tempfile, &document).map_err(|error| {
            Error::DriftWrite {
                destination_path: path.to_path_buf(),
                reason: format!("serialize: {error}"),
            }
        })?;
        writeln!(tempfile).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("write trailing newline: {error}"),
        })?;
        tempfile.persist(path).map_err(|error| Error::DriftWrite {
            destination_path: path.to_path_buf(),
            reason: format!("rename: {error}"),
        })?;
        Ok(())
    }
}

#[ractor::async_trait]
impl Actor for Reconciler {
    type Msg = Message;
    type State = State;
    type Arguments = Arguments;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        arguments: Arguments,
    ) -> std::result::Result<Self::State, ActorProcessingErr> {
        Ok(State {
            arguments,
            phase: Phase::Idle,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Message,
        state: &mut State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        match message {
            Message::Run => {
                state.phase = match Self::apply(&state.arguments) {
                    Ok(()) => Phase::Committed,
                    Err(error) => Phase::Failed(error),
                };
            }
            Message::GetPhase { reply_port } => {
                let _ = reply_port.send(state.phase.report());
            }
        }
        Ok(())
    }
}
