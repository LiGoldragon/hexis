//! Root supervisor actor.
//!
//! Owns the hexis config, spawns one `Reconciler` per managed file,
//! spawns the (stub) `Proposer`, and routes inbound CLI commands to
//! the appropriate child. The only place `Actor::spawn` (unrooted) is
//! called — every other actor is `spawn_linked` from here.

use std::collections::HashMap;
use ractor::{Actor, ActorProcessingErr, ActorRef};

use crate::error::Error;
use crate::types::FileId;
use crate::{proposer, reconciler};

pub struct Supervisor;

pub struct State {
    reconcilers: HashMap<FileId, ActorRef<reconciler::Message>>,
    proposer: ActorRef<proposer::Message>,
}

pub struct Arguments {
    pub reconciler_targets: Vec<reconciler::Arguments>,
}

pub enum Message {
    Apply { file_id: FileId },
    ApplyAll,
    Propose,
    Shutdown,
}

#[ractor::async_trait]
impl Actor for Supervisor {
    type Msg = Message;
    type State = State;
    type Arguments = Arguments;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        arguments: Arguments,
    ) -> std::result::Result<Self::State, ActorProcessingErr> {
        let (proposer, _proposer_handle) = Actor::spawn_linked(
            Some("proposer".to_string()),
            proposer::Proposer,
            proposer::Arguments,
            myself.get_cell(),
        )
        .await?;

        let mut reconcilers = HashMap::new();
        for target_arguments in arguments.reconciler_targets {
            let file_id = target_arguments.file_id.clone();
            let actor_name = format!("reconciler-{file_id}");
            let (reconciler_ref, _handle) = Actor::spawn_linked(
                Some(actor_name),
                reconciler::Reconciler,
                target_arguments,
                myself.get_cell(),
            )
            .await?;
            reconcilers.insert(file_id, reconciler_ref);
        }

        Ok(State { reconcilers, proposer })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Message,
        state: &mut State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        match message {
            Message::Apply { file_id } => {
                if let Some(reconciler_ref) = state.reconcilers.get(&file_id) {
                    ractor::cast!(reconciler_ref, reconciler::Message::Run)?;
                }
            }
            Message::ApplyAll => {
                for reconciler_ref in state.reconcilers.values() {
                    ractor::cast!(reconciler_ref, reconciler::Message::Run)?;
                }
            }
            Message::Propose => {
                ractor::cast!(state.proposer, proposer::Message::ProposeNow)?;
            }
            Message::Shutdown => {
                myself.stop(None);
            }
        }
        Ok(())
    }
}

impl Supervisor {
    /// Spawn the supervisor as the root of the hexis actor tree.
    ///
    /// The only place a bare `Actor::spawn` is called per the ractor
    /// convention (every other actor uses `spawn_linked` from inside
    /// `pre_start`).
    pub async fn start(
        arguments: Arguments,
    ) -> std::result::Result<(ActorRef<Message>, tokio::task::JoinHandle<()>), Error> {
        Actor::spawn(Some("supervisor".to_string()), Supervisor, arguments)
            .await
            .map_err(|error| Error::ActorSpawn(error.to_string()))
    }
}
