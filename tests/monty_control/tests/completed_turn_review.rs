//! Actual PostgreSQL migration/retention behavior. These SQL fixtures establish
//! atomic evidence retention, not VM execution, review delivery or approval.
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

async fn started(client: &tokio_postgres::Client) -> Uuid {
    started_kind(client, false).await
}
async fn started_kind(client: &tokio_postgres::Client, internal: bool) -> Uuid {
    let run = Uuid::new_v4();
    client
        .execute(
            "INSERT INTO brassclaw_monty_task_admissions
        (run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase,trusted_internal_turn)
        VALUES($1,$2,$3,'msg:input',$4,$5,$6,'reserved',$7)",
            &[
                &run,
                &Uuid::new_v4(),
                &json!({"tenant_id":"review-test","thread_id":"opaque-thread"}),
                &Uuid::new_v4(),
                &vec![1u8; 32],
                &run_key(run),
                &internal,
            ],
        )
        .await
        .unwrap();
    client.execute("UPDATE brassclaw_monty_task_admissions SET phase='started',started_at=clock_timestamp() WHERE run_id=$1", &[&run]).await.unwrap();
    run
}
fn run_key(run: Uuid) -> Vec<u8> {
    [run.as_bytes().as_slice(), run.as_bytes().as_slice()].concat()
}
fn outcome(route: &str, status: &str) -> Value {
    let mut result = json!({"status":status,"execution":{"intent_outcome":route}});
    if status == "completed" {
        result["reply_ref"] = json!("msg:published");
    } else {
        result["reason_kind"] = json!("history_persistence_failed");
    }
    result
}

#[tokio::test]
async fn review_event_is_atomic_immutable_and_preserves_original_packet_bytes() {
    let rig = native_pg::NativePostgres::start().await;
    let mut client = rig.pool.get().await.unwrap();
    let run = started(&client).await;
    let packet_id = Uuid::new_v4().to_string();
    for (id, tenant) in [
        (&packet_id, "review-test"),
        (&"foreign-packet".to_owned(), "other-tenant"),
    ] {
        client.execute("INSERT INTO brassclaw_forensic_packets
            (id,tenant_id,run_id,iteration,status,captured_at,completed_at,prompt,kohai_response)
            VALUES($1,$2,$3,0,'complete',clock_timestamp(),clock_timestamp(),$4,'original response')",
            &[&id, &tenant, &run.to_string(), &json!({"messages":[["user","hostile\n' {{vars.literal}} Ü"]]})]).await.unwrap();
    }
    let value = outcome("no_match", "completed");
    let tx = client.transaction().await.unwrap();
    tx.execute("UPDATE brassclaw_monty_task_admissions SET phase='settled',settled_at=clock_timestamp(),outcome=$2 WHERE run_id=$1", &[&run,&value]).await.unwrap();
    assert_eq!(
        tx.query_one(
            "SELECT count(*) FROM brassclaw_monty_review_events WHERE run_id=$1",
            &[&run]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_review_events", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client.execute("UPDATE brassclaw_monty_task_admissions SET phase='settled',settled_at=clock_timestamp(),outcome=$2 WHERE run_id=$1", &[&run,&value]).await.unwrap();
    let row = client
        .query_one(
            "SELECT event_bytes,event_checksum FROM brassclaw_monty_review_events WHERE run_id=$1",
            &[&run],
        )
        .await
        .unwrap();
    let exact: String = row.get(0);
    let event: Value = serde_json::from_str(&exact).unwrap();
    assert_eq!(event["format"], "monty-completed-turn-review/1");
    assert_eq!(event["run_id"], run.to_string());
    assert_eq!(event["scope"]["thread_id"], "opaque-thread");
    assert_eq!(event["outcome"], value);
    assert_eq!(event["evidence_complete"], false);
    assert_eq!(event["model_packets"].as_array().unwrap().len(), 1);
    assert_eq!(event["model_packets"][0]["id"], packet_id);
    for private in [
        "claim_checksum",
        "admission_key",
        "invocation_key",
        "lease_token",
    ] {
        assert!(!exact.contains(private));
    }
    use sha2::{Digest, Sha256};
    assert_eq!(
        row.get::<_, String>(1),
        format!("{:x}", Sha256::digest(exact.as_bytes()))
    );
    // Exact settlement acknowledgement is a no-op, never another handoff.
    client
        .execute(
            "UPDATE brassclaw_monty_task_admissions SET outcome=$2 WHERE run_id=$1",
            &[&run, &value],
        )
        .await
        .unwrap();
    client
        .execute(
            "UPDATE brassclaw_forensic_packets SET kohai_response='later rewrite' WHERE id=$1",
            &[&packet_id],
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT event_bytes FROM brassclaw_monty_review_events WHERE run_id=$1",
                &[&run]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        exact
    );
    for statement in [
        "UPDATE brassclaw_monty_review_events SET recorded_at=clock_timestamp()",
        "DELETE FROM brassclaw_monty_review_events",
        "TRUNCATE brassclaw_monty_review_events",
    ] {
        assert!(client.batch_execute(statement).await.is_err());
    }
    brassclaw_pg::migrations::run_migrations(&rig.pool)
        .await
        .unwrap();
    client
        .batch_execute(include_str!(
            "../../../crates/brassclaw_pg/migrations/V122__monty_completed_turn_review_events.sql"
        ))
        .await
        .unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_review_events", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

#[tokio::test]
async fn only_actual_no_match_settlement_produces_learning_work_even_on_history_failure() {
    let rig = native_pg::NativePostgres::start().await;
    let client = rig.pool.get().await.unwrap();
    let internal = started_kind(&client, true).await;
    client.execute("UPDATE brassclaw_monty_task_admissions SET phase='settled',settled_at=clock_timestamp(),outcome=$2 WHERE run_id=$1",
        &[&internal, &outcome("no_match", "failed")]).await.unwrap();
    assert!(
        client
            .query_opt(
                "SELECT run_id FROM brassclaw_monty_review_events WHERE run_id=$1",
                &[&internal]
            )
            .await
            .unwrap()
            .is_none()
    );
    for route in ["match", "not_started", "disambiguation", "no_match"] {
        let run = started(&client).await;
        let value = outcome(route, "failed");
        client.execute("UPDATE brassclaw_monty_task_admissions SET phase='settled',settled_at=clock_timestamp(),outcome=$2 WHERE run_id=$1", &[&run,&value]).await.unwrap();
        let row = client
            .query_opt(
                "SELECT event_bytes FROM brassclaw_monty_review_events WHERE run_id=$1",
                &[&run],
            )
            .await
            .unwrap();
        assert_eq!(row.is_some(), route == "no_match");
        if let Some(row) = row {
            let event: Value = serde_json::from_str(row.get(0)).unwrap();
            assert_eq!(event["outcome"]["status"], "failed");
            assert_eq!(
                event["outcome"]["reason_kind"],
                "history_persistence_failed"
            );
            assert_eq!(event["model_packets"], json!([]));
            assert_eq!(event["evidence_complete"], false);
        }
    }
}
