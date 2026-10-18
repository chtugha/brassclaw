use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use brassclaw_turns::{TurnRunId, TurnScope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentGoal {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubagentGoalStoreError {
    #[error("subagent goal for run {run_id} not found")]
    NotFound { run_id: TurnRunId },
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

/// Retain every accepted goal until its owner explicitly deletes it. The
/// historical type name does not impose a second count or payload quota;
/// admission/execution bounds belong to the caller. Never evict a live goal.
#[derive(Default)]
pub struct InMemoryBoundedSubagentGoalStore {
    inner: Mutex<GoalStoreInner>,
}

#[derive(Default)]
struct GoalStoreInner {
    goals: HashMap<GoalKey, SubagentGoal>,
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
        let mut inner = lock(&self.inner)?;
        let key = GoalKey::new(scope, run_id);
        if inner.goals.contains_key(&key) {
            return Err(SubagentGoalStoreError::DuplicateKey { run_id });
        }
        inner
            .goals
            .try_reserve(1)
            .map_err(|_| SubagentGoalStoreError::Backend {
                reason: "subagent goal storage capacity unavailable".into(),
            })?;
        inner.goals.insert(key, goal);
        Ok(())
    }

    pub fn get(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<SubagentGoal, SubagentGoalStoreError> {
        let inner = lock(&self.inner)?;
        inner
            .goals
            .get(&GoalKey::new(scope, run_id))
            .cloned()
            .ok_or(SubagentGoalStoreError::NotFound { run_id })
    }

    fn delete_inner(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), SubagentGoalStoreError> {
        let mut inner = lock(&self.inner)?;
        inner.goals.remove(&GoalKey::new(scope, run_id));
        if inner.goals.is_empty() {
            *inner = GoalStoreInner::default();
        }
        Ok(())
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        lock(&self.inner).unwrap().goals.len()
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
        self.delete_inner(scope, run_id)
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
        self.delete_inner(scope, run_id).map_err(map_goal_error)
    }
}

fn map_goal_error(
    error: SubagentGoalStoreError,
) -> brassclaw_turns::run_profile::AgentLoopHostError {
    let kind = match error {
        SubagentGoalStoreError::NotFound { .. } => {
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::InvalidInvocation
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

fn lock(
    inner: &Mutex<GoalStoreInner>,
) -> Result<MutexGuard<'_, GoalStoreInner>, SubagentGoalStoreError> {
    inner.lock().map_err(|_| SubagentGoalStoreError::Backend {
        reason: "subagent goal storage state unavailable".into(),
    })
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

    async fn connect(&self) -> Result<deadpool_postgres::Object, SubagentGoalStoreError> {
        self.pool
            .get()
            .await
            .map_err(|error| SubagentGoalStoreError::Backend {
                reason: format!("pg subagent goal store connect: {error}"),
            })
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
        let run_id_str = run_id.to_string();
        // Surrogate ULID primary key — unique per row, not derived from run_id.
        let id = ulid::Ulid::new().to_string();
        let client = self.connect().await?;
        // Conflict on the (tenant_id, run_id) unique constraint, not on the
        // surrogate PK (id).  Return DuplicateKey when 0 rows are affected so
        // the caller knows a goal for this run already exists.
        let rows = client
            .execute(
                "INSERT INTO brassclaw_subagent_goals \
                     (id, tenant_id, run_id, task, handoff) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT ON CONSTRAINT brassclaw_subagent_goals_tenant_run_unique \
                     DO NOTHING",
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
        if rows == 0 {
            return Err(SubagentGoalStoreError::DuplicateKey { run_id });
        }
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
        let task: String =
            row.try_get("task")
                .map_err(|error| SubagentGoalStoreError::Backend {
                    reason: format!("pg subagent goal read task: {error}"),
                })?;
        let handoff: Option<String> =
            row.try_get("handoff")
                .map_err(|error| SubagentGoalStoreError::Backend {
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
            SubagentGoal {
                task: goal.task,
                handoff: goal.handoff,
            },
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
    use std::sync::Arc;

    use super::*;
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId};

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
    async fn complete_large_escaped_goal_and_handoff_are_retained() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let run_id = TurnRunId::new();
        let expected = SubagentGoal {
            task: "ü\n\"\\{{vars.source}}".repeat(8000),
            handoff: Some("full handoff 🦀\n".repeat(8000)),
        };
        assert!(expected.task.len() > 64 * 1024);
        assert!(expected.handoff.as_ref().unwrap().len() > 64 * 1024);
        store
            .put_goal(&owner_scope, run_id, expected.clone())
            .await
            .unwrap();
        assert_eq!(
            store.get_goal(&owner_scope, run_id).await.unwrap(),
            expected
        );
        store.delete_goal(&owner_scope, run_id).await.unwrap();
        assert_eq!(store.len(), 0);
        assert_eq!(lock(&store.inner).unwrap().goals.capacity(), 0);
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
    async fn crossing_old_count_retains_every_live_goal_until_explicit_deletion() {
        let store = InMemoryBoundedSubagentGoalStore::new();
        let owner_scope = scope("thread-goal");
        let mut ids = Vec::new();
        for index in 0..4097 {
            let id = TurnRunId::new();
            store
                .put_goal(&owner_scope, id, goal(&format!("goal-{index}")))
                .await
                .unwrap();
            ids.push(id);
        }
        assert_eq!(store.len(), ids.len());
        for (index, id) in ids.iter().copied().enumerate() {
            assert_eq!(
                store.get_goal(&owner_scope, id).await.unwrap(),
                goal(&format!("goal-{index}"))
            );
        }
        for id in ids {
            store.delete_goal(&owner_scope, id).await.unwrap();
        }
        assert_eq!(store.len(), 0);
        assert_eq!(lock(&store.inner).unwrap().goals.capacity(), 0);
    }

    #[tokio::test]
    async fn poisoned_storage_rejects_reads_writes_and_deletes() {
        let store = Arc::new(InMemoryBoundedSubagentGoalStore::new());
        let poisoned = store.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = poisoned.inner.lock().unwrap();
                panic!("intentional storage poison");
            })
            .join()
            .is_err()
        );
        let owner_scope = scope("poisoned-goal");
        let id = TurnRunId::new();
        assert!(matches!(
            store.put_goal(&owner_scope, id, goal("private")).await,
            Err(SubagentGoalStoreError::Backend { .. })
        ));
        assert!(matches!(
            store.get_goal(&owner_scope, id).await,
            Err(SubagentGoalStoreError::Backend { .. })
        ));
        assert!(matches!(
            store.delete_goal(&owner_scope, id).await,
            Err(SubagentGoalStoreError::Backend { .. })
        ));
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
        assert_eq!(store.len(), 0);
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

    #[test]
    fn goal_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<dyn SubagentGoalStore>>();
        assert_send_sync::<InMemoryBoundedSubagentGoalStore>();
    }
}
