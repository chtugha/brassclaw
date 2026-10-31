//! Same actual consumer contract against native persistent PostgreSQL.
#[path = "../../brassclaw_filesystem/tests/support/conditional_write_backend.rs"]
mod conditional_backend;
#[path = "../../brassclaw_conversations/tests/support/conditional_write_contract.rs"]
mod conversations;
#[path = "common/native_pg.rs"]
mod native_pg;
#[path = "../../brassclaw_outbound/tests/support/conditional_write_contract.rs"]
mod outbound;
#[path = "../../brassclaw_processes/tests/support/conditional_write_contract.rs"]
mod processes;
#[path = "../../brassclaw_run_state/tests/support/conditional_write_contract.rs"]
mod run_state;
#[path = "../../brassclaw_threads/tests/support/conditional_write_contract.rs"]
mod threads;
#[path = "../../brassclaw_turns/tests/support/conditional_write_contract.rs"]
mod turns;

#[tokio::test]
async fn native_filesystem_consumers_preserve_exact_conditions() {
    let rig = native_pg::NativePostgres::start().await;
    let backend = std::sync::Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        (*rig.pool).clone(),
    ));
    backend.run_migrations().await.unwrap();
    run_state::verify(backend.clone()).await;
    processes::verify(backend.clone()).await;
    conversations::verify(backend.clone()).await;
    outbound::verify(backend.clone()).await;
    threads::verify(backend.clone()).await;
    turns::verify(backend.clone()).await;
}
