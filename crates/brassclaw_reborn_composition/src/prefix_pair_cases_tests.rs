//! Development qualification of source projection, contained Monty stages and
//! actual retained-asset native encoding. Not an admitted/approved Recipe, final
//! provider transport, full-prefix capacity, cache or answer-quality acceptance.
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_host_runtime::ModelEncodingBackend;
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn canonical(value: &Value) -> String {
    crate::prefix_generations::canonical(value).unwrap()
}
async fn stage(phase: &str, data: Value) -> Result<Value, String> {
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let output = utility::execute(
        &installed_worker().unwrap(),
        UtilityRequest::Evaluate {
            source: format!(
                "{}\npair_case_stage(inputs)\n",
                brassclaw_skills::prefix_pair_case_components::CASE_SOURCE
            ),
            inputs: BTreeMap::from([(
                "inputs".into(),
                json!({"phase":phase,"data_bytes":canonical(&data)}),
            )]),
            bounds: limits.bounds,
            max_compute_time: Duration::from_secs(10),
        },
        limits.process,
    )
    .await
    .map_err(|error| format!("{error:?}"))?;
    let UtilityOutput::Evaluated { value, stdout } = output else {
        return Err("actual case result required".into());
    };
    assert!(stdout.is_empty());
    let text = value.as_str().ok_or("literal case result required")?;
    serde_json::from_str(text).map_err(|error| error.to_string())
}
fn arguments(prepared: &Value, observation: Value) -> Value {
    json!({"bank_bytes":prepared["bank_bytes"],"digests":prepared["texts"].as_array().unwrap().iter()
        .map(|text|sha(text.as_str().unwrap().as_bytes())).collect::<Vec<_>>(),"observation_bytes":observation})
}

#[tokio::test]
async fn native_pair_cases_use_actual_store_original_template_worker_and_monty() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-pair-case-projection";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), namespace);
    repo.initialize().await.unwrap();
    let definition=serde_json::from_value(json!({"id":"pair-case-retained","aliases":[],"protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:1/v1","base_url_required":true,"api_key_required":false,
        "model_env":"UNUSED_PAIR_CASE_FIXTURE","default_model":"retained-ornith","description":"Offline retained template only"})).unwrap();
    let provider = repo.replace_revision(definition, None).await.unwrap();
    let models =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(rig.pool.clone(), namespace.into());
    let specification = models
        .capture(
            brassclaw_product_workflow::CapturePrefixModelRequest {
                provider_id: "pair-case-retained".into(),
                expected_provider_revision: provider.revision,
                model: "retained-ornith".into(),
            },
            vec![brassclaw_product_workflow::PrefixModelTarget {
                provider_id: "pair-case-retained".into(),
                provider_revision: provider.revision,
                model: "retained-ornith".into(),
                consumer: "configured".into(),
                configured_context_window: None,
            }],
        )
        .await
        .unwrap();
    let directory = PathBuf::from(
        std::env::var("BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT")
            .expect("actual frozen originals required"),
    );
    let mut files = serde_json::Map::new();
    let mut originals = BTreeMap::new();
    for name in [
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "chat_template.jinja",
        "vocab.json",
        "merges.txt",
    ] {
        let bytes = std::fs::read(directory.join(name)).unwrap();
        files.insert(
            name.into(),
            json!({"encoding":"base64","data":STANDARD.encode(&bytes),"sha256":sha(&bytes)}),
        );
        originals.insert(name, bytes);
    }
    let assets = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        rig.pool.clone(),
        namespace.into(),
    )
    .retain_with_origin(
        brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
            specification: specification.id.clone(),
            artifact_set_bytes:
                json!({"format":"brassclaw-prefix-model-artifact-set/1","artifacts":files})
                    .to_string(),
        },
        crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {
            actor: "native-pair-case-operator".into(),
        },
    )
    .await
    .unwrap();
    let fixture = std::process::Command::new("python3")
        .arg("-B")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/prefix/test_prefix_pair_cases.py"),
        )
        .arg("--native-fixture")
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    assert!(
        fixture.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let fixture: Value = serde_json::from_slice(&fixture.stdout).unwrap();
    let mut document = crate::prefix_pair_construction::tests::draft(
        &specification.id,
        &assets.id,
        "chat_template.jinja",
        &sha(&originals["chat_template.jinja"]),
        Value::Null,
        &sha(&originals["chat_template.jinja"]),
        &sha(&originals["config.json"]),
    );
    document["sample_reference"] = fixture["sample_reference"].clone();
    document["worked_cases"] = fixture["cases"].clone();
    document["construction"]["supported_modes"] = json!(
        fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["mode"].clone())
            .collect::<Vec<_>>()
    );
    let retained = models
        .retain_pair_proposal(
            brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                specification: specification.id.clone(),
                proposal_bytes: document.to_string(),
            },
        )
        .await
        .unwrap();
    let packet = models
        .case_packet(&specification.id, &retained.id)
        .await
        .unwrap();
    assert_eq!(packet.proposal, retained.id);
    let prepared = stage(
        "prepare",
        json!({"case_packet_bytes":packet.case_packet_bytes}),
    )
    .await
    .unwrap();
    assert_eq!(
        prepared["texts"].as_array().unwrap().len(),
        1 + 3 * fixture["cases"].as_array().unwrap().len()
    );
    let settings = Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
    // The real retained-tokenizer fixture uses the same explicit 1 GiB utility
    // setting as runtime/tests/model_encoding_tests.rs. The production default
    // is unchanged; its insufficient-memory result remains in run-1 evidence.
    let fixture_limits = brassclaw_host_api::MontyExecutionLimits {
        utility_worker_memory_bytes: 1024 * 1024 * 1024,
        ..Default::default()
    };
    settings
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: 1,
            limits: fixture_limits,
        })
        .unwrap();
    let worker = Arc::new(crate::source_inspection::ContainedSourceInspection::new());
    worker.bind(settings).await.unwrap();
    let backend = crate::model_encoding::ContainedModelEncoding::new(rig.pool.clone(), worker);
    let scope = brassclaw_host_api::ResourceScope {
        tenant_id: brassclaw_host_api::TenantId::new(namespace).unwrap(),
        user_id: brassclaw_host_api::UserId::new("pair-case-operator").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: brassclaw_host_api::InvocationId::new(),
    };
    let result = backend
        .encode(
            &specification.id,
            &assets.id,
            prepared["probes_json"].as_str().unwrap(),
            None,
            &scope,
        )
        .await
        .unwrap();
    let observed = result.output;
    let compared = stage("compare", arguments(&prepared, observed.clone()))
        .await
        .unwrap();
    assert_eq!(compared["all_cases_match"], true);
    assert_eq!(compared["stable_prefix_encoding"], true);
    assert_eq!(compared["provider_wire_qualified"], false);
    assert!(compared["context_fit"].is_null());
    assert_eq!(compared["qualification"], "retained-template-cases-only");
    assert_eq!(
        compared["cases"].as_array().unwrap().len(),
        fixture["cases"].as_array().unwrap().len()
    );
    for (pointer, value) in [
        ("/template_origin", json!("supplied-template")),
        (
            "/report/probes/0/partition/render_context_sha256",
            json!("0".repeat(64)),
        ),
    ] {
        let mut forged: Value = serde_json::from_str(observed.as_str().unwrap()).unwrap();
        *forged.pointer_mut(pointer).unwrap() = value;
        assert!(
            stage("compare", arguments(&prepared, json!(canonical(&forged))))
                .await
                .is_err(),
            "{pointer}"
        );
    }
    let mut wrong = document.clone();
    wrong["worked_cases"][0]["formatted_prompt"] = json!(format!(
        "{}wrong",
        document["worked_cases"][0]["formatted_prompt"]
            .as_str()
            .unwrap()
    ));
    wrong["worked_cases"][0]["expected_request"] = json!(format!(
        "{}wrong",
        document["worked_cases"][0]["expected_request"]
            .as_str()
            .unwrap()
    ));
    let wrong = models
        .retain_pair_proposal(
            brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                specification: specification.id.clone(),
                proposal_bytes: wrong.to_string(),
            },
        )
        .await
        .unwrap();
    let packet = models
        .case_packet(&specification.id, &wrong.id)
        .await
        .unwrap();
    let wrong_prepared = stage(
        "prepare",
        json!({"case_packet_bytes":packet.case_packet_bytes}),
    )
    .await
    .unwrap();
    let wrong_observed = backend
        .encode(
            &specification.id,
            &assets.id,
            wrong_prepared["probes_json"].as_str().unwrap(),
            None,
            &scope,
        )
        .await
        .unwrap()
        .output;
    let mismatch = stage("compare", arguments(&wrong_prepared, wrong_observed))
        .await
        .unwrap();
    assert_eq!(mismatch["all_cases_match"], false);
    assert_eq!(mismatch["cases"][0]["matches"], false);
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_generations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    eprintln!(
        "pair default-template cases: four independent HF contexts match actual retained-asset worker/Monty; changed suffix is false; no acceptance or serving"
    );
}

#[tokio::test]
async fn pair_case_recipe_retains_original_usages_and_complete_preload_graph() {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        },
    };
    use brassclaw_skills::{
        component_revision::RetainedComponentSnapshot,
        prefix_pair_case_components::{PairCaseIds, pair_case_drafts},
    };
    let native = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let identity = |name| {
        serde_json::to_value(
            native
                .native_identity(&brassclaw_host_api::CapabilityId::new(name).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let digest =
        brassclaw_skills::utf8_digest_components::usage_ids(Uuid::new_v4(), |_| Uuid::new_v4());
    let encoding =
        brassclaw_skills::model_encoding_components::usage_ids(Uuid::new_v4(), false, |_| {
            Uuid::new_v4()
        });
    let ids = PairCaseIds::allocate(|_| Uuid::new_v4());
    let mut drafts = pair_case_drafts(ids, digest, encoding).unwrap();
    assert!(
        pair_case_drafts(
            PairCaseIds {
                prepare: ids.library,
                ..ids
            },
            digest,
            encoding
        )
        .is_err()
    );
    drafts.extend(
        brassclaw_skills::utf8_digest_components::utf8_digest_usage_drafts(digest).unwrap(),
    );
    drafts.extend(
        brassclaw_skills::model_encoding_components::model_encoding_usage_drafts(encoding).unwrap(),
    );
    drafts.push(
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            digest.tool,
            identity(brassclaw_host_runtime::JSON_CAPABILITY_ID),
        )
        .unwrap(),
    );
    drafts.push(
        brassclaw_skills::model_encoding_components::model_encoding_tool_draft(
            encoding.tool,
            identity(brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID),
        )
        .unwrap(),
    );
    let snapshot = RetainedComponentSnapshot::new(
        &[ids.recipe, digest.recipe, encoding.recipe],
        drafts
            .into_iter()
            .map(|draft| draft.at_version(1).unwrap())
            .collect(),
    )
    .unwrap();
    let graph = Arc::new(snapshot.select_roots(&[ids.recipe]).unwrap());
    let program = prepare_retained_tool_program(
        compile_retained_recipe(graph, ids.recipe, "selected", WorkflowClass::Deterministic)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(program.bindings().len(), 2);
    assert_eq!(program.bindings()["0:3"].skill().uuid, digest.skill);
    assert_eq!(program.bindings()["0:5"].skill().uuid, encoding.skill);
    assert!(
        program.inputs().instruction().snapshot().revisions()[&ids.recipe]
            .draft()
            .document()["input_layouts"]["selected"]["steps"]["0:5"]
            .get("template_utf8")
            .is_none()
    );
    InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
}
