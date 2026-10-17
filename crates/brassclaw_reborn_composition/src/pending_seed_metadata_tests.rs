//! Raw legacy metadata remains pending as requested. Labels cannot establish
//! immutable v3 approval, a native registration or a complete Skill interface.
use crate::{
    pg_skill_store::{NewPgSkill, PgSkillStore},
    pg_tool_skill_store::{NewPgToolSkill, PgToolSkillStore},
    pg_tool_store::{NewPgTool, PgToolStore},
};
use serde_json::json;

#[tokio::test]
async fn native_authored_system_label_stays_pending_and_catalogue_queue_failure_rolls_back() {
    use crate::{
        pg_extension_catalogue_store::{NewPgExtensionCatalogue, PgExtensionCatalogueStore},
        pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore},
        pg_recipe_store::{NewPgRecipe, PgRecipeStore},
        validation_queue::ValidationQueueStore,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let recipes = PgRecipeStore::new(rig.pool.clone());
    let codes = PgPythonCodeStore::new(rig.pool.clone());
    let catalogues = PgExtensionCatalogueStore::new(rig.pool.clone());
    let queue = ValidationQueueStore::new(rig.pool.clone());
    // Incomplete unreviewed legacy drafts, deliberately not activated or run.
    let recipe = NewPgRecipe {
        tenant_id: "authored-status-test".into(),
        user_id: "owner".into(),
        agent_id: "agent".into(),
        project_id: "data".into(),
        name: "pending-system-recipe".into(),
        description: "Unreviewed draft".into(),
        trigger: None,
        steps: json!({"llm_call_required":false,"steps":[]}),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec![],
        intent_examples: None,
        source: "system".into(),
        step_descriptions: None,
        variants: None,
        dependency_registry: None,
        validates_class_code: None,
    };
    let code = NewPgPythonCode {
        tenant_id: recipe.tenant_id.clone(),
        user_id: recipe.user_id.clone(),
        agent_id: recipe.agent_id.clone(),
        project_id: recipe.project_id.clone(),
        name: "pending-system-code".into(),
        description: "Unreviewed draft".into(),
        content: "result = None".into(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec![],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
        includes: vec![],
        content_checksum: None,
    };
    let catalogue = NewPgExtensionCatalogue {
        tenant_id: recipe.tenant_id.clone(),
        user_id: recipe.user_id.clone(),
        agent_id: recipe.agent_id.clone(),
        project_id: recipe.project_id.clone(),
        name: "pending-system-catalogue".into(),
        description: "Unreviewed draft".into(),
        version: "1.0".into(),
        overview_doc: "Draft metadata only".into(),
        task_groups: json!([]),
        child_component_ids: vec![],
        intent_index: None,
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec![],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
    };
    let plain = [
        recipes.insert(recipe.clone()).await.unwrap(),
        codes.insert(code.clone()).await.unwrap(),
        catalogues.insert(catalogue.clone()).await.unwrap(),
    ];
    let mut queued_recipe = recipe.clone();
    queued_recipe.name.push_str("-queued");
    let mut queued_code = code.clone();
    queued_code.name.push_str("-queued");
    let mut queued_catalogue = catalogue.clone();
    queued_catalogue.name.push_str("-queued");
    let queued = [
        recipes
            .create_and_submit(queued_recipe, &queue)
            .await
            .unwrap(),
        codes.create_and_submit(queued_code, &queue).await.unwrap(),
        catalogues
            .create_and_submit(queued_catalogue, &queue)
            .await
            .unwrap(),
    ];
    let client = rig.pool.get().await.unwrap();
    for ((table, class), (plain_id, queued_id)) in [
        ("reborn_recipes", 21),
        ("reborn_python_code", 22),
        ("reborn_extension_catalogues", 23),
    ]
    .into_iter()
    .zip(plain.into_iter().zip(queued))
    {
        let query = format!("SELECT validation_status FROM {table} WHERE id=ANY($1) ORDER BY id");
        let rows = client
            .query(&query, &[&vec![plain_id, queued_id]])
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.get::<_, String>(0) == "pending"));
        let row = client
            .query_one(
                "SELECT state,component_class FROM reborn_validation_queue WHERE component_id=$1",
                &[&queued_id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, i16>(0), 1);
        assert_eq!(row.get::<_, i16>(1), class);
    }
    let tiers = client
        .query(
            "SELECT tier,wilson_lower FROM reborn_recipes WHERE id=ANY($1)",
            &[&vec![plain[0], queued[0]]],
        )
        .await
        .unwrap();
    assert!(
        tiers
            .iter()
            .all(|row| row.get::<_, String>(0) == "seedling" && row.get::<_, f64>(1) == 0.0)
    );
    // An ordinary source label cannot select the installation-only method.
    let mut wrong_recipe = recipe;
    wrong_recipe.source = "user".into();
    let mut wrong_code = code;
    wrong_code.source = "user".into();
    let mut wrong_catalogue = catalogue.clone();
    wrong_catalogue.source = "user".into();
    assert!(
        recipes
            .insert_installation_projection(wrong_recipe, true)
            .await
            .is_err()
    );
    assert!(
        codes
            .insert_installation_projection(wrong_code)
            .await
            .is_err()
    );
    assert!(
        catalogues
            .insert_installation_projection(wrong_catalogue)
            .await
            .is_err()
    );
    client.batch_execute("CREATE FUNCTION reject_metadata_catalogue_queue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='authored-status-test' AND NEW.component_class=23 THEN RAISE EXCEPTION 'deliberate queue rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_metadata_catalogue_queue BEFORE INSERT ON reborn_validation_queue FOR EACH ROW EXECUTE FUNCTION reject_metadata_catalogue_queue();").await.unwrap();
    let mut rejected = catalogue;
    rejected.name = "atomic-queue-rejected".into();
    assert!(
        catalogues
            .create_and_submit(rejected, &queue)
            .await
            .is_err()
    );
    let count:i64=client.query_one("SELECT count(*) FROM reborn_extension_catalogues WHERE tenant_id='authored-status-test' AND name='atomic-queue-rejected'",&[]).await.unwrap().get(0);
    assert_eq!(
        count, 0,
        "queue failure must roll back the newly inserted catalogue"
    );
    client.batch_execute("DROP TRIGGER reject_metadata_catalogue_queue ON reborn_validation_queue; DROP FUNCTION reject_metadata_catalogue_queue();").await.unwrap();
}

#[tokio::test]
async fn native_system_label_preserves_explicit_pending_metadata_and_conflicts() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tools = PgToolStore::new(rig.pool.clone());
    let bindings = PgToolSkillStore::new(rig.pool.clone());
    let skills = PgSkillStore::new(rig.pool.clone());
    // Deliberately incomplete unreviewed metadata. No usage/executable/artifact
    // association, registered handler, component approval or active route.
    let tool = NewPgTool {
        tenant_id: "metadata-test".into(),
        user_id: "owner".into(),
        agent_id: "agent".into(),
        project_id: "data".into(),
        name: "pending-json-tool".into(),
        description: "Unreviewed primitive metadata".into(),
        param_schema: None,
        param_template: None,
        effect_type: "read".into(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec![],
        source: "system".into(),
        validation_status: "pending".into(),
        capability_id: "builtin.json".into(),
    };
    let binding = NewPgToolSkill {
        tenant_id: tool.tenant_id.clone(),
        user_id: tool.user_id.clone(),
        agent_id: tool.agent_id.clone(),
        project_id: tool.project_id.clone(),
        name: "pending-json-binding".into(),
        description: "Unreviewed binding metadata".into(),
        content: "Pending binding declaration".into(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("json".into()),
        param_schema: None,
        param_template: None,
        consumer_tags: vec![],
        intent_examples: None,
        source: "system".into(),
        validation_status: "pending".into(),
        includes: vec![],
        content_checksum: None,
    };
    let skill = NewPgSkill {
        tenant_id: tool.tenant_id.clone(),
        user_id: tool.user_id.clone(),
        agent_id: tool.agent_id.clone(),
        project_id: tool.project_id.clone(),
        name: "pending-json-usage".into(),
        description: "Unreviewed usage metadata".into(),
        body: "Pending interface; associated code has not been submitted.".into(),
        class_code: 1,
        consumer_tags: vec![],
        intent_examples: json!([{"input":"Parse this unreviewed metadata example.","class":3}]),
        source: "system".into(),
        validation_status: "pending".into(),
        content_checksum: None,
    };
    let tool_id = tools.insert(tool.clone()).await.unwrap().unwrap();
    let binding_id = bindings.insert(binding.clone()).await.unwrap().unwrap();
    let skill_id = skills.insert(skill.clone()).await.unwrap().unwrap();
    // Conflicting inserts must not rewrite requested state, body or identity.
    // The conflicting label is never persisted and is not fabricated approval.
    let mut duplicate_tool = tool;
    duplicate_tool.validation_status = "validated".into();
    let mut duplicate_binding = binding;
    duplicate_binding.validation_status = "validated".into();
    let mut duplicate_skill = skill;
    duplicate_skill.validation_status = "validated".into();
    assert!(tools.insert(duplicate_tool).await.unwrap().is_none());
    assert!(bindings.insert(duplicate_binding).await.unwrap().is_none());
    assert!(skills.insert(duplicate_skill).await.unwrap().is_none());
    crate::system_seed::repair_packaged_legacy_prompts(
        &rig.pool,
        "metadata-test",
        "owner",
        "agent",
        "data",
    )
    .await
    .expect("unselected pending metadata is not a required prompt");
    let client = rig.pool.get().await.unwrap();
    for (table, id) in [
        ("reborn_tools", tool_id),
        ("reborn_tool_skills", binding_id),
        ("reborn_skills", skill_id),
    ] {
        let query = format!("SELECT validation_status,source FROM {table} WHERE id=$1");
        let row = client.query_one(&query, &[&id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), "pending", "{table}");
        assert_eq!(row.get::<_, String>(1), "system");
    }
    let routes: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_intent_inputs WHERE component_id=$1",
            &[&skill_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(routes, 0, "pending metadata must not seed a legacy intent");
    let ids = vec![tool_id, binding_id, skill_id];
    let revisions: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_component_revision_heads WHERE component_id=ANY($1)",
            &[&ids],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        revisions, 0,
        "raw labels do not manufacture v3 revisions or approvals"
    );
    // A required packaged prompt with altered prose still blocks repair; the
    // correction must not turn the old blanket status gate into an integrity
    // bypass or promote an operator's changed prompt by its system label.
    let protected = NewPgSkill {
        tenant_id: "metadata-test".into(),
        user_id: "owner".into(),
        agent_id: "agent".into(),
        project_id: "data".into(),
        name: "failure_explanation".into(),
        description: "Deliberately altered protected prompt".into(),
        body: "This is not the packaged failure explanation.".into(),
        class_code: 10,
        consumer_tags: vec![],
        intent_examples: json!([]),
        source: "system".into(),
        validation_status: "pending".into(),
        content_checksum: None,
    };
    let protected_id = skills.insert(protected.clone()).await.unwrap().unwrap();
    let error = crate::system_seed::repair_packaged_legacy_prompts(
        &rig.pool,
        "metadata-test",
        "owner",
        "agent",
        "data",
    )
    .await
    .unwrap_err();
    assert!(error.contains("failure_explanation (checksum mismatch)"));
    let unchanged = client
        .query_one(
            "SELECT body,validation_status FROM reborn_skills WHERE id=$1",
            &[&protected_id],
        )
        .await
        .unwrap();
    assert_eq!(unchanged.get::<_, String>(0), protected.body);
    assert_eq!(unchanged.get::<_, String>(1), "pending");
}
