use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    /// Trusted admission adapter only: the accepted input is not committed.
    /// Return only before VM execution, provider calls or side effects. The
    /// worker may relinquish this attempt until the bounded admission deadline.
    #[error("agent loop input admission has not committed")]
    InputAdmissionPending,
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

/// Owned handoff of one claimed task to Monty. The host retains the exact opaque
/// conversation and accepted-message identities and the neutral host ports.
/// Ownership permits a service to retain them across waits without borrowing the
/// runner's stack or manufacturing a UUID engine Thread.
///
/// This is Rust-only transport: it is deliberately neither serializable nor
/// debug-printable. Claim tokens and host authority must not enter Python/model
/// payloads. The hosting adapter constructs the separate, validated VM values.
pub struct MontyTaskHandoff {
    request: AgentLoopDriverRunRequest,
    attempt: super::MontyTaskAttempt,
    host: Arc<dyn AgentLoopDriverHost + Send + Sync>,
}

impl MontyTaskHandoff {
    pub fn new(
        request: AgentLoopDriverRunRequest,
        attempt: super::MontyTaskAttempt,
        host: Arc<dyn AgentLoopDriverHost + Send + Sync>,
    ) -> Result<Self, AgentLoopDriverError> {
        let context = host.run_context();
        if request.run_id != context.run_id
            || request.turn_id != context.turn_id
            || request.resolved_run_profile != context.resolved_run_profile
            || attempt.run_id != context.run_id
            || context.thread_id != context.scope.thread_id
        {
            return Err(AgentLoopDriverError::InvalidRequest {
                reason: "Monty request, claim and host context do not identify the same task"
                    .to_owned(),
            });
        }
        Ok(Self {
            request,
            attempt,
            host,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        AgentLoopDriverRunRequest,
        super::MontyTaskAttempt,
        Arc<dyn AgentLoopDriverHost + Send + Sync>,
    ) {
        (self.request, self.attempt, self.host)
    }
}

/// Handoff port to the instance-wide Monty orchestrator. The runner owns claims,
/// heartbeats and exit application; Python/Recipes own task sequencing. An owned
/// handoff must retain task identity and host access across waits, and must never
/// imply a new global VM for a conversation or a turn.
///
/// The production composition adapter delivers admitted attempts to the
/// supervised global service; hosts and retained selections stay attempt-local.
#[async_trait]
pub trait MontyTurnDriverPort: Send + Sync {
    /// Transfer one claimed task and await its exit. The service may retain the
    /// host independently of this waiting future. Dropping the future is not a
    /// cancellation acknowledgement; use addressed `stop_attempt`.
    async fn drive_turn(&self, handoff: MontyTaskHandoff)
    -> Result<LoopExit, AgentLoopDriverError>;

    /// Stop one claimed attempt. A completed/missing attempt is an idempotent
    /// no-op; another claim of the same run must never receive this signal.
    /// Success acknowledges actual owned attempt settlement, not merely
    /// acceptance or consumption of a stop signal. The adapter must fence the
    /// attempt before waiting and bound settlement with the acknowledged live
    /// cancellation policy captured for this wait. Later policy changes apply
    /// to new waits. Callers await this bounded result without another deadline;
    /// failure stops worker admission and retains unacknowledged attempt/effect
    /// evidence for reconciliation. It never proves an external effect absent.
    async fn stop_attempt(
        &self,
        _attempt: super::MontyTaskAttempt,
    ) -> Result<(), AgentLoopDriverError> {
        Err(AgentLoopDriverError::Unavailable {
            reason: "this Monty adapter does not support addressed cancellation".to_owned(),
        })
    }
}
