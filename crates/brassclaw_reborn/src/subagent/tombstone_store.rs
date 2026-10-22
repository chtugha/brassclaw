use std::collections::HashMap;

use async_trait::async_trait;
use brassclaw_turns::TurnRunId;

use crate::subagent::spawn_result::SubagentResultTombstone;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TombstoneStoreError {
    #[error("subagent tombstone store backend failed: {reason}")]
    Backend { reason: String },
    #[error("subagent tombstone conflicts with the retained child outcome")]
    Conflict,
}

#[async_trait]
pub trait SubagentResultTombstoneStore: Send + Sync {
    async fn write_tombstone(
        &self,
        tombstone: SubagentResultTombstone,
    ) -> Result<(), TombstoneStoreError>;

    async fn read_tombstone(
        &self,
        child_run_id: TurnRunId,
    ) -> Result<Option<SubagentResultTombstone>, TombstoneStoreError>;

    /// Release one exact retained outcome after its owner has durably handled
    /// it. No age/count eviction is allowed to substitute for that handling.
    /// This method does not itself establish durable acknowledgement evidence.
    async fn delete_tombstone(
        &self,
        expected: &SubagentResultTombstone,
    ) -> Result<(), TombstoneStoreError>;
}

/// Retained child outcomes with explicit deletion. The historical type name
/// remains for callers; records are not evicted by age or count. This in-memory
/// store does not implement durable recovery or its owner acknowledgement.
#[derive(Default)]
pub struct BoundedSubagentResultTombstoneStore {
    inner: std::sync::Mutex<TombstoneInner>,
}

#[derive(Default)]
struct TombstoneInner {
    by_child: HashMap<TurnRunId, SubagentResultTombstone>,
}

impl BoundedSubagentResultTombstoneStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SubagentResultTombstoneStore for BoundedSubagentResultTombstoneStore {
    async fn write_tombstone(
        &self,
        tombstone: SubagentResultTombstone,
    ) -> Result<(), TombstoneStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| TombstoneStoreError::Backend {
                reason: "subagent tombstone store mutex poisoned".to_string(),
            })?;
        let child_run_id = tombstone.child_run_id;
        if let Some(existing) = inner.by_child.get(&child_run_id) {
            return if existing == &tombstone {
                Ok(())
            } else {
                Err(TombstoneStoreError::Conflict)
            };
        }
        inner
            .by_child
            .try_reserve(1)
            .map_err(|_| TombstoneStoreError::Backend {
                reason: "subagent tombstone storage capacity unavailable".into(),
            })?;
        inner.by_child.insert(child_run_id, tombstone);
        Ok(())
    }

    async fn read_tombstone(
        &self,
        child_run_id: TurnRunId,
    ) -> Result<Option<SubagentResultTombstone>, TombstoneStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| TombstoneStoreError::Backend {
                reason: "subagent tombstone store mutex poisoned".to_string(),
            })?;
        Ok(inner.by_child.get(&child_run_id).cloned())
    }

    async fn delete_tombstone(
        &self,
        expected: &SubagentResultTombstone,
    ) -> Result<(), TombstoneStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| TombstoneStoreError::Backend {
                reason: "subagent tombstone store mutex poisoned".into(),
            })?;
        if let Some(existing) = inner.by_child.get(&expected.child_run_id) {
            if existing != expected {
                return Err(TombstoneStoreError::Conflict);
            }
            inner.by_child.remove(&expected.child_run_id);
        }
        if inner.by_child.is_empty() {
            inner.by_child = HashMap::new();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use brassclaw_turns::TurnStatus;

    use crate::subagent::spawn_result::{SubagentResultDisposition, SubagentResultTombstone};

    use super::*;

    #[tokio::test]
    async fn tombstone_store_is_idempotent_by_child_run() {
        let store = BoundedSubagentResultTombstoneStore::new();
        let child_run_id = TurnRunId::new();
        let tombstone = SubagentResultTombstone {
            child_run_id,
            terminal_status: TurnStatus::Cancelled,
            disposition: SubagentResultDisposition::DiscardedByParentCancel,
        };

        store.write_tombstone(tombstone.clone()).await.unwrap();
        store.write_tombstone(tombstone.clone()).await.unwrap();

        assert_eq!(
            store.read_tombstone(child_run_id).await.unwrap(),
            Some(tombstone)
        );
    }

    #[tokio::test]
    async fn tombstones_survive_former_count_until_exact_explicit_deletion() {
        let store = BoundedSubagentResultTombstoneStore::new();
        let mut outcomes = Vec::new();
        for _ in 0..4200 {
            let outcome = tombstone(TurnRunId::new());
            store.write_tombstone(outcome.clone()).await.unwrap();
            outcomes.push(outcome);
        }
        let original = &outcomes[0];
        let mut conflicting = original.clone();
        conflicting.terminal_status = TurnStatus::Failed;
        assert_eq!(
            store
                .write_tombstone(conflicting.clone())
                .await
                .unwrap_err(),
            TombstoneStoreError::Conflict
        );
        assert_eq!(
            store.delete_tombstone(&conflicting).await.unwrap_err(),
            TombstoneStoreError::Conflict
        );
        for outcome in &outcomes {
            assert_eq!(
                store
                    .read_tombstone(outcome.child_run_id)
                    .await
                    .unwrap()
                    .as_ref(),
                Some(outcome)
            );
            store.delete_tombstone(outcome).await.unwrap();
            store.delete_tombstone(outcome).await.unwrap();
            assert!(
                store
                    .read_tombstone(outcome.child_run_id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(store.inner.lock().unwrap().by_child.capacity(), 0);
    }

    #[tokio::test]
    async fn poisoned_tombstone_storage_rejects_read_write_and_delete() {
        let store = std::sync::Arc::new(BoundedSubagentResultTombstoneStore::new());
        let poisoned = store.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = poisoned.inner.lock().unwrap();
                panic!("intentional tombstone store poison");
            })
            .join()
            .is_err()
        );
        let outcome = tombstone(TurnRunId::new());
        assert!(matches!(
            store.write_tombstone(outcome.clone()).await,
            Err(TombstoneStoreError::Backend { .. })
        ));
        assert!(matches!(
            store.read_tombstone(outcome.child_run_id).await,
            Err(TombstoneStoreError::Backend { .. })
        ));
        assert!(matches!(
            store.delete_tombstone(&outcome).await,
            Err(TombstoneStoreError::Backend { .. })
        ));
    }

    fn tombstone(child_run_id: TurnRunId) -> SubagentResultTombstone {
        SubagentResultTombstone {
            child_run_id,
            terminal_status: TurnStatus::Cancelled,
            disposition: SubagentResultDisposition::DiscardedByParentCancel,
        }
    }
}
