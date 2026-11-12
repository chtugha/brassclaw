// Real original completion/PG cases for the private invocation read contract.
// No reconstructed service receipt or artificial validator-success payload.
use crate::global_monty_driver::SettledInternalCompletion;
use crate::pg_monty_admission::{PgMontyAdmission, PrivateOriginalTerminalStatus};

async fn read_completed_snapshot(
    fixture: &Fixture,
    inspector: &PgPool,
    claim: &ClaimedPrivateWork,
    completion: &SettledInternalCompletion,
) {
    let id = claim.request().work_id();
    // Prefix work may have no invocation rows. READ ONLY must work even for
    // that empty reference set. Positive writer-lock evidence is supplied by
    // the separate actual Source-review/parser workflow, not this fixture.
    let mut blocker = inspector.get().await.unwrap();
    let locked = blocker.transaction().await.unwrap();
    let rows = locked.query(
        "SELECT invocation_id,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1 FOR UPDATE",
        &[&claim.attempt().run_id.as_uuid()],
    ).await.unwrap();
    assert!(rows.iter().all(|row| row.get::<_, &str>(1) == "answered"));
    let mut client = fixture.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let checksum = tokio::time::timeout(Duration::from_secs(3), async {
        let delivery = PgMontyAdmission::read_private_delivery(&tx, id, claim.request().context())
            .await
            .unwrap()
            .expect("authentic delivery in READ ONLY snapshot");
        assert!(delivery.original().request() == claim.request());
        let live = PgMontyAdmission::verify_private_live_completion(&tx, completion)
            .await
            .unwrap()
            .expect("original opaque native proof is still required");
        assert!(live.request() == claim.request());
        let terminal =
            PgMontyAdmission::read_private_terminal_status(&tx, id, claim.request().context())
                .await
                .unwrap()
                .expect("original historical terminal");
        assert!(matches!(
            terminal,
            PrivateOriginalTerminalStatus::Successful
        ));
        delivery.checksum()
    })
    .await
    .expect("READ ONLY authentication must complete against the original invocation snapshot");
    tx.commit().await.unwrap();
    drop(client);
    locked.rollback().await.unwrap();
    drop(blocker);
    let projection = serde_json::to_value(
        fixture
            .runtime
            .inspect_prefix_job("base-prompt", id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(projection["delivery_pending"], false);
    assert_eq!(projection["delivery_checksum"], hex::encode(checksum));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_delivery_commit_ack_loss_reads_original_in_read_only_snapshot_without_replay() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                id.as_bytes().to_vec(),
                b"native-post-factory-acknowledgement".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let fixture = Fixture::start(&rig, transport.pool.clone()).await;
    fixture.original(id).await;
    fixture.ready(id).await;
    let originals = prefix_original_bytes(&rig.pool, id).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let driver = fixture.runtime.monty_test_controls.0.clone();
    let factory = fixture.reservations.owner.clone();
    let pause = factory.pause_private_settlement(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let mut drive = Box::pin(driver.drive_private(claim.clone()));
    tokio::time::timeout(Duration::from_secs(30), async {
        tokio::select! {
            _ = pause.reached.notified() => (),
            _ = &mut drive => panic!("hold the real completed Factory settlement"),
        }
    })
    .await
    .unwrap();
    transport.arm(1).unwrap();
    pause.release.notify_one();
    tokio::time::timeout(Duration::from_secs(30), &mut drive)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "cut the actual post-Factory delivery COMMIT response"
    );
    let driven = waiter.wait().await.unwrap();
    let completion = driven
        .internal_completion
        .as_ref()
        .expect("original live producer must accept its real readback");
    read_completed_snapshot(&fixture, &rig.pool, &claim, completion).await;
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    assert!(prefix_original_bytes(&rig.pool, id).await == originals);
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert!(driver.drive_private(claim).await.is_err());
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    assert_eq!(transport.cut_count(), 1);
    drop(drive);
    fixture.finish().await;
    transport.shutdown().await.unwrap();
}
