//! Real canonical producer, immutable /1 bytes and catalogue prepare/reload.
//! Negative draft successors are rolled back, never approved or dispatched.
use super::*;
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::compile_retained_recipe,
        retained_tools::prepare_retained_tool_program,
    },
};
use secrecy::ExposeSecret;

async fn changed_selection(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    original: &InspectedRetainedProgram,
    change: &str,
    tx: &tokio_postgres::Transaction<'_>,
) -> InspectedRetainedProgram {
    let RetainedProgram::Tools(original_program) = original.program() else {
        panic!("canonical Tool program required")
    };
    let instruction = original_program.inputs().instruction();
    let pure = original_program.inputs().components()["0:1"];
    let mut revisions = Vec::new();
    let helper = Uuid::new_v4();
    for revision in instruction.snapshot().revisions().values() {
        let mut document = revision.draft().document().clone();
        let mut dependencies = revision.draft().dependencies().clone();
        let mut version = revision.reference().version;
        if revision.reference() == pure {
            match change {
                "version" => version += 1,
                "source" => {
                    document["content"] = json!(format!(
                        "{}\n# Unapproved successor\n",
                        document["content"].as_str().unwrap()
                    ));
                }
                "export" => {
                    let export = document["preload"]["exports"]
                        .as_object_mut()
                        .unwrap()
                        .remove("prepare_generation_revision")
                        .unwrap();
                    document["preload"]["exports"]["different_public_export"] = export;
                    document["preload"]["default_export"] = json!("different_public_export");
                }
                "dependency" => {
                    dependencies.insert(helper);
                    document["preload"]["dependencies"] = json!([helper]);
                }
                _ => {}
            }
        }
        if revision.reference() == instruction.recipe() {
            match change {
                "export" => {
                    document["invocations"]["selected"]["0:1"] = json!("different_public_export")
                }
                "layout" => {
                    document["input_layouts"]["selected"]["steps"]["0:1"]["revision"] =
                        json!({"kind":"constant","value":"2"})
                }
                _ => {}
            }
        }
        if version == revision.reference().version
            && document == *revision.draft().document()
            && dependencies == *revision.draft().dependencies()
        {
            revisions.push(revision.clone());
            continue;
        }
        // Preserve original ordering/bytes for the version-only case and all
        // unchanged prerequisites; reserialization is never historical repair.
        let draft = if document == *revision.draft().document()
            && dependencies == *revision.draft().dependencies()
        {
            revision.draft().clone()
        } else {
            let mut envelope: Value = serde_json::from_str(revision.draft().exact_bytes()).unwrap();
            envelope["document"] = document;
            if dependencies != *revision.draft().dependencies() {
                envelope["dependencies"] = json!(dependencies);
            }
            ComponentRevisionDraft::from_json(&envelope.to_string()).unwrap()
        };
        version = PgComponentRevisionStore::stage_in_transaction(
            tx,
            &draft,
            revision.reference().version,
        )
        .await
        .unwrap()
        .version;
        revisions.push(draft.at_version(version).unwrap());
    }
    if change == "dependency" {
        let draft = ComponentRevisionDraft::from_json(
            &json!({
                "format":"component-revision/1","uuid":helper,"class_code":22,
                "document":{"content":"def unused_helper():\n    return 1\n",
                    "preload":{"format":"python-preload/2","exports":{},
                        "private_functions":{"unused_helper":[]},"constants":[],"imports":[],
                        "dependencies":[],"default_export":null}},
                "dependencies":[],"association":null
            })
            .to_string(),
        )
        .unwrap();
        let reference = PgComponentRevisionStore::stage_in_transaction(tx, &draft, 0)
            .await
            .unwrap();
        revisions.push(draft.at_version(reference.version).unwrap());
    }
    let references: Vec<_> = revisions.iter().map(|r| r.reference()).collect();
    let snapshot = Arc::new(
        PgComponentRevisionStore::read_exact_in_transaction(
            tx,
            &[instruction.recipe().uuid],
            &references,
        )
        .await
        .unwrap(),
    );
    let compiled = compile_retained_recipe(
        snapshot,
        instruction.recipe().uuid,
        &instruction.variant().variant_key,
        instruction.class(),
    )
    .unwrap();
    let program = prepare_retained_tool_program(compiled)
        .unwrap()
        .retain_approvals_from(original_program)
        .unwrap();
    InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Tools(Arc::new(program)),
        &catalogue.worker,
        original.preparation_limits(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_canonical_pure_coverage_preserves_exact_v1_prepare_recheck_and_reload() {
    let rig = super::super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let pool = rig.pool.clone();
    let model_requests = Arc::new(StdMutex::new(Vec::new()));
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                crate::RebornBuildInput::postgres_with_reborn_home(
                    "canonical-pure-coverage-owner",
                    (*pool).clone(),
                    brassclaw_secrets::SecretMaterial::from(rig.url.expose_secret().to_owned()),
                    root.path().to_path_buf(),
                )
                .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_model_gateway_override(Arc::new(RecordingGateway {
                reply: "unexpected model response".into(),
                requests: model_requests.clone(),
            })),
        ))
        .await
        .unwrap(),
    );
    let catalogue = &runtime.installed_catalogue;
    let recipes: Vec<brassclaw_product_workflow::CatalogueRecipeSelection> = catalogue
        ._generation_candidates
        .iter()
        .map(|candidate| {
            let instruction = candidate.program().inputs().instruction();
            serde_json::from_value(json!({
                "recipe_id":instruction.recipe().uuid,"workflow_class":"deterministic",
                "components":instruction.snapshot().revisions().values()
                    .map(|r| reference(r.reference())).collect::<Vec<_>>(),
                "association_approvals":{}
            }))
            .unwrap()
        })
        .collect();
    let mut client = pool.get().await.unwrap();
    let before: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let prepared = catalogue
        .prepare_authored(&tx, &recipes, catalogue.preparation_policy().await.unwrap())
        .await
        .unwrap();
    assert_eq!(prepared.programs.len(), 2);
    for (candidate, selected) in catalogue
        ._generation_candidates
        .iter()
        .zip(&prepared.programs)
    {
        catalogue
            .verify_selected_approvals(&tx, selected)
            .await
            .unwrap();
        let instruction = candidate.program().inputs().instruction();
        // Golden /1 shape from the previous serializer. Pins come exclusively
        // from the real canonical producer used by startup, never SQL success.
        let RetainedProgram::Tools(program) = selected.program() else {
            panic!("Tool program required")
        };
        let mut pins = serde_json::Map::new();
        for (step, binding) in program.bindings() {
            let approval = catalogue
                .generation_approvals
                .iter()
                .find_map(|owner| owner.usage_approval(binding))
                .unwrap();
            pins.insert(step.clone(), json!({"approval_id":approval.approval_id(),
                "checksum":hex::encode(<sha2::Sha256 as sha2::Digest>::digest(approval.exact_bytes().as_bytes()))}));
        }
        let recipe = instruction.snapshot().revisions()[&instruction.recipe().uuid]
            .draft()
            .document();
        let expected = json!({"format":"monty-retained-recipe-selection/1",
            "recipe":reference(instruction.recipe()),"variant":instruction.variant(),
            "workflow_class":"deterministic","input_layout":recipe["input_layouts"][&instruction.variant().variant_key],
            "invocation_layout":recipe.get("invocations"),"step_order":instruction.ordered().step_order(),
            "components":instruction.snapshot().revisions().values().map(|r| reference(r.reference())).collect::<Vec<_>>(),
            "association_approvals":pins}).to_string();
        assert_eq!(
            selected
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .exact_bytes(),
            expected
        );
    }
    let retain = prepared
        .programs
        .iter()
        .find(|p| p.invocation("0:1").is_some())
        .unwrap();
    tx.commit().await.unwrap();
    for change in ["version", "source", "export", "layout", "dependency"] {
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        // Deliberately unauthenticated negative data, with no real Q1/behavior
        // evidence or closure index. V200 existence must grant no execution.
        // The positive cases above use only the actual canonical producer.
        let pure = retain.program().inputs().components()["0:1"];
        let approval_id = Uuid::new_v4();
        let q1 = Uuid::new_v4();
        let bytes = json!({"format":"pure-component-approval/1","approval_id":approval_id,
            "root":reference(pure),"components":[reference(pure)],"validation_mode":"system_seed",
            "q1_ref":q1,"q2_ref":null,"behavioral_refs":[Uuid::new_v4()]})
        .to_string();
        let checksum = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(bytes.as_bytes()));
        tx.execute("INSERT INTO reborn_pure_component_approvals
            (approval_id,root_id,root_version,root_checksum,validation_mode,q1_ref,q2_ref,approval_bytes,checksum)
            VALUES($1,$2,$3,$4,'system_seed',$5,NULL,$6,$7)",
            &[&approval_id, &pure.uuid, &(pure.version as i64), &hex::encode(pure.checksum), &q1, &bytes, &checksum])
            .await.unwrap();
        let changed = changed_selection(catalogue, retain, change, &tx).await;
        assert!(
            matches!(
                catalogue.verify_selected_approvals(&tx, &changed).await,
                Err(brassclaw_product_workflow::RecipeStoreError::Invalid(_))
            ),
            "{change}"
        );
        let instruction = changed.program().inputs().instruction();
        let selection = serde_json::from_value(json!({
            "recipe_id":instruction.recipe().uuid,"workflow_class":"deterministic",
            "components":instruction.snapshot().revisions().values().map(|r| reference(r.reference())).collect::<Vec<_>>(),
            "association_approvals":{}
        })).unwrap();
        assert!(
            matches!(
                catalogue
                    .prepare_authored(
                        &tx,
                        &[selection],
                        catalogue.preparation_policy().await.unwrap()
                    )
                    .await,
                Err(brassclaw_product_workflow::RecipeStoreError::Invalid(_))
            ),
            "{change}"
        );
        tx.rollback().await.unwrap();
    }

    let initial = catalogue.active.read().await.generation;
    catalogue
        .activate_authored(
            "canonical-coverage-operator",
            serde_json::from_value(json!({
                "activation_id":Uuid::new_v4(),"expected_generation":initial,"recipes":recipes
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let original = catalogue.active.read().await.clone();
    for _ in 0..2 {
        catalogue.restore_authored().await.unwrap();
        let restored = catalogue.active.read().await.clone();
        assert_eq!(restored.bytes, original.bytes);
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        catalogue
            .verify_source_selection(&tx, &restored)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    catalogue.activate_authored("canonical-coverage-operator", serde_json::from_value(json!({
        "activation_id":Uuid::new_v4(),"expected_generation":original.generation,"recipes":[]
    })).unwrap()).await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    // This is also the existing child capture's retained-view checker. Withdrawal
    // affects fresh capture only; no unsupported generation chat/child is run.
    catalogue
        .verify_retained_selection(&tx, &original)
        .await
        .unwrap();
    assert!(
        catalogue
            .verify_source_selection(&tx, &original)
            .await
            .is_err()
    );
    assert_eq!(
        tx.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        before
    );
    tx.commit().await.unwrap();
    assert!(model_requests.lock().unwrap().is_empty());
    shutdown_shared_runtime(runtime).await.unwrap();
}
