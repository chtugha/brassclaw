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
}
impl Request {
    /// Shared physical containment for small closed installation fixtures. The
    /// caller supplies the retained root and exact selection; this constructs
    /// transport limits only and cannot choose/approve a workflow.
    pub(super) fn contained(
        worker: PathBuf,
        limits: brassclaw_monty_host::utility::UtilityLimits,
        root: &brassclaw_skills::orchestrator_contract::GlobalRootDefinition,
        source: Arc<dyn InternalRecipeSource>,
    ) -> Result<Self, RebornBuildError> {
        let mut process = limits.process;
        let frame = (8 * 1024 * 1024)
            .min(process.max_frame_bytes)
            .min(limits.bounds.max_value_bytes);
        if limits.bounds.max_source_bytes > frame || limits.bounds.max_stdout_bytes > frame {
            return Err(invalid("closed fixture exceeds configured transport"));
        }
        process.max_frame_bytes = frame;
        let adapter = brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
        let heap = process
            .hard_memory_bytes
            .checked_sub(2 * frame + adapter)
            .filter(|n| *n > frame)
            .ok_or_else(|| invalid("closed fixture exceeds configured memory"))?;
        let mut bounds = limits.bounds;
        bounds.max_value_bytes = frame;
        Ok(Self {
            worker,
            process,
            source,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 2 * frame,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: 2 * frame,
            },
            capacity: RecipeCapacity {
                revision: 1,
                max_recipes: 8,
                data_limits: brassclaw_skills::value_contract::ContractLimits {
                    max_bytes: frame,
                    ..brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS
                },
            },
            boot: RootBoot {
                source: root.source().to_owned(),
                checksum: root.source_checksum(),
                aliases: root.ports().iter().cloned().collect(),
                bounds: brassclaw_monty_host::global::GlobalBounds {
                    values: bounds,
                    workers: 1,
                    max_pending_calls: 8,
                },
                heap_settings: Some(brassclaw_monty_host::heap::HeapSettings {
                    revision: 1,
                    max_vm_bytes: heap,
                }),
                adapter_reserve_bytes: adapter,
                startup_timeout: std::time::Duration::from_secs(10),
                task_settings: brassclaw_monty_host::process::TaskSettings {
                    revision: 1,
                    max_compute_time: std::time::Duration::from_secs(30),
                    token_budgets_enabled: false,
                },
                max_recipe_contexts: 8,
            },
        })
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
/// Caller owns admission, exact subject validation, semantic/effect oracles,
/// dispatch intent and final durable settlement. This observer grants none.
pub(super) async fn execute(
    admitted: &Admitted,
    request: Request,
) -> Result<Observation, RebornBuildError> {
    if admitted.input["user_input"].as_str() != Some(request.source.reference()) {
        return Err(invalid("bootstrap source reference differs from admission"));
    }
    let ports = Arc::new(GlobalRecipePorts::for_internal(
        admitted.host.clone(),
        admitted.admission.clone(),
        request.source.clone(),
        None,
        true,
        Arc::new(Capacity(request.capacity)),
    ));
    let mut owner = ServiceOwner::start(
        &request.worker,
        request.boot,
        request.process,
        request.actor,
        1,
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
        let report = ports
            .settlement_report(&receipt)
            .await
            .map_err(|error| invalid(format!("bootstrap Recipe settlement: {error:?}")))?;
        Ok::<_, RebornBuildError>((receipt, report))
    }
    .await;
    owner.request_shutdown();
    let exit = owner
        .join()
        .await
        .map_err(|error| invalid(format!("bootstrap Recipe shutdown: {error:?}")))?;
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
