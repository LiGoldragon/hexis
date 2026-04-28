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

use ractor::{Actor, ActorProcessingErr, ActorRef};

use crate::drift::DriftPatch;
use crate::types::FileId;

pub struct Proposer;

pub struct State;

pub struct Arguments;

pub enum Message {
    DriftObserved(DriftEmitted),
    Tick,
    ProposeNow,
}

pub struct DriftEmitted {
    pub file_id: FileId,
    pub drift: DriftPatch,
}

#[ractor::async_trait]
impl Actor for Proposer {
    type Msg = Message;
    type State = State;
    type Arguments = Arguments;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _arguments: Arguments,
    ) -> std::result::Result<Self::State, ActorProcessingErr> {
        Ok(State)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Message,
        _state: &mut State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        // v0.1: drop everything on the floor. v2 batches and dispatches.
        let _ = message;
        Ok(())
    }
}
