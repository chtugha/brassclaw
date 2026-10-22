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
        self.submit_current_prefix_snapshot(id, profile, expected_revision, caller, false)
            .await
    }

    /// Capture the declared release-document inventory and current eligible
    /// ordinary catalogue. It remains source-only and quarantined, not complete
    /// library/model/quality acceptance or normal Generate readiness.
    pub async fn submit_current_release_prefix_snapshot(
        &self,
        id: Uuid,
        profile: &str,
        expected_revision: u64,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
    ) -> Result<Uuid, RebornRuntimeError> {
        self.submit_current_prefix_snapshot(id, profile, expected_revision, caller, true)
            .await
    }

    async fn submit_current_prefix_snapshot(
        &self,
        id: Uuid,
        profile: &str,
        expected_revision: u64,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
        release_documents: bool,
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
                let captured = if release_documents {
                    self.installed_catalogue
                        .capture_declared_release_prefix_sources(profile)
                        .await
                } else {
                    self.installed_catalogue
                        .capture_core_prefix_sources(profile)
                        .await
                };
                captured.map_err(|error| RebornRuntimeError::InvalidArgument {
                    reason: error.to_string(),
                })?
            };
        self.submit_prefix_source_snapshot(
            id,
            profile,
            expected_revision,
            &snapshot,
            caller,
            release_documents,
        )
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
        self.submit_prefix_source_snapshot(
            id,
            profile,
            expected_revision,
            snapshot_bytes,
            caller,
            false,
        )
        .await
    }

    async fn submit_prefix_source_snapshot(
        &self,
        id: Uuid,
        profile: &str,
        expected_revision: u64,
        snapshot_bytes: &str,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
        release_documents: bool,
    ) -> Result<Uuid, RebornRuntimeError> {
        use crate::prefix_job_definitions::{Reservation, Store};
        let invalid = |reason: &str| RebornRuntimeError::InvalidArgument {
            reason: reason.into(),
        };
        if id.is_nil()
            || expected_revision == 0
            || !matches!(profile, "base-prompt" | "prefix-maker" | "sempai")
        {
            return Err(invalid("unsupported source-only prefix snapshot request"));
        }
        let mut request = serde_json::json!({"profile":profile,"expected_revision":expected_revision,"mode":"source_only"});
        if release_documents {
            request["source_capture"] = serde_json::json!("declared_release");
        }
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
        self.admit_prefix_definition(definition).await
    }

    /// Offline diagnostic over a retained generation and unaccepted design.
    /// Completion reports capacity only; it cannot select/activate either one.
    pub async fn submit_prefix_measurement(
        &self,
        id: Uuid,
        profile: &str,
        request: brassclaw_product_workflow::MeasurePrefixRequest,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
    ) -> Result<Uuid, RebornRuntimeError> {
        use crate::prefix_job_definitions::{Reservation, Store};
        let invalid = |reason: &str| RebornRuntimeError::InvalidArgument {
            reason: reason.into(),
        };
        if id.is_nil()
            || request.expected_profile_revision == 0
            || !matches!(profile, "base-prompt" | "prefix-maker" | "sempai")
            || caller.tenant_id.as_str() != self.webui_tenant_id()
        {
            return Err(invalid("invalid offline prefix measurement request"));
        }
        let metadata = crate::prefix_measurement_capture::metadata(profile, &request)
            .map_err(|_| invalid("invalid measurement metadata"))?;
        let store = Store::new(self.installed_catalogue.pool.clone());
        let definition =
            if let Some(existing) = store.get(id).await.map_err(|e| invalid(&e.to_string()))? {
                if existing.request != metadata
                    || existing.profile != profile
                    || existing.revision != request.expected_profile_revision
                    || existing.operation != "measure"
                    || existing.mode != "target"
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
                let input = crate::prefix_measurement_capture::capture(
                    self.installed_catalogue.pool.clone(),
                    caller.tenant_id.as_str(),
                    profile,
                    &request,
                )
                .await
                .map_err(|e| invalid(&e.to_string()))?;
                let program = self
                    .installed_catalogue
                    .measurement_qualification
                    .as_ref()
                    .ok_or_else(|| invalid("measurement workflow qualification unavailable"))?
                    .program();
                let scope = TurnScope::new_with_owner(
                    caller.tenant_id.clone(),
                    Some(self.thread_scope.agent_id.clone()),
                    caller.project_id.clone(),
                    ThreadId::new(format!("prefix-job-{id}"))
                        .map_err(|_| invalid("invalid prefix job identity"))?,
                    Some(caller.user_id.clone()),
                );
                let thread_scope = brassclaw_threads::ThreadScope {
                    tenant_id: caller.tenant_id,
                    agent_id: self.thread_scope.agent_id.clone(),
                    project_id: caller.project_id,
                    owner_user_id: Some(caller.user_id.clone()),
                };
                store
                    .reserve(Reservation {
                        id,
                        profile,
                        revision: request.expected_profile_revision,
                        operation: "measure",
                        mode: "target",
                        request: &metadata,
                        program: &program,
                        input: &input,
                        scope: &scope,
                        thread_scope: &thread_scope,
                        created_by: caller.user_id.as_str(),
                    })
                    .await
                    .map_err(|e| invalid(&e.to_string()))?
            };
        self.admit_prefix_definition(definition).await
    }

    async fn admit_prefix_definition(
        &self,
        definition: crate::prefix_job_definitions::Definition,
    ) -> Result<Uuid, RebornRuntimeError> {
        let invalid = |reason: &str| RebornRuntimeError::InvalidArgument {
            reason: reason.into(),
        };
        let id = definition.id;
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
pub(super) async fn assert_global_snapshot_submission(
    runtime: &Arc<RebornRuntime>,
) -> (Uuid, Uuid) {
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
    // Explicit experimental layout over the owner's real pinned core sources;
    // this changes neither installed defaults nor an accepted model design.
    let mut captured: Value = serde_json::from_str(
        &runtime
            .installed_catalogue
            .capture_core_prefix_sources("base-prompt")
            .await
            .unwrap(),
    )
    .unwrap();
    captured["evidence_view"] = json!("literal-reference/v5-contextual-sharing");
    let snapshot = crate::prefix_generations::canonical(&captured).unwrap();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    let rejected = Uuid::new_v4();
    let mut misidentified = captured.clone();
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
    crate::prefix_generations::validate_package("base-prompt", &bytes).unwrap();
    let package: Value = serde_json::from_str(&bytes).unwrap();
    use base64::Engine;
    let read = |name: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(package["artifacts"][name]["data"].as_str().unwrap())
            .unwrap()
    };
    assert_eq!(read("source_snapshot.json"), snapshot.as_bytes());
    let design: Value = serde_json::from_slice(&read("profile_design.json")).unwrap();
    assert_eq!(design["format"], "brassclaw-prefix-compiler-design/2");
    assert!(
        design["source_contexts"]
            .as_object()
            .unwrap()
            .values()
            .any(|scope| scope == "component-definition")
    );
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
    let scope_caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    assert!(
        runtime
            .submit_current_release_prefix_snapshot(
                core_id,
                "base-prompt",
                revision as u64,
                scope_caller
            )
            .await
            .is_err(),
        "a release request cannot silently reuse a core-only capture"
    );
    let release_id = Uuid::new_v4();
    let release_caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    runtime
        .submit_current_release_prefix_snapshot(
            release_id,
            "base-prompt",
            revision as u64,
            release_caller.clone(),
        )
        .await
        .unwrap();
    let release_definition = crate::prefix_job_definitions::Store::new(pool.clone())
        .get(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .wait_for_terminal(
                &release_definition.scope,
                TurnRunId::from_uuid(release_id),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Completed
    );
    let release_job = runtime
        .inspect_prefix_job("base-prompt", release_id)
        .await
        .unwrap();
    assert_eq!(release_job.answered_tool_calls, 5);
    assert_eq!(
        release_definition.request["source_capture"],
        "declared_release"
    );

    let release_capture: Value =
        serde_json::from_str(release_definition.input["snapshot_bytes"].as_str().unwrap()).unwrap();
    assert!(
        release_capture["sources"].as_array().unwrap().len()
            > source["sources"].as_array().unwrap().len()
    );
    assert!(
        release_capture["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["path"] == "scripts/prefix/release-reference-inventory.json")
    );
    runtime
        .submit_current_release_prefix_snapshot(
            release_id,
            "base-prompt",
            revision as u64,
            release_caller,
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .inspect_prefix_job("base-prompt", release_id)
            .await
            .unwrap()
            .answered_tool_calls,
        5
    );
    eprintln!(
        "declared release source job completed: sources={} snapshot_bytes={}; 5 native calls, no replay/activation",
        release_capture["sources"].as_array().unwrap().len(),
        release_definition.input["snapshot_bytes"]
            .as_str()
            .unwrap()
            .len()
    );
    let sempai_revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='sempai'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(
        runtime
            .installed_catalogue
            .capture_core_prefix_sources("sempai")
            .await
            .is_err()
    );
    let sempai_id = Uuid::new_v4();
    let sempai_caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    runtime
        .submit_current_release_prefix_snapshot(
            sempai_id,
            "sempai",
            sempai_revision as u64,
            sempai_caller.clone(),
        )
        .await
        .unwrap();
    let sempai = crate::prefix_job_definitions::Store::new(pool.clone())
        .get(sempai_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .wait_for_terminal(
                &sempai.scope,
                TurnRunId::from_uuid(sempai_id),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Completed
    );
    let sempai_capture: Value =
        serde_json::from_str(sempai.input["snapshot_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(
        sempai_capture["evidence_view"],
        "literal-reference/v5-contextual-sharing"
    );
    let selected = sempai_capture["sources"].as_array().unwrap();
    for path in [
        "skills.md",
        "recipe.md",
        "scripts/prefix/sempai-authoring-reference.md",
        "scripts/prefix/sempai-monty-compatibility.md",
        "scripts/prefix/monty-reference/v1.0.0/docs/host-functions.md",
    ] {
        assert!(
            selected
                .iter()
                .any(|row| row["path"] == path && row["disposition"] == "included")
        );
    }
    for path in [
        "scripts/prefix/sempai-worked-examples.md",
        "crates/brassclaw_engine/prompts/sempai_audit.md",
        "scripts/prefix/prefix-maker-reference.md",
    ] {
        assert!(
            selected
                .iter()
                .any(|row| row["path"] == path && row["disposition"] == "development-only")
        );
    }
    runtime
        .submit_current_release_prefix_snapshot(
            sempai_id,
            "sempai",
            sempai_revision as u64,
            sempai_caller,
        )
        .await
        .unwrap();
    let job = runtime
        .inspect_prefix_job("sempai", sempai_id)
        .await
        .unwrap();
    assert_eq!(job.answered_tool_calls, 5);
    assert_eq!(job.run_status, Some(TurnStatus::Completed));
    assert_eq!(job.admission_phase.as_deref(), Some("settled"));
    eprintln!(
        "Sempai declared-release source job: sources={}; 5 calls, no replay or activation",
        selected.len()
    );
    assert_eq!(
        runtime.global_monty_owner.client().root_identity(),
        identity
    );
    assert_eq!(
        outcome["execution"]["all_selected_recipes_complete"],
        json!(true)
    );
    (core_id, sempai_id)
}
