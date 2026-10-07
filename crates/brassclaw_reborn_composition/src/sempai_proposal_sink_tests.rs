//! Real proposal sink and PostgreSQL: these candidates stay unreviewed. No
//! fixture pretends their declared contracts are approved or runnable in Monty.
use super::PgSempaiProposalSink;
use brassclaw_interceptor::{ComponentProposal, SempaiProposalSink};
use serde_json::{Value, json};
use uuid::Uuid;

const TENANT: &str = "sempai-draft-instance";
const AUTHOR: &str = "author";
const AGENT: &str = "default";
const PROJECT: &str = "project";

fn recipe(name: &str, code: Uuid) -> Value {
    json!({
        "name": name, "description": "A pending pure-logic workflow",
        "trigger": {"type":"exact", "payload":"inspect 猫"},
        "steps": [], "prior_knowledge_content":"Complete source \"{{vars.name}}\"",
        "override_prompt_creation":true, "consumer_tags":["02:orchestrator"],
        "intent_examples":[{"input":"inspect 猫", "class":1}],
        "step_descriptions":[{
            "desc_idx":0, "label":"Inspect", "yaml_source":"preserved\n  indentation",
            "steps":[{"stepnumber":1,"knowledge":"orchestrator", "type":"component",
                "goal":"Inspect data", "content":"Run reusable logic", "include":[code],
                "tool_bindings":[], "dependencies":null}]
        }],
        "variants":[{"variant_key":"inspect", "description":"Inspect a typed input",
            "step_link":"0:1-0:E", "intent_examples":["inspect 猫"],
            "variable_patterns":[]}],
        "dependency_registry":{"code":code}, "validates_class_code":21
    })
}

fn python_code(name: &str, nested: Uuid) -> Value {
    json!({
        "name":name, "description":"Pending reusable logic",
        "content":"# 猫 and {{vars.name}} remain source\nresult = inputs[\"value\"]\n",
        "prior_knowledge_content":"Complete usage and prerequisites",
        "override_prompt_creation":true, "consumer_tags":["02:orchestrator"],
        "intent_examples":null, "dependency_registry":{"nested":nested},
        "includes":[nested]
    })
}

#[tokio::test]
async fn native_sempai_preserves_constructor_fields_and_rejects_lossy_proposals() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sink = PgSempaiProposalSink::new(rig.pool.clone(), TENANT, AGENT);
    let code = Uuid::new_v4();
    let current = recipe("sempai-ibs-draft", code);
    let legacy = recipe("sempai-legacy-ibs-draft", code);
    let python = python_code("sempai-code-draft", Uuid::new_v4());
    let result = sink
        .submit_proposals(
            AUTHOR,
            PROJECT,
            std::slice::from_ref(&legacy),
            &[],
            &[
                ComponentProposal {
                    class_code: 21,
                    payload: current.clone(),
                },
                ComponentProposal {
                    class_code: 22,
                    payload: python.clone(),
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(result.recipe_updates_queued, 1);
    assert_eq!(result.components_queued, 2);
    // Exercise the real delivery readers: routing metadata and the newly
    // supported provenance label must not make these candidates deliverable.
    assert!(
        crate::pg_recipe_store::PgRecipeStore::new(rig.pool.clone())
            .fetch_validated(TENANT, AUTHOR, AGENT, PROJECT)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::pg_python_code_store::PgPythonCodeStore::new(rig.pool.clone())
            .fetch_validated(TENANT, AUTHOR, AGENT, PROJECT)
            .await
            .unwrap()
            .is_empty()
    );
    let client = rig.pool.get().await.unwrap();
    let rows = client
        .query(
            "SELECT id, name, description, trigger, steps, prior_knowledge_content,
                override_prompt_creation, intent_examples, step_descriptions,
                variants, dependency_registry, validates_class_code, consumer_tags,
                source, validation_status, tier, wilson_lower
         FROM reborn_recipes WHERE tenant_id=$1 ORDER BY name",
            &[&TENANT],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let expected = if row.get::<_, String>(1) == current["name"] {
            &current
        } else {
            &legacy
        };
        for (column, field) in [
            (3, "trigger"),
            (4, "steps"),
            (7, "intent_examples"),
            (8, "step_descriptions"),
            (9, "variants"),
            (10, "dependency_registry"),
        ] {
            assert_eq!(row.get::<_, Value>(column), expected[field]);
        }
        assert_eq!(row.get::<_, String>(2), expected["description"]);
        assert_eq!(row.get::<_, String>(5), expected["prior_knowledge_content"]);
        assert!(row.get::<_, bool>(6));
        assert_eq!(row.get::<_, i16>(11), 21);
        assert_eq!(
            row.get::<_, Vec<String>>(12),
            ["02:orchestrator", "05:validator"]
        );
        assert_eq!(row.get::<_, String>(13), "sempai_proposal");
        assert_eq!(row.get::<_, String>(14), "pending");
        assert_eq!(row.get::<_, String>(15), "seedling");
        assert_eq!(row.get::<_, f64>(16), 0.0);
        assert_eq!(
            client
                .query_one(
                    "SELECT state FROM reborn_validation_queue WHERE component_id=$1",
                    &[&row.get::<_, Uuid>(0)]
                )
                .await
                .unwrap()
                .get::<_, i16>(0),
            1
        );
    }
    let row = client
        .query_one(
            "SELECT id,content,prior_knowledge_content,override_prompt_creation,
                intent_examples,dependency_registry,includes,consumer_tags,source,
                validation_status,content_checksum
         FROM reborn_python_code WHERE tenant_id=$1",
            &[&TENANT],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(1), python["content"]);
    assert_eq!(row.get::<_, String>(2), python["prior_knowledge_content"]);
    assert!(row.get::<_, bool>(3));
    assert_eq!(row.get::<_, Option<Value>>(4), Some(Value::Null));
    assert_eq!(row.get::<_, Value>(5), python["dependency_registry"]);
    assert_eq!(row.get::<_, Value>(6), python["includes"]);
    assert_eq!(
        row.get::<_, Vec<String>>(7),
        ["02:orchestrator", "05:validator"]
    );
    assert_eq!(row.get::<_, String>(8), "sempai_proposal");
    assert_eq!(row.get::<_, String>(9), "pending");
    assert!(row.get::<_, Option<String>>(10).is_none());
    assert_eq!(
        client
            .query_one(
                "SELECT state FROM reborn_validation_queue WHERE component_id=$1",
                &[&row.get::<_, Uuid>(0)]
            )
            .await
            .unwrap()
            .get::<_, i16>(0),
        1
    );

    // The migration must be repeatable without changing pending rows or opening
    // the provenance set to arbitrary strings.
    for _ in 0..2 {
        client
            .batch_execute(include_str!(
                "../../brassclaw_pg/migrations/V105__python_code_sempai_proposal_source.sql"
            ))
            .await
            .unwrap();
    }
    assert!(
        client
            .execute(
                "UPDATE reborn_python_code SET source='self-approved' WHERE tenant_id=$1",
                &[&TENANT]
            )
            .await
            .is_err()
    );

    let mut rejected = Vec::new();
    for class in [21, 22] {
        for field in [
            "source",
            "validation_status",
            "id",
            "class_code",
            "input_schema",
            "tier",
        ] {
            let mut payload = if class == 21 {
                current.clone()
            } else {
                python.clone()
            };
            payload["name"] = json!(format!("rejected-{class}-{field}"));
            payload[field] = json!("unsupported");
            rejected.push(ComponentProposal {
                class_code: class,
                payload,
            });
        }
        let mut payload = if class == 21 {
            current.clone()
        } else {
            python.clone()
        };
        payload["name"] = json!(format!("rejected-{class}-bool"));
        payload["override_prompt_creation"] = json!(1);
        rejected.push(ComponentProposal {
            class_code: class,
            payload,
        });
    }
    let mut payload = python.clone();
    payload["name"] = json!("rejected-null-includes");
    payload["includes"] = Value::Null;
    rejected.push(ComponentProposal {
        class_code: 22,
        payload,
    });
    let result = sink
        .submit_proposals(AUTHOR, PROJECT, &[], &[], &rejected)
        .await
        .unwrap();
    assert_eq!(result.components_queued, 0);
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_validation_queue WHERE tenant_id=$1",
                &[&TENANT]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        3
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_recipes WHERE tenant_id=$1 AND name LIKE 'rejected-%'",
                &[&TENANT]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(client.query_one("SELECT count(*) FROM reborn_python_code WHERE tenant_id=$1 AND name LIKE 'rejected-%'", &[&TENANT]).await.unwrap().get::<_, i64>(0), 0);
}

#[tokio::test]
async fn native_sempai_queue_and_commit_failures_leave_no_orphan_drafts() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sink = PgSempaiProposalSink::new(rig.pool.clone(), TENANT, AGENT);
    let client = rig.pool.get().await.unwrap();
    // Immediate queue-write failure and deferred commit failure exercise both
    // transaction exits through the real sink, separately for both classes.
    client
        .batch_execute(
            "CREATE FUNCTION fail_sempai_enqueue() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'injected queue failure'; END $$;",
        )
        .await
        .unwrap();
    for deferred in [false, true] {
        let trigger = if deferred {
            "CREATE CONSTRAINT TRIGGER fail_enqueue AFTER INSERT ON reborn_validation_queue
             DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_sempai_enqueue()"
        } else {
            "CREATE TRIGGER fail_enqueue BEFORE INSERT ON reborn_validation_queue
             FOR EACH ROW EXECUTE FUNCTION fail_sempai_enqueue()"
        };
        client.batch_execute(trigger).await.unwrap();
        let proposals = [
            ComponentProposal {
                class_code: 21,
                payload: recipe("retry-recipe", Uuid::new_v4()),
            },
            ComponentProposal {
                class_code: 22,
                payload: python_code("retry-code", Uuid::new_v4()),
            },
        ];
        let failed = sink
            .submit_proposals(AUTHOR, PROJECT, &[], &[], &proposals)
            .await
            .unwrap();
        assert_eq!(failed.components_queued, 0);
        for table in [
            "reborn_recipes",
            "reborn_python_code",
            "reborn_validation_queue",
        ] {
            let query = format!("SELECT count(*) FROM {table} WHERE tenant_id=$1");
            assert_eq!(
                client
                    .query_one(&query, &[&TENANT])
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                0
            );
        }
        client
            .batch_execute("DROP TRIGGER fail_enqueue ON reborn_validation_queue")
            .await
            .unwrap();
        let submitted = sink
            .submit_proposals(AUTHOR, PROJECT, &[], &[], &proposals)
            .await
            .unwrap();
        assert_eq!(submitted.components_queued, 2);
        client.batch_execute("DELETE FROM reborn_validation_queue; DELETE FROM reborn_recipes; DELETE FROM reborn_python_code").await.unwrap();
    }
}
