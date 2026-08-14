use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use brassclaw_host_api::{AgentId, ProjectId, RuntimeCredentialAuthRequirement, TenantId};

use crate::{
    AcceptedMessageRef, AdmissionRejection, CancelRunRequest, CancelRunResponse, GateRef,
    GetRunStateRequest, IdempotencyKey, LoopCheckpointRecord, ReplyTargetBindingRef,
    ResumeTurnRequest, ResumeTurnResponse, RunProfileResolver, SourceBindingRef,
    SubmitChildRunRequest, SubmitTurnRequest, SubmitTurnResponse, ThreadBusy,
    TurnActiveRunRefState, TurnActor, TurnAdmissionPolicy, TurnAdmissionReservationRecord,
    TurnCapacityResource, TurnCheckpointId, TurnError, TurnErrorCategory, TurnId, TurnLeaseToken,
    TurnLifecycleEvent, TurnRunId, TurnRunProfile, TurnRunState, TurnRunnerId, TurnScope,
    TurnStatus, TurnTimestamp,
    events::EventCursor,
    run_profile::{LoopCheckpointKind, LoopCheckpointStateRef, LoopModelRouteSnapshot},
    runner::TurnRunTransitionPort,
};

#[async_trait]
pub trait TurnStateStore: Send + Sync {
    async fn submit_turn(
        &self,
        request: SubmitTurnRequest,
        admission_policy: &dyn TurnAdmissionPolicy,
        run_profile_resolver: &dyn RunProfileResolver,
    ) -> Result<SubmitTurnResponse, TurnError>;

    async fn resume_turn(
        &self,
        request: ResumeTurnRequest,
    ) -> Result<ResumeTurnResponse, TurnError>;

    async fn request_cancel(
        &self,
        request: CancelRunRequest,
    ) -> Result<CancelRunResponse, TurnError>;

    /// Return the run state when the run exists in the supplied exact scope.
    ///
    /// Missing runs and runs outside the supplied scope must both return
    /// [`TurnError::ScopeNotFound`]. This keeps scoped lookups non-enumerating
    /// and gives higher-level helpers one canonical missing-run shape.
    async fn get_run_state(&self, request: GetRunStateRequest) -> Result<TurnRunState, TurnError>;
}

/// Classify an active run reference through the shared turn-state lookup.
///
/// `None` and missing records both map to [`TurnActiveRunRefState::Missing`];
/// only looked-up terminal records map to [`TurnActiveRunRefState::Terminal`].
pub async fn active_run_ref_state<S>(
    store: &S,
    scope: TurnScope,
    active_run_ref: Option<TurnRunId>,
) -> Result<TurnActiveRunRefState, TurnError>
where
    S: TurnStateStore + ?Sized,
{
    let Some(run_id) = active_run_ref else {
        return Ok(TurnActiveRunRefState::Missing);
    };
    match store
        .get_run_state(GetRunStateRequest { scope, run_id })
        .await
    {
        Ok(state) if state.status.is_terminal() => Ok(TurnActiveRunRefState::Terminal),
        Ok(_) => Ok(TurnActiveRunRefState::Nonterminal),
        Err(TurnError::ScopeNotFound) => Ok(TurnActiveRunRefState::Missing),
        Err(error) => Err(error),
    }
}

#[async_trait]
pub trait TurnSpawnTreeStateStore: TurnStateStore {
    /// Spawn-tree operations are only needed by child-run orchestration.
    /// General turn submission should stay behind `TurnStateStore`.
    async fn submit_child_turn(
        &self,
        request: SubmitChildRunRequest,
        admission_policy: &dyn TurnAdmissionPolicy,
        run_profile_resolver: &dyn RunProfileResolver,
    ) -> Result<SubmitTurnResponse, TurnError>;
    ///
    /// List child runs only when the parent is visible in the supplied scope.
    ///
    /// Implementations must not leak whether a run exists in another tenant,
    /// agent, project, or thread scope; missing and unauthorized parents should
    /// both produce an empty child list.
    async fn children_of(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<Vec<TurnRunRecord>, TurnError>;

    /// Return a run record only when it belongs to the supplied exact scope.
    async fn get_run_record(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<Option<TurnRunRecord>, TurnError>;

    /// Reserve descendant capacity for a root run after validating root scope.
    ///
    /// Missing roots must return not found and cross-scope roots must return
    /// unauthorized rather than mutating reservation state.
    async fn reserve_tree_descendants(
        &self,
        scope: &TurnScope,
        root_run_id: TurnRunId,
        delta: u32,
        cap: u32,
    ) -> Result<SpawnTreeReservation, TurnError>;

    /// Release descendant capacity for a root run after validating root scope.
    async fn release_tree_descendants(
        &self,
        scope: &TurnScope,
        root_run_id: TurnRunId,
        delta: u32,
    ) -> Result<(), TurnError>;
}

/// Combined supertrait for types that serve as the turn spawn-tree state
/// store, the turn-run transition port, and the turn event projection source.
///
/// Implemented by every concrete store type (`InMemoryTurnStateStore`,
/// `PgTurnStateStore`, `LifecyclePublishingTurnStateStore<S>`,
/// `FilesystemTurnStateStore<F>`). Allows [`TurnStateDriverBox`] to wrap an
/// `Arc<dyn TurnStateDriver>` as a concrete `Sized` type so the hybrid and
/// pure-PG runtime paths can share a single composition function body.
pub trait TurnStateDriver:
    TurnSpawnTreeStateStore + TurnRunTransitionPort + crate::events::TurnEventProjectionSource
{
}

/// Concrete, `Sized` wrapper around `Arc<dyn TurnStateDriver>`.
///
/// `DefaultPlannedRuntimeParts<T, G>` requires `T: Sized` because of the
/// implicit `Sized` bound on type parameters. `dyn TurnStateDriver` is an
/// unsized type object and cannot be used directly as `T`. This newtype wraps
/// the trait object behind an `Arc` and provides delegating implementations of
/// all three sub-traits (`TurnStateStore`, `TurnSpawnTreeStateStore`,
/// `TurnRunTransitionPort`, and `TurnEventProjectionSource`) so that
/// `TurnStateDriverBox` itself satisfies every bound while remaining `Sized`.
pub struct TurnStateDriverBox(pub Arc<dyn TurnStateDriver>);

impl TurnStateDriverBox {
    /// Wrap an `Arc<dyn TurnStateDriver>` into a `TurnStateDriverBox`.
    pub fn new(inner: Arc<dyn TurnStateDriver>) -> Self {
        Self(inner)
    }
}

#[async_trait::async_trait]
impl TurnStateStore for TurnStateDriverBox {
    async fn submit_turn(
        &self,
        request: crate::SubmitTurnRequest,
        admission_policy: &dyn crate::TurnAdmissionPolicy,
        run_profile_resolver: &dyn crate::RunProfileResolver,
    ) -> Result<crate::SubmitTurnResponse, crate::TurnError> {
        self.0
            .submit_turn(request, admission_policy, run_profile_resolver)
            .await
    }

    async fn resume_turn(
        &self,
        request: crate::ResumeTurnRequest,
    ) -> Result<crate::ResumeTurnResponse, crate::TurnError> {
        self.0.resume_turn(request).await
    }

    async fn request_cancel(
        &self,
        request: crate::CancelRunRequest,
    ) -> Result<crate::CancelRunResponse, crate::TurnError> {
        self.0.request_cancel(request).await
    }

    async fn get_run_state(
        &self,
        request: crate::GetRunStateRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.get_run_state(request).await
    }
}

#[async_trait::async_trait]
impl TurnSpawnTreeStateStore for TurnStateDriverBox {
    async fn submit_child_turn(
        &self,
        request: crate::SubmitChildRunRequest,
        admission_policy: &dyn crate::TurnAdmissionPolicy,
        run_profile_resolver: &dyn crate::RunProfileResolver,
    ) -> Result<crate::SubmitTurnResponse, crate::TurnError> {
        self.0
            .submit_child_turn(request, admission_policy, run_profile_resolver)
            .await
    }

    async fn children_of(
        &self,
        scope: &crate::TurnScope,
        run_id: crate::TurnRunId,
    ) -> Result<Vec<TurnRunRecord>, crate::TurnError> {
        self.0.children_of(scope, run_id).await
    }

    async fn get_run_record(
        &self,
        scope: &crate::TurnScope,
        run_id: crate::TurnRunId,
    ) -> Result<Option<TurnRunRecord>, crate::TurnError> {
        self.0.get_run_record(scope, run_id).await
    }

    async fn reserve_tree_descendants(
        &self,
        scope: &crate::TurnScope,
        root_run_id: crate::TurnRunId,
        delta: u32,
        cap: u32,
    ) -> Result<SpawnTreeReservation, crate::TurnError> {
        self.0
            .reserve_tree_descendants(scope, root_run_id, delta, cap)
            .await
    }

    async fn release_tree_descendants(
        &self,
        scope: &crate::TurnScope,
        root_run_id: crate::TurnRunId,
        delta: u32,
    ) -> Result<(), crate::TurnError> {
        self.0
            .release_tree_descendants(scope, root_run_id, delta)
            .await
    }
}

#[async_trait::async_trait]
impl crate::runner::TurnRunTransitionPort for TurnStateDriverBox {
    async fn claim_next_run(
        &self,
        request: crate::runner::ClaimRunRequest,
    ) -> Result<Option<crate::runner::ClaimedTurnRun>, crate::TurnError> {
        self.0.claim_next_run(request).await
    }

    async fn heartbeat(
        &self,
        request: crate::runner::HeartbeatRequest,
    ) -> Result<crate::events::EventCursor, crate::TurnError> {
        self.0.heartbeat(request).await
    }

    async fn recover_expired_leases(
        &self,
        request: crate::runner::RecoverExpiredLeasesRequest,
    ) -> Result<crate::runner::RecoverExpiredLeasesResponse, crate::TurnError> {
        self.0.recover_expired_leases(request).await
    }

    async fn record_model_route_snapshot(
        &self,
        request: crate::runner::RecordModelRouteSnapshotRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.record_model_route_snapshot(request).await
    }

    async fn block_run(
        &self,
        request: crate::runner::BlockRunRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.block_run(request).await
    }

    async fn complete_run(
        &self,
        request: crate::runner::CompleteRunRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.complete_run(request).await
    }

    async fn cancel_run(
        &self,
        request: crate::runner::CancelRunCompletionRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.cancel_run(request).await
    }

    async fn fail_run(
        &self,
        request: crate::runner::FailRunRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.fail_run(request).await
    }

    async fn relinquish_run(
        &self,
        request: crate::runner::RelinquishRunRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.relinquish_run(request).await
    }

    async fn apply_validated_loop_exit(
        &self,
        request: crate::runner::ApplyValidatedLoopExitRequest,
    ) -> Result<crate::TurnRunState, crate::TurnError> {
        self.0.apply_validated_loop_exit(request).await
    }
}

#[async_trait::async_trait]
impl crate::events::TurnEventProjectionSource for TurnStateDriverBox {
    async fn read_turn_events_after(
        &self,
        scope: &crate::TurnScope,
        owner_user_id: Option<&brassclaw_host_api::UserId>,
        after: Option<crate::events::EventCursor>,
        limit: usize,
    ) -> Result<crate::events::TurnEventPage, crate::TurnError> {
        self.0
            .read_turn_events_after(scope, owner_user_id, after, limit)
            .await
    }
}

impl TurnStateDriver for TurnStateDriverBox {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnLockVersion(u64);

impl TurnLockVersion {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn incremented(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TurnActiveLockKey {
    pub scope: TurnScope,
}

impl From<&TurnScope> for TurnActiveLockKey {
    fn from(scope: &TurnScope) -> Self {
        Self {
            scope: scope.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_id: TurnId,
    pub scope: TurnScope,
    pub actor: TurnActor,
    pub accepted_message_ref: AcceptedMessageRef,
    pub source_binding_ref: SourceBindingRef,
    pub reply_target_binding_ref: ReplyTargetBindingRef,
    pub created_at: TurnTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRunRecord {
    pub run_id: TurnRunId,
    pub turn_id: TurnId,
    pub scope: TurnScope,
    pub accepted_message_ref: AcceptedMessageRef,
    pub source_binding_ref: SourceBindingRef,
    pub reply_target_binding_ref: ReplyTargetBindingRef,
    pub status: TurnStatus,
    pub profile: TurnRunProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model_route: Option<LoopModelRouteSnapshot>,
    pub checkpoint_id: Option<TurnCheckpointId>,
    pub gate_ref: Option<GateRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credential_requirements: Vec<RuntimeCredentialAuthRequirement>,
    pub failure: Option<crate::SanitizedFailure>,
    pub event_cursor: EventCursor,
    pub runner_id: Option<TurnRunnerId>,
    pub lease_token: Option<TurnLeaseToken>,
    pub lease_expires_at: Option<TurnTimestamp>,
    pub last_heartbeat_at: Option<TurnTimestamp>,
    pub claim_count: u64,
    pub received_at: TurnTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<TurnRunId>,
    #[serde(default)]
    pub subagent_depth: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_tree_root_run_id: Option<TurnRunId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpawnTreeReservationKey {
    pub tenant_id: TenantId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    pub root_run_id: TurnRunId,
}

impl SpawnTreeReservationKey {
    pub fn new(scope: &TurnScope, root_run_id: TurnRunId) -> Self {
        Self {
            tenant_id: scope.tenant_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            root_run_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnTreeReservation {
    pub scope: TurnScope,
    pub root_run_id: TurnRunId,
    pub descendant_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnActiveLockRecord {
    pub key: TurnActiveLockKey,
    pub run_id: TurnRunId,
    pub status: TurnStatus,
    pub lock_version: TurnLockVersion,
    pub acquired_at: TurnTimestamp,
    pub updated_at: TurnTimestamp,
}

/// Serde default for `LoopCheckpointKind` — used when deserializing old
/// persisted data that predates the `kind` field.
fn default_checkpoint_kind() -> LoopCheckpointKind {
    LoopCheckpointKind::BeforeBlock
}

/// Serde default for `LoopCheckpointStateRef` — legacy sentinel used only when
/// deserializing old persisted data that predates the `state_ref` field.
fn default_checkpoint_state_ref() -> LoopCheckpointStateRef {
    LoopCheckpointStateRef::legacy_unknown()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnCheckpointRecord {
    pub checkpoint_id: TurnCheckpointId,
    pub run_id: TurnRunId,
    /// Scope of the run that created this checkpoint. `None` for legacy records
    /// persisted before scope was added to checkpoints.
    #[serde(default)]
    pub scope: Option<TurnScope>,
    pub sequence: u64,
    pub status: TurnStatus,
    pub gate_ref: GateRef,
    /// The semantic kind of checkpoint (before model, side-effect, block, final).
    #[serde(default = "default_checkpoint_kind")]
    pub kind: LoopCheckpointKind,
    /// An opaque ref describing the loop state at the time of this checkpoint.
    #[serde(default = "default_checkpoint_state_ref")]
    pub state_ref: LoopCheckpointStateRef,
    pub created_at: TurnTimestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnIdempotencyOperationKind {
    Submit,
    Resume,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnIdempotencyOutcomeKind {
    Accepted,
    ThreadBusy,
    AdmissionRejected,
    Resumed,
    CancelRecorded,
    ScopeNotFound,
    Unauthorized,
    InvalidRequest,
    Unavailable,
    Conflict,
    CapacityExceeded,
}

impl TurnIdempotencyOutcomeKind {
    pub fn from_error(error: &TurnError) -> Self {
        match error {
            TurnError::ThreadBusy(_) => Self::ThreadBusy,
            TurnError::AdmissionRejected(_) => Self::AdmissionRejected,
            TurnError::ScopeNotFound => Self::ScopeNotFound,
            TurnError::Unauthorized => Self::Unauthorized,
            TurnError::InvalidRequest { .. } => Self::InvalidRequest,
            TurnError::Unavailable { .. } => Self::Unavailable,
            TurnError::CapacityExceeded { .. } => Self::CapacityExceeded,
            TurnError::Conflict { .. }
            | TurnError::InvalidTransition { .. }
            | TurnError::LeaseMismatch => Self::Conflict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnIdempotencyReplay {
    SubmitAccepted(SubmitTurnResponse),
    SubmitThreadBusy(ThreadBusy),
    SubmitAdmissionRejected(AdmissionRejection),
    ResumeSucceeded(ResumeTurnResponse),
    CancelRecorded(CancelRunResponse),
    Error(TurnIdempotencyErrorReplay),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnIdempotencyErrorReplay {
    pub category: TurnErrorCategory,
    pub adapter_status_code: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_resource: Option<TurnCapacityResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_cap: Option<u64>,
}

impl TurnIdempotencyErrorReplay {
    pub fn from_error(error: &TurnError) -> Self {
        let (capacity_resource, capacity_cap) = match error {
            TurnError::CapacityExceeded { resource, cap } => (Some(*resource), Some(*cap)),
            _ => (None, None),
        };
        Self {
            category: error.category(),
            adapter_status_code: error.adapter_status_code(),
            capacity_resource,
            capacity_cap,
        }
    }

    fn to_error(&self) -> TurnError {
        match self.category {
            TurnErrorCategory::ScopeNotFound => TurnError::ScopeNotFound,
            TurnErrorCategory::Unauthorized => TurnError::Unauthorized,
            TurnErrorCategory::InvalidRequest => TurnError::InvalidRequest {
                reason: "replayed invalid request".to_string(),
            },
            TurnErrorCategory::Unavailable => TurnError::Unavailable {
                reason: "replayed unavailable".to_string(),
            },
            TurnErrorCategory::Conflict => TurnError::Conflict {
                reason: "replayed conflict".to_string(),
            },
            TurnErrorCategory::CapacityExceeded => TurnError::CapacityExceeded {
                resource: self
                    .capacity_resource
                    .unwrap_or(TurnCapacityResource::Replayed),
                cap: self.capacity_cap.unwrap_or_default(),
            },
            TurnErrorCategory::ThreadBusy => TurnError::Conflict {
                reason: "replayed malformed thread-busy idempotency record".to_string(),
            },
            TurnErrorCategory::AdmissionRejected => TurnError::Conflict {
                reason: "replayed malformed admission idempotency record".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnIdempotencyRecord {
    pub scope: TurnScope,
    pub operation: TurnIdempotencyOperationKind,
    pub key: IdempotencyKey,
    pub turn_id: Option<TurnId>,
    pub run_id: Option<TurnRunId>,
    pub outcome: TurnIdempotencyOutcomeKind,
    pub replay: TurnIdempotencyReplay,
    pub created_at: TurnTimestamp,
    pub expires_at: Option<TurnTimestamp>,
}

impl TurnIdempotencyRecord {
    pub fn replay_submit(&self) -> Option<Result<SubmitTurnResponse, TurnError>> {
        if self.operation != TurnIdempotencyOperationKind::Submit {
            return None;
        }
        match &self.replay {
            TurnIdempotencyReplay::SubmitAccepted(response) => Some(Ok(response.clone())),
            // Legacy persisted busy submit records are intentionally not replayable.
            // Same-thread busy is a transient lock state, not an idempotent submit outcome.
            TurnIdempotencyReplay::SubmitThreadBusy(_) => None,
            TurnIdempotencyReplay::SubmitAdmissionRejected(rejection) => {
                Some(Err(TurnError::AdmissionRejected(rejection.clone())))
            }
            TurnIdempotencyReplay::Error(error)
                if self.operation == TurnIdempotencyOperationKind::Submit =>
            {
                Some(Err(error.to_error()))
            }
            _ => None,
        }
    }

    pub fn replay_resume(&self) -> Option<Result<ResumeTurnResponse, TurnError>> {
        if self.operation != TurnIdempotencyOperationKind::Resume {
            return None;
        }
        match &self.replay {
            TurnIdempotencyReplay::ResumeSucceeded(response) => Some(Ok(response.clone())),
            TurnIdempotencyReplay::Error(error)
                if self.operation == TurnIdempotencyOperationKind::Resume =>
            {
                Some(Err(error.to_error()))
            }
            _ => None,
        }
    }

    pub fn replay_cancel(&self) -> Option<Result<CancelRunResponse, TurnError>> {
        if self.operation != TurnIdempotencyOperationKind::Cancel {
            return None;
        }
        match &self.replay {
            TurnIdempotencyReplay::CancelRecorded(response) => Some(Ok(response.clone())),
            TurnIdempotencyReplay::Error(error)
                if self.operation == TurnIdempotencyOperationKind::Cancel =>
            {
                Some(Err(error.to_error()))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TurnPersistenceSnapshot {
    pub turns: Vec<TurnRecord>,
    pub runs: Vec<TurnRunRecord>,
    pub active_locks: Vec<TurnActiveLockRecord>,
    pub checkpoints: Vec<TurnCheckpointRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loop_checkpoints: Vec<LoopCheckpointRecord>,
    pub idempotency_records: Vec<TurnIdempotencyRecord>,
    #[serde(default)]
    pub events: Vec<TurnLifecycleEvent>,
    #[serde(default)]
    pub event_retention_floor: EventCursor,
    #[serde(default)]
    pub admission_reservations: Vec<TurnAdmissionReservationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spawn_tree_reservations: Vec<SpawnTreeReservation>,
}
