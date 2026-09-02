//! v3 Phase H.12.5 — production engine [`Store`] adapter (Thread load surface).
//!
//! Implements [`Store::load_thread`] by delegating to the loop-owned
//! [`brassclaw_threads::SessionThreadService`] — the canonical durable store
//! for live Reborn threads (`brassclaw_session_threads`). Every other `Store`
//! method returns `EngineError::Store` (stub), matching the
//! `PgMemoryDocStore` / `MemoryDocLibSqlStore` precedent: this adapter exists
//! solely so the composition `PgOrchestratorLookup` (H.12.5 main) can hand the
//! engine Tier-0/Tier-1 path a real [`Thread`] with full-fidelity
//! `goal` / `id` / `tenant` / `agent` / `project` / `metadata` loaded from the
//! same rows the agent loop persists.
//!
//! Decision (user-locked 2026-09-02): Q-H12-5-THREAD = C (build a real
//! PG-backed engine `Store::load_thread`, not a thin scope-only shim);
//! Q-H12-5-STORE = C2 (wrap `SessionThreadService::read_thread`, reusing the
//! snapshot parsing and respecting `brassclaw_threads`' table ownership,
//! rather than direct `SELECT metadata` + re-parse of the private
//! `ThreadSnapshot`).
//!
//! `dead_code` is allowed module-wide: the adapter is only **constructed**
//! under the `skills-db` feature — the H.12.5 main `PgOrchestratorLookup`
//! wiring is `#[cfg(feature = "skills-db")]`-gated — so under the default
//! feature set this type is defined-but-unused. The type itself is
//! feature-agnostic (it touches only always-available `brassclaw_engine` core
//! types + `brassclaw_threads` contracts) and its unit tests run under both
//! configs, mirroring `orchestrator_effect_executor.rs`.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::capability::{CapabilityLease, LeaseId};
use brassclaw_engine::types::error::EngineError;
use brassclaw_engine::types::event::ThreadEvent;
use brassclaw_engine::types::memory::{DocId, MemoryDoc};
use brassclaw_engine::types::project::{Project, ProjectId};
use brassclaw_engine::types::step::Step;
use brassclaw_engine::types::thread::{
    Thread, ThreadConfig, ThreadId as EngineThreadId, ThreadState, ThreadType,
};
use brassclaw_host_api::{AgentId, SYSTEM_RESERVED_ID, TenantId, ThreadId as HostThreadId};
use brassclaw_threads::{
    SessionThreadError, SessionThreadRecord, SessionThreadService, ThreadHistoryRequest,
    ThreadScope,
};
use chrono::Utc;
use uuid::Uuid;

fn stub(method: &'static str) -> EngineError {
    EngineError::Store {
        reason: format!(
            "{method} is not implemented (PgThreadEngineStore — load_thread-only adapter)"
        ),
    }
}

/// Postgres-backed engine [`Store`] that loads [`Thread`] rows from
/// `brassclaw_session_threads` via [`SessionThreadService::read_thread`].
///
/// Holds the live thread service + the tenant scope to read under. The
/// `agent_id` is a `"default"` sentinel because `read_thread`'s query keys on
/// `id` + `tenant_id` only (the agent is not part of the lookup).
pub(crate) struct PgThreadEngineStore {
    thread_service: Arc<dyn SessionThreadService>,
    tenant_id: String,
}

impl PgThreadEngineStore {
    pub(crate) fn new(
        thread_service: Arc<dyn SessionThreadService>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            thread_service,
            tenant_id: tenant_id.into(),
        }
    }

    /// Pure mapping from a [`SessionThreadRecord`] (loop/threads contract) to
    /// an engine [`Thread`]. Stateless and synchronous so it can be unit-tested
    /// directly without a `SessionThreadService` backend.
    fn map_record(record: &SessionThreadRecord) -> Thread {
        let id = Uuid::parse_str(record.thread_id.as_str())
            .map(EngineThreadId)
            .unwrap_or_else(|_| EngineThreadId::new());
        let project_id = record
            .scope
            .project_id
            .as_ref()
            .and_then(|p| Uuid::parse_str(p.as_str()).ok())
            .map(ProjectId)
            .unwrap_or_default();
        // `user_id` prefers the explicit thread owner, then the creating actor,
        // then the system sentinel — never empty.
        let user_id = record
            .scope
            .owner_user_id
            .as_ref()
            .map(|u| u.as_str().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                let actor = record.created_by_actor_id.clone();
                (!actor.is_empty()).then_some(actor)
            })
            .unwrap_or_else(|| SYSTEM_RESERVED_ID.to_string());
        let metadata = record
            .metadata_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Null);
        let now = Utc::now();
        Thread {
            id,
            goal: record
                .goal
                .as_ref()
                .map(|g| g.statement.as_str().to_string())
                .unwrap_or_default(),
            title: record.title.clone(),
            thread_type: ThreadType::Foreground,
            state: ThreadState::Created,
            project_id,
            user_id,
            tenant_id: record.scope.tenant_id.as_str().to_string(),
            agent_id: record.scope.agent_id.as_str().to_string(),
            parent_id: None,
            config: ThreadConfig::default(),
            messages: Vec::new(),
            internal_messages: Vec::new(),
            events: Vec::new(),
            capability_leases: Vec::new(),
            metadata,
            created_at: now,
            updated_at: now,
            completed_at: None,
            step_count: 0,
            total_tokens_used: 0,
            total_cost_usd: 0.0,
        }
    }
}

#[async_trait]
impl Store for PgThreadEngineStore {
    async fn load_thread(&self, id: EngineThreadId) -> Result<Option<Thread>, EngineError> {
        // Engine `ThreadId(pub Uuid)` -> loop/threads `ThreadId(String)`. Loop
        // thread ids are stored as raw Uuid strings, so `from_trusted` round
        // trips the canonical textual form.
        let host_thread_id = HostThreadId::from_trusted(id.0.to_string());
        let scope = ThreadScope {
            tenant_id: TenantId::from_trusted(self.tenant_id.clone()),
            agent_id: AgentId::from_trusted("default".to_string()),
            project_id: None,
            owner_user_id: None,
        };
        let request = ThreadHistoryRequest {
            scope,
            thread_id: host_thread_id,
        };
        match self.thread_service.read_thread(request).await {
            Ok(record) => Ok(Some(Self::map_record(&record))),
            // `read_thread` returns `UnknownThread` for both "does not exist"
            // and "exists but owned by a different scope" (ownership-probe
            // semantics) — both map to `Ok(None)` so the caller degrades
            // gracefully rather than aborting the turn.
            Err(SessionThreadError::UnknownThread { .. }) => Ok(None),
            Err(error) => Err(EngineError::Store {
                reason: error.to_string(),
            }),
        }
    }

    // ── Non-Thread methods return stub errors (same contract as
    //    PgMemoryDocStore / MemoryDocLibSqlStore) ──────────────────────────

    async fn save_thread(&self, _: &Thread) -> Result<(), EngineError> {
        Err(stub("save_thread"))
    }
    async fn list_threads(&self, _: ProjectId, _: &str) -> Result<Vec<Thread>, EngineError> {
        Err(stub("list_threads"))
    }
    async fn update_thread_state(
        &self,
        _: EngineThreadId,
        _: ThreadState,
    ) -> Result<(), EngineError> {
        Err(stub("update_thread_state"))
    }
    async fn save_step(&self, _: &Step) -> Result<(), EngineError> {
        Err(stub("save_step"))
    }
    async fn load_steps(&self, _: EngineThreadId) -> Result<Vec<Step>, EngineError> {
        Err(stub("load_steps"))
    }
    async fn append_events(&self, _: &[ThreadEvent]) -> Result<(), EngineError> {
        Err(stub("append_events"))
    }
    async fn load_events(&self, _: EngineThreadId) -> Result<Vec<ThreadEvent>, EngineError> {
        Err(stub("load_events"))
    }
    async fn save_project(&self, _: &Project) -> Result<(), EngineError> {
        Err(stub("save_project"))
    }
    async fn load_project(&self, _: ProjectId) -> Result<Option<Project>, EngineError> {
        Err(stub("load_project"))
    }
    async fn save_memory_doc(&self, _: &MemoryDoc) -> Result<(), EngineError> {
        Err(stub("save_memory_doc"))
    }
    async fn load_memory_doc(&self, _: DocId) -> Result<Option<MemoryDoc>, EngineError> {
        Err(stub("load_memory_doc"))
    }
    async fn list_memory_docs(&self, _: ProjectId, _: &str) -> Result<Vec<MemoryDoc>, EngineError> {
        Err(stub("list_memory_docs"))
    }
    async fn save_lease(&self, _: &CapabilityLease) -> Result<(), EngineError> {
        Err(stub("save_lease"))
    }
    async fn load_active_leases(
        &self,
        _: EngineThreadId,
    ) -> Result<Vec<CapabilityLease>, EngineError> {
        Err(stub("load_active_leases"))
    }
    async fn revoke_lease(&self, _: LeaseId, _: &str) -> Result<(), EngineError> {
        Err(stub("revoke_lease"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::types::project::ProjectId as EngineProjectId;
    use brassclaw_engine::types::thread::ThreadId as EngineThreadId;
    use brassclaw_host_api::{
        AgentId as HostAgentId, ProjectId as HostProjectId, TenantId as HostTenantId,
        ThreadId as HostThreadId, UserId as HostUserId,
    };
    use brassclaw_threads::{
        EnsureThreadRequest, GoalStatement, InMemorySessionThreadService, SessionThreadRecord,
        ThreadGoal, ThreadScope,
    };

    fn scope(tenant: &str) -> ThreadScope {
        ThreadScope {
            tenant_id: HostTenantId::from_trusted(tenant.to_string()),
            agent_id: HostAgentId::from_trusted("default".to_string()),
            project_id: Some(HostProjectId::from_trusted(Uuid::new_v4().to_string())),
            owner_user_id: Some(HostUserId::from_trusted("user-7".to_string())),
        }
    }

    #[test]
    fn map_record_carries_full_fidelity_fields() {
        let thread_id = HostThreadId::from_trusted(Uuid::new_v4().to_string());
        let scope = scope("tenant-acme");
        let record = SessionThreadRecord {
            scope: scope.clone(),
            thread_id: thread_id.clone(),
            created_by_actor_id: "actor-1".to_string(),
            title: Some("deploy the service".to_string()),
            metadata_json: Some(serde_json::json!({"source_channel": "webui"}).to_string()),
            goal: Some(ThreadGoal {
                statement: GoalStatement::new("Ship v3 to staging").expect("valid goal"),
                refined_at_sequence: 3,
                refinement_count: 1,
            }),
        };

        let thread = PgThreadEngineStore::map_record(&record);

        assert_eq!(thread.id.0, Uuid::parse_str(thread_id.as_str()).unwrap());
        assert_eq!(thread.goal, "Ship v3 to staging");
        assert_eq!(thread.title.as_deref(), Some("deploy the service"));
        assert_eq!(thread.thread_type, ThreadType::Foreground);
        assert_eq!(thread.state, ThreadState::Created);
        assert_eq!(
            thread.project_id.0,
            Uuid::parse_str(scope.project_id.as_ref().unwrap().as_str()).unwrap()
        );
        assert_eq!(thread.user_id, "user-7");
        assert_eq!(thread.tenant_id, "tenant-acme");
        assert_eq!(thread.agent_id, "default");
        assert_eq!(thread.metadata["source_channel"], "webui");
        assert!(thread.messages.is_empty());
        assert!(thread.events.is_empty());
        assert_eq!(thread.step_count, 0);
    }

    #[test]
    fn map_record_falls_back_for_missing_identity() {
        let thread_id = HostThreadId::from_trusted(Uuid::new_v4().to_string());
        let scope = ThreadScope {
            tenant_id: HostTenantId::from_trusted("tenant-solo".to_string()),
            agent_id: HostAgentId::from_trusted("default".to_string()),
            project_id: None,
            owner_user_id: None,
        };
        let record = SessionThreadRecord {
            scope: scope.clone(),
            thread_id: thread_id.clone(),
            created_by_actor_id: String::new(),
            title: None,
            metadata_json: None,
            goal: None,
        };

        let thread = PgThreadEngineStore::map_record(&record);

        assert_eq!(thread.goal, "");
        assert!(thread.title.is_none());
        assert_eq!(thread.user_id, SYSTEM_RESERVED_ID);
        // `ProjectId::default()` mints a fresh random Uuid, so assert non-nil
        // rather than equality against another `default()`.
        assert_ne!(thread.project_id.0, Uuid::nil());
        assert_eq!(thread.metadata, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn load_thread_round_trips_through_in_memory_service() {
        let service =
            Arc::new(InMemorySessionThreadService::default()) as Arc<dyn SessionThreadService>;
        let store = PgThreadEngineStore::new(Arc::clone(&service), "tenant-acme");

        let thread_id = HostThreadId::from_trusted(Uuid::new_v4().to_string());
        // The in-memory backend enforces exact-scope ownership on `read_thread`
        // (production PG keys on id+tenant_id only), so ensure the thread under
        // the SAME scope shape `load_thread` will issue: tenant + "default"
        // agent, no project/owner. The creating actor becomes the user fallback.
        let ensure_scope = ThreadScope {
            tenant_id: HostTenantId::from_trusted("tenant-acme".to_string()),
            agent_id: HostAgentId::from_trusted("default".to_string()),
            project_id: None,
            owner_user_id: None,
        };
        let ensured = service
            .ensure_thread(EnsureThreadRequest {
                scope: ensure_scope,
                thread_id: Some(thread_id.clone()),
                created_by_actor_id: "user-7".to_string(),
                title: Some("research task".to_string()),
                metadata_json: Some(serde_json::json!({"conversation_scope": "abc"}).to_string()),
            })
            .await
            .expect("ensure_thread");

        // The engine id is the parsed Uuid of the stored thread id.
        let engine_id = EngineThreadId(Uuid::parse_str(thread_id.as_str()).unwrap());
        let loaded = store.load_thread(engine_id).await.expect("load_thread");

        let thread = loaded.expect("thread present");
        assert_eq!(thread.id.0, Uuid::parse_str(thread_id.as_str()).unwrap());
        assert_eq!(thread.tenant_id, "tenant-acme");
        assert_eq!(thread.agent_id, "default");
        assert_eq!(thread.user_id, "user-7");
        assert_eq!(thread.title.as_deref(), Some("research task"));
        assert_eq!(thread.metadata["conversation_scope"], "abc");
        assert_ne!(thread.project_id.0, Uuid::nil());
        // ensure_thread does not set a goal; loader surfaces the empty default.
        assert_eq!(thread.goal, "");
        let _ = ensured;
    }

    #[tokio::test]
    async fn load_thread_missing_returns_none() {
        let service =
            Arc::new(InMemorySessionThreadService::default()) as Arc<dyn SessionThreadService>;
        let store = PgThreadEngineStore::new(service, "tenant-acme");

        let loaded = store
            .load_thread(EngineThreadId::new())
            .await
            .expect("load_thread should not hard-error on missing");
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn load_thread_cross_tenant_returns_none() {
        let service =
            Arc::new(InMemorySessionThreadService::default()) as Arc<dyn SessionThreadService>;
        // Thread is ensured under tenant-acme; loader reads under tenant-other.
        let ensure_service = Arc::clone(&service);
        let thread_id = HostThreadId::from_trusted(Uuid::new_v4().to_string());
        ensure_service
            .ensure_thread(EnsureThreadRequest {
                scope: scope("tenant-acme"),
                thread_id: Some(thread_id.clone()),
                created_by_actor_id: "actor-1".to_string(),
                title: None,
                metadata_json: None,
            })
            .await
            .expect("ensure_thread");

        let store = PgThreadEngineStore::new(service, "tenant-other");
        let engine_id = EngineThreadId(Uuid::parse_str(thread_id.as_str()).unwrap());
        let loaded = store.load_thread(engine_id).await.expect("no hard error");
        assert!(loaded.is_none(), "cross-tenant read must degrade to None");
    }

    #[tokio::test]
    async fn non_load_thread_methods_stub() {
        let service =
            Arc::new(InMemorySessionThreadService::default()) as Arc<dyn SessionThreadService>;
        let store = PgThreadEngineStore::new(service, "tenant-acme");

        let thread = Thread::new(
            "goal",
            ThreadType::Foreground,
            EngineProjectId::default(),
            "u",
            ThreadConfig::default(),
        );
        assert!(store.save_thread(&thread).await.is_err());
        assert!(
            store
                .list_threads(EngineProjectId::default(), "u")
                .await
                .is_err()
        );
        assert!(store.load_steps(EngineThreadId::new()).await.is_err());
        assert!(store.load_events(EngineThreadId::new()).await.is_err());
        assert!(
            store
                .load_project(EngineProjectId::default())
                .await
                .is_err()
        );
        assert!(store.revoke_lease(LeaseId::default(), "x").await.is_err());
    }
}
