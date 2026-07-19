//! Postgres-backed turn-state store.
//!
//! [`PgTurnStateStore`] implements all five turn-state traits:
//! - [`TurnStateStore`]
//! - [`TurnSpawnTreeStateStore`]
//! - [`TurnEventProjectionSource`]
//! - [`LoopCheckpointStore`]
//! - [`TurnRunTransitionPort`]
//!
//! # Strategy
//!
//! The implementation follows the same load-snapshot → delegate-to-InMemory →
//! CAS-write-back pattern as [`FilesystemTurnStateStore`].  The entire
//! [`TurnPersistenceSnapshot`] is stored as a single JSONB `payload` column in
//! `brassclaw_turns`, keyed by `(tenant_id, thread_id)`.  Mutations load the
//! snapshot, apply the change through a transient [`InMemoryTurnStateStore`],
//! then write the new snapshot back with an optimistic `version` CAS check.
//!
//! This keeps the complex in-memory state machine as the single source of
//! truth for turn transitions while Postgres provides durability,
//! multi-process visibility, and crash recovery.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{ThreadId, UserId};
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::{
    AllowAllTurnAdmissionLimitProvider, CancelRunRequest, CancelRunResponse, EventCursor,
    GetLoopCheckpointRequest, GetRunStateRequest, InMemoryTurnStateStore,
    InMemoryTurnStateStoreLimits, LoopCheckpointRecord, LoopCheckpointStore,
    PutLoopCheckpointRequest, ResumeTurnRequest, ResumeTurnResponse, RunProfileResolver,
    SpawnTreeReservation, SubmitChildRunRequest, SubmitTurnRequest, SubmitTurnResponse,
    TurnAdmissionLimitProvider, TurnAdmissionPolicy, TurnError, TurnEventPage,
    TurnEventProjectionSource, TurnPersistenceSnapshot, TurnRunId, TurnRunRecord, TurnRunState,
    TurnScope, TurnSpawnTreeStateStore, TurnStateStore,
    events::project_turn_events,
    run_profile::RunProfileResolutionRequest,
    runner::{
        ApplyValidatedLoopExitRequest, BlockRunRequest, CancelRunCompletionRequest,
        ClaimRunRequest, ClaimedTurnRun, CompleteRunRequest, FailRunRequest, HeartbeatRequest,
        RecordModelRouteSnapshotRequest, RecoverExpiredLeasesRequest, RecoverExpiredLeasesResponse,
        RelinquishRunRequest, TurnRunTransitionPort,
    },
};

/// Maximum number of optimistic-CAS retries before surfacing `TurnError::Unavailable`.
const PG_CAS_RETRIES: usize = 12;

fn map_pg_pool(e: deadpool_postgres::PoolError) -> TurnError {
    TurnError::Unavailable {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> TurnError {
    TurnError::Unavailable {
        reason: e.to_string(),
    }
}

fn map_json_ser(e: serde_json::Error) -> TurnError {
    TurnError::Unavailable {
        reason: format!("turn-state snapshot serialization failed: {e}"),
    }
}

fn map_json_de(e: serde_json::Error) -> TurnError {
    TurnError::Unavailable {
        reason: format!("turn-state snapshot deserialization failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// PgTurnStateStore
// ---------------------------------------------------------------------------

/// Postgres-backed turn-state store.
///
/// Stores the entire [`TurnPersistenceSnapshot`] as JSONB in
/// `brassclaw_turns (tenant_id, user_id, payload, version)`.
/// All turn-state mutations follow the load → apply → CAS-write pattern.
pub struct PgTurnStateStore {
    pool: Arc<PgPool>,
    tenant_id: String,
    limits: InMemoryTurnStateStoreLimits,
    admission_limit_provider: Arc<dyn TurnAdmissionLimitProvider>,
}

impl PgTurnStateStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
            limits: InMemoryTurnStateStoreLimits::default(),
            admission_limit_provider: Arc::new(AllowAllTurnAdmissionLimitProvider),
        }
    }

    pub fn with_limits(mut self, limits: InMemoryTurnStateStoreLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_admission_limit_provider(
        mut self,
        provider: Arc<dyn TurnAdmissionLimitProvider>,
    ) -> Self {
        self.admission_limit_provider = provider;
        self
    }

    // ------------------------------------------------------------------
    // Snapshot persistence
    // ------------------------------------------------------------------

    /// Load the snapshot and its `version` counter for the given `thread_id`.
    ///
    /// The snapshot is keyed by `(tenant_id, thread_id)` — one snapshot row
    /// per thread, matching the structural isolation that
    /// `FilesystemTurnStateStore` provides via per-tenant `MountView`.
    ///
    /// Returns `(snapshot, version)`.  If no row exists, returns an empty
    /// snapshot with version 0.
    async fn read_snapshot(
        &self,
        thread_id: &ThreadId,
    ) -> Result<(TurnPersistenceSnapshot, i64), TurnError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let row = client
            .query_opt(
                "SELECT payload, version FROM brassclaw_turns \
                 WHERE tenant_id = $1 AND turn_id = $2",
                &[&self.tenant_id, &thread_id.as_str()],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok((TurnPersistenceSnapshot::default(), 0)),
            Some(r) => {
                let payload: Value = r.get(0);
                let version: i64 = r.get(1);
                let snapshot: TurnPersistenceSnapshot =
                    serde_json::from_value(payload).map_err(map_json_de)?;
                Ok((snapshot, version))
            }
        }
    }

    /// Write the snapshot back using an optimistic version check.
    ///
    /// Returns `Ok(true)` if the write landed, `Ok(false)` on version mismatch.
    async fn write_snapshot(
        &self,
        thread_id: &ThreadId,
        snapshot: &TurnPersistenceSnapshot,
        expected_version: i64,
    ) -> Result<bool, TurnError> {
        let payload = serde_json::to_value(snapshot).map_err(map_json_ser)?;
        let next_version = expected_version + 1;

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        // INSERT for a fresh row; on conflict UPDATE only if version matches.
        let rows = client
            .execute(
                "INSERT INTO brassclaw_turns (tenant_id, turn_id, payload, version) \
                 VALUES ($1, $2, $3, 1) \
                 ON CONFLICT (tenant_id, turn_id) DO UPDATE \
                 SET payload = excluded.payload, version = $4, updated_at = now() \
                 WHERE brassclaw_turns.version = $5",
                &[
                    &self.tenant_id,
                    &thread_id.as_str(),
                    &payload,
                    &next_version,
                    &expected_version,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(rows > 0)
    }

    fn build_in_memory_store(
        &self,
        snapshot: TurnPersistenceSnapshot,
    ) -> Result<InMemoryTurnStateStore, TurnError> {
        InMemoryTurnStateStore::from_persistence_snapshot_with_admission_limit_provider(
            snapshot,
            self.limits,
            self.admission_limit_provider.clone(),
        )
    }

    // ------------------------------------------------------------------
    // CAS apply loop
    // ------------------------------------------------------------------

    /// Load-apply-CAS the snapshot keyed by `thread_id`.
    async fn apply<T, A, Fut>(
        &self,
        thread_id: &ThreadId,
        mut apply: A,
    ) -> Result<T, TurnError>
    where
        A: FnMut(InMemoryTurnStateStore) -> Fut,
        Fut: std::future::Future<Output = (Result<T, TurnError>, InMemoryTurnStateStore)>,
    {
        for _ in 0..PG_CAS_RETRIES {
            let (snapshot, version) = self.read_snapshot(thread_id).await?;
            let old_snapshot = snapshot.clone();
            let store = self.build_in_memory_store(snapshot)?;
            let (outcome, store) = apply(store).await;
            let new_snapshot = store.persistence_snapshot();
            if new_snapshot == old_snapshot {
                return outcome;
            }
            match self
                .write_snapshot(thread_id, &new_snapshot, version)
                .await?
            {
                true => return outcome,
                false => continue,
            }
        }
        Err(TurnError::Unavailable {
            reason: "turn state Postgres CAS retries exhausted".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// TurnStateStore
// ---------------------------------------------------------------------------

#[async_trait]
impl TurnStateStore for PgTurnStateStore {
    async fn submit_turn(
        &self,
        request: SubmitTurnRequest,
        admission_policy: &dyn TurnAdmissionPolicy,
        run_profile_resolver: &dyn RunProfileResolver,
    ) -> Result<SubmitTurnResponse, TurnError> {
        // Resolve profile outside the CAS loop so we don't hold the
        // lock-equivalent across potentially-slow resolver I/O.
        let profile_resolution = run_profile_resolver
            .resolve_run_profile(RunProfileResolutionRequest {
                requested_run_profile: request.requested_run_profile.clone(),
                ..RunProfileResolutionRequest::interactive_default()
            })
            .await;
        let pre_resolved = PreResolvedRunProfileResolver::new(profile_resolution);
        let thread_id = request.scope.thread_id.clone();
        self.apply(&thread_id, |store| {
            let request = request.clone();
            let pre_resolved = pre_resolved.clone();
            async move {
                let outcome = store
                    .submit_turn(request, admission_policy, &pre_resolved)
                    .await;
                (outcome, store)
            }
        })
        .await
    }

    async fn resume_turn(
        &self,
        request: ResumeTurnRequest,
    ) -> Result<ResumeTurnResponse, TurnError> {
        let thread_id = request.scope.thread_id.clone();
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.resume_turn(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn request_cancel(
        &self,
        request: CancelRunRequest,
    ) -> Result<CancelRunResponse, TurnError> {
        let thread_id = request.scope.thread_id.clone();
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.request_cancel(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn get_run_state(&self, request: GetRunStateRequest) -> Result<TurnRunState, TurnError> {
        let thread_id = request.scope.thread_id.clone();
        let (snapshot, _) = self.read_snapshot(&thread_id).await?;
        self.build_in_memory_store(snapshot)?
            .get_run_state(request)
            .await
    }
}

// ---------------------------------------------------------------------------
// TurnSpawnTreeStateStore
// ---------------------------------------------------------------------------

#[async_trait]
impl TurnSpawnTreeStateStore for PgTurnStateStore {
    async fn submit_child_turn(
        &self,
        request: SubmitChildRunRequest,
        admission_policy: &dyn TurnAdmissionPolicy,
        run_profile_resolver: &dyn RunProfileResolver,
    ) -> Result<SubmitTurnResponse, TurnError> {
        let profile_resolution = run_profile_resolver
            .resolve_run_profile(RunProfileResolutionRequest {
                requested_run_profile: request.requested_run_profile.clone(),
                ..RunProfileResolutionRequest::interactive_default()
            })
            .await;
        let pre_resolved = PreResolvedRunProfileResolver::new(profile_resolution);
        let thread_id = request.scope.thread_id.clone();
        self.apply(&thread_id, |store| {
            let request = request.clone();
            let pre_resolved = pre_resolved.clone();
            async move {
                let outcome = store
                    .submit_child_turn(request, admission_policy, &pre_resolved)
                    .await;
                (outcome, store)
            }
        })
        .await
    }

    async fn children_of(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<Vec<TurnRunRecord>, TurnError> {
        let (snapshot, _) = self.read_snapshot(&scope.thread_id).await?;
        Ok(project_children_of(&snapshot, scope, run_id))
    }

    async fn get_run_record(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<Option<TurnRunRecord>, TurnError> {
        let (snapshot, _) = self.read_snapshot(&scope.thread_id).await?;
        Ok(project_run_record(&snapshot, scope, run_id))
    }

    async fn reserve_tree_descendants(
        &self,
        scope: &TurnScope,
        root_run_id: TurnRunId,
        delta: u32,
        cap: u32,
    ) -> Result<SpawnTreeReservation, TurnError> {
        let thread_id = scope.thread_id.clone();
        let scope = scope.clone();
        self.apply(&thread_id, |store| async move {
            let outcome = store
                .reserve_tree_descendants(&scope, root_run_id, delta, cap)
                .await;
            (outcome, store)
        })
        .await
    }

    async fn release_tree_descendants(
        &self,
        scope: &TurnScope,
        root_run_id: TurnRunId,
        delta: u32,
    ) -> Result<(), TurnError> {
        let thread_id = scope.thread_id.clone();
        let scope = scope.clone();
        self.apply(&thread_id, |store| async move {
            let outcome = store
                .release_tree_descendants(&scope, root_run_id, delta)
                .await;
            (outcome, store)
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// TurnEventProjectionSource
// ---------------------------------------------------------------------------

#[async_trait]
impl TurnEventProjectionSource for PgTurnStateStore {
    async fn read_turn_events_after(
        &self,
        scope: &TurnScope,
        owner_user_id: Option<&UserId>,
        after: Option<EventCursor>,
        limit: usize,
    ) -> Result<TurnEventPage, TurnError> {
        let (snapshot, _) = self.read_snapshot(&scope.thread_id).await?;
        Ok(project_turn_events(
            &snapshot.events,
            scope,
            owner_user_id,
            after,
            limit,
            snapshot.event_retention_floor,
        ))
    }
}

// ---------------------------------------------------------------------------
// LoopCheckpointStore (snapshot-based)
// ---------------------------------------------------------------------------

#[async_trait]
impl LoopCheckpointStore for PgTurnStateStore {
    async fn put_loop_checkpoint(
        &self,
        request: PutLoopCheckpointRequest,
    ) -> Result<LoopCheckpointRecord, TurnError> {
        let thread_id = request.scope.thread_id.clone();
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.put_loop_checkpoint(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn get_loop_checkpoint(
        &self,
        request: GetLoopCheckpointRequest,
    ) -> Result<Option<LoopCheckpointRecord>, TurnError> {
        let thread_id = request.scope.thread_id.clone();
        let (snapshot, _) = self.read_snapshot(&thread_id).await?;
        self.build_in_memory_store(snapshot)?
            .get_loop_checkpoint(request)
            .await
    }
}

// ---------------------------------------------------------------------------
// TurnRunTransitionPort
// ---------------------------------------------------------------------------

#[async_trait]
impl TurnRunTransitionPort for PgTurnStateStore {
    async fn claim_next_run(
        &self,
        request: ClaimRunRequest,
    ) -> Result<Option<ClaimedTurnRun>, TurnError> {
        // claim_next_run looks across all threads in a tenant. Use the
        // scope_filter thread_id when present; fall back to a sentinel that
        // scans the whole-tenant pool snapshot row (keyed by thread_id="__global__").
        let thread_id = request
            .scope_filter
            .as_ref()
            .map(|s| s.thread_id.clone())
            .unwrap_or_else(|| ThreadId::from_trusted("__global__".to_string()));
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.claim_next_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn heartbeat(&self, request: HeartbeatRequest) -> Result<EventCursor, TurnError> {
        // heartbeat is run-id scoped; composition must pass the correct
        // thread_id via the store instance scope. Use global sentinel fallback.
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.heartbeat(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn recover_expired_leases(
        &self,
        request: RecoverExpiredLeasesRequest,
    ) -> Result<RecoverExpiredLeasesResponse, TurnError> {
        let thread_id = request
            .scope_filter
            .as_ref()
            .map(|s| s.thread_id.clone())
            .unwrap_or_else(|| ThreadId::from_trusted("__global__".to_string()));
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.recover_expired_leases(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn record_model_route_snapshot(
        &self,
        request: RecordModelRouteSnapshotRequest,
    ) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.record_model_route_snapshot(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn block_run(&self, request: BlockRunRequest) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.block_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn complete_run(&self, request: CompleteRunRequest) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.complete_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn cancel_run(
        &self,
        request: CancelRunCompletionRequest,
    ) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.cancel_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn fail_run(&self, request: FailRunRequest) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.fail_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn relinquish_run(
        &self,
        request: RelinquishRunRequest,
    ) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.relinquish_run(request).await;
                (outcome, store)
            }
        })
        .await
    }

    async fn apply_validated_loop_exit(
        &self,
        request: ApplyValidatedLoopExitRequest,
    ) -> Result<TurnRunState, TurnError> {
        let thread_id = ThreadId::from_trusted("__global__".to_string());
        self.apply(&thread_id, |store| {
            let request = request.clone();
            async move {
                let outcome = store.apply_validated_loop_exit(request).await;
                (outcome, store)
            }
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// Snapshot projection helpers (mirrors filesystem_store.rs)
// ---------------------------------------------------------------------------

fn project_children_of(
    snapshot: &TurnPersistenceSnapshot,
    scope: &TurnScope,
    run_id: TurnRunId,
) -> Vec<TurnRunRecord> {
    let Some(parent) = snapshot.runs.iter().find(|r| r.run_id == run_id) else {
        return Vec::new();
    };
    if parent.scope != *scope {
        return Vec::new();
    }
    let mut children: Vec<TurnRunRecord> = snapshot
        .runs
        .iter()
        .filter(|r| {
            r.parent_run_id == Some(run_id)
                && r.scope.tenant_id == scope.tenant_id
                && r.scope.agent_id == scope.agent_id
                && r.scope.project_id == scope.project_id
        })
        .cloned()
        .collect();
    children.sort_by_key(|r| r.received_at);
    children
}

fn project_run_record(
    snapshot: &TurnPersistenceSnapshot,
    scope: &TurnScope,
    run_id: TurnRunId,
) -> Option<TurnRunRecord> {
    snapshot
        .runs
        .iter()
        .find(|r| r.run_id == run_id && r.scope == *scope)
        .cloned()
}

// ---------------------------------------------------------------------------
// Pre-resolved run-profile resolver (mirrors filesystem_store.rs)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct PreResolvedRunProfileResolver {
    result: Result<crate::ResolvedRunProfile, crate::RunProfileResolutionError>,
}

impl PreResolvedRunProfileResolver {
    fn new(result: Result<crate::ResolvedRunProfile, crate::RunProfileResolutionError>) -> Self {
        Self { result }
    }
}

#[async_trait]
impl RunProfileResolver for PreResolvedRunProfileResolver {
    async fn resolve_run_profile(
        &self,
        _request: crate::RunProfileResolutionRequest,
    ) -> Result<crate::ResolvedRunProfile, crate::RunProfileResolutionError> {
        self.result.clone()
    }
}
