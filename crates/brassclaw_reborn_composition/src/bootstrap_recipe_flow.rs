//! Bounded pre-readiness observation of the real global Python Recipe flow.
//! This reuses the existing service/step adapter. It contains no workflow order,
//! Tool selection, expected-result oracle, approval writer or production queue.
use crate::{
    RebornBuildError,
    bootstrap_reply_validation::invalid,
    bootstrap_validation::Admitted,
    global_recipe_ports::{
        GlobalRecipePorts, InternalRecipeSource, RecipeCapacity, RecipeCapacitySource,
    },
};
use async_trait::async_trait;
use brassclaw_monty_host::{
    process::{ProcessLimits, RootBoot},
    service::{ServiceExit, ServiceOwner, TaskEntry, TaskInput, TaskReceipt},
    transport_actor::ActorLimits,
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

pub(super) struct Request {
    pub(super) worker: PathBuf,
    pub(super) boot: RootBoot,
    pub(super) process: ProcessLimits,
    pub(super) actor: ActorLimits,
    pub(super) capacity: RecipeCapacity,
    pub(super) source: Arc<dyn InternalRecipeSource>,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    #[cfg(all(test, unix))]
    fail_worker_after_receipt: bool,
}
impl Request {
    /// The same checked creation policy used by the actual global startup.
    /// Ordinary private Work continues on the existing live global service.
    pub(super) fn contained(
        worker: PathBuf,
        executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
        root: &brassclaw_skills::orchestrator_contract::GlobalRootDefinition,
        source: Arc<dyn InternalRecipeSource>,
    ) -> Result<Self, RebornBuildError> {
        Ok(Self {
            worker,
            boot: executor.boot(root),
            process: executor.process(),
            actor: executor.actor(),
            capacity: executor.capacity(),
            source,
            executor,
            #[cfg(all(test, unix))]
            fail_worker_after_receipt: false,
        })
    }
    #[cfg(all(test, unix))]
    pub(super) fn with_forced_worker_exit(mut self) -> Self {
        self.fail_worker_after_receipt = true;
        self
    }
}
struct Capacity(RecipeCapacity);
#[async_trait]
impl RecipeCapacitySource for Capacity {
    async fn current(&self) -> Result<RecipeCapacity, brassclaw_monty_host::service::PortFailure> {
        Ok(self.0)
    }
}
pub(super) struct Observation {
    pub(super) ports: Arc<GlobalRecipePorts>,
    pub(super) receipt: Arc<TaskReceipt>,
    pub(super) report: Value,
    pub(super) exit: ServiceExit,
}
impl Observation {
    /// Actual model-free internal receipt/account/worker facts, before durable
    /// admission/Turn cleanup. This value alone is deliberately not settlement.
    pub(super) fn model_free_quiescence(&self) -> Result<Value, RebornBuildError> {
        use brassclaw_monty_host::{service::TaskOutcome, transport_actor::StopKind};
        if !matches!(self.receipt.outcome, TaskOutcome::InternalCompleted { .. })
            || self.exit.failure.is_some()
            || self.exit.root != self.receipt.root
            || !self.exit.tasks.is_empty()
            || self.exit.pending_admission.is_some()
            || !self.exit.queued_admissions.is_empty()
            || !self.exit.failed_exchanges.is_empty()
            || !self.exit.rejected_commands.is_empty()
            || self.report["root_completed"] != true
            || self.report["all_selected_recipes_complete"] != true
            || self.report["withheld_root_answers"] != 0
            || self.report["withheld_host_answers"] != false
            || self.report["model_exchange_count"] != 0
            || !self.report["reply"].is_null()
            || self.report["semantic_approval"] != false
            || self.report["catalogue_activation"] != false
        {
            return Err(invalid(
                "model-free contained receipt or quiescence incomplete",
            ));
        }
        let exit = self
            .exit
            .transport
            .as_ref()
            .map_err(|_| invalid("contained transport unresolved"))?;
        let status = exit
            .exit_status
            .as_ref()
            .ok_or_else(|| invalid("contained worker has not been reaped"))?;
        if !status.success()
            || exit.containment_error.is_some()
            || exit.reap_error.is_some()
            || exit.shutdown_failure.is_some()
        {
            return Err(invalid("contained worker closure unacknowledged"));
        }
        let stop_kind = match exit.kind {
            StopKind::Graceful => "graceful",
            StopKind::Requested => "requested",
            StopKind::TransportFailed | StopKind::InstanceFailed | StopKind::AccountingFailed => {
                return Err(invalid("contained native worker failed"));
            }
        };
        #[cfg(unix)]
        let signal = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };
        #[cfg(not(unix))]
        let signal: Option<i32> = None;
        Ok(
            serde_json::json!({"receipt_root":self.receipt.root,"accounting":self.receipt.accounting,"execution":self.report,
            "worker_exit":{"root":self.exit.root,"status":{"code":status.code(),"success":status.success(),"signal":signal},"stop_kind":stop_kind,
                "retained_tasks":self.exit.tasks.len(),"pending_admissions":usize::from(self.exit.pending_admission.is_some()),
                "queued_admissions":self.exit.queued_admissions.len(),"failed_exchanges":self.exit.failed_exchanges.len(),"rejected_commands":self.exit.rejected_commands.len()}}),
        )
    }
}
/// Caller owns admission, exact subject validation, semantic/effect oracles,
/// dispatch intent and final durable settlement. This observer grants none.
pub(super) async fn execute(
    admitted: &Admitted,
    request: Request,
) -> Result<Observation, RebornBuildError> {
    if admitted.input["user_input"].as_str() != Some(request.source.reference()) {
        return Err(invalid("bootstrap source reference differs from admission"));
    }
    // This executor is the existing contained pre-readiness/development
    // observer. Live product work continues through its already-running global
    // driver. Reuse that driver's capture owner, without starting admission or
    // accepting request metadata as native workflow qualification.
    if admitted.host.is_result_only_completion()
        && !admitted.host.run_context().trusted_internal_turn
    {
        admitted
            .admission
            .check_model_preparation(&admitted.host)
            .await
            .map_err(invalid)?;
        let program = request
            .source
            .captured_program()
            .ok_or_else(|| invalid("bootstrap model source has no original program"))?;
        crate::task_model_capture::capture_task_models(
            &admitted.host,
            &admitted.admission,
            Some(program.as_ref()),
            None,
        )
        .await
        .map_err(invalid)?;
        request
            .source
            .bind_native_runtimes()
            .map_err(|_| invalid("bootstrap model runtime binding failed"))?;
        // Capture/native preparation may await settings or storage. Recheck
        // original claim and unstarted identity before creating the worker.
        admitted
            .admission
            .check_model_preparation(&admitted.host)
            .await
            .map_err(invalid)?;
    }
    let ports = Arc::new(GlobalRecipePorts::for_internal(
        admitted.host.clone(),
        admitted.admission.clone(),
        request.source.clone(),
        None,
        true,
        Arc::new(Capacity(request.capacity)),
    ));
    let live = brassclaw_resources::LiveMontyTaskSettings::new(request.executor.task().into())
        .map_err(invalid)?;
    let mut owner = ServiceOwner::start_with_hosting_limits(
        &request.worker,
        request.boot,
        request.process,
        request.actor,
        request.executor.hosting(),
        live,
    )
    .await
    .map_err(invalid)?;
    let input = TaskInput {
        entry: TaskEntry::InternalRecipe {
            name: request.source.name().to_owned(),
        },
        conversation_id: admitted.input["conversation_id"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap conversation missing"))?
            .to_owned(),
        message_id: admitted.input["message_id"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap message missing"))?
            .to_owned(),
        turn_id: admitted.input["turn_id"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap turn missing"))?
            .to_owned(),
        run_id: admitted.input["run_id"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap run missing"))?
            .to_owned(),
        user_input: request.source.reference().to_owned(),
        history: vec![],
    };
    // Always join the contained worker, including rejected admission or receipt
    // validation. An error must retain evidence, not detach the process owner.
    let completed = async {
        let ticket = owner
            .client()
            .submit_when_available(input, ports.clone())
            .await
            .map_err(|error| invalid(format!("bootstrap Recipe admission: {error:?}")))?;
        let receipt = ticket
            .wait()
            .await
            .map_err(|error| invalid(format!("bootstrap Recipe completion: {error:?}")))?;
        let mut report = ports
            .settlement_report(&receipt)
            .await
            .map_err(|error| invalid(format!("bootstrap Recipe settlement: {error:?}")))?;
        report["controlled_executor_configuration"] =
            serde_json::json!(request.executor.exact_bytes());
        report["controlled_executor_configuration_checksum"] =
            serde_json::json!(request.executor.checksum());
        Ok::<_, RebornBuildError>((receipt, report))
    }
    .await;
    #[cfg(all(test, unix))]
    let forced_exit = if request.fail_worker_after_receipt && completed.is_ok() {
        // The real child remains owned/parked after receipt. Kill only that
        // retained worker, then always join its actual transport/OS settlement.
        match owner.worker_process_id() {
            Some(pid) => match tokio::time::timeout(
                request.process.response_timeout,
                tokio::process::Command::new("/bin/kill")
                    .arg("-KILL")
                    .arg(pid.to_string())
                    .status(),
            )
            .await
            {
                Ok(Ok(status)) if status.success() => Ok(()),
                _ => Err(invalid("controlled worker fault injection failed")),
            },
            None => Err(invalid("controlled worker process missing")),
        }
    } else {
        Ok(())
    };
    owner.request_shutdown();
    let exit = owner
        .join()
        .await
        .map_err(|error| invalid(format!("bootstrap Recipe shutdown: {error:?}")))?;
    #[cfg(all(test, unix))]
    forced_exit?;
    let (receipt, report) = completed?;
    Ok(Observation {
        ports,
        receipt,
        report,
        exit,
    })
}

fn observation_failure(reason: &str) -> brassclaw_monty_host::service::PortFailure {
    brassclaw_monty_host::service::PortFailure::new(reason).expect("constant classification")
}

impl GlobalRecipePorts {
    /// Read actual immutable observations after quiescence. A callback cannot
    /// alter the executor, approve code or advance/replay a workflow step.
    pub(crate) async fn observe_internal_execution<T>(
        &self,
        observe: impl FnOnce(&brassclaw_engine::executor::retained_recipe::RetainedRecipeExecution) -> T,
    ) -> Result<T, brassclaw_monty_host::service::PortFailure> {
        let selection = {
            let state = self.state.lock().await;
            if state.intent_outcome != Some("internal_recipe") || state.recipes.len() != 1 {
                return Err(observation_failure("internal_recipe_selection_invalid"));
            }
            state
                .recipes
                .values()
                .next()
                .cloned()
                .ok_or_else(|| observation_failure("internal_recipe_selection_invalid"))?
        };
        let execution = selection.execution.lock().await;
        Ok(observe(execution.as_ref().ok_or_else(|| {
            observation_failure("internal_recipe_execution_missing")
        })?))
    }
}
