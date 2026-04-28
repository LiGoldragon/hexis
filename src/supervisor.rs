//! Root supervisor actor.
//!
//! Owns the hexis config, spawns one `Reconciler` per managed file,
//! spawns the (stub) `Proposer`, and routes inbound CLI commands to
//! the appropriate child.
//!
//! [`Supervisor`] is a ZST — ractor's actor-identity tag, with no
//! inherent methods (per `style.md` § "No ZST method holders"). The
//! data-bearing handle is [`SupervisorHandle`], which owns the spawn
//! result (an `ActorRef` plus the tokio `JoinHandle`) and exposes
//! `start` / `actor_ref` / `shutdown`.

use std::collections::HashMap;
use ractor::{Actor, ActorProcessingErr, ActorRef};

use crate::error::Error;
use crate::types::FileId;
use crate::{proposer, reconciler};

/// Actor-identity tag for ractor. Carries no runtime data; the spawn
/// result lives on [`SupervisorHandle`].
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

impl State {
    fn apply_target(&self, file_id: &FileId) -> Result<(), ActorProcessingErr> {
        if let Some(reconciler_ref) = self.reconcilers.get(file_id) {
            ractor::cast!(reconciler_ref, reconciler::Message::Run)?;
        }
        Ok(())
    }

    fn apply_all(&self) -> Result<(), ActorProcessingErr> {
        for reconciler_ref in self.reconcilers.values() {
            ractor::cast!(reconciler_ref, reconciler::Message::Run)?;
        }
        Ok(())
    }

    fn propose(&self) -> Result<(), ActorProcessingErr> {
        ractor::cast!(self.proposer, proposer::Message::ProposeNow)?;
        Ok(())
    }
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
            Message::Apply { file_id } => state.apply_target(&file_id)?,
            Message::ApplyAll => state.apply_all()?,
            Message::Propose => state.propose()?,
            Message::Shutdown => myself.stop(None),
        }
        Ok(())
    }
}

/// Owns the supervisor's spawn result — the `ActorRef` for sending
/// messages and the `JoinHandle` for awaiting clean exit. The data-
/// bearing partner to the `Supervisor` ZST: `start` is a real
/// constructor returning Self.
pub struct SupervisorHandle {
    actor_ref: ActorRef<Message>,
    join: tokio::task::JoinHandle<()>,
}

impl SupervisorHandle {
    /// Spawn the supervisor as the root of the hexis actor tree.
    /// The only place a bare `Actor::spawn` is called per the ractor
    /// convention — every other actor uses `spawn_linked` from inside
    /// `pre_start`.
    pub async fn start(arguments: Arguments) -> Result<Self, Error> {
        let (actor_ref, join) =
            Actor::spawn(Some("supervisor".to_string()), Supervisor, arguments)
                .await
                .map_err(|error| Error::ActorSpawn(error.to_string()))?;
        Ok(Self { actor_ref, join })
    }

    pub fn actor_ref(&self) -> &ActorRef<Message> {
        &self.actor_ref
    }

    /// Send `Shutdown` and await clean exit.
    pub async fn shutdown(self) -> Result<(), Error> {
        ractor::cast!(self.actor_ref, Message::Shutdown).map_err(|error| Error::ActorCall {
            label: "supervisor.shutdown",
            reason: error.to_string(),
        })?;
        self.join
            .await
            .map_err(|error| Error::ActorSpawn(format!("join: {error}")))
    }
}
