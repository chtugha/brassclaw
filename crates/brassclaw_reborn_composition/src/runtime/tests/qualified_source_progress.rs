//! Test observation only. Original native completion remains the sole live
//! proof; the bounded SQL snapshot below reports metadata, never success.
use super::*;
use brassclaw_product_workflow::ReviewWorkView;
use brassclaw_turns::{TurnRunId, run_profile::AgentLoopDriverError};
use std::{future::Future, pin::Pin};

type Completion = Result<crate::global_monty_driver::DrivenMontyTask, AgentLoopDriverError>;

/// Historical test observation only. Captured from the real acknowledged
/// catalogue once, never evidence of today's ACK, admission or dispatch policy.
#[derive(Clone)]
pub(super) struct TestObservationFrame {
    original: brassclaw_host_api::task_preparation::RetainedPreparation,
    origin: brassclaw_host_api::task_preparation::PreparationOrigin,
    independent_diagnostic_pool: Option<Arc<brassclaw_pg::PgPool>>,
}
impl TestObservationFrame {
    pub(super) async fn capture(runtime: &RebornRuntime) -> Self {
        let captured = match runtime.installed_catalogue.capture_task_preparation().await {
            Ok(original) => original,
            Err(error) => Self::capture_failed(runtime, &error),
        };
        #[derive(serde::Deserialize)]
        struct Provenance {
            origin: brassclaw_host_api::task_preparation::PreparationOrigin,
        }
        let provenance: Provenance = match serde_json::from_str(captured.bytes()) {
            Ok(original) => original,
            Err(_) => Self::capture_failed(
                runtime,
                &crate::RebornBuildError::InvalidConfig {
                    reason: "original test observation provenance invalid".into(),
                },
            ),
        };
        Self {
            original: captured.original().clone(),
            origin: provenance.origin,
            independent_diagnostic_pool: None,
        }
    }
    pub(super) fn with_independent_diagnostic_pool(
        mut self,
        pool: Arc<brassclaw_pg::PgPool>,
    ) -> Self {
        self.independent_diagnostic_pool = Some(pool);
        self
    }
    fn capture_failed(runtime: &RebornRuntime, error: &crate::RebornBuildError) -> ! {
        let owner = runtime.global_monty_owner.ownership_check();
        // capture_task_preparation maps its sources to fixed configuration
        // reasons; no raw settings-store errors, commands or payloads enter here.
        let reason = match error {
            crate::RebornBuildError::InvalidConfig { reason } => Some(reason.as_str()),
            _ => None,
        };
        eprintln!(
            "SourceReview observation capture diagnostic {}",
            json!({
                "stage":"initial_test_observation_capture", "original_capture_reason":reason,
                "observation_frame":"not_captured", "ownership_closed":owner.is_closed(),
                "capture_error_kind":if reason.is_some() {"invalid_config"} else {"unexpected_capture_error"},
                "owner":owner.test_terminal_facts().unwrap_or_else(||json!({"phase":"no_owner_exit_observed"})),
            })
        );
        // No invented deadline or second service join on first-capture failure.
        panic!(
            "SourceReview initial test observation capture failed: {}",
            reason.unwrap_or("unexpected_capture_error")
        );
    }
    pub(super) fn duration(&self) -> Duration {
        Duration::from_millis(self.original.policy().response_timeout_millis)
    }
    pub(super) fn facts(&self) -> Value {
        json!({"scope":"historical_test_observation_only", "current_ack_asserted":false,
            "revision":self.original.revision(), "origin":self.origin,
            "response_timeout_millis":self.original.policy().response_timeout_millis,
            "capture_checksum":self.original.checksum()})
    }
}

pub(super) struct OriginalReview {
    id: Uuid,
    frame: TestObservationFrame,
    // Construct exactly once, before authoring; never replace with a late waiter.
    completion: Pin<Box<dyn Future<Output = Completion> + Send>>,
    completed: bool,
    last_work: Option<ReviewWorkView>,
    last_read: Option<crate::pg_review_submission_io::ReadObservation>,
}
impl OriginalReview {
    pub(super) fn new(runtime: &RebornRuntime, frame: &TestObservationFrame, id: Uuid) -> Self {
        let waiter = runtime
            .monty_test_controls
            .0
            .private_completion_waiter(TurnRunId::from_uuid(id))
            .unwrap();
        Self {
            id,
            frame: frame.clone(),
            completion: Box::pin(waiter.wait()),
            completed: false,
            last_work: None,
            last_read: None,
        }
    }
    pub(super) fn id(&self) -> Uuid {
        self.id
    }

    async fn fail(&self, runtime: &RebornRuntime, stage: &str) -> ! {
        if let Some(read) = &self.last_read {
            eprintln!(
                "SourceReview projection {}",
                json!({"original":self.id,"read":read.facts()})
            );
        }
        diagnose(
            runtime,
            &self.frame,
            self.id,
            stage,
            self.last_work.as_ref(),
        )
        .await;
        panic!("SourceReview test failed at {stage}; original={}", self.id);
    }

    // A successful native producer is also insufficient without its real
    // internal-completion receipt. Never poll a completed future again.
    fn accept_success(&mut self, result: Completion) -> Result<(), String> {
        self.completed = true;
        match result {
            Ok(actual) if actual.internal_completion.is_some() => Ok(()),
            Ok(_) => Err("native_completion_without_internal_receipt".into()),
            Err(error) => Err(completion_reason(&error)),
        }
    }

    pub(super) async fn confirmation(&mut self, runtime: &RebornRuntime) -> Value {
        let duration = self.frame.duration();
        let id = self.id;
        let submission_id = id.to_string();
        let wait = async {
            loop {
                // Keep the last successful projection even when a later read
                // stalls. Race the original producer against every read.
                let observation = crate::pg_review_submission_io::ReadObservation::new();
                self.last_read = Some(observation.clone());
                let reading = observation.observe(crate::pg_review_submission::get_with_catalogue(
                    &runtime.installed_catalogue.pool,
                    &submission_id,
                    &runtime.installed_catalogue,
                ));
                tokio::pin!(reading);
                let (view, producer_result) = tokio::select! {
                    result = &mut self.completion, if !self.completed => {
                        let producer_result = self.accept_success(result);
                        // Complete exactly this read, including on a producer
                        // failure, and retain its passive facts before reporting.
                        (reading.await, Some(producer_result))
                    }
                    view = &mut reading => (view, None),
                };
                if let Some(result) = producer_result {
                    result?;
                }
                let view = view.map_err(|_| "original_projection_error".to_owned())?;
                self.last_work = view.work;
                if self.last_work.as_ref().is_some_and(|work| {
                    matches!(
                        work.status,
                        ReviewWorkStatus::StructuralObservationsConfirmed
                    )
                }) {
                    if !self.completed {
                        let result = self.completion.as_mut().await;
                        self.accept_success(result)?;
                    }
                    let client = runtime
                        .installed_catalogue
                        .pool
                        .get()
                        .await
                        .map_err(|_| "confirmation_connection_error".to_owned())?;
                    let row = client.query_one("SELECT confirmation_bytes FROM reborn_component_review_work_confirmations WHERE submission_id=$1", &[&id]).await
                        .map_err(|_| "confirmation_read_error".to_owned())?;
                    let bytes: &str = row.get(0);
                    return serde_json::from_str(bytes)
                        .map_err(|_| "confirmation_decode_error".to_owned());
                }
                tokio::task::yield_now().await;
            }
        };
        match tokio::time::timeout(duration, wait).await {
            Ok(Ok(record)) => record,
            Ok(Err(reason)) => self.fail(runtime, &reason).await,
            Err(_) => self.fail(runtime, "confirmation_test_watchdog").await,
        }
    }

    pub(super) async fn reached(
        &mut self,
        runtime: &RebornRuntime,
        gate: impl Future<Output = ()>,
    ) {
        let duration = self.frame.duration();
        let reached = async {
            tokio::select! {
                biased;
                () = gate => Ok(()),
                result = &mut self.completion => {
                    let reason = match self.accept_success(result) {
                        Ok(()) => "producer_completed_before_settlement_pause".to_owned(),
                        Err(reason) => reason,
                    };
                    Err(reason)
                }
            }
        };
        match tokio::time::timeout(duration, reached).await {
            Ok(Ok(())) => (),
            Ok(Err(reason)) => self.fail(runtime, &reason).await,
            Err(_) => self.fail(runtime, "settlement_pause_test_watchdog").await,
        }
    }

    pub(super) async fn settled(&mut self, runtime: &RebornRuntime) {
        assert!(!self.completed, "the original future is consumed once");
        match tokio::time::timeout(self.frame.duration(), self.completion.as_mut()).await {
            Ok(result) => {
                if let Err(reason) = self.accept_success(result) {
                    self.fail(runtime, &reason).await;
                }
            }
            Err(_) => self.fail(runtime, "native_completion_test_watchdog").await,
        }
    }

    pub(super) async fn failed(&mut self, runtime: &RebornRuntime, expected: Option<&str>) {
        assert!(!self.completed, "the original future is consumed once");
        let result =
            match tokio::time::timeout(self.frame.duration(), self.completion.as_mut()).await {
                Ok(result) => result,
                Err(_) => self.fail(runtime, "expected_failure_test_watchdog").await,
            };
        self.completed = true;
        match result {
            Err(AgentLoopDriverError::Failed { reason_kind })
                if expected.is_none_or(|expected| reason_kind == expected) =>
            {
                diagnose(
                    runtime,
                    &self.frame,
                    self.id,
                    &safe_reason(&reason_kind),
                    self.last_work.as_ref(),
                )
                .await;
            }
            Err(error) => self.fail(runtime, &completion_reason(&error)).await,
            Ok(_) => self.fail(runtime, "unexpected_native_success").await,
        }
    }
}

fn completion_reason(error: &AgentLoopDriverError) -> String {
    match error {
        AgentLoopDriverError::Failed { reason_kind } => safe_reason(reason_kind),
        AgentLoopDriverError::InputAdmissionPending => "input_admission_pending".into(),
        AgentLoopDriverError::InvalidRequest { .. } => "producer_invalid_request".into(),
        AgentLoopDriverError::Unavailable { .. } => "producer_unavailable".into(),
    }
}
fn safe_reason(reason: &str) -> String {
    if !reason.is_empty()
        && reason.len() <= 128
        && reason
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        reason.to_owned()
    } else {
        "nonstandard_reason_redacted".into()
    }
}

pub(super) async fn watchdog<T>(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    id: Uuid,
    stage: &str,
    future: impl Future<Output = T>,
) -> T {
    // Use the original fixture observation frame through failures and settings
    // edits. It asserts neither a current ACK nor a production workflow budget.
    match tokio::time::timeout(frame.duration(), future).await {
        Ok(value) => value,
        Err(_) => {
            diagnose(runtime, frame, id, stage, None).await;
            panic!("SourceReview test watchdog at {stage}; original={id}");
        }
    }
}

pub(super) async fn diagnose(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    id: Uuid,
    stage: &str,
    last_work: Option<&ReviewWorkView>,
) {
    let driver = &runtime.monty_test_controls.0;
    let service = runtime.global_monty_owner.client();
    let heap = service.heap_observation();
    let memory = service.memory_admission_policy();
    let service_wait_observation = json!({
        "scope":"last_published_observation_before_diagnostic_io",
        "heap_pending_reduction":heap.status.pending_reduction,
        "memory_admission_revision":memory.revision,
        "memory_backpressure":memory.backpressure,
    });
    // Filter the existing observer: no Debug error strings, attempts, claims,
    // receipts or transport payloads enter the failure report.
    let settlement = driver
        .test_settlement_observations(TurnRunId::from_uuid(id))
        .map(|rows| {
            rows.into_iter()
                .map(|row| {
                    let receipt = &row["service_receipt"];
                    let outcome = receipt["Ok"]["outcome"].as_str().map(safe_reason);
                    json!({"phase":row["phase"],"native_outcome":outcome,
                "submission_boundary":row["submission_boundary"],
                "submission_future":row["submission_future"],
                "cancelled":row["cancelled"],
                "private_intent":row["private_intent"],
                "native_receipt_error":receipt.get("Err").is_some(),
                "settlement_started":row["settlement_started"],
                "acknowledgement_present":!row["acknowledgement"].is_null(),
                "acknowledgement_error":row["acknowledgement"].get("Err").is_some(),
                "host_has_withheld_results":row["host_has_withheld_results"]})
                })
                .collect::<Vec<_>>()
        })
        .ok();
    // Only the size-one Source fixture supplies its already existing independent
    // pool. Observe activity first so a later original-row read cannot erase it.
    let postgres_activity = match frame.independent_diagnostic_pool.as_deref() {
        Some(pool) => match tokio::time::timeout(frame.duration(), activity_snapshot(pool)).await {
            Ok(Ok(value)) => value,
            Ok(Err(())) => json!({"snapshot":"unavailable"}),
            Err(_) => json!({"snapshot":"test_watchdog_elapsed"}),
        },
        None => json!({"snapshot":"not_requested"}),
    };
    let snapshot_pool = frame
        .independent_diagnostic_pool
        .as_deref()
        .unwrap_or(runtime.installed_catalogue.pool.as_ref());
    let snapshot = tokio::time::timeout(frame.duration(), async {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            snapshot_pool.get().await.map_err(|_| ())?);
        let tx = client.build_transaction().isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true).start().await.map_err(|_| ())?;
        let row = tx.query_one("SELECT w.status,w.private_claim_generation::bigint,w.private_claim_attempts::bigint,w.private_cancel_revision::bigint,a.phase,
            a.private_submission_intent_at IS NOT NULL,
            EXISTS(SELECT 1 FROM brassclaw_monty_private_preparations p WHERE p.work_id=$1),
            EXISTS(SELECT 1 FROM reborn_component_validator_selections s WHERE s.submission_id=$1),
            (SELECT count(*) FROM brassclaw_monty_tool_invocations i WHERE i.run_id=$1),
            EXISTS(SELECT 1 FROM brassclaw_monty_private_terminals t WHERE t.work_id=$1),
            EXISTS(SELECT 1 FROM brassclaw_monty_private_deliveries d WHERE d.work_id=$1),
            EXISTS(SELECT 1 FROM reborn_component_review_work_confirmations c WHERE c.submission_id=$1)
            FROM (SELECT $1::uuid AS id) original LEFT JOIN brassclaw_turns w ON w.work_id=original.id AND w.record_kind='private_work'
            LEFT JOIN brassclaw_monty_task_admissions a ON a.run_id=w.private_admission_run", &[&id]).await.map_err(|_| ())?;
        let value = json!({"work_status":row.get::<_,Option<String>>(0),
            "generation":row.get::<_,Option<i64>>(1),"attempts":row.get::<_,Option<i64>>(2),
            "cancel_revision":row.get::<_,Option<i64>>(3),"admission_phase":row.get::<_,Option<String>>(4),
            "submission_intent_present":row.get::<_,bool>(5),"preparation_present":row.get::<_,bool>(6),
            "selection_present":row.get::<_,bool>(7),"tool_invocations":row.get::<_,i64>(8),
            "terminal_present":row.get::<_,bool>(9),"delivery_present":row.get::<_,bool>(10),
            "confirmation_present":row.get::<_,bool>(11)});
        tx.commit().await.map_err(|_| ())?;
        client.recycle_after_commit();
        Ok::<_, ()>(value)
    }).await;
    let metadata = match snapshot {
        Ok(Ok(value)) => value,
        Ok(Err(())) => json!({"snapshot":"unavailable"}),
        Err(_) => json!({"snapshot":"test_watchdog_elapsed"}),
    };
    let pool = runtime.installed_catalogue.pool.status();
    let owner = runtime.global_monty_owner.ownership_check();
    // Admission closure is not an exit. If it has happened, wait only for
    // this owner's already-running join to publish its closed projection.
    let owner_facts = tokio::select! {
        biased;
        () = service.closed() => match tokio::time::timeout(frame.duration(), owner.wait_test_terminal_facts()).await {
            Ok(Some(facts)) => facts,
            Ok(None) => json!({"phase":"owner_observer_closed"}),
            Err(_) => json!({"phase":"actual_owner_exit_pending"}),
        },
        () = std::future::ready(()) => owner.test_terminal_facts().unwrap_or_else(|| json!({"phase":"no_owner_exit_observed"})),
    };
    eprintln!(
        "SourceReview diagnostic {}",
        json!({"original":id,"stage":stage,"test_observation_frame":frame.facts(),
        "last_successful_work_view":last_work,"settlement":settlement,"metadata":metadata,"owner":owner_facts,
        "metadata_pool":if frame.independent_diagnostic_pool.is_some() {"independent_fixture"} else {"runtime"},
        "postgres_activity":postgres_activity,
        "service_wait_observation":service_wait_observation,
        "pool":{"max_size":pool.max_size,"size":pool.size,"available":pool.available}})
    );
}

/// PostgreSQL-owned metadata only. In particular never select query text,
/// application names, connection addresses, credentials or cancellation keys.
async fn activity_snapshot(pool: &brassclaw_pg::PgPool) -> Result<Value, ()> {
    let mut client = crate::pg_transaction_client::TransactionClient::from_client(
        pool.get().await.map_err(|_| ())?,
    );
    let tx = client
        .build_transaction()
        .read_only(true)
        .start()
        .await
        .map_err(|_| ())?;
    let rows = tx
        .query(
            "SELECT pid,state,wait_event_type,wait_event,pg_blocking_pids(pid)
        FROM pg_stat_activity WHERE datname=current_database()
        AND backend_type='client backend' AND pid<>pg_backend_pid() ORDER BY pid",
            &[],
        )
        .await
        .map_err(|_| ())?;
    let facts: Vec<_> = rows.into_iter().map(|row| json!({
        "backend_pid":row.get::<_,i32>(0),"state":row.get::<_,Option<String>>(1),
        "wait_event_type":row.get::<_,Option<String>>(2),"wait_event":row.get::<_,Option<String>>(3),
        "blocker_pids":row.get::<_,Vec<i32>>(4),
    })).collect();
    tx.commit().await.map_err(|_| ())?;
    client.recycle_after_commit();
    Ok(json!({"snapshot":"observed","backends":facts}))
}
