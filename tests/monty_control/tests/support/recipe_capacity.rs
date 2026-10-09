//! Actual worker/IBS/transcript/PostgreSQL settlement beyond the former eight
//! Recipe ceiling. The explicit validation root and draft catalogue are never
//! activation evidence or an alternative production orchestration path.
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef, component_revision::ComponentRevisionDraft,
    revision_store::PgComponentRevisionStore,
};
use brassclaw_threads::SessionThreadService;
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{
    ActorLimits, AdmissionInput, Duration, MontyTaskHost, MontyTurnDriverPort,
    NativeTaskPortsFactory, PgPool, PgSessionThreadService, ProviderHold, RecordingProvider,
    SOURCE, SelectedPrefix, TaskOutcome, admitted_with_text, assert_settlement_outcome,
    global_monty_driver, global_monty_owner,
    global_recipe_ports::{
        MontyIntentSelection, MontyTaskCatalogue, RecipeCapacity, RecipeCapacitySource,
        SelectedMontyRecipe,
    },
    global_task_factory, native_pg,
    pg_monty_admission::PgMontyAdmission,
    reply_workflows, support,
};
use brassclaw_monty_host::service::{PortFailure, TaskInput};

struct Capacity(tokio::sync::RwLock<RecipeCapacity>);
#[async_trait]
impl RecipeCapacitySource for Capacity {
    async fn current(&self) -> Result<RecipeCapacity, PortFailure> {
        Ok(*self.0.read().await)
    }
}
struct Provider {
    pool: Arc<PgPool>,
    roots: Vec<Uuid>,
    refs: Vec<ComponentRevisionRef>,
    reply: Arc<RetainedToolProgram>,
}
struct Catalogue(BTreeMap<String, SelectedMontyRecipe>);
#[async_trait]
impl MontyTaskCatalogue for Catalogue {
    // This explicit draft validator never advertises an activated MCP command.
    async fn refresh_command_qualification(&self) {}

    async fn resolve_intent(&self, _query: &str) -> Result<MontyIntentSelection, PortFailure> {
        // This regression exercises retained named selections. It does not
        // manufacture a matcher result or fall back through a model.
        Err(PortFailure::new("intent_resolution_failed").unwrap())
    }

    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure> {
        let selected = self
            .0
            .get(name)
            .ok_or_else(|| PortFailure::new("recipe_composition_failed").unwrap())?;
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: selected.inspected.clone(),
            inputs: selected.inputs.clone(),
            tools: selected.tools.clone(),
        })
    }
}
#[async_trait]
impl global_task_factory::MontyCatalogueProvider for Provider {
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, brassclaw_turns::run_profile::AgentLoopDriverError>
    {
        let mut client = self.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let snapshot = Arc::new(
            PgComponentRevisionStore::read_exact_in_transaction(&tx, &self.roots, &self.refs)
                .await
                .unwrap(),
        );
        tx.commit().await.unwrap();
        drop(client);
        let mut selections = BTreeMap::new();
        for (index, root) in self.roots[..8].iter().enumerate() {
            let instruction = compile_retained_recipe(
                snapshot.clone(),
                *root,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap();
            let program = Arc::new(prepare_retained_unbound_program(instruction).unwrap());
            selections.insert(
                format!("part-{index}"),
                SelectedMontyRecipe {
                    normal_match: None,
                    inspected: Arc::new(
                        InspectedRetainedProgram::inspect(
                            RetainedProgram::Unbound(program),
                            support::worker(),
                        )
                        .await
                        .unwrap(),
                    ),
                    inputs: None,
                    tools: None,
                },
            );
        }
        let instruction = compile_retained_recipe(
            snapshot,
            self.reply.inputs().instruction().recipe().uuid,
            "selected",
            WorkflowClass::Deterministic,
        )
        .unwrap();
        let reply = Arc::new(prepare_retained_tool_program(instruction).unwrap());
        let reply =
            reply_workflows::prepare_named_reply(host, admission, reply, self.pool.clone(), false)
                .await;
        selections.insert("host-post-reply".into(), reply.select());
        Ok(Arc::new(Catalogue(selections)))
    }
}

async fn catalogue(pool: Arc<PgPool>) -> Provider {
    let store = PgComponentRevisionStore::new(pool.clone());
    let reply = reply_workflows::reply_program(pool.clone()).await;
    let mut refs: Vec<_> = reply
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect();
    let code = Uuid::new_v4();
    let inputs = json!({"value":{"type":"string","required":true,"checks":[]}});
    let draft = ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1","uuid":code,"class_code":22,
            "document":{"content":"result = inputs['value'] + '|next'",
                "input_contract":inputs,"result_contract":{"type":"string","checks":[]}},
            "dependencies":[],"association":null,
        })
        .to_string(),
    )
    .unwrap();
    refs.push(store.stage(&draft, 0).await.unwrap());
    let mut roots = Vec::new();
    for index in 0..8 {
        let root = Uuid::new_v4();
        let draft = ComponentRevisionDraft::from_json(&json!({
            "format":"component-revision/1","uuid":root,"class_code":21,
            "document":{
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E",
                    "intent_examples":[format!("part-{index} %")],
                    "variable_patterns":[{"name":"value","pattern":null,"description":null}]}],
                "step_descriptions":[{"desc_idx":0,"label":"pure typed logic","yaml_source":"",
                    "steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"Append a literal suffix",
                        "content":"","type":"component","include":[code]}]}],
                "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,
                    "steps":{"0:1":{"value":{"kind":"task_input","reference":"{{vars.value}}"}}}}}
            },"dependencies":[code],"association":null,
        }).to_string()).unwrap();
        refs.push(store.stage(&draft, 0).await.unwrap());
        roots.push(root);
    }
    roots.push(reply.inputs().instruction().recipe().uuid);
    Provider {
        pool,
        roots,
        refs,
        reply,
    }
}

#[tokio::test]
async fn nine_executed_recipes_settle_after_live_selection_capacity_is_reduced() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let catalogue = catalogue(database.pool.clone()).await;
    let recipe_ids = catalogue.roots.clone();
    // Reuse the actual root's dispatch/worker/accounting code. This explicit
    // validation task calls eight pure-logic Recipes and one real reply Recipe.
    // No fabricated report, completion or effect result enters settlement.
    let start = SOURCE.find("async def _execute_task(task):\n").unwrap();
    let end = SOURCE.find("async def _worker(worker_id):\n").unwrap();
    assert!(start < end);
    let task = r#"async def _execute_task(task):
    token = task["task_token"]
    value = task["user_input"]
    for index in range(8):
        selected = await host.resolve_component_by_name(token, "part-" + str(index), 21)
        value = await _execute_recipe(token, selected["id"], selected["step_link"], {"value": value})
    selected = await host.resolve_component_by_name(token, "host-post-reply", 21)
    reply_ref = await _execute_recipe(token, selected["id"], selected["step_link"], {"answer": value})
    answer = await host.resolve_reply(token, reply_ref)
    if answer != value:
        raise RuntimeError("recipe_reply_invalid")
    return reply_ref


"#;
    let source = format!("{}{task}{}", &SOURCE[..start], &SOURCE[end..]);
    let mut boot = support::boot(&source);
    boot.max_recipe_contexts = 16;
    let live = crate::LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            ownership: global_monty_owner::OwnershipLimits::from_execution(Default::default()),
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
            queue_bytes: 64 * 1024 * 1024,
            max_pending_settings: 8,
            max_retained_attempts: 1,
        },
    )
    .await
    .unwrap();
    let identity = owner.client().root_identity();
    let capacity = Arc::new(Capacity(tokio::sync::RwLock::new(RecipeCapacity {
        revision: 1,
        max_recipes: 9,
    })));
    let factory = Arc::new(NativeTaskPortsFactory::with_catalogue_capacity(
        database.pool.clone(),
        owner.ownership_check(),
        owner.client(),
        Arc::new(catalogue),
        capacity.clone(),
    ));
    let hold = Arc::new(ProviderHold::default());
    *factory.settlement_hold.lock().unwrap() = Some(hold.clone());
    let driver = Arc::new(
        global_monty_driver::GlobalMontyDriver::new(
            owner.client(),
            Arc::new(PgSessionThreadService::new(
                database.pool.clone(),
                "native-global-host",
            )),
            factory.clone(),
        )
        .unwrap(),
    );
    let provider = Arc::new(RecordingProvider::default());
    let input = "quotes ' Ü {{vars.value}} host.forbidden()";
    let expected = format!("{input}{}", "|next".repeat(8));
    let (_, handoff, threads, scope, _) = admitted_with_text(
        database.pool.clone(),
        provider.clone(),
        AdmissionInput {
            name: "nine-recipes",
            text: input,
        },
        Arc::new(SelectedPrefix("unused model prefix".into())),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, host) = handoff.into_parts();
    let handoff = crate::MontyTaskHandoff::new(request, attempt, host).unwrap();
    let running = tokio::spawn({
        let driver = driver.clone();
        async move { driver.drive_turn(handoff).await }
    });
    tokio::time::timeout(Duration::from_secs(30), hold.entered.notified())
        .await
        .unwrap();
    // The owned settlement has the actual completed receipt. Lowering the
    // policy must not discard selected work, repeat the reply or truncate it.
    *capacity.0.write().await = RecipeCapacity {
        revision: 2,
        max_recipes: 1,
    };
    hold.released.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(10), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(result, brassclaw_turns::LoopExit::Completed(_)));
    assert!(driver.take_settlement(attempt).unwrap().is_none());
    let host = factory.last_host.lock().unwrap().clone().unwrap();
    let reference = host.finalized_reply_ref().unwrap();
    let receipt = factory.last_receipt.lock().unwrap().clone().unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    let client = database.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "settled");
    let actual: Value = row.get(1);
    let report = assert_settlement_outcome(
        actual.clone(),
        json!({"status":"completed","reply_ref":reference.as_str()}),
    );
    let recipes = report["recipes"].as_array().unwrap();
    assert_eq!(recipes.len(), 9);
    for (index, (recipe, id)) in recipes.iter().zip(&recipe_ids).enumerate() {
        assert_eq!(recipe["recipe_id"], id.to_string());
        assert_eq!(recipe["complete"], true);
        assert_eq!(recipe["composed"], true);
        assert!(recipe["failed_step"].is_null());
        assert!(recipe["pending_step"].is_null());
        assert_eq!(
            recipe["completed_steps"],
            if index < 8 {
                json!(["0:1"])
            } else {
                json!(["0:2"])
            }
        );
    }
    assert_eq!(
        report["checked_recipe_capacity"],
        json!({"revision":1,"max_recipes":9})
    );
    assert_eq!(client.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND phase='answered' AND attempt_count=1",
        &[&attempt.run_id.as_uuid()]).await.unwrap().get::<_, i64>(0), 1);
    // This unpersisted diagnostic address cannot replace the settled admission.
    // Validate corrupted copies of the actual outcome before address lookup;
    // rejection must be a shape/completion error, never a successful rewrite.
    let probe =
        PgMontyAdmission::prepare(database.pool.clone(), host.run_context(), attempt).unwrap();
    for kind in [
        "null",
        "object",
        "missing",
        "root_incomplete",
        "recipe_incomplete",
    ] {
        let mut malformed = actual.clone();
        match kind {
            "null" => malformed["execution"]["recipes"] = Value::Null,
            "object" => malformed["execution"]["recipes"] = json!({}),
            "missing" => {
                malformed["execution"]
                    .as_object_mut()
                    .unwrap()
                    .remove("recipes");
            }
            "root_incomplete" => malformed["execution"]["root_completed"] = json!(false),
            "recipe_incomplete" => {
                malformed["execution"]["all_selected_recipes_complete"] = json!(false)
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(probe.settle(malformed).await,
            Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind })
                if reason_kind == "monty_admission_outcome_invalid"),
            "accepted corrupted {kind} outcome"
        );
    }
    assert_eq!(
        client
            .query_one(
                "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&attempt.run_id.as_uuid()]
            )
            .await
            .unwrap()
            .get::<_, Value>(0),
        actual
    );
    drop(client);
    let transcript = threads
        .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
            scope,
            thread_id: host.run_context().thread_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(transcript.messages.len(), 2);
    assert_eq!(
        transcript.messages[1].content.as_deref(),
        Some(expected.as_str())
    );
    assert!(provider.requests.lock().unwrap().is_empty());
    assert_eq!(owner.client().root_identity(), identity);
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    let service = exit.service.unwrap();
    assert_eq!(service.failure, None);
    assert!(service.tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(result) => result.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => panic!("worker did not settle"),
    }
}
