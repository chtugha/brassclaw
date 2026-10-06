//! Task projections shared by durable and local external-auth adapters.
//! The host supplies the legacy scope as an exact task-selection key. This
//! adapter does not establish WebUI roles or grant tool permission.

use async_trait::async_trait;
use brassclaw_turns::{TurnError, TurnPersistenceSnapshot, TurnScope};

#[async_trait]
pub(super) trait RunStateReadSource: Send + Sync {
    async fn snapshot_for_scope(
        &self,
        scope: &TurnScope,
    ) -> Result<TurnPersistenceSnapshot, TurnError>;
}

#[async_trait]
impl RunStateReadSource for crate::factory::LocalDevTurnStateStore {
    async fn snapshot_for_scope(
        &self,
        scope: &TurnScope,
    ) -> Result<TurnPersistenceSnapshot, TurnError> {
        let mut snapshot = self.persistence_snapshot();
        snapshot.runs.retain(|run| run.scope == *scope);
        snapshot.turns.retain(|turn| turn.scope == *scope);
        let visible: std::collections::HashSet<_> =
            snapshot.runs.iter().map(|run| run.run_id).collect();
        snapshot.checkpoints.retain(|checkpoint| {
            visible.contains(&checkpoint.run_id)
                && checkpoint
                    .scope
                    .as_ref()
                    .is_none_or(|stored| stored == scope)
        });
        Ok(TurnPersistenceSnapshot {
            turns: snapshot.turns,
            runs: snapshot.runs,
            checkpoints: snapshot.checkpoints,
            ..TurnPersistenceSnapshot::default()
        })
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl RunStateReadSource for brassclaw_turns::PgTurnStateStore {
    async fn snapshot_for_scope(
        &self,
        scope: &TurnScope,
    ) -> Result<TurnPersistenceSnapshot, TurnError> {
        self.interaction_snapshot(scope).await
    }
}
