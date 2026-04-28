//! Per-target reconciler actor.
//!
//! Owns the four-step apply for one (declared_path, live_path) pair.
//! Phase transitions: `Idle → Loaded → Planned → Applied → Committed`
//! with `Failed(Error)` as the absorbing failure state.
//!
//! The actor's mutable state holds an opaque `Phase`; observers see a
//! `PhaseReport` (a Clone-able projection) via `Message::GetPhase`.

use std::path::PathBuf;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::declared::Declared;
use crate::drift::DriftPatch;
use crate::error::Error;
use crate::live::Live;
use crate::plan::Plan;
use crate::snapshot::Snapshot;
use crate::types::FileId;

pub struct Reconciler;

pub struct State {
    #[allow(dead_code)]
    arguments: Arguments,
    phase: Phase,
}

pub struct Arguments {
    pub file_id: FileId,
    pub declared_path: PathBuf,
    pub live_path: PathBuf,
    pub snapshot_dir: PathBuf,
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
    Loaded,
    Planned,
    Applied,
    Committed,
    Failed(String),
}

enum Phase {
    Idle,
    #[allow(dead_code)]
    Loaded {
        declared: Declared,
        live: Live,
        snapshot: Snapshot,
    },
    #[allow(dead_code)]
    Planned {
        plan: Plan,
    },
    #[allow(dead_code)]
    Applied {
        drift: DriftPatch,
        new_live: Live,
    },
    #[allow(dead_code)]
    Committed,
    Failed(Error),
}

impl Phase {
    fn report(&self) -> PhaseReport {
        match self {
            Self::Idle => PhaseReport::Idle,
            Self::Loaded { .. } => PhaseReport::Loaded,
            Self::Planned { .. } => PhaseReport::Planned,
            Self::Applied { .. } => PhaseReport::Applied,
            Self::Committed => PhaseReport::Committed,
            Self::Failed(error) => PhaseReport::Failed(error.to_string()),
        }
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
                // v0.1 stub: the four-step Read → Plan → Apply → Commit
                // chain (each as a self-cast) lands when the value layer
                // is filled in.
                state.phase = Phase::Failed(Error::NotYetImplemented("reconciler.run"));
            }
            Message::GetPhase { reply_port } => {
                let _ = reply_port.send(state.phase.report());
            }
        }
        Ok(())
    }
}
