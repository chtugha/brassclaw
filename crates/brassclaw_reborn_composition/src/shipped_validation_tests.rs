use super::*;

#[tokio::test]
async fn package_transfer_scope_is_private_and_does_not_leak() {
    assert!(!transferring());
    installation(async {
        assert!(transferring());
        assert!(!tokio::spawn(async { transferring() }).await.unwrap());
    })
    .await;
    assert!(!transferring());
    qualify_package(installation(async {
        assert!(!transferring());
    }))
    .await;
    assert!(!transferring());
    let transfer = Transfer::new(json!({"canonical":"test identity"}));
    let value: Value = serde_json::from_str(&transfer.bytes()).unwrap();
    assert_eq!(value["report"]["validation_origin"], "provisioning");
    assert_eq!(value["report"]["installation_execution"], false);
    assert!(value["report"].get("observed").is_none());
    assert_ne!(transfer.q1_id(), transfer.id());
    assert_ne!(
        transfer.id(),
        Transfer::new(json!({"canonical":"changed identity"})).id()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_and_repeated_start_transfer_shipped_validation_without_qualification_work() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let mut originals = Vec::<(Uuid, String)>::new();
    for _ in 0..2 {
        let runtime = Box::pin(crate::build_reborn_runtime(
            crate::RebornRuntimeInput::from_services(
                rig.build_input("shipped-transfer", home.path())
                    .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
            ),
        ))
        .await
        .unwrap();
        let client = rig.pool.get().await.unwrap();
        let admissions: i64 = client
            .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            admissions, 0,
            "installation must not admit qualification Recipes"
        );
        for table in [
            "reborn_validation_queue",
            "reborn_component_review_submissions",
            "reborn_prefix_model_specifications",
            "reborn_prefix_model_artifact_sets",
        ] {
            let sql = format!("SELECT count(*) FROM {table}");
            let count: i64 = client.query_one(&sql, &[]).await.unwrap().get(0);
            assert_eq!(
                count, 0,
                "fresh/repeated installation must not create review requests or qualification fixtures in {table}"
            );
        }
        let records = client.query("SELECT evidence_id,evidence_bytes FROM reborn_component_review_evidence ORDER BY evidence_id", &[])
            .await.unwrap().into_iter().map(|row| (row.get::<_,Uuid>(0),row.get::<_,String>(1))).collect::<Vec<_>>();
        assert!(!records.is_empty());
        for (_, bytes) in &records {
            let value: Value = serde_json::from_str(bytes).unwrap();
            assert_eq!(value["report"]["format"], "shipped-validation-transfer/1");
            assert_eq!(value["report"]["installation_execution"], false);
        }
        for row in client
            .query(
                "SELECT approval_bytes FROM reborn_skill_association_approvals",
                &[],
            )
            .await
            .unwrap()
        {
            let value: Value = serde_json::from_str(row.get::<_, String>(0).as_str()).unwrap();
            assert_eq!(value["validation_mode"], "system_seed");
            assert!(value["q2_ref"].is_null());
            assert_ne!(value["q1_ref"], value["behavioral_refs"][0]);
        }
        if originals.is_empty() {
            originals = records;
        } else {
            assert_eq!(records, originals);
        }
        drop(client);
        // Exercise the real connected authored-source preparation consumers:
        // their qualified prerequisites now come from the shipped package.
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        runtime
            .installed_catalogue
            .pure_source_review
            .assert_shipped_readback(&tx)
            .await;
        tx.commit().await.unwrap();
        drop(client);
        runtime.shutdown().await.unwrap();
    }
    // An arbitrary caller cannot create or relabel a transfer outside the
    // private packaged-construction scope, even with an existing database.
    assert!(
        Transfer::new(json!({"source":"system_seed"}))
            .install(&rig.pool)
            .await
            .is_err()
    );
}
