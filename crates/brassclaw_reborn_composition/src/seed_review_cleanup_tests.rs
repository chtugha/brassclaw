use super::*;

#[tokio::test]
async fn native_boot_retires_only_exact_untouched_seeds_and_preserves_originals() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let booted = crate::booted_db::run_migrations_and_return_booted_db(rig.pool.clone())
        .await
        .unwrap();
    // Invalid canonical constructors fail before any retirement transaction;
    // error handling must never turn malformed metadata into an empty match.
    struct SerializationFailure;
    impl serde::Serialize for SerializationFailure {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom(
                "injected constructor serialization failure",
            ))
        }
    }
    scope(async {
        expect("reborn_tools", &SerializationFailure);
        assert_eq!(
            retire(&rig.pool).await.unwrap_err(),
            "canonical seed definition could not be serialized"
        );
    })
    .await;
    scope(async {
        expect("reborn_tools", &json!([]));
        assert_eq!(
            retire(&rig.pool).await.unwrap_err(),
            "canonical seed definition has invalid scope or name"
        );
    })
    .await;
    scope(async {
        expect(
            "reborn_extension_catalogues",
            &json!({"tenant_id":"test", "user_id":"test",
            "agent_id":"test", "project_id":"test", "name":"test", "child_component_ids":null}),
        );
        children("test", "test", &[Uuid::new_v4()]);
        assert_eq!(
            retire(&rig.pool).await.unwrap_err(),
            "canonical seed catalogue children are not an array"
        );
    })
    .await;
    assert_eq!(
        retire(&rig.pool).await.unwrap_err(),
        "seed cleanup requires the canonical boot scope"
    );
    assert!(!canonical_matches(
        "reborn_tools",
        &json!({"class_code":0,"validation_status":"pending"}),
        &json!([])
    ));
    assert!(!canonical_matches(
        "reborn_extension_catalogues",
        &json!({"class_code":23,"validation_status":"pending","child_component_ids":null}),
        &json!({"child_component_ids":[]})
    ));
    assert!(!canonical_matches(
        "reborn_extension_catalogues",
        &json!({"class_code":23,"validation_status":"pending","child_component_ids":[]}),
        &json!({"child_component_ids":null})
    ));
    let tenant = "retire-seed-test";
    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
    let definitions = scope(async {
        crate::seed_builtin_host::seed_builtin_host_components(&booted, tenant)
            .await
            .unwrap();
        crate::builtin_bootstrap::seed_builtin_components(&booted, tenant)
            .await
            .unwrap();
        PACKAGE.with(|p| p.borrow().definitions.clone())
    })
    .await;
    let client = rig.pool.get().await.unwrap();
    let mut selected = BTreeMap::new();
    for ((table, _), expected) in &definitions {
        if selected.contains_key(table) {
            continue;
        }
        if [
            "orchestrator:main",
            "failure_explanation",
            "compaction_summarizer_fresh",
        ]
        .contains(&expected["name"].as_str().unwrap())
        {
            continue;
        }
        let name = expected["name"].as_str().unwrap();
        let row = client
            .query_one(
                &format!("SELECT id,to_jsonb(c) FROM {table} c WHERE tenant_id=$1 AND name=$2"),
                &[&tenant, &name],
            )
            .await
            .unwrap();
        let actual: Value = row.get(1);
        if canonical_matches(table, &actual, expected) {
            selected.insert(table.clone(), (row.get::<_, Uuid>(0), expected.clone()));
        }
    }
    assert_eq!(
        selected.len(),
        7,
        "each legacy seed table must be exercised: {:?}",
        selected.keys()
    );
    // A legacy exact packaged row may still carry pending projection status.
    client
        .execute(
            "UPDATE reborn_python_code SET validation_status='pending' WHERE id=$1",
            &[&selected["reborn_python_code"].0],
        )
        .await
        .unwrap();
    let mut originals = Vec::new();
    for (table, (id, _)) in &selected {
        // Legacy writer fixture; this creates no Q1/Q2 approval.
        client.execute(&format!("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class)
            SELECT tenant_id,user_id,agent_id,project_id,id,class_code FROM {table} WHERE id=$1"),&[id]).await.unwrap();
        let row = client
            .query_one(
                "SELECT id,to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
                &[id],
            )
            .await
            .unwrap();
        originals.push((row.get::<_, Uuid>(0), row.get::<_, String>(1)));
    }
    // A real queued edit to a different shipped seed remains pending.
    let edit: Uuid = client
        .query_one(
            "SELECT id FROM reborn_skills WHERE tenant_id=$1 AND name='orchestrator:main'",
            &[&tenant],
        )
        .await
        .unwrap()
        .get(0);
    assert_ne!(edit, selected["reborn_skills"].0);
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,proposed_payload)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code,'{\"description\":\"operator edit\"}'::jsonb FROM reborn_skills WHERE id=$1",&[&edit]).await.unwrap();
    // An additional metadata edit matters even when the body is unchanged.
    let altered: Uuid = client
        .query_one(
            "SELECT id FROM reborn_skills WHERE tenant_id=$1 AND name='failure_explanation'",
            &[&tenant],
        )
        .await
        .unwrap()
        .get(0);
    assert_ne!(altered, selected["reborn_skills"].0);
    client
        .execute(
            "UPDATE reborn_skills SET allowed_tools=ARRAY['operator-tool'] WHERE id=$1",
            &[&altered],
        )
        .await
        .unwrap();
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code FROM reborn_skills WHERE id=$1",&[&altered]).await.unwrap();
    // Keep a genuine historical failed review on another exact package row.
    let historical: Uuid = client.query_one("SELECT id FROM reborn_skills WHERE tenant_id=$1 AND name='compaction_summarizer_fresh'",&[&tenant]).await.unwrap().get(0);
    assert_ne!(historical, selected["reborn_skills"].0);
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,validation_errors)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code,ARRAY['historical finding'] FROM reborn_skills WHERE id=$1",&[&historical]).await.unwrap();
    // A real immutable authored subject prevents cleanup, even with pristine
    // queue fields and a byte-identical current package definition.
    let bound: Uuid = client.query_one("SELECT id FROM reborn_python_code WHERE tenant_id=$1 AND id<>$2 AND includes='[]'::jsonb LIMIT 1", &[&tenant,&selected["reborn_python_code"].0]).await.unwrap().get(0);
    let document: Value = client
        .query_one(
            "SELECT to_jsonb(c) FROM reborn_python_code c WHERE id=$1",
            &[&bound],
        )
        .await
        .unwrap()
        .get(0);
    let draft = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1","uuid":bound,"class_code":22,
            "document":document,"dependencies":[],"association":null
        })
        .to_string(),
    )
    .unwrap();
    let submission_id = Uuid::new_v4();
    let submission = brassclaw_skills::review_submission_store::ReviewSubmissionDraft::new(
        submission_id,
        "operator",
        draft,
        None,
        Vec::new(),
    )
    .unwrap();
    brassclaw_skills::review_submission_store::PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(&submission)
        .await
        .unwrap();
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code FROM reborn_python_code WHERE id=$1", &[&bound]).await.unwrap();
    let subject_before: String = client
        .query_one(
            "SELECT subject_bytes FROM reborn_component_review_submissions WHERE submission_id=$1",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    let forged: Uuid = client.query_one("INSERT INTO reborn_skills(tenant_id,user_id,agent_id,project_id,name,description,body,class_code,source,validation_status)
        SELECT tenant_id,'operator',agent_id,project_id,name,description,body,class_code,'system','pending' FROM reborn_skills WHERE id=$1 RETURNING id", &[&altered]).await.unwrap().get(0);
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code FROM reborn_skills WHERE id=$1", &[&forged]).await.unwrap();
    let preserved = client.query("SELECT id,to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=ANY($1) ORDER BY id",&[&vec![edit,altered,historical,bound,forged]]).await.unwrap().into_iter().map(|r|(r.get::<_,Uuid>(0),r.get::<_,String>(1))).collect::<Vec<_>>();
    drop(client);
    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    assert_eq!(client.query_one("SELECT subject_bytes FROM reborn_component_review_submissions WHERE submission_id=$1", &[&submission_id]).await.unwrap().get::<_,String>(0),subject_before);
    assert_eq!(
        client
            .query_one(
                "SELECT validation_status FROM reborn_python_code WHERE id=$1",
                &[&selected["reborn_python_code"].0]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "validated"
    );
    for (id, bytes) in &originals {
        let row = client.query_one("SELECT queue_bytes,queue_checksum FROM reborn_retired_seed_review_requests WHERE queue_id=$1",&[id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), *bytes);
        assert_eq!(row.get::<_, String>(1), crate::checksum::sha256_hex(bytes));
        assert!(
            client
                .query_opt("SELECT id FROM reborn_validation_queue WHERE id=$1", &[id])
                .await
                .unwrap()
                .is_none()
        );
    }
    for (id, bytes) in &preserved {
        let row = client
            .query_one(
                "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE id=$1",
                &[id],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, String>(0),
            *bytes,
            "authored and historical rows stay byte-identical"
        );
    }
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_retired_seed_review_requests",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        7
    );
    assert!(
        client
            .execute(
                "DELETE FROM reborn_retired_seed_review_requests WHERE queue_id=$1",
                &[&originals[0].0]
            )
            .await
            .is_err()
    );
    // Archive failure must leave the original queue intact (atomic rollback).
    let id = selected["reborn_tools"].0;
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class)
        SELECT tenant_id,user_id,agent_id,project_id,id,class_code FROM reborn_tools WHERE id=$1",&[&id]).await.unwrap();
    client.batch_execute("CREATE FUNCTION fail_seed_archive() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'archive failure'; END $$;
        CREATE TRIGGER fail_seed_archive BEFORE INSERT ON reborn_retired_seed_review_requests FOR EACH ROW EXECUTE FUNCTION fail_seed_archive()").await.unwrap();
    drop(client);
    assert!(
        crate::component_boot::initialize_runtime_components(&booted, tenant)
            .await
            .is_err()
    );
    let client = rig.pool.get().await.unwrap();
    assert!(
        client
            .query_opt(
                "SELECT id FROM reborn_validation_queue WHERE component_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .is_some()
    );
    client.batch_execute("DROP TRIGGER fail_seed_archive ON reborn_retired_seed_review_requests; DROP FUNCTION fail_seed_archive()").await.unwrap();
    drop(client);
    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
}
