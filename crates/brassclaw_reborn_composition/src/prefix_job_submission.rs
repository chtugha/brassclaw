//! Snapshot submission through existing durable turn admission. No private task
//! queue, background executor, provider request or automatic activation.
use super::*;

impl RebornRuntime {
    /// Capture only the explicitly supported installed core references. This is
    /// not the normal complete-library Generate action or target qualification.
    pub async fn submit_current_core_prefix_snapshot(
        &self,
        id: Uuid,
        profile: &str,
        expected_revision: u64,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
    ) -> Result<Uuid, RebornRuntimeError> {
        let snapshot =
            if crate::prefix_job_definitions::Store::new(self.installed_catalogue.pool.clone())
                .get(id)
                .await
                .map_err(|error| RebornRuntimeError::InvalidArgument {
                    reason: error.to_string(),
                })?
                .is_some()
            {
                // The shared submission boundary validates the retained request and
                // reconstructs its scopes. A retry never recollects current sources.
                String::new()
            } else {
                self.installed_catalogue
                    .capture_core_prefix_sources(profile)
                    .await
                    .map_err(|error| RebornRuntimeError::InvalidArgument {
                        reason: error.to_string(),
                    })?
            };
        self.submit_prefix_generation_snapshot(id, profile, expected_revision, &snapshot, caller)
            .await
    }

    /// Retain a source-only snapshot and asynchronously admit its pinned compiler
    /// through the already-running global orchestrator. This low-level boundary
    /// does not collect sources or qualify their knowledge/model/cache design.
    /// The installed operator API must use an authoritative collector before
    /// exposing a normal Generate action. Completed output stays quarantined.
    pub async fn submit_prefix_generation_snapshot(
        &self,
        id: Uuid,
        profile: &str,
        expected_revision: u64,
        snapshot_bytes: &str,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
    ) -> Result<Uuid, RebornRuntimeError> {
        use crate::prefix_job_definitions::{Reservation, Store};
        let invalid = |reason: &str| RebornRuntimeError::InvalidArgument {
            reason: reason.into(),
        };
        if id.is_nil()
            || expected_revision == 0
            || !matches!(profile, "base-prompt" | "prefix-maker")
        {
            return Err(invalid("unsupported source-only prefix snapshot request"));
        }
        let request = serde_json::json!({"profile":profile,"expected_revision":expected_revision,"mode":"source_only"});
        let store = Store::new(self.installed_catalogue.pool.clone());
        let existing = store
            .get(id)
            .await
            .map_err(|error| invalid(&error.to_string()))?;
        let definition = if let Some(existing) = existing {
            if existing.request != request
                || existing.profile != profile
                || existing.revision != expected_revision
                || existing.operation != "generate"
                || existing.mode != "source_only"
                || existing.created_by != caller.user_id.as_str()
                || existing.scope.tenant_id != caller.tenant_id
                || existing.scope.project_id != caller.project_id
            {
                return Err(invalid(
                    "prefix job request conflicts with its retained definition",
                ));
            }
            existing
        } else {
            crate::prefix_job_execution::validate_snapshot(profile, snapshot_bytes).map_err(
                |_| {
                    invalid("canonical source snapshot/procedures must match the selected compiler")
                },
            )?;
            let thread = ThreadId::new(format!("prefix-job-{id}"))
                .map_err(|_| invalid("invalid prefix job identity"))?;
            let scope = TurnScope::new_with_owner(
                caller.tenant_id.clone(),
                Some(self.thread_scope.agent_id.clone()),
                caller.project_id.clone(),
                thread,
                Some(caller.user_id.clone()),
            );
            let thread_scope = brassclaw_threads::ThreadScope {
                tenant_id: caller.tenant_id.clone(),
                agent_id: self.thread_scope.agent_id.clone(),
                project_id: caller.project_id.clone(),
                owner_user_id: Some(caller.user_id.clone()),
            };
            let program = self
                .installed_catalogue
                ._compiler_program
                .as_ref()
                .ok_or_else(|| invalid("source compiler qualification unavailable"))?;
            let input = serde_json::json!({"profile":profile,"revision":expected_revision.to_string(),"snapshot_bytes":snapshot_bytes});
            store
                .reserve(Reservation {
                    id,
                    profile,
                    revision: expected_revision,
                    operation: "generate",
                    mode: "source_only",
                    request: &request,
                    program,
                    input: &input,
                    scope: &scope,
                    thread_scope: &thread_scope,
                    created_by: caller.user_id.as_str(),
                })
                .await
                .map_err(|error| invalid(&error.to_string()))?
        };
        let conversation = ConversationId(definition.scope.thread_id.clone());
        let send_lock = self.send_lock_for(&conversation).await;
        let _guard = send_lock.lock().await;
        self.thread_service
            .ensure_thread(EnsureThreadRequest {
                scope: definition.thread_scope.clone(),
                thread_id: Some(conversation.0.clone()),
                created_by_actor_id: definition.created_by.clone(),
                title: Some(format!("Prefix build: {}", definition.profile)),
                metadata_json: None,
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;
        // This existing transport map is reconstructed from immutable storage on
        // every submission/reconnect; it is not the owner of durable job scope.
        self.internal_conversation_scopes.lock().await.insert(
            conversation.clone(),
            (definition.thread_scope, definition.scope),
        );
        let source =
            SourceBindingRef::new(brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING)
                .map_err(|reason| invalid(&reason))?;
        let admitted = self
            .admit_user_message_internal(
                &conversation,
                &format!("prefix-job:{id}"),
                &CancellationToken::new(),
                Some(source),
                Some(id),
            )
            .await?;
        if admitted.run_id.as_uuid() != id {
            return Err(invalid("prefix job correlation differs from admitted run"));
        }
        Ok(id)
    }
}

#[cfg(test)]
pub(super) async fn assert_global_snapshot_submission(runtime: &Arc<RebornRuntime>) -> Uuid {
    use serde_json::{Value, json};
    let id = Uuid::new_v4();
    let pool = runtime.installed_catalogue.pool.clone();
    let client = pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    drop(client);
    let fixture = crate::bootstrap_compiler_fixture::CompilerFixture::scoped(
        "base-prompt",
        "literal-reference/v2-deduplicated",
        &format!("global-source-job/{id}"),
    )
    .unwrap();
    let snapshot = crate::prefix_generations::canonical(fixture.snapshot()).unwrap();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    let rejected = Uuid::new_v4();
    let mut misidentified = fixture.snapshot().clone();
    misidentified["procedures"]["compiler/entry.py"] = json!("different procedure");
    assert!(
        runtime
            .submit_prefix_generation_snapshot(
                rejected,
                "base-prompt",
                revision as u64,
                &crate::prefix_generations::canonical(&misidentified).unwrap(),
                caller.clone()
            )
            .await
            .is_err()
    );
    assert!(
        crate::prefix_job_definitions::Store::new(pool.clone())
            .get(rejected)
            .await
            .unwrap()
            .is_none()
    );
    let identity = runtime.global_monty_owner.client().root_identity();
    assert_eq!(
        runtime
            .submit_prefix_generation_snapshot(
                id,
                "base-prompt",
                revision as u64,
                &snapshot,
                caller.clone()
            )
            .await
            .unwrap(),
        id
    );
    let definition = crate::prefix_job_definitions::Store::new(pool.clone())
        .get(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .wait_for_terminal(
                &definition.scope,
                TurnRunId::from_uuid(id),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Completed
    );
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "settled");
    let outcome: Value = row.get(1);
    assert_eq!(outcome["status"], "completed");
    assert!(outcome.get("reply_ref").is_none());
    assert_eq!(
        outcome["execution"]["format"],
        "monty-internal-task-execution/1"
    );
    let receipt = outcome["internal_receipt"].as_str().unwrap();
    let generation = receipt
        .strip_prefix("prefix-generation:base-prompt:")
        .unwrap();
    let bytes: String = client.query_one("SELECT generation_bytes FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1", &[&generation]).await.unwrap().get(0);
    fixture.verify(&bytes).unwrap();
    let count = || async {
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get::<_, i64>(0)
    };
    assert_eq!(count().await, 5);
    // A reconnect uses the retained snapshot even if its caller now offers
    // unrelated text. Durable ingress deduplication must not replay effects.
    runtime
        .submit_prefix_generation_snapshot(
            id,
            "base-prompt",
            revision as u64,
            "a changed capture is never substituted",
            caller.clone(),
        )
        .await
        .unwrap();
    assert_eq!(count().await, 5);
    assert!(
        runtime
            .submit_prefix_generation_snapshot(
                id,
                "prefix-maker",
                revision as u64,
                &snapshot,
                caller
            )
            .await
            .is_err()
    );
    let core_started = std::time::Instant::now();
    let core_id = Uuid::new_v4();
    let core_caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    runtime
        .submit_current_core_prefix_snapshot(
            core_id,
            "base-prompt",
            revision as u64,
            core_caller.clone(),
        )
        .await
        .unwrap();
    let core = crate::prefix_job_definitions::Store::new(pool.clone())
        .get(core_id)
        .await
        .unwrap()
        .unwrap();
    let status = runtime
        .wait_for_terminal(
            &core.scope,
            TurnRunId::from_uuid(core_id),
            &CancellationToken::new(),
        )
        .await;
    if !matches!(&status, Ok(TurnStatus::Completed)) {
        let admission = client.query_opt("SELECT phase,outcome->>'reason_kind' FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&core_id]).await.unwrap().map(|row| (row.get::<_, String>(0),row.get::<_, Option<String>>(1)));
        let calls = client.query("SELECT step_id,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id", &[&core_id]).await.unwrap().into_iter().map(|row| (row.get::<_, String>(0),row.get::<_, String>(1))).collect::<Vec<_>>();
        panic!(
            "core source job did not complete: {status:?}; admission={admission:?}; calls={calls:?}"
        );
    }
    eprintln!(
        "core source job elapsed_ms={} snapshot_bytes={}",
        core_started.elapsed().as_millis(),
        core.input["snapshot_bytes"].as_str().unwrap().len()
    );
    assert!(
        runtime
            .read_latest_assistant_text(
                &core.thread_scope,
                &core.scope.thread_id,
                TurnRunId::from_uuid(core_id)
            )
            .await
            .unwrap()
            .is_none()
    );
    let source: Value =
        serde_json::from_str(core.input["snapshot_bytes"].as_str().unwrap()).unwrap();
    assert!(
        source["header"]
            .as_str()
            .unwrap()
            .contains("Other runtime-private workflows")
    );
    assert!(
        source["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["path"] == "skills.md"
                && source["text"] == include_str!("../../../skills.md"))
    );
    runtime
        .submit_current_core_prefix_snapshot(core_id, "base-prompt", revision as u64, core_caller)
        .await
        .unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&core_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        5
    );
    assert_eq!(
        runtime.global_monty_owner.client().root_identity(),
        identity
    );
    assert_eq!(
        outcome["execution"]["all_selected_recipes_complete"],
        json!(true)
    );
    core_id
}
