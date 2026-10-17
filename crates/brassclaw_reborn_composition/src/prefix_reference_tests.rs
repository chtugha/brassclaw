//! Independent positive/negative rendering checks in the actual contained Monty
//! worker. These validate a building block, not installation or generation jobs.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_skills::prefix_reference_components::{
    REFERENCE_RENDER_SOURCE, reference_render_draft,
};
use serde_json::{Value, json};
use sha2::Digest;
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

#[tokio::test]
async fn prefix_reference_renderer_is_effect_free_and_runs_in_monty() {
    let draft = reference_render_draft(Uuid::new_v4()).unwrap();
    for (usage, prepare) in [
        (
            brassclaw_skills::prefix_generation_components::GenerationUsage::Read,
            None,
        ),
        (
            brassclaw_skills::prefix_generation_components::GenerationUsage::Retain,
            Some(Uuid::new_v4()),
        ),
    ] {
        let ids = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
            recipe: Uuid::new_v4(),
            python_code: Uuid::new_v4(),
            tool_skill: Uuid::new_v4(),
            tool: Uuid::new_v4(),
            skill: Uuid::new_v4(),
            formatter: prepare,
        };
        for draft in
            brassclaw_skills::prefix_generation_components::generation_usage_drafts(ids, usage)
                .unwrap()
        {
            if draft.class_code() == 22 {
                let contract = brassclaw_skills::value_contract::InputContract::from_value(
                    &draft.document()["input_contract"],
                    brassclaw_skills::component_revision::REVISION_LIMITS,
                )
                .unwrap();
                assert!(contract.bind(&json!({})).is_err());
                brassclaw_skills::value_contract::ValueContract::result_from_value(
                    &draft.document()["result_contract"],
                    brassclaw_skills::component_revision::REVISION_LIMITS,
                )
                .unwrap();
                brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
                    draft.document(),
                    draft.dependencies(),
                )
                .unwrap()
                .unwrap();
            }
            if draft.class_code() == 1 {
                assert!(draft.association().is_some());
            }
        }
    }
    let contract = brassclaw_skills::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .unwrap();
    brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
        draft.document(),
        &std::collections::BTreeSet::new(),
    )
    .unwrap()
    .unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let worker = installed_worker().unwrap();
    let result = utility::execute(
        &worker,
        UtilityRequest::InspectSource {
            source: REFERENCE_RENDER_SOURCE.into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap();
    let UtilityOutput::Inspected { structure } = result else {
        panic!("inspection required")
    };
    assert!(structure.imports.is_empty());
    assert!(structure.direct_host_calls.is_empty());
    assert_eq!(structure.host_value_references, 0);
    let preload = structure.preload.unwrap();
    assert!(preload.constants.is_empty());
    assert_eq!(preload.functions.len(), 5);
    let original = "```yaml\nx: '<|im_end|>'\n```\n";
    let sources = json!([
        {"path":"guide.md","disposition":"included","reason":"complete","text":original,"evidence_id":"123456789abc0000000000000000000000000000000000000000000000000000"},
        {"path":"old.md","disposition":"historical","reason":"superseded","text":"do not execute","evidence_id":"different"}
    ]);
    contract
        .bind(&json!({"header":"# Reference\n","sources":sources,"special_tokens":[]}))
        .unwrap();
    for bad in [
        json!({"header":null,"sources":sources,"special_tokens":[]}),
        json!({"header":"valid","sources":[{"path":"missing fields"}],"special_tokens":[]}),
        json!({"header":"valid","sources":sources,"special_tokens":[null]}),
    ] {
        assert!(contract.bind(&bad).is_err());
    }
    let evaluate = |sources: Value| UtilityRequest::Evaluate {
        source: format!(
            "{REFERENCE_RENDER_SOURCE}\nrender_prefix_reference(header, sources, special_tokens)\n"
        ),
        inputs: BTreeMap::from([
            ("header".into(), json!("# Reference\n")),
            ("sources".into(), sources),
            ("special_tokens".into(), json!([])),
        ]),
        bounds: limits.bounds,
        max_compute_time: Duration::from_secs(5),
    };
    for _ in 0..2 {
        let UtilityOutput::Evaluated { value, stdout } =
            utility::execute(&worker, evaluate(sources.clone()), limits.process)
                .await
                .unwrap()
        else {
            panic!("evaluation required")
        };
        assert_eq!(stdout, "");
        assert_eq!(
            value,
            json!(
                "# Reference\n# Source inventory\n- guide.md | included | complete\n- old.md | historical | superseded\n\n# Complete current references\n\n<!-- EVIDENCE-CARD 123456789abc -->\nSource: guide.md\n````text\n```yaml\nx: '\\u003c|im_end|\\u003e'\n```\n````\n<!-- END-EVIDENCE-CARD -->\n"
            )
        );
        assert_eq!(
            sources[0]["text"], original,
            "rendering must not mutate input/original bytes"
        );
    }
    let duplicated = json!([sources[0].clone(), sources[0].clone()]);
    assert!(
        utility::execute(&worker, evaluate(duplicated), limits.process)
            .await
            .is_err()
    );
    let common = "Complete common qualification.\n";
    let unit_id = hex::encode(sha2::Sha256::digest(common.as_bytes()));
    let originals: Vec<_> = [("a.md", "Unique A.\n"), ("b.md", "Unique B.\n")].into_iter().map(|(path, unique)| {
        let full = format!("{common}{unique}");
        json!({"path":path,"disposition":"included","reason":"independent fixture","text":full,"sha256":hex::encode(sha2::Sha256::digest(full.as_bytes())),"evidence_id":hex::encode(sha2::Sha256::digest(path.as_bytes())),
            "units":[{"id":unit_id,"line_start":1,"line_end":1,"text":common},{"id":hex::encode(sha2::Sha256::digest(unique.as_bytes())),"line_start":2,"line_end":2,"text":unique}]})
    }).collect();
    let request = UtilityRequest::Evaluate {
        source: format!(
            "{REFERENCE_RENDER_SOURCE}\nrender_deduplicated_prefix_reference(header, sources, special_tokens)\n"
        ),
        inputs: BTreeMap::from([
            ("header".into(), json!("# Reference\n")),
            ("sources".into(), json!(originals)),
            ("special_tokens".into(), json!([])),
        ]),
        bounds: limits.bounds,
        max_compute_time: Duration::from_secs(5),
    };
    let UtilityOutput::Evaluated { value, stdout } =
        utility::execute(&worker, request, limits.process)
            .await
            .unwrap()
    else {
        panic!("deduplicated evaluation required")
    };
    assert!(stdout.is_empty());
    let context = value["context"].as_str().unwrap();
    assert_eq!(context.matches(common).count(), 1);
    assert!(context.contains("Unique A."));
    assert!(context.contains("Unique B."));
    let shared = value["render_map"]["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["unit_id"] == unit_id)
        .unwrap();
    assert_eq!(
        shared["canonical"],
        json!({"source":"a.md","line_start":1,"line_end":1})
    );
    assert_eq!(shared["origins"].as_array().unwrap().len(), 2);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn prefix_legacy_upgrade_preserves_complete_existing_configuration() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    // A separate pre-upgrade schema exercises the actual forward migrations,
    // with real current legacy columns and no changes to the runtime fixture.
    let schema = format!("prefix_upgrade_{}", Uuid::new_v4().simple());
    tx.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema},public;
         CREATE TABLE {schema}.reborn_basic_prompt_store
             (LIKE public.reborn_basic_prompt_store INCLUDING ALL);"
    ))
    .await
    .unwrap();
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V139__prefix_profile_registry.sql"
    ))
    .await
    .unwrap();
    tx.execute("UPDATE reborn_prefix_profiles SET revision=7,label='Operator custom base' WHERE name='base-prompt'", &[]).await.unwrap();
    let bundle = "# Operator's exact prefix\r\n```yaml\r\nvalue: ü\r\n```\r\n";
    let serving = Uuid::new_v4();
    tx.execute("INSERT INTO reborn_basic_prompt_store(tenant_id,user_id,agent_id,project_id,bundle_json,fingerprint,is_stale,assembled_at,prewarm_last_at,generation_ms,generation_id,updated_at)
        VALUES('old-tenant','old-user','old-agent','old-project',$1,'operator-fingerprint',true,'2026-10-01T12:01:02Z','2026-10-02T13:14:15Z',12345,$2,'2026-10-03T14:15:16Z')",
        &[&json!(bundle),&serving]).await.unwrap();
    let before: Value = tx
        .query_one("SELECT to_jsonb(s) FROM reborn_basic_prompt_store s", &[])
        .await
        .unwrap()
        .get(0);
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V142__immutable_prefix_generations.sql"
    ))
    .await
    .unwrap();
    let partial: String = tx
        .query_one(
            "SELECT generation_bytes FROM reborn_prefix_generations",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let migration =
        include_str!("../../brassclaw_pg/migrations/V152__complete_legacy_prefix_archive.sql");
    tx.batch_execute(migration).await.unwrap();
    tx.batch_execute(migration).await.unwrap();
    let rows = tx.query("SELECT generation_bytes,generation_id,profile_revision FROM reborn_prefix_generations ORDER BY generation_id",&[]).await.unwrap();
    assert_eq!(
        rows.len(),
        2,
        "repetition must retain both historical and complete archives once"
    );
    assert!(rows.iter().any(|r| r.get::<_, String>(0) == partial));
    let full = rows
        .iter()
        .find(|r| {
            serde_json::from_str::<Value>(&r.get::<_, String>(0)).unwrap()["format"]
                == "brassclaw-legacy-prefix-bundle/2"
        })
        .unwrap();
    let bytes: String = full.get(0);
    let archive: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(archive["original_row"], before);
    assert_eq!(archive["qualification"], "legacy-unverified");
    assert_eq!(full.get::<_, i64>(2), 7);
    assert_eq!(
        full.get::<_, String>(1),
        hex::encode(sha2::Sha256::digest(bytes.as_bytes()))
    );
    let after: Value = tx
        .query_one("SELECT to_jsonb(s) FROM reborn_basic_prompt_store s", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        before, after,
        "upgrade must leave every serving/configuration field unchanged"
    );
    let label: String = tx
        .query_one(
            "SELECT label FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(label, "Operator custom base");
    tx.rollback().await.unwrap();
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn prefix_installation_registers_profiles_before_ui_and_preserves_overrides() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "DELETE FROM reborn_prefix_profiles WHERE name='prefix-maker'",
            &[],
        )
        .await
        .unwrap();
    client.execute("UPDATE reborn_prefix_profiles SET label='Operator base',revision=9,compiler_entry='Operator-compiler.py' WHERE name='base-prompt'",&[]).await.unwrap();
    drop(client);
    crate::prefix_profile_seeds::retain(rig.pool.as_ref())
        .await
        .unwrap();
    crate::prefix_profile_seeds::retain(rig.pool.as_ref())
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_profiles", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        6
    );
    let row = client.query_one("SELECT label,revision,compiler_entry FROM reborn_prefix_profiles WHERE name='base-prompt'",&[]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "Operator base");
    assert_eq!(row.get::<_, i64>(1), 9);
    assert_eq!(row.get::<_, String>(2), "Operator-compiler.py");
    assert_eq!(
        client
            .query_one(
                "SELECT compiler_entry FROM reborn_prefix_profiles WHERE name='prefix-maker'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "prefix-maker-compiler.py"
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_generations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "profile metadata must not fabricate a prefix or readiness evidence"
    );
}

#[test]
fn prefix_worker_package_requires_retained_build_adapter_bytes() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg("import json,sys; from test_prefix_build_worker import request; import prefix_build_worker; sys.stdout.buffer.write(prefix_build_worker.build(json.dumps(request()).encode()))")
        .env("PYTHONPATH", checkout.join("scripts/prefix"))
        .env("HF_HUB_OFFLINE", "1")
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let bytes = result["generation_bytes"].as_str().unwrap();
    crate::prefix_generations::validate_package("base-prompt", bytes).unwrap();
    let mut package: Value = serde_json::from_str(bytes).unwrap();
    let original = STANDARD
        .decode(
            package["artifacts"]["compiler/worker.py"]["data"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        original,
        std::fs::read(checkout.join("scripts/prefix/prefix_build_worker.py")).unwrap()
    );
    // Recompute the generic package identity after removing only the worker
    // linkage. File hashes alone cannot establish its procedure declaration.
    let mut manifest: Value = serde_json::from_slice(
        &STANDARD
            .decode(
                package["artifacts"]["manifest.json"]["data"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    manifest["compiler"]
        .as_object_mut()
        .unwrap()
        .remove("worker_sha256");
    let identity = json!({"profile":manifest["profile"],"compiler":manifest["compiler"],"files":manifest["files"],"geometry":manifest["geometry"]});
    manifest["generation"] = json!(hex::encode(sha2::Sha256::digest(
        crate::prefix_generations::canonical(&identity)
            .unwrap()
            .as_bytes()
    )));
    package["artifacts"]["manifest.json"]["data"] =
        json!(STANDARD.encode(crate::prefix_generations::canonical(&manifest).unwrap()));
    assert!(
        crate::prefix_generations::validate_package("base-prompt", &package.to_string()).is_err(),
        "a retained worker file requires its exact declared procedure checksum"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn prefix_tool_registration_preserves_retained_identity_and_operator_block() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let expected = crate::installed_monty_catalogue::id("prefix-generation:tool");
    let registry = brassclaw_host_runtime::builtin_native_first_party_handlers(
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
    let identity = registry.native_identity(&capability).unwrap();
    let draft = brassclaw_skills::prefix_generation_components::generation_tool_draft(
        expected,
        serde_json::to_value(identity).unwrap(),
    )
    .unwrap();
    brassclaw_skills::revision_store::PgComponentRevisionStore::new(rig.pool.clone())
        .retain_packaged_draft(&draft)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "INSERT INTO brassclaw_instance_tool_settings(tool_id,enabled) VALUES($1,false)",
            &[&expected],
        )
        .await
        .unwrap();
    drop(client);
    for _ in 0..2 {
        let actual =
            crate::builtin_bootstrap::seed_prefix_generation_tool(rig.pool.clone(), "default")
                .await
                .unwrap();
        assert_eq!(actual, expected);
    }
    let client = rig.pool.get().await.unwrap();
    assert!(
        !client
            .query_one(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&expected]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_tools WHERE capability_id=$1",
                &[&brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

/// Real packaged guides, independent CPython/SHA oracle, and the contained
/// Monty evaluator. This is building-block evidence, not activated job coverage.
#[tokio::test]
async fn prefix_compiler_stages_preserve_originals_in_monty() {
    use brassclaw_skills::prefix_compiler_components::*;
    let units = Uuid::new_v4();
    let renderer = Uuid::new_v4();
    let library = Uuid::new_v4();
    let mut drafts = vec![
        unit_segmentation_draft(units).unwrap(),
        reference_render_draft(renderer).unwrap(),
        compiler_library_draft(library, units, renderer).unwrap(),
    ];
    for stage in ["prepare", "reference", "render", "identity", "finalize"] {
        drafts.push(compiler_stage_draft(Uuid::new_v4(), library, stage).unwrap());
    }
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let worker = installed_worker().unwrap();
    for draft in &drafts {
        brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
            draft.document(),
            draft.dependencies(),
        )
        .unwrap()
        .unwrap();
        brassclaw_skills::value_contract::InputContract::from_value(
            &draft.document()["input_contract"],
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .unwrap();
        brassclaw_skills::value_contract::ValueContract::result_from_value(
            &draft.document()["result_contract"],
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .unwrap();
        let result = utility::execute(
            &worker,
            UtilityRequest::InspectSource {
                source: draft.document()["content"].as_str().unwrap().into(),
                bounds: limits.bounds,
            },
            limits.process,
        )
        .await
        .unwrap();
        let UtilityOutput::Inspected { structure } = result else {
            panic!("inspection required")
        };
        assert!(structure.direct_host_calls.is_empty());
        assert_eq!(structure.host_value_references, 0);
        assert!(
            structure.preload.is_some(),
            "{} is not an effect-free preload",
            draft.document()["name"]
        );
    }
    let segmentation = unit_segmentation_draft(Uuid::new_v4()).unwrap();
    for (suffix, text, expected) in [
        (
            ".rst",
            "Heading\n=======\nQualifications.\n\nSubheading\n----------\nDetails.\n",
            json!([
            {"line_start":1,"line_end":4,"text":"Heading\n=======\nQualifications.\n\n","parent":""},
            {"line_start":5,"line_end":7,"text":"Subheading\n----------\nDetails.\n","parent":"Heading\n=======\nQualifications.\n\n"}]),
        ),
        (
            ".py",
            "# comment\nprint('hi')\n",
            json!([{"line_start":1,"line_end":2,"text":"# comment\nprint('hi')\n","parent":""}]),
        ),
        (
            ".md",
            "# Real\nBefore.\n```md\n# Fake\n```\n## Next\nDetails.\n",
            json!([
            {"line_start":1,"line_end":5,"text":"# Real\nBefore.\n```md\n# Fake\n```\n","parent":""},
            {"line_start":6,"line_end":7,"text":"## Next\nDetails.\n","parent":"# Real\nBefore.\n```md\n# Fake\n```\n"}]),
        ),
    ] {
        let result = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: format!(
                    "{}\nsegment_prefix_units(inputs)\n",
                    segmentation.document()["content"].as_str().unwrap()
                ),
                inputs: BTreeMap::from([("inputs".into(), json!({"text":text,"suffix":suffix}))]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap();
        let UtilityOutput::Evaluated { value, stdout } = result else {
            panic!("segmentation evaluation required")
        };
        assert!(stdout.is_empty());
        assert_eq!(value, expected);
    }
    let receipt = generation_receipt_draft(Uuid::new_v4()).unwrap();
    let receipt_source = format!(
        "{}\nformat_prefix_generation_receipt(inputs)\n",
        receipt.document()["content"].as_str().unwrap()
    );
    let digest = "a".repeat(64);
    for (inputs, valid) in [
        (
            json!({"profile":"base-prompt","expected_profile":"base-prompt","generation":digest,"qualification":"quarantined"}),
            true,
        ),
        (
            json!({"profile":"base-prompt","expected_profile":"prefix-maker","generation":digest,"qualification":"quarantined"}),
            false,
        ),
        (
            json!({"profile":"base-prompt","expected_profile":"base-prompt","generation":"bad","qualification":"quarantined"}),
            false,
        ),
        (
            json!({"profile":"base-prompt","expected_profile":"base-prompt","generation":digest,"qualification":"active"}),
            false,
        ),
    ] {
        let result = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: receipt_source.clone(),
                inputs: BTreeMap::from([("inputs".into(), inputs)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await;
        assert_eq!(result.is_ok(), valid);
        if valid {
            let UtilityOutput::Evaluated { value, stdout } = result.unwrap() else {
                panic!("receipt evaluation required")
            };
            assert!(stdout.is_empty());
            assert_eq!(
                value,
                json!(format!("prefix-generation:base-prompt:{digest}"))
            );
        }
    }
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let definitions = format!("{UNIT_SOURCE}\n{REFERENCE_RENDER_SOURCE}\n{COMPILER_SOURCE}\n");
    for profile in ["base-prompt", "prefix-maker"] {
        for view in ["literal-reference/v1", "literal-reference/v2-deduplicated"] {
            let output = std::process::Command::new("python3")
                .args(["-c", "import json,sys; from test_prefix_compile_logic import native_fixture; print(json.dumps(native_fixture(sys.argv[1],sys.argv[2]),ensure_ascii=False))", profile, view])
                .env("PYTHONPATH", checkout.join("scripts/prefix")).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let cases: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
            for case in cases {
                let function = case["function"].as_str().unwrap();
                assert!(
                    [
                        "prefix_prepare",
                        "prefix_reference",
                        "prefix_render",
                        "prefix_identity",
                        "prefix_finalize"
                    ]
                    .contains(&function)
                );
                let result = utility::execute(
                    &worker,
                    UtilityRequest::Evaluate {
                        source: format!("{definitions}\n{function}(inputs)\n"),
                        inputs: BTreeMap::from([("inputs".into(), case["inputs"].clone())]),
                        bounds: limits.bounds,
                        max_compute_time: Duration::from_secs(60),
                    },
                    limits.process,
                )
                .await
                .unwrap_or_else(|error| panic!("{profile}/{view}/{function}: {error:?}"));
                let UtilityOutput::Evaluated { value, stdout } = result else {
                    panic!("evaluation required")
                };
                assert!(stdout.is_empty());
                assert!(
                    value == case["expected"],
                    "{profile}/{view}/{function} differs from independent oracle"
                );
                if function == "prefix_finalize" {
                    crate::prefix_generations::validate_package(profile, value.as_str().unwrap())
                        .unwrap();
                    let mut changed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
                    let mut input = prefix_test_json_artifact(&changed, "source_snapshot.json");
                    input["header"] = json!("Different original design frame");
                    prefix_test_replace_json(&mut changed, "source_snapshot.json", &input);
                    prefix_test_resign(&mut changed);
                    assert!(
                        crate::prefix_generations::validate_package(profile, &changed.to_string())
                            .is_err()
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn prefix_compile_recipe_prepares_exact_reusable_preload_graph() {
    prepared_prefix_compiler_test().await;
}

pub(crate) async fn prepared_prefix_compiler_test() -> (
    Vec<brassclaw_skills::component_revision::ComponentRevisionDraft>,
    brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
) {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        },
    };
    use brassclaw_skills::{
        component_revision::RetainedComponentSnapshot, prefix_compiler_components::*,
        prefix_generation_components::*,
    };
    use std::sync::Arc;
    let ids = SourceCompilerIds::allocate(Uuid::new_v4(), Uuid::new_v4(), |_| Uuid::new_v4());
    let digest = ids.digest;
    let retain = ids.retain;
    let recipe = ids.recipe;
    let registry = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let native = |name: &str| {
        serde_json::to_value(
            registry
                .native_identity(&brassclaw_host_api::CapabilityId::new(name).unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let json_tool = brassclaw_skills::utf8_digest_components::json_text_tool_draft(
        digest.tool,
        native("builtin.json"),
    )
    .unwrap();
    let mut drafts = vec![
        json_tool,
        generation_tool_draft(retain.tool, native("builtin.prefix_generation")).unwrap(),
    ];
    drafts.extend(source_compiler_drafts(ids).unwrap());
    let all_snapshot = RetainedComponentSnapshot::new(
        &[recipe, digest.recipe, retain.recipe],
        drafts
            .iter()
            .cloned()
            .map(|d| d.at_version(1).unwrap())
            .collect(),
    )
    .unwrap();
    let snapshot = Arc::new(all_snapshot.select_roots(&[recipe]).unwrap());
    let digest_snapshot = Arc::new(all_snapshot.select_roots(&[digest.recipe]).unwrap());
    let digest_instruction = compile_retained_recipe(
        digest_snapshot,
        digest.recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let digest_program = prepare_retained_tool_program(digest_instruction).unwrap();
    assert_eq!(
        digest_program
            .inputs()
            .bind_variant_example(
                "digest utf8 json=%",
                "digest utf8 json=[\"first\",\"second\"]",
                &json!({})
            )
            .unwrap(),
        json!({"texts_json":"[\"first\",\"second\"]"})
    );
    let parser = drafts
        .iter()
        .find(|draft| Some(draft.uuid()) == digest.formatter)
        .unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    for (text, expected) in [
        ("[\"first\",\"é\",\"\"]", Some(json!(["first", "é", ""]))),
        ("[]", Some(json!([]))),
        ("[null]", None),
        ("{}", None),
        ("__import__('os')", None),
    ] {
        let result = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: format!(
                    "{}\nparse_utf8_texts(inputs)\n",
                    parser.document()["content"].as_str().unwrap()
                ),
                inputs: BTreeMap::from([("inputs".into(), json!({"texts_json":text}))]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await;
        if let Some(expected) = expected {
            let UtilityOutput::Evaluated { value, stdout } = result.unwrap() else {
                panic!("parser evaluation required")
            };
            assert!(stdout.is_empty());
            assert_eq!(value, expected);
        } else {
            assert!(result.is_err());
        }
    }
    InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(digest_program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
    let instruction =
        compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic)
            .unwrap();
    let program = prepare_retained_tool_program(instruction).unwrap();
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(program)),
        &installed_worker().unwrap(),
    )
    .await
    .unwrap();
    (drafts, inspected)
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn prefix_json_registration_preserves_existing_identity_metadata_and_block() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let legacy = Uuid::new_v4();
    let store = crate::pg_tool_store::PgToolStore::new(rig.pool.clone());
    let row = crate::pg_tool_store::NewPgTool {
        tenant_id: "default".into(),
        user_id: brassclaw_host_api::SYSTEM_RESERVED_ID.into(),
        agent_id: "default".into(),
        project_id: "system".into(),
        name: "json".into(),
        description: "Operator description".into(),
        param_schema: None,
        param_template: None,
        effect_type: "read".into(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec![],
        source: "authored".into(),
        validation_status: "pending".into(),
        capability_id: "builtin.json".into(),
    };
    assert_eq!(
        store.insert_with_id(row, Some(legacy)).await.unwrap(),
        Some(legacy)
    );
    let registry = brassclaw_host_runtime::builtin_native_first_party_handlers(
        brassclaw_host_runtime::BuiltinFirstPartyTools::default(),
        brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        brassclaw_host_runtime::NativeVerificationLimits::new(1024 * 1024, Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let native = registry
        .native_identity(&brassclaw_host_api::CapabilityId::new("builtin.json").unwrap())
        .unwrap();
    let draft = brassclaw_skills::utf8_digest_components::json_text_tool_draft(
        legacy,
        serde_json::to_value(native).unwrap(),
    )
    .unwrap();
    brassclaw_skills::revision_store::PgComponentRevisionStore::new(rig.pool.clone())
        .retain_packaged_draft(&draft)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "INSERT INTO brassclaw_instance_tool_settings(tool_id,enabled) VALUES($1,false)",
            &[&legacy],
        )
        .await
        .unwrap();
    drop(client);
    for _ in 0..2 {
        assert_eq!(
            crate::builtin_bootstrap::seed_json_primitive_tool(rig.pool.clone(), "default")
                .await
                .unwrap(),
            legacy
        );
    }
    let client = rig.pool.get().await.unwrap();
    let row=client.query_one("SELECT t.description,s.enabled FROM reborn_tools t JOIN brassclaw_instance_tool_settings s ON s.tool_id=t.id WHERE t.id=$1",&[&legacy]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "Operator description");
    assert!(!row.get::<_, bool>(1));
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_tools WHERE capability_id='builtin.json'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

fn prefix_test_json_artifact(package: &Value, name: &str) -> Value {
    use base64::{Engine, engine::general_purpose::STANDARD};
    serde_json::from_slice(
        &STANDARD
            .decode(package["artifacts"][name]["data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn prefix_test_replace_json(package: &mut Value, name: &str, value: &Value) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    package["artifacts"][name] = json!({"encoding":"base64","data":STANDARD.encode(crate::prefix_generations::canonical(value).unwrap())});
}
/// Independently repair the generic cryptographic envelope after a semantic
/// corruption. Negative cases must fail relationship checks, not stale hashes.
fn prefix_test_resign(package: &mut Value) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut manifest = prefix_test_json_artifact(package, "manifest.json");
    let files = package["artifacts"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.as_str() != "manifest.json")
        .map(|(name, value)| {
            (
                name.clone(),
                json!(hex::encode(sha2::Sha256::digest(
                    STANDARD.decode(value["data"].as_str().unwrap()).unwrap()
                ))),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    manifest["files"] = Value::Object(files);
    let identity = json!({"profile":manifest["profile"],"compiler":manifest["compiler"],"files":manifest["files"],"geometry":manifest["geometry"]});
    manifest["generation"] = json!(hex::encode(sha2::Sha256::digest(
        crate::prefix_generations::canonical(&identity)
            .unwrap()
            .as_bytes()
    )));
    prefix_test_replace_json(package, "manifest.json", &manifest);
}

#[test]
fn prefix_source_archive_links_reject_resigned_inconsistent_metadata() {
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output=std::process::Command::new("python3").args(["-c","import json,sys; from test_prefix_build_worker import request; import prefix_build_worker; sys.stdout.buffer.write(prefix_build_worker.build(json.dumps(request()).encode()))"]).env("PYTHONPATH",checkout.join("scripts/prefix")).env("HF_HUB_OFFLINE","1").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    let original: Value =
        serde_json::from_str(response["generation_bytes"].as_str().unwrap()).unwrap();
    crate::prefix_generations::validate_package("base-prompt", &original.to_string()).unwrap();
    let sources = prefix_test_json_artifact(&original, "sources.json");
    assert_eq!(sources["format"], "brassclaw-prefix-generation-sources/1");
    assert_eq!(sources["input_artifact"], "input_sources.json");
    for mode in [
        "pointer",
        "original-input",
        "citation",
        "range",
        "count",
        "missing-input",
    ] {
        let mut changed = original.clone();
        match mode {
            "pointer" => {
                let mut value = sources.clone();
                value["sources"][0]["artifact"] = json!(format!("raw/{}", "0".repeat(64)));
                prefix_test_replace_json(&mut changed, "sources.json", &value);
            }
            "original-input" => {
                let mut value = prefix_test_json_artifact(&changed, "input_sources.json");
                value["sources"][0]["reason"] = json!("Different original request");
                prefix_test_replace_json(&mut changed, "input_sources.json", &value);
            }
            "citation" => {
                let mut value = prefix_test_json_artifact(&changed, "evidence.json");
                value[0]["id"] = json!("0".repeat(64));
                prefix_test_replace_json(&mut changed, "evidence.json", &value);
            }
            "range" => {
                let mut value = prefix_test_json_artifact(&changed, "evidence.json");
                value[0]["line_start"] = json!(2);
                prefix_test_replace_json(&mut changed, "evidence.json", &value);
            }
            "count" => {
                let mut value = prefix_test_json_artifact(&changed, "manifest.json");
                value["source_count"] = json!(999999);
                prefix_test_replace_json(&mut changed, "manifest.json", &value);
            }
            "missing-input" => {
                changed["artifacts"]
                    .as_object_mut()
                    .unwrap()
                    .remove("input_sources.json");
            }
            _ => unreachable!(),
        }
        prefix_test_resign(&mut changed);
        assert!(
            crate::prefix_generations::validate_package("base-prompt", &changed.to_string())
                .is_err(),
            "accepted resigned {mode} corruption"
        );
    }
    // Historical packages use an input-namespace manifest. Preserve their
    // readable raw-by-hash layout instead of rewriting immutable generations.
    let mut legacy = original.clone();
    let mut inventory = sources;
    inventory["format"] = json!("brassclaw-prefix-source-package/1");
    inventory.as_object_mut().unwrap().remove("input_artifact");
    for source in inventory["sources"].as_array_mut().unwrap() {
        source["artifact"] = json!(format!("objects/{}", source["sha256"].as_str().unwrap()));
    }
    prefix_test_replace_json(&mut legacy, "sources.json", &inventory);
    legacy["artifacts"]
        .as_object_mut()
        .unwrap()
        .remove("input_sources.json");
    prefix_test_resign(&mut legacy);
    crate::prefix_generations::validate_package("base-prompt", &legacy.to_string()).unwrap();
}
