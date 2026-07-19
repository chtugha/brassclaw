use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use brassclaw_turns::{TurnRunId, TurnScope};
use serde::{Deserialize, Serialize};

pub const MAX_GOAL_ENTRIES: usize = 4096;
pub const MAX_GOAL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentGoal {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<String>,
}

impl SubagentGoal {
    fn byte_len(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubagentGoalStoreError {
    #[error("subagent goal for run {run_id} not found")]
    NotFound { run_id: TurnRunId },
    #[error("subagent goal payload too large: {bytes} bytes (max {max})")]
    PayloadTooLarge { bytes: usize, max: usize },
    #[error("subagent goal for run {run_id} already stored")]
    DuplicateKey { run_id: TurnRunId },
    #[error("subagent goal store backend failed: {reason}")]
    Backend { reason: String },
}

#[async_trait]
pub trait SubagentGoalStore: Send + Sync {
    async fn put_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: SubagentGoal,
    ) -> Result<(), SubagentGoalStoreError>;

    async fn get_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<SubagentGoal, SubagentGoalStoreError>;

    async fn delete_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), SubagentGoalStoreError>;
}

#[derive(Default)]
pub struct InMemoryBoundedSubagentGoalStore {
    inner: Mutex<GoalStoreInner>,
}

#[derive(Default)]
struct GoalStoreInner {
    goals: HashMap<GoalKey, SubagentGoal>,
    insertion_order: VecDeque<GoalKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GoalKey {
    scope: TurnScope,
    run_id: TurnRunId,
}

impl GoalKey {
    fn new(scope: &TurnScope, run_id: TurnRunId) -> Self {
        Self {
            scope: scope.clone(),
            run_id,
        }
    }
}

impl InMemoryBoundedSubagentGoalStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: SubagentGoal,
    ) -> Result<(), SubagentGoalStoreError> {
        let bytes = goal.byte_len();
        if bytes > MAX_GOAL_BYTES {
            return Err(SubagentGoalStoreError::PayloadTooLarge {
                bytes,
                max: MAX_GOAL_BYTES,
            });
        }
        let mut inner = lock(&self.inner);
        let key = GoalKey::new(scope, run_id);
        if inner.goals.contains_key(&key) {
            return Err(SubagentGoalStoreError::DuplicateKey { run_id });
        }
        if inner.goals.len() >= MAX_GOAL_ENTRIES {
            while let Some(oldest) = inner.insertion_order.pop_front() {
                if inner.goals.remove(&oldest).is_some() {
                    tracing::debug!(
                        evicted_run_id = %oldest.run_id,
                        "subagent goal store at capacity; evicted oldest goal"
                    );
                    break;
                }
            }
        }
        inner.goals.insert(key.clone(), goal);
        inner.insertion_order.push_back(key);
        Ok(())
    }

    pub fn get(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<SubagentGoal, SubagentGoalStoreError> {
        let inner = lock(&self.inner);
        inner
            .goals
            .get(&GoalKey::new(scope, run_id))
            .cloned()
            .ok_or(SubagentGoalStoreError::NotFound { run_id })
    }

    fn delete_inner(&self, scope: &TurnScope, run_id: TurnRunId) {
        let mut inner = lock(&self.inner);
        let key = GoalKey::new(scope, run_id);
        inner.goals.remove(&key);
        inner.insertion_order.retain(|queued| *queued != key);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        lock(&self.inner).goals.len()
    }

    #[cfg(test)]
    fn insertion_order_len(&self) -> usize {
        lock(&self.inner).insertion_order.len()
    }
}

#[async_trait]
impl SubagentGoalStore for InMemoryBoundedSubagentGoalStore {
    async fn put_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: SubagentGoal,
    ) -> Result<(), SubagentGoalStoreError> {
        self.put(scope, run_id, goal)
    }

    async fn get_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<SubagentGoal, SubagentGoalStoreError> {
        self.get(scope, run_id)
    }

    async fn delete_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), SubagentGoalStoreError> {
        self.delete_inner(scope, run_id);
        Ok(())
    }
}

#[async_trait]
impl brassclaw_loop_support::SubagentSpawnGoalStore for InMemoryBoundedSubagentGoalStore {
    async fn put_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: brassclaw_loop_support::SubagentGoalRecord,
    ) -> Result<(), brassclaw_turns::run_profile::AgentLoopHostError> {
        self.put(
            scope,
            run_id,
            SubagentGoal {
                task: goal.task,
                handoff: goal.handoff,
            },
        )
        .map_err(map_goal_error)
    }

    async fn delete_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), brassclaw_turns::run_profile::AgentLoopHostError> {
        self.delete_inner(scope, run_id);
        Ok(())
    }
}

fn map_goal_error(
    error: SubagentGoalStoreError,
) -> brassclaw_turns::run_profile::AgentLoopHostError {
    let kind = match error {
        SubagentGoalStoreError::NotFound { .. } => {
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::InvalidInvocation
        }
        SubagentGoalStoreError::PayloadTooLarge { .. } => {
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::BudgetExceeded
        }
        SubagentGoalStoreError::DuplicateKey { .. } => {
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::InvalidInvocation
        }
        SubagentGoalStoreError::Backend { .. } => {
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::Unavailable
        }
    };
    brassclaw_turns::run_profile::AgentLoopHostError::new(kind, error.to_string())
}

fn lock(inner: &Mutex<GoalStoreInner>) -> MutexGuard<'_, GoalStoreInner> {
    match inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

// ── PgSubagentGoalStore ───────────────────────────────────────────────────

/// PostgreSQL-backed subagent goal store.
///
/// One row per `(tenant_id, run_id)`. Table created by V024.
pub struct PgSubagentGoalStore {
    pool: deadpool_postgres::Pool,
}

impl PgSubagentGoalStore {
    pub fn new(pool: deadpool_postgres::Pool) -> Self {
        Self { pool }
    }

    async fn connect(
        &self,
    ) -> Result<deadpool_postgres::Object, SubagentGoalStoreError> {
        self.pool.get().await.map_err(|error| SubagentGoalStoreError::Backend {
            reason: format!("pg subagent goal store connect: {error}"),
        })
    }

    fn validate(goal: &SubagentGoal) -> Result<(), SubagentGoalStoreError> {
        let bytes = goal.byte_len();
        if bytes > MAX_GOAL_BYTES {
            return Err(SubagentGoalStoreError::PayloadTooLarge {
                bytes,
                max: MAX_GOAL_BYTES,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl SubagentGoalStore for PgSubagentGoalStore {
    async fn put_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: SubagentGoal,
    ) -> Result<(), SubagentGoalStoreError> {
        Self::validate(&goal)?;
        let run_id_str = run_id.to_string();
        let id = format!("{}:{}", scope.tenant_id.as_str(), run_id_str);
        let client = self.connect().await?;
        client
            .execute(
                "INSERT INTO brassclaw_subagent_goals \
                     (id, tenant_id, run_id, task, handoff) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT (id) DO NOTHING",
                &[
                    &id,
                    &scope.tenant_id.as_str(),
                    &run_id_str,
                    &goal.task.as_str(),
                    &goal.handoff.as_deref(),
                ],
            )
            .await
            .map_err(|error| SubagentGoalStoreError::Backend {
                reason: format!("pg subagent goal put: {error}"),
            })?;
        Ok(())
    }

    async fn get_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<SubagentGoal, SubagentGoalStoreError> {
        let run_id_str = run_id.to_string();
        let client = self.connect().await?;
        let row = client
            .query_opt(
                "SELECT task, handoff \
                 FROM brassclaw_subagent_goals \
                 WHERE tenant_id = $1 AND run_id = $2 \
                 LIMIT 1",
                &[&scope.tenant_id.as_str(), &run_id_str],
            )
            .await
            .map_err(|error| SubagentGoalStoreError::Backend {
                reason: format!("pg subagent goal get: {error}"),
            })?;
        let Some(row) = row else {
            return Err(SubagentGoalStoreError::NotFound { run_id });
        };
        let task: String = row.try_get("task").map_err(|error| SubagentGoalStoreError::Backend {
            reason: format!("pg subagent goal read task: {error}"),
        })?;
        let handoff: Option<String> =
            row.try_get("handoff").map_err(|error| SubagentGoalStoreError::Backend {
                reason: format!("pg subagent goal read handoff: {error}"),
            })?;
        Ok(SubagentGoal { task, handoff })
    }

    async fn delete_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), SubagentGoalStoreError> {
        let run_id_str = run_id.to_string();
        let client = self.connect().await?;
        client
            .execute(
                "DELETE FROM brassclaw_subagent_goals \
                 WHERE tenant_id = $1 AND run_id = $2",
                &[&scope.tenant_id.as_str(), &run_id_str],
            )
            .await
            .map_err(|error| SubagentGoalStoreError::Backend {
                reason: format!("pg subagent goal delete: {error}"),
            })?;
        Ok(())
    }
}

#[async_trait]
impl brassclaw_loop_support::SubagentSpawnGoalStore for PgSubagentGoalStore {
    async fn put_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        goal: brassclaw_loop_support::SubagentGoalRecord,
    ) -> Result<(), brassclaw_turns::run_profile::AgentLoopHostError> {
        SubagentGoalStore::put_goal(
            self,
            scope,
            run_id,
            SubagentGoal { task: goal.task, handoff: goal.handoff },
        )
        .await
        .map_err(map_goal_error)
    }

    async fn delete_goal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), brassclaw_turns::run_profile::AgentLoopHostError> {
        SubagentGoalStore::delete_goal(self, scope, run_id)
            .await
            .map_err(map_goal_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "filesystem-goal-store")]
    use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId};
    #[cfg(feature = "filesystem-goal-store")]
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};

    fn scope(thread_id: &str) -> TurnScope {
        TurnScope::new(
            TenantId::new("tenant-alpha").unwrap(),
            Some(AgentId::new("agent-alpha").unwrap()),
            Some(ProjectId::new("project-alpha").unwrap()),
            ThreadId::new(thread_id).unwrap(),
        )
    }

    fn goal(task: &str) -> SubagentGoal {
        SubagentGoal {
            task: task.to_string(),
            handoff: None,
        }
    }

    #[tokio::test]
    async fn put_then_get_round_trips() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();
        let expected = SubagentGoal {
            task: "summarize this".to_string(),
            handoff: Some("context".to_string()),
        };

        store
            .put_goal(&owner_scope, run_id, expected.clone())
            .await
            .unwrap();

        assert_eq!(
            store.get_goal(&owner_scope, run_id).await.unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn get_miss_is_not_found_error() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();

        assert_eq!(
            store.get_goal(&owner_scope, run_id).await.unwrap_err(),
            SubagentGoalStoreError::NotFound { run_id }
        );
    }

    #[tokio::test]
    async fn put_rejects_oversized_payload() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();
        let large = SubagentGoal {
            task: "x".repeat(MAX_GOAL_BYTES + 1),
            handoff: None,
        };

        assert!(matches!(
            store.put_goal(&owner_scope, run_id, large).await,
            Err(SubagentGoalStoreError::PayloadTooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn put_rejects_payload_when_json_overhead_exceeds_limit() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();
        let large = SubagentGoal {
            task: "x".repeat(MAX_GOAL_BYTES - 8),
            handoff: None,
        };

        assert!(
            large.task.len() <= MAX_GOAL_BYTES,
            "raw string payload stays below the limit"
        );
        assert!(matches!(
            store.put_goal(&owner_scope, run_id, large).await,
            Err(SubagentGoalStoreError::PayloadTooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn put_rejects_duplicate_key() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();

        store
            .put_goal(&owner_scope, run_id, goal("first"))
            .await
            .unwrap();

        assert_eq!(
            store
                .put_goal(&owner_scope, run_id, goal("second"))
                .await
                .unwrap_err(),
            SubagentGoalStoreError::DuplicateKey { run_id }
        );
    }

    #[tokio::test]
    async fn bounded_store_evicts_oldest() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let first = TurnRunId::new();
        let second = TurnRunId::new();
        store
            .put_goal(&owner_scope, first, goal("first"))
            .await
            .unwrap();
        store
            .put_goal(&owner_scope, second, goal("second"))
            .await
            .unwrap();
        for index in 2..=MAX_GOAL_ENTRIES {
            store
                .put_goal(
                    &owner_scope,
                    TurnRunId::new(),
                    goal(&format!("goal-{index}")),
                )
                .await
                .unwrap();
        }

        assert!(matches!(
            store.get_goal(&owner_scope, first).await,
            Err(SubagentGoalStoreError::NotFound { .. })
        ));
        assert_eq!(
            store.get_goal(&owner_scope, second).await.unwrap(),
            goal("second")
        );
        assert_eq!(store.len(), MAX_GOAL_ENTRIES);
    }

    #[tokio::test]
    async fn delete_goal_is_idempotent_and_removes_row() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();

        store
            .put_goal(&owner_scope, run_id, goal("task"))
            .await
            .unwrap();
        store.delete_goal(&owner_scope, run_id).await.unwrap();
        store.delete_goal(&owner_scope, run_id).await.unwrap();

        assert!(matches!(
            store.get_goal(&owner_scope, run_id).await,
            Err(SubagentGoalStoreError::NotFound { .. })
        ));
        assert_eq!(store.insertion_order_len(), 0);
    }

    #[tokio::test]
    async fn bounded_store_keys_goals_by_scope_and_run_id() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let first_scope = scope("thread-goal-a");
        let second_scope = scope("thread-goal-b");
        let run_id = TurnRunId::new();

        store
            .put_goal(&first_scope, run_id, goal("scoped task"))
            .await
            .unwrap();
        assert!(matches!(
            store.get_goal(&second_scope, run_id).await,
            Err(SubagentGoalStoreError::NotFound { .. })
        ));

        store.delete_goal(&second_scope, run_id).await.unwrap();

        assert_eq!(
            store.get_goal(&first_scope, run_id).await.unwrap(),
            goal("scoped task")
        );
    }

    #[cfg(feature = "filesystem-goal-store")]
    fn scoped_goal_filesystem() -> Arc<ScopedFilesystem<InMemoryBackend>> {
        let mounts = MountView::new(vec![MountGrant::new(
            MountAlias::new("/turns").unwrap(),
            VirtualPath::new("/turns").unwrap(),
            MountPermissions::read_write_list_delete(),
        )])
        .unwrap();
        Arc::new(ScopedFilesystem::with_fixed_view(
            Arc::new(InMemoryBackend::new()),
            mounts,
        ))
    }

    #[cfg(feature = "filesystem-goal-store")]
    async fn assert_goal_store_contract(store: &dyn SubagentGoalStore) {
        let owner_scope = scope("thread-goal");
        let other_scope = scope("thread-goal-other");
        let run_id = TurnRunId::new();
        let expected = SubagentGoal {
            task: "durable task".to_string(),
            handoff: Some("handoff".to_string()),
        };

        store
            .put_goal(&owner_scope, run_id, expected.clone())
            .await
            .unwrap();
        assert_eq!(
            store.get_goal(&owner_scope, run_id).await.unwrap(),
            expected
        );
        assert!(matches!(
            store.get_goal(&other_scope, run_id).await,
            Err(SubagentGoalStoreError::NotFound { .. })
        ));
        store.delete_goal(&other_scope, run_id).await.unwrap();
        assert!(store.get_goal(&owner_scope, run_id).await.is_ok());
        assert_eq!(
            store
                .put_goal(&owner_scope, run_id, goal("duplicate"))
                .await
                .unwrap_err(),
            SubagentGoalStoreError::DuplicateKey { run_id }
        );
        assert!(matches!(
            store
                .put_goal(
                    &owner_scope,
                    TurnRunId::new(),
                    SubagentGoal {
                        task: "x".repeat(MAX_GOAL_BYTES + 1),
                        handoff: None,
                    },
                )
                .await,
            Err(SubagentGoalStoreError::PayloadTooLarge { .. })
        ));
        store.delete_goal(&owner_scope, run_id).await.unwrap();
        store.delete_goal(&owner_scope, run_id).await.unwrap();
        assert!(matches!(
            store.get_goal(&owner_scope, run_id).await,
            Err(SubagentGoalStoreError::NotFound { .. })
        ));
    }

    #[cfg(feature = "filesystem-goal-store")]
    #[tokio::test]
    async fn filesystem_goal_store_satisfies_subagent_goal_contract() {
        let store = FilesystemSubagentGoalStore::new(scoped_goal_filesystem());
        assert_goal_store_contract(&store).await;
    }

    #[cfg(feature = "filesystem-goal-store")]
    #[test]
    fn filesystem_goal_path_uses_alias_relative_named_scope_axes() {
        let owner_scope = scope("thread-goal-path");
        let run_id = TurnRunId::new();

        let path = goal_path(&owner_scope, run_id).unwrap();

        assert_eq!(
            path.as_str(),
            format!(
                "/turns/subagent-goals/agents/agent-alpha/projects/project-alpha/threads/thread-goal-path/{}.json",
                run_id.as_uuid()
            )
        );
        assert!(
            !path.as_str().contains("tenant-alpha"),
            "resource scope already supplies tenant isolation"
        );
    }

    #[cfg(feature = "filesystem-goal-store")]
    #[tokio::test]
    async fn filesystem_goal_store_reopens_over_same_backend() {
        let filesystem = scoped_goal_filesystem();
        let first = FilesystemSubagentGoalStore::new(Arc::clone(&filesystem));
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();
        let expected = goal("survives reopen");

        first
            .put_goal(&owner_scope, run_id, expected.clone())
            .await
            .unwrap();
        let reopened = FilesystemSubagentGoalStore::new(filesystem);

        assert_eq!(
            reopened.get_goal(&owner_scope, run_id).await.unwrap(),
            expected
        );
    }

    #[test]
    fn goal_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<dyn SubagentGoalStore>>();
        assert_send_sync::<InMemoryBoundedSubagentGoalStore>();
        #[cfg(feature = "filesystem-goal-store")]
        assert_send_sync::<FilesystemSubagentGoalStore<InMemoryBackend>>();
    }
}

