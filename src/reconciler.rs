//! Per-target reconciler actor.
//!
//! The work — the four-step `Read → Plan → Apply → Commit` chain — is
//! a method on [`State`] (which owns the [`Arguments`]). [`Reconciler`]
//! is a ZST that exists only as ractor's actor-identity tag; per
//! `style.md` § "No ZST method holders," it has *only* the Actor trait
//! impl, which delegates to `state.apply()`. The same `State::apply`
//! is what the v0.1 CLI calls directly without an actor harness.

use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use fs2::FileExt;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use serde_json::{Map, Value};

use crate::declared::Declared;
use crate::drift::{DriftEntry, DriftJournal, DriftPatch};
use crate::error::Error;
use crate::live::Live;
use crate::plan::{Action, Plan};
use crate::snapshot::{Marker, Snapshot};
use crate::types::FileId;

/// Actor-identity tag for ractor's `Actor::spawn_linked`. Carries no
/// runtime data — that lives on [`State`].
pub struct Reconciler;

pub struct State {
    arguments: Arguments,
    phase: Phase,
}

#[derive(Clone)]
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
    Failed(String),
}

impl Phase {
    fn report(&self) -> PhaseReport {
        match self {
            Self::Idle => PhaseReport::Idle,
            Self::Committed => PhaseReport::Committed,
            Self::Failed(message) => PhaseReport::Failed(message.clone()),
        }
    }
}

impl State {
    pub fn new(arguments: Arguments) -> Self {
        Self {
            arguments,
            phase: Phase::Idle,
        }
    }

    pub fn arguments(&self) -> &Arguments {
        &self.arguments
    }

    pub fn phase_report(&self) -> PhaseReport {
        self.phase.report()
    }

    /// Run the four-step apply for this target: `Read → Plan → Apply →
    /// Commit`. Synchronous — IO is std::fs (millisecond-scale).
    ///
    /// Holds an advisory exclusive flock on
    /// `<snapshot_dir>/<file_id>.lock` for the entire window. The same
    /// lock is what the v0.3 `wrapWithHexis` wrapper coordinates against
    /// for apps (Chrome, Firefox) that own their config at runtime.
    ///
    /// On success, atomically writes the new live, the new snapshot,
    /// and (if drift was observed) appends to the drift journal. With
    /// `dry_run = true`, runs steps 1–3 and skips the writes.
    ///
    /// `self.phase` is updated to `Committed` or `Failed(message)`
    /// regardless of outcome; the returned `Result` is the source of
    /// truth for the caller.
    pub fn apply(&mut self) -> Result<(), Error> {
        let result = self.apply_inner();
        self.phase = match &result {
            Ok(()) => Phase::Committed,
            Err(error) => Phase::Failed(error.to_string()),
        };
        result
    }

    fn apply_inner(&mut self) -> Result<(), Error> {
        let _lock_file = self.acquire_lock()?;

        let declared = Declared::from_path(&self.arguments.declared_path)?;
        let live = Live::from_path_or_empty(&self.arguments.live_path)?;
        let snapshot_path = self.snapshot_path();
        let mut snapshot = Snapshot::from_path_or_empty(&snapshot_path)?;

        let plan = Plan::build(&declared, &snapshot);

        let mut new_live_data = live.data().clone();
        if !new_live_data.is_object() {
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

        let drift = if snapshot.image().is_null() {
            DriftPatch::empty()
        } else {
            DriftPatch::between(snapshot.image(), live.data())
        };
        snapshot.set_image(new_live_data.clone());

        if self.arguments.dry_run {
            return Ok(());
        }

        let mut updated_live = live;
        updated_live.set_data(new_live_data);
        updated_live.write_atomic(&self.arguments.live_path)?;
        snapshot.write_atomic(&snapshot_path)?;

        if !drift.is_empty() {
            self.append_drift(drift, &now)?;
        }

        Ok(())
    }

    fn snapshot_path(&self) -> PathBuf {
        self.arguments
            .snapshot_dir
            .join(format!("{}.json", self.arguments.file_id))
    }

    fn drift_path(&self) -> PathBuf {
        self.arguments
            .drift_dir
            .join(format!("{}.json", self.arguments.file_id))
    }

    /// Append a drift entry to this target's journal at `<drift_dir>/<file_id>.json`,
    /// rotating older entries off when the cap is exceeded. Migrates
    /// legacy single-entry drift files transparently.
    fn append_drift(&self, drift: DriftPatch, applied_at: &str) -> Result<(), Error> {
        let path = self.drift_path();
        let mut journal = DriftJournal::from_path_or_empty(&path)?;
        journal.append(DriftEntry::new(applied_at.to_string(), drift));
        journal.write_atomic(&path)
    }

    /// Acquire an advisory exclusive POSIX flock on the per-target lock
    /// file. The returned `File` handle holds the lock; closing it (via
    /// drop at end of `apply`'s scope) releases it.
    fn acquire_lock(&self) -> Result<std::fs::File, Error> {
        fs::create_dir_all(&self.arguments.snapshot_dir).map_err(|error| Error::Lock {
            path: self.arguments.snapshot_dir.clone(),
            reason: format!("create snapshot dir: {error}"),
        })?;
        let lock_path = self
            .arguments
            .snapshot_dir
            .join(format!("{}.lock", self.arguments.file_id));
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
        Ok(State::new(arguments))
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Message,
        state: &mut State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        match message {
            Message::Run => {
                // Apply errors are recorded in state.phase; the actor
                // itself stays alive (per-attempt failures are
                // recoverable, not actor crashes).
                let _ = state.apply();
            }
            Message::GetPhase { reply_port } => {
                let _ = reply_port.send(state.phase_report());
            }
        }
        Ok(())
    }
}

