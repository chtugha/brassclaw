//! Closed candidate execution against real Monty, kernel HTTP and PostgreSQL.
//! This is not ordinary job admission, durable invocation accounting or approval.
use brassclaw_engine::{
    executor::{
        retained_recipe::{
            RetainedProgram, RetainedRecipeExecution, RetainedToolInvocation, RetainedToolPort,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolBinding, prepare_retained_tool_program},
        typed_bindings::PreparedReference,
    },
};
use brassclaw_host_api::*;
use brassclaw_host_runtime::*;
use brassclaw_monty_host::{
    global::GlobalBounds,
    heap::HeapSettings,
    process::{PortAnswer, RootBoot, TaskSettings, installed_worker},
    transport_actor::ActorLimits,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_skills::{
    component_revision::RetainedComponentSnapshot, global_bootstrap_components::UsageComponentIds,
    prefix_registered_source_components::*, prefix_source_check_components::HttpSourceCheckIds,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

fn usage(tool: Uuid, parser: bool) -> UsageComponentIds {
    UsageComponentIds {
        tool,
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        formatter: parser.then(Uuid::new_v4),
    }
}
fn ids() -> RegisteredSourceCheckIds {
    let source = Uuid::new_v4();
    RegisteredSourceCheckIds {
        check: HttpSourceCheckIds {
            recipe: Uuid::new_v4(),
            preflight: Uuid::new_v4(),
            texts: Uuid::new_v4(),
            digest_value: Uuid::new_v4(),
            classifier: Uuid::new_v4(),
            http: usage(Uuid::new_v4(), false),
            digest: usage(Uuid::new_v4(), true),
        },
        read: usage(source, true),
        retain: usage(source, true),
        parse: Uuid::new_v4(),
        prepare: Uuid::new_v4(),
        pack: Uuid::new_v4(),
        receipt: Uuid::new_v4(),
    }
}
fn canonical(value: &Value) -> String {
    crate::prefix_generations::canonical(value).unwrap()
}

// A test-only transport adapter to the actual retained kernel. It supplies no
// fabricated outcomes, DB receipts, workflow policy or production approval.
struct NativePort {
    runtimes: BTreeMap<String, RetainedFirstPartyCapability>,
    effects: Vec<EffectKind>,
}
#[async_trait::async_trait]
impl RetainedToolPort for NativePort {
    async fn dispatch(
        &self,
        _invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        use brassclaw_trust::{
            AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance,
        };
        let capability = CapabilityId::new(binding.capability_id()).unwrap();
        let execution = ExecutionContext::local_default(
            UserId::new("registered-source-operator").unwrap(),
            ExtensionId::new("registered-source-candidate").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        let request = RuntimeCapabilityRequest::new(
            execution,
            capability,
            ResourceEstimate::default(),
            arguments,
            TrustDecision {
                effective_trust: EffectiveTrustClass::user_trusted(),
                authority_ceiling: AuthorityCeiling {
                    allowed_effects: self.effects.clone(),
                    max_resource_ceiling: None,
                },
                provenance: TrustProvenance::AdminConfig,
                evaluated_at: chrono::Utc::now(),
            },
        );
        match self.runtimes[binding.capability_id()]
            .invoke(request)
            .await
            .unwrap()
        {
            RuntimeCapabilityOutcome::Completed(done) => PortAnswer::Return {
                value: done.output.into(),
            },
            RuntimeCapabilityOutcome::Failed(_) => PortAnswer::TerminalError {
                reason_kind: "candidate_native_failure".into(),
            },
            other => panic!("unexpected real runtime outcome: {}", other.kind()),
        }
    }
}

#[tokio::test]
async fn registered_source_recipe_reads_fetches_hashes_and_retains_unqualified_diagnostics() {
    use brassclaw_authorization::{
        InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
        ToolExecutionRules,
    };
    use brassclaw_product_workflow::{RegisterPrefixSourceRequest, WebUiAuthenticatedCaller};
    use brassclaw_trust::{AdminConfig, AdminEntry, HostTrustAssignment, HostTrustPolicy};
    let rig = crate::runtime::test_pg::pg_rig().await;
    crate::prefix_profile_seeds::retain(&rig.pool)
        .await
        .unwrap();
    let original = "Complete original: ä\r\n'; host.forbidden() #\r\n".repeat(3000);
    assert!(
        original.len() > 65536,
        "complete response exceeds observation row capacity"
    );
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let body = original.clone();
    let app = axum::Router::new().route(
        "/document",
        axum::routing::get(move || {
            let count = calls.clone();
            let body = body.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                body
            }
        }),
    );
    let (address, stop, _server) = crate::test_loopback_http::spawn(app).await;
    let ids = ids();
    let effects = vec![
        EffectKind::DispatchCapability,
        EffectKind::Network,
        EffectKind::ExternalWrite,
    ];
    let capabilities = [
        ("builtin.http", ids.check.http.tool),
        ("builtin.json", ids.check.digest.tool),
        (PREFIX_SOURCE_CAPABILITY_ID, ids.read.tool),
    ];
    let policy = |revision, http_enabled| StableToolPolicySnapshot {
        revision,
        capabilities: capabilities
            .iter()
            .map(|(name, id)| (CapabilityId::new(*name).unwrap(), *id))
            .collect(),
        tools: capabilities
            .iter()
            .map(|(name, id)| {
                (
                    *id,
                    InstanceToolRule {
                        enabled: *name != "builtin.http" || http_enabled,
                        revision,
                        execution: ToolExecutionRules {
                            allowed_effects: effects.clone(),
                            mounts: MountView::default(),
                            network: NetworkPolicy {
                                allowed_targets: vec![NetworkTargetPattern {
                                    scheme: Some(NetworkScheme::Http),
                                    host_pattern: "127.0.0.1".into(),
                                    port: Some(address.port()),
                                }],
                                deny_private_ip_ranges: false,
                                max_egress_bytes: Some((original.len() * 2) as u64),
                            },
                            secrets: vec![],
                            resource_ceiling: None,
                        },
                    },
                )
            })
            .collect(),
    };
    let live = Arc::new(LiveStableToolPolicy::new(policy(1, true)).unwrap());
    let backend = Arc::new(crate::pg_prefix_source_backend::PgPrefixSourceBackend::new(
        rig.pool.clone(),
    ));
    let native = builtin_native_first_party_handlers(
        BuiltinFirstPartyTools::default().with_prefix_source_backend(backend),
        MontyExecutionLimits::default().max_native_artifact_bytes,
        NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10)).unwrap(),
    )
    .await
    .unwrap();
    let mut drafts = registered_source_check_drafts(ids).unwrap();
    let original_recipe =
        brassclaw_skills::prefix_source_check_components::http_source_check_drafts(ids.check)
            .unwrap()
            .into_iter()
            .find(|draft| draft.uuid() == ids.check.recipe)
            .unwrap();
    let extended_recipe = drafts
        .iter()
        .find(|draft| draft.uuid() == ids.check.recipe)
        .unwrap();
    for field in ["input_layouts", "invocations"] {
        assert_eq!(
            extended_recipe.document()[field]["selected"],
            original_recipe.document()[field]["selected"]
        );
    }
    assert_eq!(
        extended_recipe.document()["step_descriptions"][0],
        original_recipe.document()["step_descriptions"][0]
    );

    let mut invalid = ids;
    invalid.prepare = ids.parse;
    assert!(registered_source_check_drafts(invalid).is_err());
    invalid = ids;
    invalid.retain.tool = Uuid::new_v4();
    assert!(registered_source_check_drafts(invalid).is_err());
    drafts.push(
        brassclaw_skills::http_get_components::http_tool_draft(
            ids.check.http.tool,
            serde_json::to_value(
                native
                    .native_identity(&CapabilityId::new("builtin.http").unwrap())
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    drafts.push(
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            ids.check.digest.tool,
            serde_json::to_value(
                native
                    .native_identity(&CapabilityId::new("builtin.json").unwrap())
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    drafts.push(
        brassclaw_skills::prefix_source_components::source_tool_draft(
            ids.read.tool,
            serde_json::to_value(
                native
                    .native_identity(&CapabilityId::new(PREFIX_SOURCE_CAPABILITY_ID).unwrap())
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let retained = Arc::new(
        RetainedComponentSnapshot::new(
            &[
                ids.check.recipe,
                ids.check.http.recipe,
                ids.check.digest.recipe,
                ids.read.recipe,
                ids.retain.recipe,
            ],
            drafts
                .into_iter()
                .map(|d| d.at_version(1).unwrap())
                .collect(),
        )
        .unwrap(),
    );
    // Existing direct diagnostics are still a separate supported variant.
    for (variant, count) in [("selected", 2), ("registered", 4)] {
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(retained.select_roots(&[ids.check.recipe]).unwrap()),
                ids.check.recipe,
                variant,
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(program.bindings().len(), count);
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(Arc::new(program)),
            &installed_worker().unwrap(),
        )
        .await
        .unwrap();
    }
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(retained.select_roots(&[ids.check.recipe]).unwrap()),
                ids.check.recipe,
                "registered",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(program.bindings()["1:3"].tool().uuid, ids.read.tool);
    assert_eq!(program.bindings()["1:7"].tool().uuid, ids.check.http.tool);
    assert_eq!(
        program.bindings()["1:10"].tool().uuid,
        ids.check.digest.tool
    );
    assert_eq!(program.bindings()["1:15"].tool().uuid, ids.read.tool);
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program.clone()),
            &installed_worker().unwrap(),
        )
        .await
        .unwrap(),
    );
    let mut extensions = brassclaw_extensions::ExtensionRegistry::new();
    extensions
        .insert(builtin_first_party_package().unwrap())
        .unwrap();
    let trust = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("builtin").unwrap(),
            "/system/extensions/builtin/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            effects.clone(),
            None,
        ),
    ]))])
    .unwrap();
    let services = HostRuntimeServices::new(
        Arc::new(extensions),
        Arc::new(brassclaw_filesystem::InMemoryBackend::new()),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
        Arc::new(InstanceToolAuthorizer::new(live.clone())),
        brassclaw_processes::ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("registered-source-candidate").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(native))
    .with_secret_store(Arc::new(brassclaw_secrets::InMemorySecretStore::new()))
    .with_trust_policy(Arc::new(trust))
    .with_runtime_policy(brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
        deployment: brassclaw_host_api::runtime_policy::DeploymentMode::LocalSingleUser,
        requested_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        resolved_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        filesystem_backend:
            brassclaw_host_api::runtime_policy::FilesystemBackendKind::HostWorkspace,
        process_backend: brassclaw_host_api::runtime_policy::ProcessBackendKind::LocalHost,
        network_mode: brassclaw_host_api::runtime_policy::NetworkMode::DirectLogged,
        secret_mode: brassclaw_host_api::runtime_policy::SecretMode::ScrubbedEnv,
        approval_policy: brassclaw_host_api::runtime_policy::ApprovalPolicy::AskDestructive,
        audit_mode: brassclaw_host_api::runtime_policy::AuditMode::LocalMinimal,
    })
    .try_with_host_http_egress(brassclaw_network::PolicyNetworkHttpEgress::new(
        brassclaw_network::ReqwestNetworkTransport::new(Duration::from_secs(5)),
    ))
    .unwrap();
    let snapshot = services.capture_first_party_capabilities().unwrap();
    let mut runtimes = BTreeMap::new();
    for (name, _) in capabilities {
        let capability = CapabilityId::new(name).unwrap();
        let selected = snapshot.retain(&capability).unwrap();
        let runtime = snapshot
            .retain_native(&capability, selected.native_identity().unwrap())
            .unwrap();
        runtime.verify_native_artifact().await.unwrap();
        runtimes.insert(name.into(), runtime);
    }
    let port = NativePort {
        runtimes,
        effects: effects.clone(),
    };
    let sources = crate::prefix_sources::PgPrefixSources::new(rig.pool.clone());
    let caller = WebUiAuthenticatedCaller::new(
        TenantId::new("default").unwrap(),
        UserId::new("registered-source-operator").unwrap(),
        None,
        None,
    );
    let limits = inspected.preparation_limits();
    let mut observations = 0i64;
    for case in ["complete", "missing", "wrong-pin", "overflow", "blocked"] {
        let path = if case == "missing" {
            "/missing"
        } else {
            "/document"
        };
        let registration = sources
            .register(
                "sempai",
                RegisterPrefixSourceRequest {
                    reference: format!("http://{address}{path}"),
                    title: format!("Pinned {path}"),
                    applicability: "Native registered-source candidate".into(),
                    required: false,
                },
                &caller,
            )
            .await
            .unwrap();
        let mut source = json!({"profile":"sempai","source_id":registration.id,"revision":registration.revision,"registration_sha256":registration.checksum});
        if case == "wrong-pin" {
            source["registration_sha256"] = json!("0".repeat(64));
        }
        if case == "blocked" {
            live.publish(1, policy(2, false)).unwrap();
        }
        let request = json!({"source":source,"response_body_limit":if case=="overflow" {1} else {original.len()+1024},"timeout_ms":5000});
        let task_inputs = program
            .inputs()
            .bind_task_inputs(&json!({"request_json":canonical(&request)}))
            .unwrap();
        // A utility worker's physical ceiling is not a global VM heap. Reserve
        // both global IPC frames and adapter headroom separately, deriving a
        // sufficient frame from the full fixture and captured source bounds.
        let fixture_bytes = serde_json::to_vec(&json!({"response": original}))
            .unwrap()
            .len();
        let frame = limits
            .bounds
            .max_source_bytes
            .max(limits.bounds.max_compiled_source_bytes)
            .max(limits.bounds.max_stdout_bytes)
            .max(fixture_bytes.checked_mul(2).unwrap());
        assert!(frame <= limits.process.max_frame_bytes);
        assert!(frame <= limits.bounds.max_value_bytes);
        let mut process = limits.process;
        process.max_frame_bytes = frame;
        let mut bounds = limits.bounds;
        bounds.max_value_bytes = frame;
        let adapter = DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
        let heap = limits
            .process
            .hard_memory_bytes
            .checked_sub(frame * 2 + adapter)
            .filter(|heap| *heap > frame)
            .unwrap();
        let boot = RootBoot {
            source: crate::bootstrap_reply_validation::ROOT.into(),
            checksum: Sha256::digest(crate::bootstrap_reply_validation::ROOT.as_bytes()).into(),
            aliases: ["await_next_task", "enter_task", "validation_hold"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            bounds: GlobalBounds {
                values: bounds,
                workers: 1,
                max_pending_calls: 2,
            },
            heap_settings: Some(HeapSettings {
                revision: 1,
                max_vm_bytes: heap,
            }),
            adapter_reserve_bytes: adapter,
            startup_timeout: Duration::from_secs(10),
            task_settings: TaskSettings {
                revision: 1,
                max_compute_time: Duration::from_secs(30),
                token_budgets_enabled: false,
            },
            max_recipe_contexts: 1,
        };
        let actor = ActorLimits {
            max_unclaimed: 2,
            max_reserved_frame_bytes: frame * 2,
            max_control_unclaimed: 2,
            max_control_reserved_frame_bytes: frame * 2,
        };
        // WorkerRecipes adds its fresh routing token to the six host fields.
        let input = json!({"conversation_id":"candidate","message_id":case,"turn_id":case,"run_id":case,"user_input":"contained source diagnostic","history":[]});
        let (mut owner, transport, _, task) = crate::bootstrap_validation::start(
            &installed_worker().unwrap(),
            boot,
            process,
            actor,
            input,
        )
        .await
        .unwrap();
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected.clone())
                .unwrap();
        let before = count.load(Ordering::SeqCst);
        let mut results = BTreeMap::<String, Value>::new();
        let mut failed = false;
        let mut failure = None;
        for step in program.inputs().instruction().ordered().step_order() {
            let Some(layout) = program.inputs().layout().steps().get(step) else {
                continue;
            };
            let mut supplied = serde_json::Map::new();
            for (name, reference) in layout {
                let value = match reference {
                    PreparedReference::Input { name } => task_inputs[name].clone(),
                    PreparedReference::Constant { value } => value.clone(),
                    PreparedReference::Result { step_id, path } => {
                        let mut value = &results[step_id];
                        for field in path {
                            value = &value[field];
                        }
                        value.clone()
                    }
                };
                supplied.insert(name.clone(), value);
            }
            let inputs = program
                .inputs()
                .bind_step_inputs(step, &Value::Object(supplied))
                .unwrap();
            match execution
                .run_step(&transport, step, &inputs, Some(&port))
                .await
            {
                Ok(value) => {
                    results.insert(step.clone(), value);
                }
                Err(error) => {
                    failed = true;
                    failure = Some(format!("{error:?}"));
                    break;
                }
            }
        }
        assert_eq!(
            failed,
            !matches!(case, "complete" | "missing"),
            "{case}: {failure:?}"
        );
        if !failed {
            assert!(execution.is_complete());
            assert_eq!(execution.host_answers().len(), 4);
            let receipt = &results["1:16"];
            assert_eq!(receipt["storage_receipt"]["qualification"], "unqualified");
            let diagnostic: Value =
                serde_json::from_str(receipt["diagnostic_bytes"].as_str().unwrap()).unwrap();
            assert_eq!(diagnostic["source"], source);
            assert!(diagnostic["classification"]["expected_sha256"].is_null());
            if case == "complete" {
                assert_eq!(
                    diagnostic["classification"]["reason"],
                    "content-identity-not-established"
                );
                assert_eq!(
                    diagnostic["response"]["text_sha256"],
                    hex::encode(Sha256::digest(original.as_bytes()))
                );
                assert_eq!(results["1:7"]["body_text"], original);
                assert!(receipt["diagnostic_bytes"].as_str().unwrap().len() < 65536);
            } else {
                assert_eq!(diagnostic["classification"]["reason"], "missing");
            }
            let client = rig.pool.get().await.unwrap();
            let id: Uuid = receipt["storage_receipt"]["observation_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            let row=client.query_one("SELECT observation_bytes,checksum FROM reborn_prefix_source_observations WHERE observation_id=$1",&[&id]).await.unwrap();
            let bytes: String = row.get(0);
            let stored: Value = serde_json::from_str(&bytes).unwrap();
            assert_eq!(stored["diagnostic_bytes"], receipt["diagnostic_bytes"]);
            assert_eq!(stored["qualification"], "unqualified");
            assert_eq!(
                hex::encode(Sha256::digest(bytes.as_bytes())),
                row.get::<_, String>(1)
            );
            observations += 1;
        } else {
            assert!(!execution.is_complete());
            assert_eq!(
                execution.failed_step_id(),
                Some(if case == "wrong-pin" { "1:3" } else { "1:7" })
            );
            assert_eq!(
                execution.host_answers().len(),
                if case == "wrong-pin" { 1 } else { 2 }
            );
            assert_eq!(
                count.load(Ordering::SeqCst),
                before + usize::from(case == "overflow"),
                "no replay/fetch after failed pin or live block"
            );
        }
        owner.request_termination();
        let exit = owner.join().await.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(exit.containment_error.is_none());
        assert!(exit.reap_error.is_none());
        let client = rig.pool.get().await.unwrap();
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM reborn_prefix_source_observations",
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            observations
        );
        let inventory = sources.inventory("sempai", None).await.unwrap();
        assert!(!inventory.checks_available);
        assert!(
            inventory
                .registered
                .iter()
                .all(|r| r.availability == "not-checked"
                    && r.review_status == "pending-collection-review")
        );
    }
    stop.cancel();
}

#[tokio::test]
async fn registered_source_handoffs_reject_override_and_forged_qualification() {
    use brassclaw_skills::{
        component_revision::REVISION_LIMITS,
        value_contract::{InputContract, ValueContract},
    };
    let ids = ids();
    let drafts = registered_source_check_drafts(ids).unwrap();
    let source = json!({"profile":"sempai","source_id":Uuid::new_v4(),"revision":1,"registration_sha256":"a".repeat(64)});
    let registration = json!({"format":"brassclaw-prefix-source-registration/1","profile":"sempai","kind":"web","reference":"https://example.invalid/'; host.forbidden() #"});
    let detail = json!({"source":source,"registration_bytes":canonical(&registration)});
    let mut mismatch = detail.clone();
    mismatch["source"]["revision"] = json!(2);
    let mut component = registration.clone();
    component["kind"] = json!("component");
    let component_detail = json!({"source":source,"registration_bytes":canonical(&component)});
    let response = json!({"status":200,"headers":[],"body_text":"literal data","request_bytes":0,"response_bytes":12,"redaction_applied":false});
    let classification = json!({"availability":"check-error","reason":"content-identity-not-established","content_identity":"unverified","expected_sha256":null,"observed_sha256":"a".repeat(64)});
    let mut green = classification.clone();
    green["availability"] = json!("available");
    let prepared = json!({"url":"https://example.invalid/original","expected_sha256":null});
    let receipt = json!({"observation_id":Uuid::new_v4(),"checksum":"b".repeat(64),"qualification":"unqualified"});
    let cases = [
        (
            ids.parse,
            json!({"request_json":canonical(&json!({"source":source,"response_body_limit":1024,"timeout_ms":5000}))}),
            true,
        ),
        (
            ids.parse,
            json!({"request_json":canonical(&json!({"source":source,"response_body_limit":1024,"timeout_ms":5000,"url":"https://override.invalid"}))}),
            false,
        ),
        (
            ids.parse,
            json!({"request_json":canonical(&json!({"source":source,"response_body_limit":1024,"timeout_ms":5000,"expected_sha256":"a".repeat(64)}))}),
            false,
        ),
        (ids.prepare, json!({"source":source,"detail":detail}), true),
        (
            ids.prepare,
            json!({"source":source,"detail":mismatch}),
            false,
        ),
        (
            ids.prepare,
            json!({"source":source,"detail":component_detail}),
            false,
        ),
        (
            ids.pack,
            json!({"source":source,"prepared":prepared,"response":response,"classification":classification,"digest":"a".repeat(64)}),
            true,
        ),
        (
            ids.pack,
            json!({"source":source,"prepared":prepared,"response":response,"classification":green,"digest":"a".repeat(64)}),
            false,
        ),
        (
            ids.receipt,
            json!({"source":source,"diagnostic_bytes":"{}","receipt":receipt}),
            true,
        ),
        (
            ids.receipt,
            json!({"source":source,"diagnostic_bytes":"{}","receipt":{"observation_id":Uuid::new_v4(),"checksum":"b".repeat(64),"qualification":"trusted"}}),
            false,
        ),
    ];
    let limits = UtilityLimits::from_execution(MontyExecutionLimits::default()).unwrap();
    for (id, inputs, success) in cases {
        let draft = drafts.iter().find(|d| d.uuid() == id).unwrap();
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS)
            .unwrap()
            .bind(&inputs)
            .unwrap();
        let name = draft.document()["preload"]["default_export"]
            .as_str()
            .unwrap();
        let answer = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!(
                    "{}\n{name}(inputs)\n",
                    draft.document()["content"].as_str().unwrap()
                ),
                inputs: BTreeMap::from([("inputs".into(), inputs)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await;
        assert_eq!(answer.is_ok(), success, "{name}");
        if success {
            let UtilityOutput::Evaluated { value, stdout } = answer.unwrap() else {
                panic!("pure result required")
            };
            assert!(stdout.is_empty());
            ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
                .unwrap()
                .validate(&value)
                .unwrap();
        }
    }
}
