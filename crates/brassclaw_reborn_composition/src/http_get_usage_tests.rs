//! Separate structural/preload, pure classification and real retained-native
//! HTTP checks. None approves an association, updates legacy rows or activates
//! source availability checking; the native test uses only a loopback fixture.
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_monty_host::{process::installed_worker, utility::UtilityLimits};
use brassclaw_skills::{
    component_revision::RetainedComponentSnapshot, global_bootstrap_components::UsageComponentIds,
    http_get_components::*,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

use crate::test_loopback_http as loopback_http;

/// Real retained native HTTP and policy/transport behavior. This does not claim
/// durable source-job qualification or Sources UI activation.
#[tokio::test]
async fn retained_http_fetches_complete_content_and_obeys_live_policy_and_bounds() {
    use brassclaw_authorization::{
        InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
        ToolExecutionRules,
    };
    use brassclaw_host_api::*;
    use brassclaw_host_runtime::{
        CapabilitySurfaceVersion, HostRuntimeServices, RuntimeCapabilityOutcome,
        RuntimeCapabilityRequest, RuntimeFailureKind, builtin_first_party_package,
    };
    use brassclaw_trust::{
        AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
        HostTrustPolicy, TrustDecision, TrustProvenance,
    };
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = axum::Router::new()
        .route(
            "/document",
            axum::routing::get(move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    "Complete original: ä\n<script>source data</script>\n"
                }
            }),
        )
        .route(
            "/redirect",
            axum::routing::get(|| async { axum::response::Redirect::temporary("/document") }),
        );
    let (address, stop, _server) = loopback_http::spawn(app).await;
    let capability = CapabilityId::new("builtin.http").unwrap();
    let tool = Uuid::new_v4();
    let effects = vec![EffectKind::DispatchCapability, EffectKind::Network];
    let policy = |revision, enabled| StableToolPolicySnapshot {
        revision,
        capabilities: HashMap::from([(capability.clone(), tool)]),
        tools: HashMap::from([(
            tool,
            InstanceToolRule {
                enabled,
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
                        max_egress_bytes: Some(65536),
                    },
                    secrets: vec![],
                    resource_ceiling: None,
                },
            },
        )]),
    };
    let live = Arc::new(LiveStableToolPolicy::new(policy(1, true)).unwrap());
    let mut extensions = brassclaw_extensions::ExtensionRegistry::new();
    extensions
        .insert(builtin_first_party_package().unwrap())
        .unwrap();
    let native = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let identity = native.native_identity(&capability).unwrap();
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
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
        CapabilitySurfaceVersion::new("source-check-http-validation").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(native))
    .with_secret_store(Arc::new(brassclaw_secrets::InMemorySecretStore::new()))
    .with_trust_policy(Arc::new(trust_policy))
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
    let retained = snapshot.retain_native(&capability, identity).unwrap();
    retained.verify_native_artifact().await.unwrap();
    let invoke = |path: &str, limit| {
        let execution = ExecutionContext::local_default(
            UserId::new("source-check-operator").unwrap(),
            ExtensionId::new("source-check-validation").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        RuntimeCapabilityRequest::new(
            execution,
            capability.clone(),
            ResourceEstimate::default(),
            json!({"url":format!("http://{address}{path}"),"method":"get","response_body_limit":limit,"timeout_ms":5000}),
            TrustDecision {
                effective_trust: EffectiveTrustClass::user_trusted(),
                authority_ceiling: AuthorityCeiling {
                    allowed_effects: effects.clone(),
                    max_resource_ceiling: None,
                },
                provenance: TrustProvenance::AdminConfig,
                evaluated_at: chrono::Utc::now(),
            },
        )
    };
    let RuntimeCapabilityOutcome::Completed(done) =
        retained.invoke(invoke("/document", 65536)).await.unwrap()
    else {
        panic!("real complete HTTP response required")
    };
    assert_eq!(done.output["status"], 200);
    assert_eq!(
        done.output["body_text"],
        "Complete original: ä\n<script>source data</script>\n"
    );
    let RuntimeCapabilityOutcome::Completed(redirect) =
        retained.invoke(invoke("/redirect", 65536)).await.unwrap()
    else {
        panic!("redirect status must remain data")
    };
    assert_eq!(redirect.output["status"], 307);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "redirect must not follow another target"
    );
    let RuntimeCapabilityOutcome::Completed(missing) =
        retained.invoke(invoke("/missing", 65536)).await.unwrap()
    else {
        panic!("missing status must remain data")
    };
    assert_eq!(missing.output["status"], 404);
    assert!(
        matches!(
            retained.invoke(invoke("/document", 1)).await.unwrap(),
            RuntimeCapabilityOutcome::Failed(_)
        ),
        "overflow must fail, never return clipped successful source content"
    );
    let before = calls.load(Ordering::SeqCst);
    live.publish(1, policy(2, false)).unwrap();
    let RuntimeCapabilityOutcome::Failed(blocked) =
        retained.invoke(invoke("/document", 65536)).await.unwrap()
    else {
        panic!("current block must prevent a retained dispatch")
    };
    assert_eq!(blocked.kind, RuntimeFailureKind::Authorization);
    assert_eq!(calls.load(Ordering::SeqCst), before);
    live.publish(2, policy(3, true)).unwrap();
    assert!(matches!(
        retained.invoke(invoke("/document", 65536)).await.unwrap(),
        RuntimeCapabilityOutcome::Completed(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), before + 1);
    stop.cancel();
}

#[tokio::test]
async fn bounded_http_get_prepares_one_typed_effect_free_preload_usage() {
    // IDs stand for the exact existing family resolved by an installation owner;
    // this closed test never creates matching rows or claims bootstrap approval.
    let ids = UsageComponentIds {
        tool: Uuid::from_u128(81),
        tool_skill: Uuid::from_u128(82),
        skill: Uuid::from_u128(83),
        python_code: Uuid::from_u128(84),
        recipe: Uuid::from_u128(85),
        formatter: None,
    };
    let registry = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let identity = registry
        .native_identity(
            &brassclaw_host_api::CapabilityId::new(brassclaw_host_runtime::HTTP_CAPABILITY_ID)
                .unwrap(),
        )
        .unwrap();
    let mut drafts = bounded_http_get_drafts(ids).unwrap();
    drafts.push(http_tool_draft(ids.tool, serde_json::to_value(identity).unwrap()).unwrap());
    let snapshot = Arc::new(
        RetainedComponentSnapshot::new(
            &[ids.recipe],
            drafts
                .into_iter()
                .map(|draft| draft.at_version(1).unwrap())
                .collect(),
        )
        .unwrap(),
    );
    let instruction = compile_retained_recipe(
        snapshot,
        ids.recipe,
        "bounded",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let program = prepare_retained_tool_program(instruction).unwrap();
    assert_eq!(program.bindings().len(), 1);
    // Prepared bindings are indexed by their executable invocation step, not
    // the preceding metadata-only ToolSkill step.
    assert_eq!(program.bindings()["0:2"].tool().uuid, ids.tool);
    let inputs = json!({"url":"https://example.invalid/quoted?value='; result=999 #",
        "response_body_limit":65536,"timeout_ms":5000});
    assert_eq!(
        program.inputs().bind_step_inputs("0:2", &inputs).unwrap(),
        inputs
    );
    for (field, value) in [
        ("url", json!(null)),
        ("response_body_limit", json!(0)),
        ("response_body_limit", json!(true)),
        ("timeout_ms", json!(4294967296_u64)),
    ] {
        let mut invalid = inputs.clone();
        invalid[field] = value;
        assert!(
            program.inputs().bind_step_inputs("0:2", &invalid).is_err(),
            "{field}"
        );
    }
    let mut extra = inputs.clone();
    extra["method"] = json!("post");
    assert!(program.inputs().bind_step_inputs("0:2", &extra).is_err());
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    // The actual retained Monty inspector must accept definitions without
    // invoking host.http; no network callback or approval is supplied here.
    InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Tools(Arc::new(program)),
        &installed_worker().unwrap(),
        limits,
    )
    .await
    .unwrap();

    // The composed diagnostic reuses the same HTTP usage and JSON digest usage.
    // Distinct binding/invocation steps must preserve both real Tool identities.
    use brassclaw_skills::prefix_source_check_components::{
        HttpSourceCheckIds, http_source_check_drafts,
    };
    let digest_ids =
        brassclaw_skills::utf8_digest_components::usage_ids(Uuid::new_v4(), |_| Uuid::new_v4());
    let check_ids = HttpSourceCheckIds {
        recipe: Uuid::new_v4(),
        preflight: Uuid::new_v4(),
        texts: Uuid::new_v4(),
        digest_value: Uuid::new_v4(),
        classifier: Uuid::new_v4(),
        http: ids,
        digest: digest_ids,
    };
    let mut drafts = http_source_check_drafts(check_ids).unwrap();
    let preflight = drafts
        .iter()
        .find(|draft| draft.uuid() == check_ids.preflight)
        .unwrap();
    let preflight_source = format!(
        "{}\nvalidate_original_digest(inputs)\n",
        preflight.document()["content"].as_str().unwrap()
    );
    use brassclaw_monty_host::utility::{self, UtilityOutput, UtilityRequest};
    for expected in [
        serde_json::Value::Null,
        json!("a".repeat(64)),
        json!("A".repeat(64)),
        json!("too short"),
    ] {
        let answer = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: preflight_source.clone(),
                inputs: std::collections::BTreeMap::from([(
                    "inputs".into(),
                    json!({"expected_sha256":expected}),
                )]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await;
        if expected.is_null() || expected == "a".repeat(64) {
            let UtilityOutput::Evaluated { value, stdout } = answer.unwrap() else {
                panic!("actual identity preflight required")
            };
            assert_eq!(value, expected);
            assert!(stdout.is_empty());
        } else {
            assert!(
                answer.is_err(),
                "invalid expected identity must fail before any HTTP binding/invocation"
            );
        }
    }
    drafts.push(http_tool_draft(ids.tool, serde_json::to_value(identity).unwrap()).unwrap());
    let json_identity = registry
        .native_identity(&brassclaw_host_api::CapabilityId::new("builtin.json").unwrap())
        .unwrap();
    drafts.push(
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            digest_ids.tool,
            serde_json::to_value(json_identity).unwrap(),
        )
        .unwrap(),
    );
    let all = RetainedComponentSnapshot::new(
        &[check_ids.recipe, ids.recipe, digest_ids.recipe],
        drafts
            .into_iter()
            .map(|d| d.at_version(1).unwrap())
            .collect(),
    )
    .unwrap();
    let instruction = compile_retained_recipe(
        Arc::new(all.select_roots(&[check_ids.recipe]).unwrap()),
        check_ids.recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let program = prepare_retained_tool_program(instruction).unwrap();
    assert_eq!(program.bindings().len(), 2);
    assert_eq!(program.bindings()["0:3"].tool().uuid, ids.tool);
    assert_eq!(program.bindings()["0:6"].tool().uuid, digest_ids.tool);
    let mut check_input = inputs.clone();
    check_input["expected_sha256"] = serde_json::Value::Null;
    assert_eq!(
        program.inputs().bind_task_inputs(&check_input).unwrap(),
        check_input
    );
    InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Tools(Arc::new(program)),
        &installed_worker().unwrap(),
        limits,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn exact_source_classifier_requires_real_content_identity_not_http_success() {
    use brassclaw_monty_host::utility::{self, UtilityOutput, UtilityRequest};
    use brassclaw_skills::{
        component_revision::REVISION_LIMITS,
        prefix_source_check_components::*,
        value_contract::{InputContract, ValueContract},
    };
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    let classifier = exact_content_classifier_draft(Uuid::from_u128(91)).unwrap();
    let extractor = response_text_draft(Uuid::from_u128(92)).unwrap();
    let inputs =
        InputContract::from_value(&classifier.document()["input_contract"], REVISION_LIMITS)
            .unwrap();
    let result = ValueContract::result_from_value(
        &classifier.document()["result_contract"],
        REVISION_LIMITS,
    )
    .unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let body = "Complete original: ä\n<script>source data</script>\n";
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    let response = |status, text: &str| {
        json!({"status":status,"headers":[],"body_text":text,
        "request_bytes":0,"response_bytes":text.len(),"redaction_applied":false})
    };
    let mut binary = response(200, "");
    binary.as_object_mut().unwrap().remove("body_text");
    binary["body_base64"] = json!("AA==");
    let mut ambiguous = response(200, body);
    ambiguous["body_base64"] = json!("AA==");
    let cases = [
        (
            response(200, body),
            json!(digest),
            "available",
            "exact-content",
            "exact-original-match",
        ),
        (
            response(200, body),
            json!("a".repeat(64)),
            "check-error",
            "content-changed",
            "changed",
        ),
        (
            response(200, body),
            Value::Null,
            "check-error",
            "content-identity-not-established",
            "unverified",
        ),
        (
            response(200, " \t\n"),
            Value::Null,
            "unavailable",
            "empty-content",
            "not-checked",
        ),
        (
            response(404, "missing"),
            Value::Null,
            "unavailable",
            "missing",
            "not-checked",
        ),
        (
            response(401, "login"),
            Value::Null,
            "unavailable",
            "authentication-required",
            "not-checked",
        ),
        (
            response(302, "redirect"),
            Value::Null,
            "check-error",
            "redirect-not-followed",
            "not-checked",
        ),
        (
            response(429, "limited"),
            Value::Null,
            "check-error",
            "rate-limited",
            "not-checked",
        ),
        (
            response(503, "failure"),
            Value::Null,
            "check-error",
            "server-error",
            "not-checked",
        ),
        (
            binary,
            Value::Null,
            "check-error",
            "unsupported-content-representation",
            "not-checked",
        ),
        (
            ambiguous,
            Value::Null,
            "check-error",
            "invalid-response-representation",
            "not-checked",
        ),
    ];
    for (response, expected, availability, reason, identity) in cases {
        let extraction = json!({"response":response});
        let UtilityOutput::Evaluated {
            value: text,
            stdout,
        } = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!("{TEXT_SOURCE}\nhttp_reference_text(inputs)\n"),
                inputs: BTreeMap::from([("inputs".into(), extraction)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap()
        else {
            panic!("actual text projection required")
        };
        assert!(stdout.is_empty());
        let observed = hex::encode(Sha256::digest(text.as_str().unwrap().as_bytes()));
        let supplied =
            json!({"response":response,"expected_sha256":expected,"observed_sha256":observed});
        let bound = Value::Object(inputs.bind(&supplied).unwrap());
        let UtilityOutput::Evaluated { value, stdout } = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!("{CLASSIFY_SOURCE}\nclassify_exact_http_source(inputs)\n"),
                inputs: BTreeMap::from([("inputs".into(), bound)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap() else {
            panic!("actual classification required")
        };
        assert!(stdout.is_empty());
        result.validate(&value).unwrap();
        assert_eq!(value["availability"], availability, "{reason}");
        assert_eq!(value["reason"], reason);
        assert_eq!(value["content_identity"], identity);
        assert_eq!(value["expected_sha256"], expected);
        if availability == "available" {
            assert_eq!(value["observed_sha256"], digest);
        }
    }
    let extraction_contract =
        InputContract::from_value(&extractor.document()["input_contract"], REVISION_LIMITS)
            .unwrap();
    assert!(
        extraction_contract
            .bind(&json!({"response":response(200,body),"unexpected":true}))
            .is_err()
    );
    let invalid = json!({"response":response(200,body),"expected_sha256":null,"observed_sha256":"not a digest"});
    assert!(
        utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!("{CLASSIFY_SOURCE}\nclassify_exact_http_source(inputs)\n"),
                inputs: BTreeMap::from([("inputs".into(), invalid)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5)
            },
            limits.process
        )
        .await
        .is_err()
    );
}
