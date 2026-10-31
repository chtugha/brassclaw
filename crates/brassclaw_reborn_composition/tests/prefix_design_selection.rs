//! Actual Monty + real Rust tokenizer behavior and retained Recipe preparation.
//! This deliberately does not claim bootstrap approval, native effect custody,
//! final-envelope geometry, accepted model design, quality or serving activation.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn canonical(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}
async fn evaluate(source: &str, function: &str, inputs: Value) -> Value {
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let UtilityOutput::Evaluated { value, stdout } = utility::execute(
        &installed_worker().unwrap(),
        UtilityRequest::Evaluate {
            source: format!("{source}\n{function}(inputs)\n"),
            inputs: BTreeMap::from([("inputs".into(), inputs)]),
            bounds: limits.bounds,
            max_compute_time: Duration::from_secs(10),
        },
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("actual pure design result required")
    };
    assert!(stdout.is_empty());
    value
}

#[tokio::test]
async fn archived_original_projection_preserves_bytes_and_reuses_generation_read() {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        },
    };
    use brassclaw_skills::{
        component_revision::RetainedComponentSnapshot,
        global_bootstrap_components::UsageComponentIds,
        prefix_generation_original_components::{ORIGINAL_SOURCE, generation_original_drafts},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = std::process::Command::new("python3")
        .arg(root.join("scripts/prefix/test_prefix_generation_original.py"))
        .arg("--native-fixture")
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let fixture: Value = serde_json::from_slice(&fixture.stdout).unwrap();
    assert_eq!(
        evaluate(
            ORIGINAL_SOURCE,
            "project_prefix_generation_source",
            fixture["inputs"].clone()
        )
        .await,
        fixture["expected"]
    );
    assert_eq!(
        evaluate(
            ORIGINAL_SOURCE,
            "project_prefix_generation_source",
            fixture["historical"]["inputs"].clone()
        )
        .await,
        fixture["historical"]["expected"]
    );
    let native = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let capability = brassclaw_host_api::CapabilityId::new(
        brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID,
    )
    .unwrap();
    let read = UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool: Uuid::new_v4(),
        formatter: None,
    };
    let mut drafts = generation_original_drafts(read, Uuid::new_v4()).unwrap();
    assert!(generation_original_drafts(read, read.python_code).is_err());
    let recipe = drafts
        .iter()
        .find(|draft| draft.uuid() == read.recipe)
        .unwrap();
    assert!(
        recipe.document()["input_layouts"]["source"]["task_inputs"]
            .get("detail")
            .is_none()
    );
    assert_eq!(
        recipe.document()["input_layouts"]["source"]["steps"]["1:3"]["detail"]["kind"],
        "result"
    );
    drafts.push(
        brassclaw_skills::prefix_generation_components::generation_tool_draft(
            read.tool,
            serde_json::to_value(native.native_identity(&capability).unwrap()).unwrap(),
        )
        .unwrap(),
    );
    let snapshot = RetainedComponentSnapshot::new(
        &[read.recipe],
        drafts
            .into_iter()
            .map(|draft| draft.at_version(1).unwrap())
            .collect(),
    )
    .unwrap();
    for (variant, step) in [("selected", "0:2"), ("source", "1:2")] {
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(snapshot.select_roots(&[read.recipe]).unwrap()),
                read.recipe,
                variant,
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(program.bindings().len(), 1);
        assert_eq!(program.bindings()[step].tool().uuid, read.tool);
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(Arc::new(program)),
            &installed_worker().unwrap(),
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn profile_floor_and_design_sources_reuse_captured_compiler_requirements() {
    use brassclaw_skills::prefix_compiler_components::{COMPILER_SOURCE, UNIT_SOURCE};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = std::process::Command::new("python3")
        .arg(root.join("scripts/prefix/test_prefix_compile_logic.py"))
        .arg("--design-fixture")
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let fixture: Value = serde_json::from_slice(&fixture.stdout).unwrap();
    let mut inputs = fixture["inputs"].clone();
    inputs["phase"] = json!("design-inputs");
    let source = format!("{UNIT_SOURCE}\n{COMPILER_SOURCE}");
    let actual = evaluate(&source, "compile_stage", inputs).await;
    assert_eq!(
        serde_json::from_str::<Value>(actual.as_str().unwrap()).unwrap(),
        fixture["expected"]
    );
    // Declaration construction must expose the same retained typed stage.
    let stage = brassclaw_skills::prefix_compiler_components::compiler_stage_draft(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "design-inputs",
    )
    .unwrap();
    assert_eq!(
        stage.document()["preload"]["default_export"],
        "prepare_prefix_design_inputs"
    );
}

#[tokio::test]
async fn measured_design_uses_whole_units_and_actual_monty_and_tokenizer() {
    use brassclaw_skills::{
        prefix_compiler_components::UNIT_SOURCE, prefix_design_components::DESIGN_SOURCE,
        prefix_reference_components::REFERENCE_RENDER_SOURCE,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = std::process::Command::new("python3")
        .arg(root.join("scripts/prefix/test_prefix_design_selection.py"))
        .arg("--native-fixture")
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let fixture: Value = serde_json::from_slice(&fixture.stdout).unwrap();
    let source = format!("{UNIT_SOURCE}\n{REFERENCE_RENDER_SOURCE}\n{DESIGN_SOURCE}");
    let graph_source = format!(
        "{source}\ndef inspect_design_graph(inputs):\n    groups, order = design_groups(inputs['band'], inputs['units'])\n    return {{'order':order,'closure':design_closure(['g0000'], groups)}}\n"
    );
    let graph = evaluate(
        &graph_source,
        "inspect_design_graph",
        fixture["graph"].clone(),
    )
    .await;
    assert_eq!(graph["order"], fixture["graph"]["expected_order"]);
    let mut expected_closure = fixture["graph"]["expected_order"]
        .as_array()
        .unwrap()
        .clone();
    expected_closure.reverse();
    assert_eq!(graph["closure"], json!(expected_closure));
    // Exercise the same exported stage used by reusable PythonCode, rather
    // than only comparing the standalone CPython implementation with itself.
    let coverage = evaluate(
        &source,
        "prefix_design_stage",
        json!({"phase":"coverage","data_bytes":canonical(&fixture["coverage_inputs"])}),
    )
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(coverage.as_str().unwrap()).unwrap(),
        fixture["coverage"]
    );
    let declared_rules: Value =
        serde_json::from_str(fixture["inputs"]["design_bytes"].as_str().unwrap()).unwrap();
    let rules: brassclaw_skills::prefix_generation_rules::PrefixGenerationRules =
        serde_json::from_value(declared_rules.clone()).unwrap();
    assert!(rules.valid_for_profile(declared_rules["profile"].as_str().unwrap()));
    let mut inputs = fixture["inputs"].clone();
    let prepared = evaluate(&source, "prepare_prefix_design_candidates", inputs.clone()).await;
    assert_eq!(prepared, fixture["prepared"]);
    let assets: brassclaw_model_encoding::Assets =
        serde_json::from_value(fixture["assets"].clone()).unwrap();
    let probes: Vec<brassclaw_model_encoding::Probe> =
        serde_json::from_str(prepared["probes_json"].as_str().unwrap()).unwrap();
    let report = brassclaw_model_encoding::analyze(brassclaw_model_encoding::Request {
        format: brassclaw_model_encoding::PROTOCOL.into(),
        assets,
        probes,
    })
    .unwrap();
    let minimum = report.probes[0].token_count;
    assert!(report.probes.last().unwrap().token_count > minimum);
    // The immutable observation binds the real algorithm inputs. Worker identity
    // here names the actual utility executable; this is no native-custody receipt.
    let worker_checksum = sha(&std::fs::read(installed_worker().unwrap()).unwrap());
    let observation = json!({"format":"brassclaw-model-encoding-observation/1","specification":sha(b"specification"),"artifact_set":sha(b"artifacts"),"preparation_revision":1,"worker_checksum":worker_checksum,"model_chat_template_sha256":report.chat_template_sha256,"template_origin":"model-artifact-template","report":report,"qualification":"offline-measurement-only"});
    let arguments = |prepared: &Value| json!({"bank_bytes":prepared["bank_bytes"],"digests":prepared["texts"].as_array().unwrap().iter().map(|t|sha(t.as_str().unwrap().as_bytes())).collect::<Vec<_>>(),"observation_bytes":canonical(&observation),"expected_specification":sha(b"specification"),"expected_artifact_set":sha(b"artifacts")});
    let large: Value = serde_json::from_str(
        evaluate(
            &source,
            "select_prefix_design_candidate",
            arguments(&prepared),
        )
        .await
        .as_str()
        .unwrap(),
    )
    .unwrap();
    assert_eq!(large["status"], "reference-selected");
    assert!(
        large["selected"]["groups"]
            .as_array()
            .unwrap()
            .contains(&json!("detail"))
    );
    // The candidate strings are invariant across budgets in this same band.
    // Reusing their exact real observation changes no model/tokenizer input.
    inputs["prefix_budget"] = json!(minimum);
    let small = evaluate(&source, "prepare_prefix_design_candidates", inputs.clone()).await;
    assert_eq!(small["probes_json"], prepared["probes_json"]);
    let selected: Value = serde_json::from_str(
        evaluate(&source, "select_prefix_design_candidate", arguments(&small))
            .await
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(selected["token_count"], minimum);
    assert_eq!(
        selected["selected"]["groups"],
        json!(["example", "contract"])
    );
    assert!(
        large["selected"]["text"]
            .as_str()
            .unwrap()
            .starts_with(selected["selected"]["text"].as_str().unwrap())
    );
    assert_eq!(selected["sources_sha256"], large["sources_sha256"]);
    inputs["prefix_budget"] = json!(minimum - 1);
    let blocked = evaluate(&source, "prepare_prefix_design_candidates", inputs).await;
    let blocked: Value = serde_json::from_str(
        evaluate(
            &source,
            "select_prefix_design_candidate",
            arguments(&blocked),
        )
        .await
        .as_str()
        .unwrap(),
    )
    .unwrap();
    assert_eq!(blocked["status"], "capacity-blocked");
    assert!(blocked["selected"].is_null());
    // Current source membership changes under the exact same generation rules.
    // Resolve new mandatory namespace content in the actual Monty implementation.
    let mut grown = fixture["inputs"].clone();
    let mut inventory: Value =
        serde_json::from_str(grown["sources_bytes"].as_str().unwrap()).unwrap();
    let new_code = "def native_new_component(value):\n    return value + 1\n";
    inventory["sources"].as_array_mut().unwrap().push(
        json!({"path":"components/new.py","text":new_code,"sha256":sha(new_code.as_bytes())}),
    );
    grown["sources_bytes"] = json!(canonical(&inventory));
    let prepared_grown = evaluate(&source, "prepare_prefix_design_candidates", grown).await;
    let bank: Value = serde_json::from_str(prepared_grown["bank_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(bank["design_bytes"], fixture["inputs"]["design_bytes"]);
    assert!(
        bank["candidates"][0]["units"]
            .as_array()
            .unwrap()
            .contains(&json!("components/new.py#0"))
    );
    assert!(
        bank["candidates"][0]["text"]
            .as_str()
            .unwrap()
            .contains(new_code)
    );
}

#[tokio::test]
async fn design_recipe_reuses_native_usages_and_preloads_the_exact_dependency_graph() {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        },
    };
    use brassclaw_skills::{
        component_revision::RetainedComponentSnapshot,
        global_bootstrap_components::UsageComponentIds, prefix_design_components::*,
    };
    let native = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let identity = |capability: &str| {
        serde_json::to_value(
            native
                .native_identity(&brassclaw_host_api::CapabilityId::new(capability).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let digest =
        brassclaw_skills::utf8_digest_components::usage_ids(Uuid::new_v4(), |_| Uuid::new_v4());
    let encoding = UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool: Uuid::new_v4(),
        formatter: None,
    };
    let ids = DesignSelectionIds::allocate(|_| Uuid::new_v4());
    let captured = CapturedDesignSelectionIds {
        compiler_library: Uuid::new_v4(),
        source_prepare: Uuid::new_v4(),
        design_inputs: Uuid::new_v4(),
        coverage: Uuid::new_v4(),
        select: Uuid::new_v4(),
    };
    let units = Uuid::new_v4();
    let renderer = Uuid::new_v4();
    let original_drafts =
        captured_design_selection_drafts(ids, captured, units, renderer, digest, encoding).unwrap();
    let budgeted = BudgetedDesignIds {
        budget: Uuid::new_v4(),
        receipt: Uuid::new_v4(),
    };
    let mut drafts = budgeted_design_selection_drafts(
        ids, captured, budgeted, units, renderer, digest, encoding,
    )
    .unwrap();
    for original in &original_drafts {
        let upgraded = drafts
            .iter()
            .find(|draft| draft.uuid() == original.uuid())
            .unwrap();
        if original.uuid() == ids.recipe {
            for name in ["selected", "captured"] {
                assert_eq!(
                    upgraded.document()["input_layouts"][name],
                    original.document()["input_layouts"][name]
                );
                assert_eq!(
                    upgraded.document()["invocations"][name],
                    original.document()["invocations"][name]
                );
            }
            assert_eq!(
                &upgraded.document()["variants"].as_array().unwrap()[..2],
                original.document()["variants"].as_array().unwrap()
            );
            assert_eq!(
                &upgraded.document()["step_descriptions"].as_array().unwrap()[..2],
                original.document()["step_descriptions"].as_array().unwrap()
            );
        } else {
            assert_eq!(upgraded.exact_bytes(), original.exact_bytes());
        }
    }
    assert!(
        budgeted_design_selection_drafts(
            ids,
            captured,
            BudgetedDesignIds {
                budget: units,
                ..budgeted
            },
            units,
            renderer,
            digest,
            encoding,
        )
        .is_err()
    );
    let invalid = CapturedDesignSelectionIds {
        select: captured.coverage,
        ..captured
    };
    assert!(
        captured_design_selection_drafts(ids, invalid, units, renderer, digest, encoding).is_err()
    );
    drafts.push(
        brassclaw_skills::prefix_compiler_components::unit_segmentation_draft(units).unwrap(),
    );
    drafts.push(
        brassclaw_skills::prefix_reference_components::reference_render_draft(renderer).unwrap(),
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
    drafts.extend(
        brassclaw_skills::utf8_digest_components::utf8_digest_usage_drafts(digest).unwrap(),
    );
    drafts.extend(
        brassclaw_skills::model_encoding_components::model_encoding_usage_drafts(encoding).unwrap(),
    );
    let snapshot = RetainedComponentSnapshot::new(
        &[ids.recipe, digest.recipe, encoding.recipe],
        drafts
            .into_iter()
            .map(|draft| draft.at_version(1).unwrap())
            .collect(),
    )
    .unwrap();
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot.select_roots(&[ids.recipe]).unwrap()),
            ids.recipe,
            "selected",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(program.bindings().len(), 2);
    assert_eq!(program.bindings()["0:3"].tool().uuid, digest.tool);
    assert_eq!(program.bindings()["0:5"].tool().uuid, encoding.tool);
    InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
    let captured_program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot.select_roots(&[ids.recipe]).unwrap()),
            ids.recipe,
            "captured",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(captured_program.bindings().len(), 3);
    assert_eq!(captured_program.bindings()["1:3"].tool().uuid, digest.tool);
    assert_eq!(captured_program.bindings()["1:8"].tool().uuid, digest.tool);
    assert_eq!(
        captured_program.bindings()["1:10"].tool().uuid,
        encoding.tool
    );
    InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(captured_program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
    let budgeted_program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot.select_roots(&[ids.recipe]).unwrap()),
            ids.recipe,
            "budgeted",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(budgeted_program.bindings().len(), 3);
    assert_eq!(budgeted_program.bindings()["2:4"].tool().uuid, digest.tool);
    assert_eq!(budgeted_program.bindings()["2:9"].tool().uuid, digest.tool);
    assert_eq!(
        budgeted_program.bindings()["2:11"].tool().uuid,
        encoding.tool
    );
    InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(budgeted_program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
}
