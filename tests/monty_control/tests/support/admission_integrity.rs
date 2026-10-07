//! Real migration/legacy-writer probes against an actual admitted run.
//! These never submit work or manufacture service completion.
use brassclaw_pg::PgPool;
use brassclaw_turns::TurnRunId;

pub async fn reject_reserved_rewrite(pool: &PgPool, run: TurnRunId) {
    let client = pool.get().await.unwrap();
    // Reapplication preserves a reservation retained after catalogue failure.
    client.batch_execute(include_str!("../../../../crates/brassclaw_pg/migrations/V107__protect_monty_admission_settlement.sql")).await.unwrap();
    for query in [
        "UPDATE brassclaw_monty_task_admissions SET accepted_message_ref='msg:tampered' WHERE run_id=$1",
        "UPDATE brassclaw_monty_task_admissions SET claim_checksum=decode(repeat('00',32),'hex') WHERE run_id=$1",
        "UPDATE brassclaw_monty_task_admissions SET reserved_at=reserved_at+interval '1 second' WHERE run_id=$1",
        "INSERT INTO brassclaw_monty_task_admissions (run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase,started_at,settled_at,outcome)
         SELECT run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,'settled',clock_timestamp(),clock_timestamp(),'{\"status\":\"completed\",\"reply_ref\":\"msg:tampered\"}'::jsonb FROM brassclaw_monty_task_admissions WHERE run_id=$1 ON CONFLICT DO NOTHING",
    ] {
        assert_eq!(client.execute(query, &[&run.as_uuid()]).await.unwrap_err().code(), Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION));
    }
}

pub async fn reject_terminal_rewrite(pool: &PgPool, run: TurnRunId) {
    let client = pool.get().await.unwrap();
    for query in [
        "UPDATE brassclaw_monty_task_admissions SET outcome=jsonb_set(outcome,'{status}','\"failed\"') WHERE run_id=$1",
        "UPDATE brassclaw_monty_task_admissions SET started_at=started_at+interval '1 second' WHERE run_id=$1",
        "UPDATE brassclaw_monty_task_admissions SET settled_at=settled_at+interval '1 second' WHERE run_id=$1",
        "UPDATE brassclaw_monty_task_admissions SET phase='started',outcome=NULL,settled_at=NULL WHERE run_id=$1",
        "DELETE FROM brassclaw_monty_task_admissions WHERE run_id=$1",
    ] {
        assert_eq!(
            client
                .execute(query, &[&run.as_uuid()])
                .await
                .unwrap_err()
                .code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
    }
    assert_eq!(
        client
            .execute("TRUNCATE brassclaw_monty_task_admissions CASCADE", &[])
            .await
            .unwrap_err()
            .code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
}
