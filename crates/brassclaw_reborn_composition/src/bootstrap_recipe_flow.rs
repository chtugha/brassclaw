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
    let ports = Arc::new(GlobalRecipePorts::for_bootstrap(
        admitted,
        request.source.clone(),
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
