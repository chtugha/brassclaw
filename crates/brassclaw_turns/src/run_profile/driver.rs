use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{LoopExit, RunProfileVersion, TurnCheckpointId, TurnId, TurnRunId};

use super::{
    host::AgentLoopDriverHost,
    refs::{CheckpointSchemaId, LoopDriverId},
    snapshot::ResolvedRunProfile,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLoopDriverDescriptor {
    pub id: LoopDriverId,
    pub version: RunProfileVersion,
    pub checkpoint_schema_id: Option<CheckpointSchemaId>,
    pub checkpoint_schema_version: Option<RunProfileVersion>,
}

impl AgentLoopDriverDescriptor {
    pub fn new(id: impl Into<String>, version: RunProfileVersion) -> Result<Self, String> {
        Ok(Self {
            id: LoopDriverId::new(id)?,
            version,
            checkpoint_schema_id: None,
            checkpoint_schema_version: None,
        })
    }

    pub fn from_trusted_static(
        id: &'static str,
        version: RunProfileVersion,
    ) -> Result<Self, String> {
        Ok(Self {
            id: LoopDriverId::new(id)?,
            version,
            checkpoint_schema_id: None,
            checkpoint_schema_version: None,
        })
    }

    pub fn with_checkpoint_schema(
        mut self,
        checkpoint_schema_id: impl Into<String>,
        checkpoint_schema_version: RunProfileVersion,
    ) -> Result<Self, String> {
        self.checkpoint_schema_id = Some(CheckpointSchemaId::new(checkpoint_schema_id)?);
        self.checkpoint_schema_version = Some(checkpoint_schema_version);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLoopDriverRunRequest {
    pub turn_id: TurnId,
    pub run_id: TurnRunId,
    pub resolved_run_profile: ResolvedRunProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLoopDriverResumeRequest {
    pub turn_id: TurnId,
    pub run_id: TurnRunId,
    pub checkpoint_id: TurnCheckpointId,
    pub resolved_run_profile: ResolvedRunProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentLoopDriverError {
    #[error("agent loop driver rejected request: {reason}")]
    InvalidRequest { reason: String },
    #[error("agent loop driver is unavailable: {reason}")]
    Unavailable { reason: String },
    #[error("agent loop driver failed: {reason_kind}")]
    Failed { reason_kind: String },
}

/// Userland loop implementation contract.
///
/// Implementations own loop mechanics and return a [`LoopExit`] handshake to the
/// trusted runner. They do not mutate turn state directly and do not receive raw
/// authority handles.
#[async_trait]
pub trait AgentLoopDriver: Send + Sync {
    fn descriptor(&self) -> AgentLoopDriverDescriptor;

    async fn run(
        &self,
        request: AgentLoopDriverRunRequest,
        host: &(dyn AgentLoopDriverHost + Send + Sync),
    ) -> Result<LoopExit, AgentLoopDriverError>;

    async fn resume(
        &self,
        request: AgentLoopDriverResumeRequest,
        host: &(dyn AgentLoopDriverHost + Send + Sync),
    ) -> Result<LoopExit, AgentLoopDriverError>;
}

/// Cross-turn-persistent Monty (Python Orchestrator) turn driver port (C.6
/// slice 4b).
///
/// The Reborn turn runner holds an `Arc<dyn MontyTurnDriverPort>` and calls
/// [`MontyTurnDriverPort::drive_turn`] directly for every Monty turn, bypassing
/// the `driver_registry` / canonical stage pipeline (C.6 slice 5 retires that
/// pipeline). The composition-side implementation owns the conversation-keyed
/// Monty session registry plus the engine dependencies needed to load the
/// Thread, build or resume a parked Monty session, and drive it to a yield.
///
/// Unlike [`AgentLoopDriver`], there is no `resume` split: cross-turn
/// persistence is handled by parking the live Monty VM in the registry between
/// turns, so every turn is a uniform `drive_turn`. The returned [`LoopExit`] is
/// applied through the same trusted applier as an `AgentLoopDriver` exit.
#[async_trait]
pub trait MontyTurnDriverPort: Send + Sync {
    /// Drive one turn of the persistent Monty orchestrator for the conversation
    /// identified by the run context's scope. On a fresh conversation a session
    /// is built; on a subsequent turn the parked session is resumed with the
    /// new turn's input.
    async fn drive_turn(
        &self,
        request: AgentLoopDriverRunRequest,
        host: &(dyn AgentLoopDriverHost + Send + Sync),
    ) -> Result<LoopExit, AgentLoopDriverError>;
}
