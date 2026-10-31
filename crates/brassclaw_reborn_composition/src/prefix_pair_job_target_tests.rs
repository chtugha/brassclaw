//! Real PostgreSQL target metadata and historical DDL upgrade, without admitting
//! a job, dispatching a Tool, approving a graph or activating any prefix.
use super::*;
use brassclaw_engine::{
    executor::retained_recipe::RetainedProgram,
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
use brassclaw_monty_host::{process::installed_worker, utility::UtilityLimits};
use brassclaw_skills::{
    component_revision::RetainedComponentSnapshot,
    prefix_pair_case_components::{PairCaseIds, pair_case_drafts},
};
use serde_json::json;
use std::time::Duration;

async fn program() -> Arc<InspectedRetainedProgram> {
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
    Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(Arc::new(program)),
            &installed_worker().unwrap(),
            UtilityLimits::from_execution(Default::default()).unwrap(),
        )
        .await
        .unwrap(),
    )
}

#[tokio::test]
async fn pair_job_target_retains_complete_source_owned_input_and_original_identity_without_role() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = rig.pool.clone();
    let fixture = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone())
        .await
        .unwrap();
    let namespace = crate::bootstrap_validation::NAMESPACE;
    let models =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(pool.clone(), namespace.into());
    let template = crate::bootstrap_encoding_fixture::TEMPLATE;
    let hash = crate::bootstrap_encoding_fixture::checksum;
    // Consistency-only literal cases: metadata capture never renders or accepts
    // their content. Full native execution is covered by the journal package.
    let draft = crate::prefix_pair_construction::tests::draft(
        &fixture.specification,
        &fixture.artifacts,
        "tokenizer_config.json",
        &hash(json!({"chat_template":template}).to_string().as_bytes()),
        json!("/chat_template"),
        &hash(template.as_bytes()),
        &hash(b"{}"),
    );
    let proposal = models
        .retain_pair_proposal(
            brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                specification: fixture.specification.clone(),
                proposal_bytes: draft.to_string(),
            },
        )
        .await
        .unwrap();
    let packet = models
        .case_packet(&fixture.specification, &proposal.id)
        .await
        .unwrap();
    let input = json!({"case_packet_bytes":packet.case_packet_bytes});
    let target = Target::Pair {
        specification: fixture.specification.clone(),
        proposal: proposal.id.clone(),
    };
    let request = target.pair_request().unwrap();
    let selected = program().await;
    let id = Uuid::new_v4();
    let actor = UserId::new("pair-metadata-operator").unwrap();
    let tenant = TenantId::new(namespace).unwrap();
    let agent = AgentId::new("pair-metadata").unwrap();
    let project = ProjectId::new("system").unwrap();
    let scope = TurnScope::new_with_owner(
        tenant.clone(),
        Some(agent.clone()),
        Some(project.clone()),
        ThreadId::new(format!("prefix-job-{id}")).unwrap(),
        Some(actor.clone()),
    );
    let thread = ThreadScope {
        tenant_id: tenant,
        agent_id: agent,
        project_id: Some(project),
        owner_user_id: Some(actor),
    };
    let store = Store::new(pool.clone());
    let reservation = Reservation {
        original_input_limits: brassclaw_skills::value_contract::ContractLimits {
            max_depth: selected.preparation_limits().bounds.max_value_depth,
            max_nodes: selected.preparation_limits().bounds.max_value_nodes,
            max_bytes: selected.preparation_limits().bounds.max_value_bytes,
        },
        id,
        target: &target,
        operation: "check_cases",
        mode: "target",
        request: &request,
        program: &selected,
        input: &input,
        scope: &scope,
        thread_scope: &thread,
        created_by: "pair-metadata-operator",
    };
    // All failures precede metadata insertion, admission and native effects.
    for change in [
        "case-subset",
        "extra-input",
        "wrong-target",
        "extra-request",
        "mode",
        "foreign-namespace",
    ] {
        let mut altered = input.clone();
        let mut offered = request.clone();
        let mut bad_scope = scope.clone();
        let mut bad_thread = thread.clone();
        let mut other_target = target.clone();
        let mut mode = "target";
        match change {
            "case-subset" => {
                let mut packet: Value =
                    serde_json::from_str(altered["case_packet_bytes"].as_str().unwrap()).unwrap();
                packet["cases"] = json!([]);
                altered["case_packet_bytes"] =
                    json!(crate::prefix_generations::canonical(&packet).unwrap());
            }
            "extra-input" => altered["extra"] = json!(true),
            "wrong-target" => {
                other_target = Target::Pair {
                    specification: fixture.specification.clone(),
                    proposal: "0".repeat(64),
                }
            }
            "extra-request" => offered["max_prefix_tokens"] = json!(100000),
            "mode" => mode = "source_only",
            "foreign-namespace" => {
                let tenant = TenantId::new("foreign-pair-metadata").unwrap();
                bad_scope.tenant_id = tenant.clone();
                bad_thread.tenant_id = tenant;
            }
            _ => unreachable!(),
        }
        assert!(
            store
                .reserve(Reservation {
                    target: &other_target,
                    request: &offered,
                    input: &altered,
                    scope: &bad_scope,
                    thread_scope: &bad_thread,
                    mode,
                    ..reservation
                })
                .await
                .is_err(),
            "{change}"
        );
        assert!(store.get(id).await.unwrap().is_none(), "{change}");
    }
    let first = store.reserve(Reservation { ..reservation }).await.unwrap();
    assert_eq!(first.target, target);
    assert!(first.target.profile().is_none());
    assert_eq!(first.input, input);
    let header = store.header(id).await.unwrap().unwrap();
    assert_eq!(header.target, target);
    assert_eq!(
        header.input_sha256,
        sha(&serde_json::to_string(&input).unwrap())
    );
    let client = pool.get().await.unwrap();
    let row=client.query_one("SELECT profile_name,profile_revision FROM brassclaw_prefix_job_definitions WHERE job_id=$1",&[&id]).await.unwrap();
    assert!(row.get::<_, Option<String>>(0).is_none());
    assert!(row.get::<_, Option<i64>>(1).is_none());
    let before = first.program_bytes.clone();
    let replacement = program().await;
    assert_ne!(
        replacement
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes(),
        before
    );
    let repeat = store
        .reserve(Reservation {
            program: &replacement,
            ..reservation
        })
        .await
        .unwrap();
    assert_eq!(repeat.program_bytes, before);
    assert_eq!(repeat.input, first.input);
    let mut changed = input.clone();
    changed["case_packet_bytes"] = json!("changed explicitly offered packet");
    assert!(matches!(
        store
            .reserve(Reservation {
                input: &changed,
                ..reservation
            })
            .await,
        Err(Error::Conflict)
    ));
    let mut changed_request = request.clone();
    changed_request["proposal"] = json!("f".repeat(64));
    let changed_target = Target::Pair {
        specification: fixture.specification.clone(),
        proposal: "f".repeat(64),
    };
    assert!(matches!(
        store
            .reserve(Reservation {
                target: &changed_target,
                request: &changed_request,
                ..reservation
            })
            .await,
        Err(Error::Conflict)
    ));
    let mut other_draft = draft.clone();
    other_draft["rationale"] = json!("Distinct immutable source for a real insert-winner conflict");
    let other_proposal = models
        .retain_pair_proposal(
            brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                specification: fixture.specification.clone(),
                proposal_bytes: other_draft.to_string(),
            },
        )
        .await
        .unwrap();
    let other_packet = models
        .case_packet(&fixture.specification, &other_proposal.id)
        .await
        .unwrap();
    let other_input = json!({"case_packet_bytes":other_packet.case_packet_bytes});
    let other_target = Target::Pair {
        specification: fixture.specification.clone(),
        proposal: other_proposal.id,
    };
    let other_request = other_target.pair_request().unwrap();
    let race_id = Uuid::new_v4();
    let mut race_scope = scope.clone();
    race_scope.thread_id = ThreadId::new(format!("prefix-job-{race_id}")).unwrap();
    let (left, right) = tokio::join!(
        store.reserve(Reservation {
            id: race_id,
            scope: &race_scope,
            ..reservation
        }),
        store.reserve(Reservation {
            id: race_id,
            scope: &race_scope,
            target: &other_target,
            request: &other_request,
            input: &other_input,
            ..reservation
        }),
    );
    let winner = match (left, right) {
        (Ok(winner), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(winner)) => winner,
        _ => panic!("one original pair definition and one conflict required"),
    };
    assert!(winner.target == target || winner.target == other_target);
    let retained = store.get(race_id).await.unwrap().unwrap();
    assert_eq!(retained.target, winner.target);
    assert_eq!(retained.input, winner.input);
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE job_id=$1",
                &[&race_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert!(
        client
            .execute(
                "UPDATE brassclaw_prefix_job_definitions SET mode='source_only' WHERE job_id=$1",
                &[&id]
            )
            .await
            .is_err()
    );
    for table in [
        "brassclaw_monty_task_admissions",
        "brassclaw_monty_tool_invocations",
    ] {
        let count: i64 = client
            .query_one(
                &format!("SELECT count(*) FROM {table} WHERE run_id=$1"),
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
    }
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn pair_target_migration_preserves_historical_rows_and_rejects_null_scope_bypasses() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    tx.batch_execute(
        "CREATE SCHEMA prefix_job_upgrade; SET LOCAL search_path TO prefix_job_upgrade,public",
    )
    .await
    .unwrap();
    // Execute the actual original migration texts, not an imitation table.
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V167__prefix_job_definitions.sql"
    ))
    .await
    .unwrap();
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V171__prefix_measurement_jobs.sql"
    ))
    .await
    .unwrap();
    let legacy = Uuid::new_v4();
    let empty = "{}";
    let checksum = sha(empty);
    tx.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by) VALUES($1,'base-prompt',1,'generate','source_only',$2,$3,$2,$3,$2,$3,'{}','{}','historical-ddl-fixture')",&[&legacy,&empty,&checksum]).await.unwrap();
    let before: Value = tx
        .query_one(
            "SELECT to_jsonb(j) FROM brassclaw_prefix_job_definitions j WHERE job_id=$1",
            &[&legacy],
        )
        .await
        .unwrap()
        .get(0);
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V181__prefix_pair_job_targets.sql"
    ))
    .await
    .unwrap();
    let after: Value = tx
        .query_one(
            "SELECT to_jsonb(j) FROM brassclaw_prefix_job_definitions j WHERE job_id=$1",
            &[&legacy],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after);
    let request =
        json!({"operation":"check_cases","specification":"a".repeat(64),"proposal":"b".repeat(64)});
    for change in [
        "valid",
        "legacy-no-profile",
        "half-profile",
        "pair-profile",
        "zero-revision",
        "null-specification",
        "numeric-proposal",
        "extra-field",
        "wrong-mode",
    ] {
        let id = Uuid::new_v4();
        let mut profile: Option<&str> = None;
        let mut revision: Option<i64> = None;
        let mut operation = "check_cases";
        let mut mode = "target";
        let mut metadata = request.clone();
        match change {
            "legacy-no-profile" => operation = "generate",
            "half-profile" => profile = Some("base-prompt"),
            "pair-profile" => {
                profile = Some("base-prompt");
                revision = Some(1);
            }
            "zero-revision" => {
                operation = "generate";
                profile = Some("base-prompt");
                revision = Some(0);
            }
            "null-specification" => metadata["specification"] = Value::Null,
            "numeric-proposal" => metadata["proposal"] = json!(42),
            "extra-field" => metadata["max_prefix_tokens"] = json!(100000),
            "wrong-mode" => mode = "source_only",
            _ => {}
        }
        let bytes = metadata.to_string();
        let hash = sha(&bytes);
        tx.batch_execute("SAVEPOINT candidate").await.unwrap();
        let inserted=tx.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,'{}',$8,'{}',$8,'{}','{}','ddl-constraint-fixture')",&[&id,&profile,&revision,&operation,&mode,&bytes,&hash,&checksum]).await;
        if change == "valid" {
            assert!(inserted.is_ok());
        } else {
            assert!(inserted.is_err(), "{change}");
        }
        tx.batch_execute("ROLLBACK TO SAVEPOINT candidate")
            .await
            .unwrap();
    }
    tx.batch_execute("SAVEPOINT immutable").await.unwrap();
    assert!(
        tx.execute(
            "UPDATE brassclaw_prefix_job_definitions SET profile_revision=2 WHERE job_id=$1",
            &[&legacy]
        )
        .await
        .is_err()
    );
    tx.batch_execute("ROLLBACK TO SAVEPOINT immutable")
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}
