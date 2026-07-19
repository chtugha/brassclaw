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
use heck::ToSnakeCase;
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

/// Convert a [`TurnStatus`] to the snake_case DB column value.
///
/// `TurnStatus` has no `#[serde(rename_all = "snake_case")]` — the serde
/// representation is PascalCase. The DB `status` column stores snake_case
/// values derived by converting PascalCase variant names via `heck::ToSnakeCase`
/// (e.g. `RecoveryRequired` → `"recovery_required"`). Do NOT use `.to_lowercase()`
/// — `"RecoveryRequired".to_lowercase()` yields `"recoveryrequired"` (missing
/// underscore). This function will be used when writing per-run indexed rows.
// TODO(S6): used when per-run row indexing is added alongside snapshot rows.
#[allow(dead_code)]
fn turn_status_str(status: crate::TurnStatus) -> String {
    format!("{:?}", status).to_snake_case()
}

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
                 WHERE tenant_id = $1 AND turn_id = $2 AND status = 'snapshot'",
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
        // Snapshot rows use the thread_id as both `id` (PK) and `turn_id`.
        // `run_id` is NULL for snapshot rows; `status` uses the 'snapshot' sentinel.
        // The unique index on (tenant_id, turn_id) drives the ON CONFLICT CAS.
        let thread_id_str = thread_id.as_str();

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .execute(
                "INSERT INTO brassclaw_turns \
                 (id, tenant_id, turn_id, status, payload, version) \
                 VALUES ($1, $2, $1, 'snapshot', $3, 1) \
                 ON CONFLICT (tenant_id, turn_id) DO UPDATE \
                 SET payload = excluded.payload, version = $4, updated_at = now() \
                 WHERE brassclaw_turns.version = $5",
                &[
                    &thread_id_str,
                    &self.tenant_id,
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

    /// Return all snapshot `thread_id`s for this tenant, ordered by `updated_at DESC`.
    ///
    /// Used by whole-tenant scans in [`claim_next_run`] and
    /// [`recover_expired_leases`] when no `scope_filter` is provided.
    async fn list_snapshot_thread_ids(&self) -> Result<Vec<ThreadId>, TurnError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .query(
                "SELECT turn_id FROM brassclaw_turns \
                 WHERE tenant_id = $1 AND status = 'snapshot' \
                 ORDER BY updated_at DESC",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let s: String = r.get(0);
                ThreadId::from_trusted(s)
            })
            .collect())
    }

    /// Resolve the [`ThreadId`] of the snapshot that owns `run_id`.
    ///
    /// `TurnRunTransitionPort` methods only carry a `run_id`; the snapshot is
    /// keyed by `thread_id`. This helper scans snapshot rows whose `payload`
    /// contains a run record with the given `run_id` and returns the first
    /// matching `turn_id` (= `thread_id` for snapshot rows).
    ///
    /// Returns `TurnError::ScopeNotFound` if no snapshot contains this run.
    async fn find_thread_id_for_run(
        &self,
        run_id: TurnRunId,
    ) -> Result<ThreadId, TurnError> {
        let run_id_str = run_id.to_string();
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        // The payload is a TurnPersistenceSnapshot; runs is an array of
        // TurnRunRecord objects each of which has a "run_id" field.
        let row = client
            .query_opt(
                "SELECT turn_id FROM brassclaw_turns \
                 WHERE tenant_id = $1 AND status = 'snapshot' \
                   AND payload->'runs' @> $2::jsonb",
                &[
                    &self.tenant_id,
                    &format!("[{{\"run_id\":\"{}\"}}]", run_id_str),
                ],
            )
            .await
            .map_err(map_pg)?;
        match row {
            Some(r) => {
                let thread_id_str: String = r.get(0);
                Ok(ThreadId::from_trusted(thread_id_str))
            }
            None => Err(TurnError::ScopeNotFound),
        }
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
        let thread_id = request.child_scope.thread_id.clone();
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
        self.apply(&thread_id, |store| {
            let scope = scope.clone();
            async move {
                let outcome = store
                    .reserve_tree_descendants(&scope, root_run_id, delta, cap)
                    .await;
                (outcome, store)
            }
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
        self.apply(&thread_id, |store| {
            let scope = scope.clone();
            async move {
                let outcome = store
                    .release_tree_descendants(&scope, root_run_id, delta)
                    .await;
                (outcome, store)
            }
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
        // When a scope_filter is present, use its thread_id directly — the
        // caller knows which thread to look in.  When absent (whole-tenant
        // claim), iterate all snapshot rows for this tenant: load each one
        // into a transient InMemoryTurnStateStore and attempt claim_next_run;
        // stop at the first snapshot that yields a run.
        if let Some(scope) = &request.scope_filter {
            let thread_id = scope.thread_id.clone();
            return self
                .apply(&thread_id, |store| {
                    let request = request.clone();
                    async move {
                        let outcome = store.claim_next_run(request).await;
                        (outcome, store)
                    }
                })
                .await;
        }
        // Whole-tenant scan: load all snapshot thread_ids, try each.
        let thread_ids = self.list_snapshot_thread_ids().await?;
        for thread_id in thread_ids {
            let result = self
                .apply(&thread_id, |store| {
                    let request = request.clone();
                    async move {
                        let outcome = store.claim_next_run(request).await;
                        (outcome, store)
                    }
                })
                .await?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }

    async fn heartbeat(&self, request: HeartbeatRequest) -> Result<EventCursor, TurnError> {
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        if let Some(scope) = &request.scope_filter {
            let thread_id = scope.thread_id.clone();
            return self
                .apply(&thread_id, |store| {
                    let request = request.clone();
                    async move {
                        let outcome = store.recover_expired_leases(request).await;
                        (outcome, store)
                    }
                })
                .await;
        }
        // Whole-tenant expiry sweep: apply to every snapshot.
        let thread_ids = self.list_snapshot_thread_ids().await?;
        let mut all_recovered = Vec::new();
        for thread_id in thread_ids {
            let response = self
                .apply(&thread_id, |store| {
                    let request = request.clone();
                    async move {
                        let outcome = store.recover_expired_leases(request).await;
                        (outcome, store)
                    }
                })
                .await?;
            all_recovered.extend(response.recovered);
        }
        Ok(RecoverExpiredLeasesResponse {
            recovered: all_recovered,
        })
    }

    async fn record_model_route_snapshot(
        &self,
        request: RecordModelRouteSnapshotRequest,
    ) -> Result<TurnRunState, TurnError> {
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
        let thread_id = self.find_thread_id_for_run(request.run_id).await?;
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
