//! Message-free lifecycle metadata for the existing PostgreSQL turn/work owner.
//! Domain selection, input bytes, component refs and approval stay with their
//! existing composition owners. A work claim grants no Tool permission.
use brassclaw_host_api::{AgentId, ProjectId, TenantId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{TurnActor, TurnLeaseToken, TurnRunId, TurnRunnerId};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateExecutionContext {
    pub tenant_id: TenantId,
    #[serde(deserialize_with = "present_option")]
    pub agent_id: Option<AgentId>,
    #[serde(deserialize_with = "present_option")]
    pub project_id: Option<ProjectId>,
    #[serde(deserialize_with = "present_actor")]
    pub actor: Option<TurnActor>,
}

fn present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
fn present_actor<'de, D>(deserializer: D) -> Result<Option<TurnActor>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ActorWire {
        user_id: brassclaw_host_api::UserId,
    }
    Ok(Option::<ActorWire>::deserialize(deserializer)?.map(|actor| TurnActor::new(actor.user_id)))
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "purpose", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrivateOriginalRef {
    ValidatorStructural {
        submission_id: Uuid,
        subject_checksum: [u8; 32],
    },
    PrefixSourceOnly {
        job_id: Uuid,
        definition_checksum: [u8; 32],
    },
}

/// Original metadata only. The verifier of the referenced domain owns the raw
/// request and proves the correspondence before persisting this reference.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", deny_unknown_fields)]
pub enum PrivateWorkRequest {
    #[serde(rename = "private-work-request/1")]
    V1 {
        work_id: Uuid,
        run_id: TurnRunId,
        original: PrivateOriginalRef,
        context: PrivateExecutionContext,
        #[serde(deserialize_with = "present_option")]
        original_preparation_checksum: Option<[u8; 32]>,
        #[serde(deserialize_with = "present_option")]
        original_workflow_checksum: Option<[u8; 32]>,
    },
}
impl PrivateWorkRequest {
    pub fn original_preparation_checksum(&self) -> Option<[u8; 32]> {
        let Self::V1 {
            original_preparation_checksum,
            ..
        } = self;
        *original_preparation_checksum
    }
    pub fn original_workflow_checksum(&self) -> Option<[u8; 32]> {
        let Self::V1 {
            original_workflow_checksum,
            ..
        } = self;
        *original_workflow_checksum
    }
    pub fn work_id(&self) -> Uuid {
        let Self::V1 { work_id, .. } = self;
        *work_id
    }
    pub fn run_id(&self) -> TurnRunId {
        let Self::V1 { run_id, .. } = self;
        *run_id
    }
    pub fn original(&self) -> &PrivateOriginalRef {
        let Self::V1 { original, .. } = self;
        original
    }
    pub fn context(&self) -> &PrivateExecutionContext {
        let Self::V1 { context, .. } = self;
        context
    }
}

/// Presence is discriminated: an unprepared request cannot carry a Ready seal.
/// Definition bytes are immutable evidence owned by composition, not turnstate.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrivatePreparationState {
    AwaitingPreparation { reason: PrivatePreparationReason },
    Ready { definition_checksum: [u8; 32] },
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivatePreparationReason {
    PreparationPending,
    QualificationUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateWorkStatus {
    AwaitingPreparation,
    Ready,
    Claimed,
    ReconciliationRequired,
    Settled,
    Cancelled,
}

/// Read projection only; this cannot construct a claim or resume execution.
pub struct PrivateWorkState {
    pub request: PrivateWorkRequest,
    pub request_checksum: [u8; 32],
    pub status: PrivateWorkStatus,
    pub version: u64,
    pub cancellation_revision: u64,
    pub terminal_checksum: Option<[u8; 32]>,
}

/// Only PgTurnStateStore may construct this handle after a real locked claim
/// and reservation. Never deserialize or expose its fencing token in diagnostics.
#[derive(Clone)]
pub struct ClaimedPrivateWork {
    pub(crate) request: PrivateWorkRequest,
    pub(crate) request_checksum: [u8; 32],
    pub(crate) definition_checksum: [u8; 32],
    pub(crate) runner_id: TurnRunnerId,
    pub(crate) lease_token: TurnLeaseToken,
    pub(crate) generation: u64,
    pub(crate) cancellation_revision: u64,
}
impl ClaimedPrivateWork {
    pub fn request(&self) -> &PrivateWorkRequest {
        &self.request
    }
    pub fn request_checksum(&self) -> [u8; 32] {
        self.request_checksum
    }
    pub fn definition_checksum(&self) -> [u8; 32] {
        self.definition_checksum
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn cancellation_revision(&self) -> u64 {
        self.cancellation_revision
    }
    pub fn attempt(&self) -> crate::run_profile::MontyTaskAttempt {
        crate::run_profile::MontyTaskAttempt {
            run_id: self.request.run_id(),
            runner_id: self.runner_id,
            lease_token: self.lease_token,
        }
    }
}

/// A worker uses this exhaustive address; only the chat branch has a Turn.
#[derive(Clone)]
pub enum ClaimedWork {
    Chat(Box<crate::runner::ClaimedTurnRun>),
    Private(std::sync::Arc<ClaimedPrivateWork>),
}
impl ClaimedWork {
    pub fn attempt(&self) -> crate::run_profile::MontyTaskAttempt {
        match self {
            Self::Chat(claimed) => crate::run_profile::MontyTaskAttempt {
                run_id: claimed.state.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
            },
            Self::Private(claimed) => claimed.attempt(),
        }
    }
}

#[derive(Clone)]
pub enum WorkHeartbeatRequest {
    Chat(crate::runner::HeartbeatRequest),
    Private(std::sync::Arc<ClaimedPrivateWork>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkHeartbeat {
    Current,
    CancellationRequested,
    /// Exact original claim already settled. No lease renewal or dispatch
    /// authority; keep only the original driver's completion delivery alive.
    OriginalSettled,
}
impl ClaimedWork {
    pub fn heartbeat_request(&self) -> WorkHeartbeatRequest {
        let attempt = self.attempt();
        match self {
            Self::Chat(_) => WorkHeartbeatRequest::Chat(crate::runner::HeartbeatRequest {
                run_id: attempt.run_id,
                runner_id: attempt.runner_id,
                lease_token: attempt.lease_token,
            }),
            Self::Private(claimed) => WorkHeartbeatRequest::Private(claimed.clone()),
        }
    }
}
