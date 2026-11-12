//! Faults on the real PostgreSQL transport/transactions around actual native
//! execution. These cases manufacture neither an answer nor a settlement.
use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};

struct Case<'a> {
    catalogue: &'a crate::installed_monty_catalogue::InstalledMontyCatalogue,
    candidate: &'a InspectedRetainedProgram,
    reply: &'a RetainedToolProgram,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
}
impl Case<'_> {
    async fn run(
        &self,
        pool: Arc<PgPool>,
        id: Uuid,
    ) -> Result<Arc<QualifiedSourceInventoryUsage>, RebornBuildError> {
        qualify(
            pool,
            self.catalogue.kernel.clone(),
            InventoryExecutor {
                worker: self.catalogue.worker_image.clone(),
                configuration: self.executor.clone(),
                root: self.catalogue.root_definition.clone(),
            },
            self.candidate,
            self.reply,
            id,
            self.catalogue.resource_store_settings.clone(),
        )
        .await
    }
    async fn bytes(&self, id: Uuid) -> Option<String> {
        let mut client = self.catalogue.pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        crate::bootstrap_evidence::read_evidence(&tx, id)
            .await
            .unwrap()
    }
    async fn count(&self) -> i64 {
        self.catalogue
            .pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0)
    }
    async fn assert_unreplayed(&self, id: Uuid) {
        let calls = self.count().await;
        let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
        let original = self.bytes(intent).await.unwrap();
        assert!(self.run(self.catalogue.pool.clone(), id).await.is_err());
        assert_eq!(self.count().await, calls);
        assert_eq!(self.bytes(intent).await.as_deref(), Some(original.as_str()));
    }
}

pub(crate) async fn assert_actual_ack_and_timeout_faults(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    candidate: &InspectedRetainedProgram,
    reply: &RetainedToolProgram,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    connection: &str,
) {
    let case = Case {
        catalogue,
        candidate,
        reply,
        executor,
    };
    let transport = AckCutTransport::start(
        connection,
        vec![
            AckCutRule::after_bind(
                1,
                vec![b"inventory-primitive-native-observation/1".to_vec()],
                vec![],
                AckBoundary::Commit,
            ),
            AckCutRule::after_bind(
                2,
                vec![
                    b"{\"kind\":\"return\",\"value\":\"".to_vec(),
                    b"python-source-inventory/1".to_vec(),
                ],
                vec![],
                AckBoundary::Commit,
            ),
            AckCutRule::after_bind(
                3,
                vec![b"inventory-qualification-completion/1".to_vec()],
                vec![],
                AckBoundary::Commit,
            ),
        ],
    )
    .await
    .unwrap();
    for boundary in [1, 2, 3] {
        let id = Uuid::new_v4();
        let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
        let cuts = transport.cut_count();
        transport.arm(boundary).unwrap();
        let result = case.run(transport.pool.clone(), id).await;
        assert_eq!(
            transport.cut_count(),
            cuts + 1,
            "actual selected COMMIT ACK was cut"
        );
        let positive = Uuid::new_v5(&intent, b"primitive-valid");
        assert!(
            case.bytes(Uuid::new_v5(&positive, b"native-return"))
                .await
                .is_some()
        );
        assert!(
            case.bytes(Uuid::new_v5(&positive, b"outer-host-ack"))
                .await
                .is_some()
        );
        if boundary == 2 {
            // The actual native answer committed, but Monty received the
            // persistence failure. Native bytes alone cannot complete that VM.
            assert!(result.is_err());
            assert!(
                case.bytes(Uuid::new_v5(&intent, b"completed-original"))
                    .await
                    .is_none()
            );
            case.assert_unreplayed(id).await;
        } else {
            result.expect("exact original readback resolves this metadata COMMIT ACK loss");
            let calls = case.count().await;
            case.run(catalogue.pool.clone(), id)
                .await
                .expect("completed original cold read");
            assert_eq!(case.count().await, calls);
        }
    }
    transport.shutdown().await.unwrap();

    // Reject only the real close after the receipt and quiescence observations
    // already exist. Do not fail an earlier native call to imitate this case.
    let id = Uuid::new_v4();
    let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
    let client = catalogue.pool.get().await.unwrap();
    client.batch_execute(&format!(r#"
        CREATE FUNCTION reject_inventory_original_close() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.phase='settled' AND EXISTS (
            SELECT 1 FROM reborn_component_review_evidence WHERE evidence_id='{intent}'::uuid
            AND evidence_bytes::jsonb #>> '{{report,header,run_id}}'=NEW.run_id::text
          ) THEN RAISE EXCEPTION 'contained original close rejection'; END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER reject_inventory_original_close BEFORE UPDATE ON brassclaw_monty_task_admissions
        FOR EACH ROW EXECUTE FUNCTION reject_inventory_original_close();
    "#)).await.unwrap();
    drop(client);
    assert!(case.run(catalogue.pool.clone(), id).await.is_err());
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-observations"))
            .await
            .is_some()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-quiescence-observation"))
            .await
            .is_some()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"completed-original"))
            .await
            .is_none()
    );
    catalogue.pool.get().await.unwrap().batch_execute(
        "DROP TRIGGER reject_inventory_original_close ON brassclaw_monty_task_admissions; DROP FUNCTION reject_inventory_original_close()",
    ).await.unwrap();
    case.assert_unreplayed(id).await;

    // A real successful native receipt is observed, then the still-owned
    // worker is killed before shutdown settlement. Original behavior remains,
    // but no quiescence/completion handle may be minted from that failed exit.
    let id = Uuid::new_v4();
    let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
    assert!(
        qualify_inner(
            catalogue.pool.clone(),
            catalogue.kernel.clone(),
            InventoryExecutor {
                worker: catalogue.worker_image.clone(),
                configuration: case.executor.clone(),
                root: catalogue.root_definition.clone(),
            },
            QualificationSelection {
                candidate: match candidate.program() {
                    RetainedProgram::Tools(p) => p,
                    _ => panic!("actual Tool program"),
                },
                preparation: candidate.preparation_limits(),
                qualification_id: id,
                allow_new: true,
            },
            reply,
            catalogue.resource_store_settings.clone(),
            Some(QualificationFault::WorkerExit),
        )
        .await
        .is_err()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-observations"))
            .await
            .is_some()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-quiescence-observation"))
            .await
            .is_none()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"completed-original"))
            .await
            .is_none()
    );
    case.assert_unreplayed(id).await;

    // Actual late cancellation uses the original Turn owner/claim, after
    // durable native and worker quiescence observations, before Turn completion.
    let id = Uuid::new_v4();
    let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
    let cancelled = qualify_inner(
        catalogue.pool.clone(),
        catalogue.kernel.clone(),
        InventoryExecutor {
            worker: catalogue.worker_image.clone(),
            configuration: case.executor.clone(),
            root: catalogue.root_definition.clone(),
        },
        QualificationSelection {
            candidate: match candidate.program() {
                RetainedProgram::Tools(p) => p,
                _ => panic!("actual Tool program"),
            },
            preparation: candidate.preparation_limits(),
            qualification_id: id,
            allow_new: true,
        },
        reply,
        catalogue.resource_store_settings.clone(),
        Some(QualificationFault::LateCancellation),
    )
    .await
    .err()
    .expect("cancelled original cannot qualify");
    let message = cancelled.to_string();
    assert!(message.contains("original cleanup confirmed Cancelled"));
    assert!(!message.contains("unacknowledged") && !message.contains("unresolved"));
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-observations"))
            .await
            .is_some()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"actual-quiescence-observation"))
            .await
            .is_some()
    );
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"completed-original"))
            .await
            .is_none()
    );
    {
        use brassclaw_turns::TurnSpawnTreeStateStore;
        let original: Value = serde_json::from_str(&case.bytes(intent).await.unwrap()).unwrap();
        let quiet: Value = serde_json::from_str(
            &case
                .bytes(Uuid::new_v5(&intent, b"actual-quiescence-observation"))
                .await
                .unwrap(),
        )
        .unwrap();
        let scope: brassclaw_turns::TurnScope =
            serde_json::from_value(quiet["report"]["turn_scope"].clone()).unwrap();
        let run = brassclaw_turns::TurnRunId::from_uuid(
            serde_json::from_value(original["report"]["header"]["run_id"].clone()).unwrap(),
        );
        let actual = brassclaw_turns::PgTurnStateStore::new(
            catalogue.pool.clone(),
            scope.tenant_id.as_str(),
        )
        .get_run_record(&scope, run)
        .await
        .unwrap()
        .unwrap();
        assert_eq!(actual.status, brassclaw_turns::TurnStatus::Cancelled);
        assert!(
            actual.runner_id.is_none()
                && actual.lease_token.is_none()
                && actual.lease_expires_at.is_none()
        );
    }
    case.assert_unreplayed(id).await;

    // The actual native success reaches its observer, then its INSERT exceeds
    // the existing acknowledged technical deadline. The outer host ACK retains
    // those received bytes even when the timed-out transaction rolls back.
    let timeout = catalogue
        .resource_store_settings
        .snapshot()
        .unwrap()
        .validate()
        .unwrap()
        .timeout;
    let delay = timeout.checked_mul(3).unwrap();
    let client = catalogue.pool.get().await.unwrap();
    client.batch_execute(&format!(r#"
        CREATE FUNCTION delay_inventory_native_observation() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.evidence_bytes::jsonb #>> '{{report,format}}'='inventory-primitive-native-observation/1' THEN
            PERFORM pg_advisory_xact_lock(547121);
            PERFORM pg_sleep({});
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER delay_inventory_native_observation BEFORE INSERT ON reborn_component_review_evidence
        FOR EACH ROW EXECUTE FUNCTION delay_inventory_native_observation();
    "#, delay.as_secs_f64())).await.unwrap();
    drop(client);
    let id = Uuid::new_v4();
    let intent = Uuid::new_v5(&id, b"inventory-qualification-intent/1");
    assert!(
        tokio::time::timeout(
            timeout.checked_mul(8).unwrap(),
            case.run(catalogue.pool.clone(), id)
        )
        .await
        .expect("technical native observation deadline returns")
        .is_err()
    );
    let client = catalogue.pool.get().await.unwrap();
    // Wait for the real timed-out transaction to release its lock; the test
    // does not infer rollback from the client-side timeout.
    tokio::time::timeout(
        timeout.checked_mul(8).unwrap(),
        client.query_one("SELECT pg_advisory_lock(547121)", &[]),
    )
    .await
    .unwrap()
    .unwrap();
    client
        .query_one("SELECT pg_advisory_unlock(547121)", &[])
        .await
        .unwrap();
    client.batch_execute("DROP TRIGGER delay_inventory_native_observation ON reborn_component_review_evidence; DROP FUNCTION delay_inventory_native_observation()").await.unwrap();
    drop(client);
    let positive = Uuid::new_v5(&intent, b"primitive-valid");
    let ack = case
        .bytes(Uuid::new_v5(&positive, b"outer-host-ack"))
        .await
        .unwrap();
    let ack = strict_json(&ack, REVISION_LIMITS).unwrap();
    assert_eq!(ack["report"]["host_outcome"]["kind"], "completed");
    let received = strict_json(
        ack["report"]["native_observation_bytes"].as_str().unwrap(),
        REVISION_LIMITS,
    )
    .unwrap();
    let native = strict_json(
        received["report"]["native_bytes"].as_str().unwrap(),
        REVISION_LIMITS,
    )
    .unwrap();
    assert_eq!(native["outcome"]["kind"], "returned");
    assert_eq!(native["outcome"]["usage"]["process_count"], 1);
    assert!(
        case.bytes(Uuid::new_v5(&intent, b"completed-original"))
            .await
            .is_none()
    );
    case.assert_unreplayed(id).await;
}
