//! Actual operator/PostgreSQL function subjects. No fixture grants approval.
use super::tests::{app, draft, request, send};
use super::*;
use axum::http::StatusCode;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;

const URI: &str = "/api/webchat/v2/component-review-submissions";

fn primitive(id: Uuid, class: i32, dependencies: &[Uuid]) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1", "uuid":id, "class_code":class,
            "document":{"name":"unreviewed dependency fixture"},
            "dependencies":dependencies, "association":null,
        })
        .to_string(),
    )
    .unwrap()
}
pub(crate) fn skill(
    id: Uuid,
    code: Uuid,
    binding: Uuid,
    tool: Uuid,
    prose: &str,
) -> ComponentRevisionDraft {
    let association = json!({"format":"skill-association/1", "skill_uuid":id,
        "python_code_uuid":code,"tool_skill_uuid":binding,"tool_uuid":tool,
        "callable":"host.read_file","inputs":{},"arguments":{},"code_arguments":{},
        "result":{"type":"string"},"failure":{"action":"stop","max_attempts":1,
            "idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}});
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1", "uuid":id, "class_code":1,
            "document":{"name":"unreviewed function fixture","content":prose},
            "dependencies":[code,binding,tool],"association":association.to_string(),
        })
        .to_string(),
    )
    .unwrap()
}
pub(crate) async fn prerequisites(
    store: &PgComponentRevisionStore,
) -> (ComponentRevisionRef, ComponentRevisionRef) {
    let tool = store
        .stage(&primitive(Uuid::new_v4(), 0, &[]), 0)
        .await
        .unwrap();
    let binding = store
        .stage(&primitive(Uuid::new_v4(), 13, &[tool.uuid]), 0)
        .await
        .unwrap();
    (tool, binding)
}
fn changed_partner(
    candidate: &ComponentRevisionDraft,
    base: Option<ComponentRevisionRef>,
    head: u64,
) -> Value {
    json!({"role":"changed","candidate_bytes":candidate.exact_bytes(),
        "base":base.map(selection),"expected_previous_version":head})
}
async fn subject(
    pool: &Arc<PgPool>,
    id: Uuid,
) -> brassclaw_skills::review_submission_store::RetainedValidatorSubject {
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let value =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
    tx.commit().await.unwrap();
    value
}

#[tokio::test]
async fn native_function_subject_atomically_retains_both_edits_and_exact_unknown_commit_retry() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let (tool, binding) = prerequisites(&store).await;
    let code = draft(
        Uuid::new_v4(),
        &[],
        "def read_value():\n    return 'ä猫 {data}'",
    );
    let usage = skill(
        Uuid::new_v4(),
        code.uuid(),
        binding.uuid,
        tool.uuid,
        "Prose and implementation are one review, not execution.",
    );
    let id = Uuid::new_v4();
    let mut input = request(id, &usage, None, &[tool, binding]);
    input["expected_previous_version"] = json!(0);
    input["function_partner"] = changed_partner(&code, None, 0);
    let application = app(database.pool.clone());
    // Exact association, changed-root separation and per-root allocation are
    // verified before any durable subject or partial candidate can survive.
    let mut malformed = input.clone();
    malformed["function_partner"]["approval"] = json!(true);
    assert_eq!(
        send(&application, "POST", URI, Some(&malformed), true)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut malformed = input.clone();
    malformed["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(json!(selection(ComponentRevisionRef {
            uuid: code.uuid(),
            class_code: 22,
            version: 1,
            checksum: code.checksum(),
        })));
    assert_eq!(
        send(&application, "POST", URI, Some(&malformed), true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut malformed = input.clone();
    malformed["function_partner"]["base"] = json!(selection(tool));
    assert_eq!(
        send(&application, "POST", URI, Some(&malformed), true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert!(
        store
            .read_allocation_head(code.uuid())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read_allocation_head(usage.uuid())
            .await
            .unwrap()
            .is_none()
    );
    let (status, receipt) = send(&application, "POST", URI, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["function_partner"]["changed"], true);
    assert_eq!(receipt["function_partner"]["reference"]["version"], 1);
    assert_eq!(receipt["review_status"], "unreviewed");
    assert_eq!(receipt["catalogue_activated"], false);
    let retained = subject(&database.pool, id).await;
    assert!(retained.is_function_review());
    assert_eq!(retained.value()["format"], "component-validator-subject/2");
    assert_eq!(
        retained.value()["candidate"]["revision_bytes"],
        usage.exact_bytes()
    );
    assert_eq!(
        retained.value()["function_partner"]["component"]["revision_bytes"],
        code.exact_bytes()
    );
    assert_eq!(retained.value()["function_partner"]["role"], "changed");
    assert_eq!(
        retained.value()["dependencies"].as_array().unwrap().len(),
        2
    );
    assert_eq!(retained.value()["actor"], "submission-test-operator");
    // A valid checksum alone must not permit a direct SQL writer to substitute
    // another code identity or wrong candidate bytes in the partner.
    let original = PgReviewSubmissionStore::new(database.pool.clone())
        .read(id)
        .await
        .unwrap()
        .unwrap();
    let mut forged: Value = serde_json::from_str(&original.subject_bytes).unwrap();
    let foreign = store
        .stage(&draft(Uuid::new_v4(), &[], "result = 'unrelated code'"), 0)
        .await
        .unwrap();
    forged["function_partner"]["candidate"] = json!(selection(foreign));
    forged["function_partner"]["candidate_bytes"] =
        json!(draft(foreign.uuid, &[], "result = 'unrelated code'").exact_bytes());
    let foreign_id = Uuid::new_v4();
    forged["submission_id"] = json!(foreign_id);
    let bytes = forged.to_string();
    let client = database.pool.get().await.unwrap();
    let error = client.execute("INSERT INTO reborn_component_review_submissions(submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum)
        VALUES($1,$2,$3,1,NULL,$4,encode(sha256(convert_to($4,'UTF8')),'hex'))",
        &[&foreign_id, &original.actor, &usage.uuid(), &bytes]).await.unwrap_err();
    assert_eq!(error.code().unwrap().code(), "23514");
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_review_requests WHERE submission_id=$1",
                &[&foreign_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(client);
    // Advancing either head never changes the original logical review or retry.
    store
        .stage(&draft(code.uuid(), &[], "result = 'later code'"), 1)
        .await
        .unwrap();
    store
        .stage(
            &skill(
                usage.uuid(),
                code.uuid(),
                binding.uuid,
                tool.uuid,
                "later prose",
            ),
            1,
        )
        .await
        .unwrap();
    let recovered = send(&application, "POST", URI, Some(&input), true).await;
    assert_eq!(recovered, (StatusCode::OK, receipt));
    assert_eq!(subject(&database.pool, id).await.value(), retained.value());
    let (status, view) = send(&application, "GET", &format!("{URI}/{id}"), None, true).await;
    assert_eq!(status, StatusCode::OK);
    let original: Value = serde_json::from_str(view["subject_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(original["format"], "component-review-submission/3");
    assert_eq!(
        original["function_partner"]["candidate_bytes"],
        code.exact_bytes()
    );
    assert_eq!(view["component_bytes"].as_array().unwrap().len(), 4);
    input["function_partner"] =
        changed_partner(&draft(code.uuid(), &[], "result = 'forgery'"), None, 0);
    assert_eq!(
        send(&application, "POST", URI, Some(&input), true).await.0,
        StatusCode::CONFLICT
    );
    let client = database.pool.get().await.unwrap();
    let counts = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_submissions),
        (SELECT count(*) FROM reborn_component_review_requests),
        (SELECT count(*) FROM reborn_component_review_evidence),
        (SELECT count(*) FROM brassclaw_monty_active_catalogue)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2),
            counts.get::<_, i64>(3)
        ),
        (1, 1, 0, 0)
    );
}

#[tokio::test]
async fn native_function_subject_keeps_unchanged_skill_as_context_without_allocating_it() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let (tool, binding) = prerequisites(&store).await;
    let code_id = Uuid::new_v4();
    let old_code = store
        .stage(&draft(code_id, &[], "result = 'old'"), 0)
        .await
        .unwrap();
    let usage = skill(
        Uuid::new_v4(),
        code_id,
        binding.uuid,
        tool.uuid,
        "unchanged context",
    );
    let old_skill = store.stage(&usage, 0).await.unwrap();
    let next = draft(code_id, &[], "result = 'new code, unchanged Skill'");
    let id = Uuid::new_v4();
    let mut input = request(id, &next, Some(old_code), &[tool, binding]);
    input["expected_previous_version"] = json!(1);
    input["function_partner"] = json!({"role":"context", "reference":selection(old_skill)});
    let application = app(database.pool.clone());
    let (status, receipt) = send(&application, "POST", URI, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["function_partner"]["changed"], false);
    let original = subject(&database.pool, id).await;
    assert_eq!(original.value()["function_partner"]["role"], "context");
    assert_eq!(
        original.value()["function_partner"]["component"]["revision_bytes"],
        usage.exact_bytes()
    );
    assert_eq!(
        original.value()["base_comparison"]["reference"]["version"],
        1
    );
    assert_eq!(
        store.read_allocation_head(old_skill.uuid).await.unwrap(),
        Some((1, 1))
    );
    assert_eq!(
        store.read_allocation_head(code_id).await.unwrap(),
        Some((22, 2))
    );
    // A successor context and an unrelated UUID cannot replace the selected one.
    store
        .stage(
            &skill(usage.uuid(), code_id, binding.uuid, tool.uuid, "new Skill"),
            1,
        )
        .await
        .unwrap();
    assert_eq!(subject(&database.pool, id).await.value(), original.value());
    input["submission_id"] = json!(Uuid::new_v4());
    input["candidate_bytes"] =
        json!(draft(Uuid::new_v4(), &[], "result = 'unrelated'").exact_bytes());
    input["base"] = Value::Null;
    input["expected_previous_version"] = json!(0);
    assert_eq!(
        send(&application, "POST", URI, Some(&input), true).await.0,
        StatusCode::BAD_REQUEST
    );
    let unrelated: Value =
        serde_json::from_str(input["candidate_bytes"].as_str().unwrap()).unwrap();
    let unrelated = Uuid::parse_str(unrelated["uuid"].as_str().unwrap()).unwrap();
    assert!(
        store
            .read_allocation_head(unrelated)
            .await
            .unwrap()
            .is_none()
    );
    // A missing exact partner is rejected, with no candidate allocation leak.
    input["function_partner"]["reference"]["version"] = json!(999);
    assert_eq!(
        send(&application, "POST", URI, Some(&input), true).await.0,
        StatusCode::BAD_REQUEST
    );
    assert!(
        store
            .read_allocation_head(unrelated)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_function_subject_second_root_conflict_rolls_back_first_and_races_coherently() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let (tool, binding) = prerequisites(&store).await;
    // Code sorts first, so a stale Skill head fails after code was staged.
    let code_id = Uuid::from_u128(1);
    let skill_id = Uuid::from_u128(2);
    let old_code = store
        .stage(&draft(code_id, &[], "result = 'old'"), 0)
        .await
        .unwrap();
    let usage = skill(skill_id, code_id, binding.uuid, tool.uuid, "old");
    let old_skill = store.stage(&usage, 0).await.unwrap();
    store
        .stage(
            &skill(
                skill_id,
                code_id,
                binding.uuid,
                tool.uuid,
                "intervening draft",
            ),
            1,
        )
        .await
        .unwrap();
    let code = draft(code_id, &[], "result = 'new'");
    let usage = skill(skill_id, code_id, binding.uuid, tool.uuid, "new");
    let mut left = request(Uuid::new_v4(), &code, Some(old_code), &[tool, binding]);
    left["expected_previous_version"] = json!(1);
    left["function_partner"] = changed_partner(&usage, Some(old_skill), 1);
    let application = app(database.pool.clone());
    assert_eq!(
        send(&application, "POST", URI, Some(&left), true).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        store.read_allocation_head(code_id).await.unwrap(),
        Some((22, 1))
    );
    assert_eq!(
        store.read_allocation_head(skill_id).await.unwrap(),
        Some((1, 2))
    );
    left["function_partner"] = changed_partner(&usage, Some(old_skill), 2);
    let mut wrong_base = left.clone();
    wrong_base["function_partner"]["base"]["checksum"] = json!("00".repeat(32));
    assert_eq!(
        send(&application, "POST", URI, Some(&wrong_base), true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        store.read_allocation_head(code_id).await.unwrap(),
        Some((22, 1))
    );
    let mut right = request(Uuid::new_v4(), &usage, Some(old_skill), &[tool, binding]);
    right["expected_previous_version"] = json!(2);
    right["function_partner"] = changed_partner(&code, Some(old_code), 1);
    let (a, b) = tokio::join!(
        send(&application, "POST", URI, Some(&left), true),
        send(&application, "POST", URI, Some(&right), true)
    );
    assert_eq!(
        usize::from(a.0 == StatusCode::OK) + usize::from(b.0 == StatusCode::OK),
        1
    );
    let original = if a.0 == StatusCode::OK { &left } else { &right };
    assert_eq!(
        send(&application, "POST", URI, Some(original), true)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        store.read_allocation_head(code_id).await.unwrap(),
        Some((22, 2))
    );
    assert_eq!(
        store.read_allocation_head(skill_id).await.unwrap(),
        Some((1, 3))
    );
    let client = database.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_requests", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

#[tokio::test]
async fn native_function_subject_forward_migration_preserves_old_subjects_and_intents() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let submissions = PgReviewSubmissionStore::new(database.pool.clone());
    let first = draft(Uuid::new_v4(), &[], "result = 'legacy first'");
    let legacy = submissions
        .submit(
            &ReviewSubmissionDraft::new(
                Uuid::new_v4(),
                "upgrade-operator",
                first.clone(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let next = draft(first.uuid(), &[], "result = 'legacy second'");
    let explicit = submissions
        .submit(
            &ReviewSubmissionDraft::new_after_head(
                Uuid::new_v4(),
                "upgrade-operator",
                next,
                Some(legacy.candidate),
                vec![],
                1,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let (tool, binding) = prerequisites(&store).await;
    let code = draft(Uuid::new_v4(), &[], "result = 'new function'");
    let usage = skill(
        Uuid::new_v4(),
        code.uuid(),
        binding.uuid,
        tool.uuid,
        "new function",
    );
    let joint = submissions
        .submit(
            &ReviewSubmissionDraft::new_function(
                Uuid::new_v4(),
                "upgrade-operator",
                usage,
                None,
                vec![tool, binding],
                0,
                FunctionReviewPartnerDraft::Changed {
                    candidate: Box::new(code),
                    base: None,
                    expected_previous_version: 0,
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let client = database.pool.get().await.unwrap();
    // Exercise the actual old table/constraint upgrade on a private schema,
    // retaining historical bytes rather than editing applied migrations or
    // disabling immutable triggers in the normal fixture's public schema.
    client.batch_execute("CREATE SCHEMA function_subject_upgrade; SET search_path TO function_subject_upgrade, public").await.unwrap();
    client
        .batch_execute(concat!(
            include_str!("../../brassclaw_pg/migrations/V101__immutable_component_revisions.sql"),
            include_str!(
                "../../brassclaw_pg/migrations/V102__immutable_association_review_records.sql"
            ),
            include_str!(
                "../../brassclaw_pg/migrations/V104__immutable_component_review_submissions.sql"
            ),
            include_str!("../../brassclaw_pg/migrations/V148__review_lineage_allocation.sql"),
            include_str!(
                "../../brassclaw_pg/migrations/V149__immutable_component_review_requests.sql"
            ),
        ))
        .await
        .unwrap();
    client.batch_execute("INSERT INTO reborn_component_revision_heads SELECT * FROM public.reborn_component_revision_heads;
        INSERT INTO reborn_component_revisions SELECT * FROM public.reborn_component_revisions;
        INSERT INTO reborn_component_review_submissions SELECT * FROM public.reborn_component_review_submissions
        WHERE subject_bytes::jsonb->>'format' IN ('component-review-submission/1','component-review-submission/2')").await.unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V193__joint_function_review_subjects.sql"
        ))
        .await
        .unwrap();
    for old in [&legacy, &explicit] {
        let row=client.query_one("SELECT s.subject_bytes,s.checksum,r.subject_checksum FROM reborn_component_review_submissions s JOIN reborn_component_review_requests r USING(submission_id) WHERE s.submission_id=$1",&[&old.id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), old.subject_bytes);
        assert_eq!(row.get::<_, String>(1), old.checksum);
        assert_eq!(row.get::<_, String>(2), old.checksum);
    }
    assert_eq!(client.execute("INSERT INTO reborn_component_review_submissions SELECT * FROM public.reborn_component_review_submissions WHERE submission_id=$1",&[&joint.id]).await.unwrap(),1);
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_requests", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        3
    );
    let error = client
        .execute(
            "UPDATE reborn_component_review_submissions SET actor=actor WHERE submission_id=$1",
            &[&joint.id],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "23514");
    client
        .batch_execute("SET search_path TO public")
        .await
        .unwrap();
    assert_eq!(
        submissions
            .read(joint.id)
            .await
            .unwrap()
            .unwrap()
            .subject_bytes,
        joint.subject_bytes
    );
}

#[cfg(feature = "skills-db")]
#[tokio::test]
async fn native_function_subject_rejects_partial_single_class_preparation_at_both_owners() {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
            retained_validator::RetainedValidatorCoverage,
        },
    };
    use brassclaw_skills::{
        review_submission_store::selection as preparations,
        validator_components::{aggregate_draft, collect_draft, structural_presence_draft},
    };
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let (tool, binding) = prerequisites(&store).await;
    let code = draft(Uuid::new_v4(), &[], "result = 'unreviewed code'");
    let usage = skill(
        Uuid::new_v4(),
        code.uuid(),
        binding.uuid,
        tool.uuid,
        "unreviewed Skill",
    );
    let id = Uuid::new_v4();
    let mut input = request(id, &code, None, &[tool, binding]);
    input["expected_previous_version"] = json!(0);
    input["function_partner"] = changed_partner(&usage, None, 0);
    assert_eq!(
        send(&app(database.pool.clone()), "POST", URI, Some(&input), true)
            .await
            .0,
        StatusCode::OK
    );
    let subject = subject(&database.pool, id).await;
    let (root, rule, collection, aggregation) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut refs = Vec::new();
    for helper in [
        structural_presence_draft(rule).unwrap(),
        collect_draft(collection, &["structural_presence".into()]).unwrap(),
        aggregate_draft(aggregation).unwrap(),
    ] {
        refs.push(store.stage(&helper, 0).await.unwrap());
    }
    let recipe = super::model_selection_tests::recipe(root, rule, collection, aggregation);
    let recipe = ComponentRevisionDraft::from_json(&recipe.to_string()).unwrap();
    refs.push(store.stage(&recipe, 0).await.unwrap());
    let snapshot = Arc::new(
        store
            .read_exact(
                &[root],
                &refs,
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let instruction =
        compile_retained_recipe(snapshot, root, "selected", WorkflowClass::RequiresModel).unwrap();
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(
            prepare_retained_tool_program(instruction).unwrap(),
        )),
        &worker,
    )
    .await
    .unwrap();
    let coverage = RetainedValidatorCoverage::from_inputs(inspected.program().inputs()).unwrap();
    assert!(coverage.require_subject(&subject).is_err());
    assert!(
        coverage
            .subject_task_inputs(&subject, inspected.program().inputs(), REVISION_LIMITS)
            .is_err()
    );
    assert!(
        RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS).is_err()
    );
    // Even checksum-correct /1 metadata made by a lower caller cannot bypass
    // the storage owner's full-subject guard and retain a partial selection.
    let selected = inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .unwrap();
    let forged=json!({"format":"component-validator-selection/1","submission_id":id,
        "subject_checksum":subject.subject_checksum(),"candidate":selection(subject.candidate()),
        "validator_selection_bytes":selected.exact_bytes(),"validator_selection_checksum":hex::encode(selected.checksum()),"scope":"preparation-only"}).to_string();
    let mut client = database.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    assert!(
        preparations::retain_in_transaction(&tx, &forged, REVISION_LIMITS)
            .await
            .is_err()
    );
    assert_eq!(
        tx.query_one(
            "SELECT count(*) FROM reborn_component_validator_selections",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    tx.commit().await.unwrap();
}
