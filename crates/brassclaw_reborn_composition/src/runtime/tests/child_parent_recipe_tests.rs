//! A complete closed parent Recipe on the instance VM. Draft observation does
//! not approve/activate a component or enter ordinary intent matching.
use super::*;
use crate::global_recipe_ports::{
    GlobalRecipePorts, InternalRecipeSource, MontyTaskCatalogue, SelectedMontyRecipe,
};
use crate::global_task_factory::MontyCatalogueProvider;
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{RetainedProgram, RetainedRecipeExecution, RetainedToolPort},
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::service::{PortFailure, TaskEntry, TaskInput, TaskOutcome, TaskPorts};
use brassclaw_turns::runner::{CompleteRunRequest, HeartbeatRequest, TurnRunTransitionPort};

struct ParentReview {
    reference: String,
    // Keep the actual parent's captured generation alive through child/wait.
    catalogue: Arc<dyn MontyTaskCatalogue>,
    inspected: Arc<InspectedRetainedProgram>,
    inputs: Value,
    tools: Arc<dyn RetainedToolPort>,
    pool: Arc<brassclaw_pg::PgPool>,
    vm_id: String,
}
fn rejected() -> PortFailure {
    PortFailure::new("child_parent_review_invalid").unwrap()
}
#[async_trait]
impl InternalRecipeSource for ParentReview {
    fn name(&self) -> &str {
        "development-child-parent"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn catalogue_reference(&self) -> Option<crate::pg_monty_admission::CatalogueReference> {
        self.catalogue.catalogue_reference()
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.transport_failure().is_some()
            || execution.host_answers().len() != 1
            || execution.observation_order() != Some(&["0:2".to_owned(), "0:3".to_owned()][..])
        {
            return Err(rejected());
        }
        let (step, PortAnswer::Return { value }) = &execution.host_answers()[0] else {
            return Err(rejected());
        };
        if step != "0:2"
            || value["status"] != "completed"
            || value["content"] != self.inputs["inputs"]["answer"]
        {
            return Err(rejected());
        }
        let id = value["child_run_id"].as_str().ok_or_else(rejected)?;
        if receipt != format!("child-review:{id}") {
            return Err(rejected());
        }
        let id = Uuid::parse_str(id).map_err(|_| rejected())?;
        let row = self.pool.get().await.map_err(|_| rejected())?.query_one(
            "SELECT outcome,catalogue_id FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
            &[&id],
        ).await.map_err(|_| rejected())?;
        let actual: Value = row.get(0);
        if actual["status"] != "completed"
            || actual["execution"]["intent_outcome"] != "child_recipe"
            || actual["execution"]["all_selected_recipes_complete"] != true
            || actual["execution"]["root"]["vm_id"] != self.vm_id
            || actual["execution"]["reply"]["reference"] != value["reply_ref"]
            || row.get::<_, Option<Uuid>>(1)
                != self
                    .catalogue
                    .catalogue_reference()
                    .map(|reference| reference.id)
        {
            return Err(rejected());
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_complete_parent_recipe_delegates_on_the_instance_global_vm() {
    let rig = super::super::super::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let directory = tempfile::tempdir().unwrap();
    let reference = format!("candidate:child-parent:{}", Uuid::new_v4());
    // Claim before the ordinary runtime worker starts, so it cannot race this
    // private review's claim. No VM or model operation happens here.
    let admitted = Arc::new(
        crate::bootstrap_validation::reserve_child_review_parent(
            rig.pool.clone(),
            "native-child-parent",
            &reference,
        )
        .await
        .unwrap(),
    );
    let attempt = admitted.attempt;
    let (stop, mut stopping) = tokio::sync::watch::channel(false);
    let lease = admitted.clone();
    let heartbeat = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = stopping.changed() => {
                    if changed.is_err() || *stopping.borrow() { return; }
                },
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    lease.state.heartbeat(HeartbeatRequest {
                        run_id: attempt.run_id,
                        runner_id: attempt.runner_id,
                        lease_token: attempt.lease_token,
                    }).await.unwrap();
                }
            }
        }
    }));
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("installation-owner", directory.path())
                    .with_tenant_id(crate::bootstrap_validation::NAMESPACE)
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_identity(RebornRuntimeIdentity {
                tenant_id: crate::bootstrap_validation::NAMESPACE.into(),
                agent_id: "native-child-parent".into(),
                ..RebornRuntimeIdentity::default()
            })
            .with_poll_settings(PollSettings {
                interval: Duration::from_millis(10),
                max_total: Duration::from_secs(60),
            }),
        ))
        .await
        .unwrap(),
    );
    let owner = runtime.installed_catalogue.clone();
    let service = runtime.global_monty_owner.client();
    let identity = service.root_identity();
    let host = admitted.host.clone();
    assert!(host.claimed_run().is_some());
    let input = TaskInput {
        entry: TaskEntry::InternalRecipe {
            name: "development-child-parent".into(),
        },
        conversation_id: admitted.input["conversation_id"].as_str().unwrap().into(),
        message_id: admitted.input["message_id"].as_str().unwrap().into(),
        turn_id: admitted.input["turn_id"].as_str().unwrap().into(),
        run_id: admitted.input["run_id"].as_str().unwrap().into(),
        user_input: reference.clone(),
        history: vec![],
    };
    let pinned = owner
        .capture(host.clone(), admitted.admission.clone(), &input)
        .await
        .unwrap();
    let captured = pinned.catalogue_reference().unwrap();
    let child = owner.reply.program().inputs().instruction();
    let literal = "parent hands over literal ' ü🙂 {{vars.goal}}\r\n```python\nraise RuntimeError('data')\n```";
    let capability = CapabilityId::new("builtin.spawn_subagent").unwrap();
    let native = owner.kernel.builtin(&capability, None).unwrap();
    let program = child_usage_candidate(
        rig.pool.clone(),
        native.native_identity().unwrap(),
        ChildCandidateShape::ParentRecipe,
    )
    .await;
    let settings = runtime
        .monty_settings_owner
        .store()
        .get("default", "default")
        .await
        .unwrap();
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(program.clone()),
            &brassclaw_monty_host::process::installed_worker().unwrap(),
            brassclaw_monty_host::utility::UtilityLimits::from_execution(settings.execution_limits)
                .unwrap(),
        )
        .await
        .unwrap(),
    );
    let tools = owner
        .child_usage_tools_for_test(host.clone(), admitted.admission.clone(), program)
        .unwrap();
    let source = Arc::new(ParentReview {
        reference,
        catalogue: pinned,
        inspected,
        inputs: json!({"workflow":{"recipe_id":child.recipe().uuid,"variant":child.variant().variant_key},"inputs":{"answer":literal}}),
        tools,
        pool: rig.pool.clone(),
        vm_id: identity.vm_id().to_string(),
    });
    let ports = Arc::new(GlobalRecipePorts::for_internal(
        host.clone(),
        admitted.admission.clone(),
        source,
        Some(runtime.global_monty_owner.ownership_check()),
        true,
        runtime
            .monty_settings_owner
            .store()
            .recipe_capacity_source(),
    ));
    let scheduled: Arc<dyn TaskPorts> = ports.clone();
    owner
        .observe_task_ports(&host, &admitted.admission, Arc::downgrade(&scheduled))
        .unwrap();
    // A coherent successor must not replace the already captured child view.
    owner
        .activate_authored(
            "installation-owner",
            serde_json::from_value(json!({
                "activation_id":Uuid::new_v4(),"expected_generation":captured.id,"recipes":[],
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(30), async {
        service
            .submit_when_available(input, scheduled)
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
    })
    .await
    .unwrap();
    let TaskOutcome::InternalCompleted { receipt_ref } = &receipt.outcome else {
        let report = ports.settlement_report(&receipt).await;
        let children = rig.pool.get().await.unwrap().query(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE scope->>'thread_id' LIKE 'monty-child-v1-%'",
            &[],
        ).await.unwrap();
        let children: Vec<Option<Value>> = children.iter().map(|row| row.get(0)).collect();
        panic!(
            "actual parent Recipe failed: {:?}; report={report:?}; children={children:?}",
            receipt.outcome
        );
    };
    assert!(receipt.withheld.is_empty());
    assert_eq!(service.root_identity(), identity);
    let report = ports.settlement_report(&receipt).await.unwrap();
    assert_eq!(report["all_selected_recipes_complete"], true);
    assert_eq!(report["model_exchange_count"], 0);
    assert!(
        host.finalized_reply_ref().is_none(),
        "the review's child owns its own reply"
    );
    let row = rig.pool.get().await.unwrap().query_one(
        "SELECT count(*)::bigint,min(attempt_count)::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND phase='answered'",
        &[&attempt.run_id.as_uuid()],
    ).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    admitted
        .admission
        .settle(json!({"status":"completed","internal_receipt":receipt_ref,
            "result_ref":crate::monty_result_reference::result_reference(attempt.run_id,receipt_ref).unwrap(),
            "execution":report}))
        .await
        .unwrap();
    stop.send(true).unwrap();
    heartbeat.await.unwrap();
    admitted
        .state
        .complete_run(CompleteRunRequest {
            run_id: attempt.run_id,
            runner_id: attempt.runner_id,
            lease_token: attempt.lease_token,
        })
        .await
        .unwrap();
    owner.release_task(attempt).unwrap();
    assert_eq!(owner.retained_child_operations_for_test(attempt), 0);
    assert!(admitted.admission.check_live_dependency().await.is_err());
    // The same global VM remains usable for an ordinary approved chat after
    // private review. No draft Recipe was installed into its routing catalogue.
    let conversation = runtime.new_conversation().await.unwrap();
    let result = runtime
        .send_user_message(&conversation, "publish literal reply after parent review")
        .await
        .unwrap();
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("after parent review"));
    assert_eq!(service.root_identity(), identity);
    shutdown_shared_runtime(runtime).await.unwrap();
}
