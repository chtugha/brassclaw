//! Explicit child entry transport through actual Monty, IBS, kernel and PG.
//! This closed draft observation supplies no child admission or approval proof.
use super::*;
use crate::global_recipe_ports::{GlobalRecipePorts, MontyTaskCatalogue};
use brassclaw_monty_host::service::TaskEntry;
use brassclaw_threads::{MessageKind, MessageStatus, ThreadHistoryRequest};

#[tokio::test]
async fn explicit_child_entry_preserves_typed_inputs_and_cannot_rematch_or_fall_back() {
    let database = native_pg::NativePostgres::start().await;
    let provider = Arc::new(RecordingProvider::default());
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
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
            max_retained_attempts: 2,
        },
    )
    .await
    .unwrap();
    let service = owner.client();
    let root = service.root_identity();
    let reply = reply_workflows::reply_program(database.pool.clone()).await;
    let literal =
        "quoted ' ü🙂 {{vars.goal}}\r\n```python\nraise RuntimeError('data')\n```\nnull\n";
    for with_selection in [true, false] {
        let (mut input, handoff, threads, scope, _) = admitted_with_text(
            database.pool.clone(),
            provider.clone(),
            AdmissionInput {
                name: if with_selection {
                    "explicit-child-entry"
                } else {
                    "missing-child-entry"
                },
                text: "A host-owned child task reference; never a fresh user command",
            },
            Arc::new(SelectedPrefix(
                "unused for this deterministic draft observation".into(),
            )),
            0,
            service.live_task_settings(),
            None,
        )
        .await;
        let host = Arc::new(MontyTaskHost::new(handoff));
        let admission = Arc::new(
            pg_monty_admission::PgMontyAdmission::reserve(
                database.pool.clone(),
                host.run_context(),
                host.attempt(),
            )
            .await
            .unwrap(),
        );
        let catalogue = Arc::new(
            task_catalogue::ValidationCatalogue::new(
                database.pool.clone(),
                host.clone(),
                None,
                admission.clone(),
                reply.clone(),
                false,
            )
            .await,
        );
        let mut selected = catalogue
            .resolve_named_recipe("host-post-reply")
            .await
            .unwrap();
        selected.inputs = Some(json!({"answer":literal}));
        let ports = GlobalRecipePorts::new(
            host.clone(),
            admission,
            catalogue,
            input.user_input.clone(),
            owner.ownership_check(),
            Arc::new(FixtureRecipeCapacity),
        );
        let ports = Arc::new(if with_selection {
            ports.with_child_recipe(selected).unwrap()
        } else {
            ports
        });
        input.entry = TaskEntry::ChildRecipe;
        // Any accidental resolver query now fails in the actual database. The
        // explicit entry must execute its captured selection without matching.
        let client = database.pool.get().await.unwrap();
        client
            .batch_execute(
                "ALTER TABLE reborn_intent_inputs RENAME TO child_entry_intents_unavailable",
            )
            .await
            .unwrap();
        let ticket = service
            .submit_when_available(input, ports.clone())
            .await
            .unwrap();
        let receipt = tokio::time::timeout(Duration::from_secs(20), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        client
            .batch_execute(
                "ALTER TABLE child_entry_intents_unavailable RENAME TO reborn_intent_inputs",
            )
            .await
            .unwrap();
        assert_eq!(service.root_identity(), root);
        assert!(receipt.withheld.is_empty());
        assert!(
            provider.requests.lock().unwrap().is_empty(),
            "explicit selection must never ask the model as a routing fallback"
        );
        let history = threads
            .list_thread_history(ThreadHistoryRequest {
                scope,
                thread_id: host.run_context().thread_id.clone(),
            })
            .await
            .unwrap();
        let replies = history
            .messages
            .iter()
            .filter(|message| {
                message.kind == MessageKind::Assistant && message.status == MessageStatus::Finalized
            })
            .collect::<Vec<_>>();
        let report = ports.settlement_report(&receipt).await.unwrap();
        if with_selection {
            let TaskOutcome::Completed { reply_ref } = &receipt.outcome else {
                panic!(
                    "actual retained reply/history execution must complete: {:?}",
                    receipt.outcome
                );
            };
            assert_eq!(replies.len(), 1);
            assert_eq!(replies[0].content.as_deref(), Some(literal));
            assert_eq!(reply_ref, &format!("msg:{}", replies[0].message_id));
            assert_eq!(report["intent_outcome"], "child_recipe");
            assert_eq!(report["all_selected_recipes_complete"], true);
            let recipes = report["recipes"].as_array().unwrap();
            assert_eq!(
                recipes.len(),
                2,
                "actual explicit reply and named history Recipe"
            );
            assert!(
                recipes
                    .iter()
                    .all(|recipe| recipe["normal_match"].is_null())
            );
            let rows=client.query("SELECT selection_bytes,arguments_bytes,answer_bytes,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id", &[&host.attempt().run_id.as_uuid()]).await.unwrap();
            assert_eq!(rows.len(), 2);
            for row in &rows {
                assert_eq!(row.get::<_, &str>(3), "answered");
            }
            let reply_row = rows
                .iter()
                .find(|row| {
                    serde_json::from_str::<Value>(row.get(0)).unwrap()["capability_id"]
                        == "host.post_reply"
                })
                .unwrap();
            let arguments: Value = serde_json::from_str(reply_row.get(1)).unwrap();
            assert_eq!(arguments["answer"], literal);
            assert!(report["accounting"]["compute_time"].is_object());
        } else {
            assert!(matches!(receipt.outcome, TaskOutcome::Failed { .. }));
            assert!(
                replies.is_empty(),
                "missing child selection is never an empty successful result"
            );
            assert_eq!(client.query_one("SELECT count(*)::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&host.attempt().run_id.as_uuid()]).await.unwrap().get::<_,i64>(0), 0);
        }
    }
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.service.unwrap().tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual quiescent settlement required")
        }
    }
}
