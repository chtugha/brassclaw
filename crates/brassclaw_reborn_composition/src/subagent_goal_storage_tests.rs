//! Actual PostgreSQL goal storage: complete data, uniqueness and explicit cleanup.
use brassclaw_host_api::{TenantId, ThreadId};
use brassclaw_reborn::subagent::goal_store::{
    PgSubagentGoalStore, SubagentGoal, SubagentGoalStore, SubagentGoalStoreError,
};
use brassclaw_turns::{TurnRunId, TurnScope};

#[tokio::test]
async fn native_subagent_goal_retains_complete_large_data_and_explicit_deletion() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = PgSubagentGoalStore::new((*rig.pool).clone());
    let scope = TurnScope::new(
        TenantId::new("goal-storage-owner").unwrap(),
        None,
        None,
        ThreadId::new("goal-storage-thread").unwrap(),
    );
    let run = TurnRunId::new();
    let expected = SubagentGoal {
        task: "ü\n\"\\{{vars.source}}".repeat(8000),
        handoff: Some("complete handoff 🦀\n".repeat(8000)),
    };
    assert!(expected.task.len() > 64 * 1024);
    assert!(expected.handoff.as_ref().unwrap().len() > 64 * 1024);
    store.put_goal(&scope, run, expected.clone()).await.unwrap();
    assert_eq!(store.get_goal(&scope, run).await.unwrap(), expected);
    assert!(matches!(
        store.put_goal(&scope, run, SubagentGoal { task: "replacement".into(), handoff: None }).await,
        Err(SubagentGoalStoreError::DuplicateKey { run_id }) if run_id == run
    ));
    assert_eq!(store.get_goal(&scope, run).await.unwrap(), expected);
    // A second native store sees the exact committed data, not a local cache.
    let reopened = PgSubagentGoalStore::new((*rig.pool).clone());
    assert_eq!(reopened.get_goal(&scope, run).await.unwrap(), expected);
    let foreign = TurnScope::new(
        TenantId::new("goal-storage-other").unwrap(),
        None,
        None,
        scope.thread_id.clone(),
    );
    assert!(matches!(
        reopened.get_goal(&foreign, run).await,
        Err(SubagentGoalStoreError::NotFound { .. })
    ));
    reopened.delete_goal(&foreign, run).await.unwrap();
    assert_eq!(reopened.get_goal(&scope, run).await.unwrap(), expected);
    reopened.delete_goal(&scope, run).await.unwrap();
    reopened.delete_goal(&scope, run).await.unwrap();
    assert!(matches!(
        store.get_goal(&scope, run).await,
        Err(SubagentGoalStoreError::NotFound { .. })
    ));
    let row_count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_subagent_goals", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(row_count, 0);
}
