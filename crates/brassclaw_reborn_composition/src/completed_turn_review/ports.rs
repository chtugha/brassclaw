use super::{
    Package,
    backend::{Backend, ReviewPrimitive, checksum},
};
use crate::monty_kernel::MontyKernelSnapshot;
use async_trait::async_trait;
use brassclaw_authorization::{
    InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule, ToolExecutionRules,
};
use brassclaw_engine::{
    executor::retained_recipe::{
        RetainedRecipeExecution, RetainedTaskContext, RetainedToolInvocation, RetainedToolPort,
    },
    memory::retained_tools::RetainedToolBinding,
};
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView,
    NetworkPolicy, ResourceEstimate, RuntimeKind, TrustClass,
};
use brassclaw_host_runtime::{
    RetainedFirstPartyCapability, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
};
use brassclaw_monty_host::{
    process::{PortAnswer, TaskHandle},
    service::{PortFailure, TaskOutcome, TaskPorts},
    transport_actor::TransportClient,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use futures::{FutureExt, future::BoxFuture};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::Mutex;
fn failure() -> PortFailure {
    PortFailure::new("internal_review_failed").expect("classified")
}
pub(super) struct ReviewPorts {
    pub backend: Arc<Backend>,
    package: Arc<Package>,
    tools: Arc<ReviewTools>,
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    task: Option<TaskHandle>,
    context: Option<RetainedTaskContext>,
    execution: Option<RetainedRecipeExecution>,
    resolved: bool,
}
impl ReviewPorts {
    pub(super) async fn new(
        backend: Arc<Backend>,
        package: Arc<Package>,
        kernel: &dyn MontyKernelSnapshot,
    ) -> Result<Arc<Self>, crate::RebornBuildError> {
        package
            .image
            .verify()
            .await
            .map_err(|e| super::invalid(e.to_string()))?;
        let mut runtimes = BTreeMap::new();
        for (index, usage) in package.ids.usages.iter().enumerate() {
            let capability = CapabilityId::new(usage.capability_id.clone())?;
            let handler = Arc::new(ReviewPrimitive {
                backend: backend.clone(),
                operation: brassclaw_skills::completed_turn_review_components::OPERATIONS[index],
            });
            let policy = Arc::new(Policy {
                backend: backend.clone(),
                tool: usage.tool,
                capability: capability.clone(),
            });
            let runtime = kernel.review(handler, &capability, policy)?;
            runtimes.insert(usage.tool, Arc::new(runtime));
        }
        let tools = Arc::new(ReviewTools {
            backend: backend.clone(),
            package: package.clone(),
            runtimes,
            answers: Mutex::new(BTreeMap::new()),
        });
        Ok(Arc::new(Self {
            backend,
            package,
            tools,
            state: Mutex::new(State::default()),
        }))
    }
    async fn dispatch(
        &self,
        task: TaskHandle,
        transport: TransportClient,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, PortFailure> {
        self.backend.check().await.map_err(|_| failure())?;
        let mut state = self.state.lock().await;
        if state.task.is_some_and(|t| t != task) {
            return Err(failure());
        }
        state.task = Some(task);
        let program = self.package.inspected.program();
        match name {
            "resolve_intent" => {
                if state.resolved || args != vec![json!(self.backend.source.to_string())] {
                    return Err(failure());
                }
                state.resolved = true;
                Ok(
                    json!({"status":"internal_review","component_id":self.package.ids.recipe,"step_link":program.inputs().instruction().variant().step_link,
                    "inputs":{"review_event_ref":self.backend.source.to_string()}}),
                )
            }
            "compose_orchestrator" => {
                let [recipe, link, inputs] = args.as_slice() else {
                    return Err(failure());
                };
                if !state.resolved
                    || state.execution.is_some()
                    || recipe != &json!(self.package.ids.recipe)
                    || link != &json!(program.inputs().instruction().variant().step_link)
                    || inputs != &json!({"review_event_ref":self.backend.source.to_string()})
                {
                    return Err(failure());
                }
                let bound = program
                    .inputs()
                    .bind_task_inputs(inputs)
                    .map_err(|_| failure())?;
                let context = state
                    .context
                    .get_or_insert_with(|| RetainedTaskContext::new(task));
                let parent = context.open(&transport).await.map_err(|_| failure())?;
                state.execution = Some(
                    RetainedRecipeExecution::new_child(
                        task,
                        parent,
                        self.package.inspected.clone(),
                    )
                    .map_err(|_| failure())?,
                );
                let steps: Vec<_> = program
                    .program()
                    .steplist
                    .iter()
                    .map(|step| json!({"step_id":step.step_id}))
                    .collect();
                Ok(
                    json!({"ok":true,"program_ref":self.backend.attempt,"steps":steps,"inputs":bound,
                    "flow":program.inputs().monty_flow().map_err(|_|failure())?}),
                )
            }
            "run_program" => {
                let [reference, step, frame] = args.as_slice() else {
                    return Err(failure());
                };
                let object = frame
                    .as_object()
                    .filter(|o| {
                        o.len() == 2 && o.contains_key("inputs") && o.contains_key("occurrence")
                    })
                    .ok_or_else(failure)?;
                let step = step.as_str().ok_or_else(failure)?;
                let index = program
                    .program()
                    .steplist
                    .iter()
                    .position(|s| s.step_id == step)
                    .ok_or_else(failure)?;
                if reference != &json!(self.backend.attempt)
                    || object["occurrence"] != json!([format!("execute_{index}")])
                {
                    return Err(failure());
                }
                let result = state
                    .execution
                    .as_mut()
                    .ok_or_else(failure)?
                    .run_step(
                        &transport,
                        step,
                        &object["inputs"],
                        Some(self.tools.as_ref()),
                    )
                    .await
                    .map_err(|_| failure())?;
                self.backend.check().await.map_err(|_| failure())?;
                Ok(json!({"ok":true,"return_value":result}))
            }
            _ => Err(failure()),
        }
    }
    pub(super) async fn retain_failure(&self) -> Result<(), PortFailure> {
        let client = self.backend.pool.get().await.map_err(|_| failure())?;
        client.execute("UPDATE brassclaw_monty_review_work SET phase=CASE WHEN model_dispatch_count=0 THEN 'failed' ELSE 'uncertain' END
            WHERE attempt_id=$1 AND owner_id=$2 AND phase NOT IN ('acknowledged','incomplete','failed','uncertain')",
            &[&self.backend.attempt,&self.backend.owner]).await.map_err(|_|failure())?;
        Ok(())
    }
}
impl TaskPorts for ReviewPorts {
    fn call(
        self: Arc<Self>,
        task: TaskHandle,
        transport: TransportClient,
        name: String,
        args: Vec<Value>,
        kwargs: BTreeMap<String, Value>,
    ) -> BoxFuture<'static, Result<Value, PortFailure>> {
        async move {
            if !kwargs.is_empty() {
                return Err(failure());
            }
            self.dispatch(task, transport, &name, args).await
        }
        .boxed()
    }
    fn finish(
        self: Arc<Self>,
        outcome: TaskOutcome,
    ) -> BoxFuture<'static, Result<TaskOutcome, PortFailure>> {
        async move {
            match &outcome {
                TaskOutcome::InternalCompleted {receipt_ref}=> {
                    self.backend.check().await.map_err(|_|failure())?;
                    if receipt_ref!=&format!("review:{}",self.backend.attempt)
                        || !self.state.lock().await.execution.as_ref().is_some_and(RetainedRecipeExecution::is_complete) {return Err(failure());}
                    let row=self.backend.pool.get().await.map_err(|_|failure())?.query_one(
                        "SELECT phase,receipt_bytes FROM brassclaw_monty_review_work WHERE attempt_id=$1 AND owner_id=$2",
                        &[&self.backend.attempt,&self.backend.owner]).await.map_err(|_|failure())?;
                    if row.get::<_,String>(0)!="acknowledged" || row.get::<_,Option<String>>(1).is_none() {return Err(failure());}
                }
                TaskOutcome::Failed {..}=>self.retain_failure().await?,
                TaskOutcome::Completed {..}=>return Err(failure()),
            }
            Ok(outcome)
        }.boxed()
    }
    fn fence(&self) {
        self.backend.closed.store(true, Ordering::Release);
    }
}
struct Policy {
    backend: Arc<Backend>,
    tool: uuid::Uuid,
    capability: CapabilityId,
}
#[async_trait]
impl InstanceToolPolicySource for Policy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        self.backend
            .check()
            .await
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        if descriptor.id != self.capability {
            return Ok(None);
        }
        let row = self
            .backend
            .pool
            .get()
            .await
            .map_err(|_| InstanceToolPolicyError::Unavailable)?
            .query_opt(
                "SELECT enabled,revision FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&self.tool],
            )
            .await
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        row.map(|r| {
            Ok(InstanceToolRule {
                enabled: r.get(0),
                revision: u64::try_from(r.get::<_, i64>(1))
                    .map_err(|_| InstanceToolPolicyError::Invalid)?,
                execution: ToolExecutionRules {
                    allowed_effects: descriptor.effects.clone(),
                    mounts: MountView::default(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            })
        })
        .transpose()
    }
}
struct ReviewTools {
    backend: Arc<Backend>,
    package: Arc<Package>,
    runtimes: BTreeMap<uuid::Uuid, Arc<RetainedFirstPartyCapability>>,
    answers: Mutex<BTreeMap<String, Value>>,
}
#[async_trait]
impl RetainedToolPort for ReviewTools {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        let result = self.invoke(invocation, binding, arguments).await;
        result.unwrap_or_else(|_| PortAnswer::TerminalError {
            reason_kind: "review_tool_failed".into(),
        })
    }
}
impl ReviewTools {
    async fn invoke(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> Result<PortAnswer, PortFailure> {
        self.backend.check().await.map_err(|_| failure())?;
        self.package.image.verify().await.map_err(|_| failure())?;
        let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(program) =
            self.package.inspected.program()
        else {
            return Err(failure());
        };
        let selected = program
            .bindings()
            .get(invocation.step_id())
            .ok_or_else(failure)?;
        if selected.tool() != binding.tool() || selected.python() != binding.python() {
            return Err(failure());
        }
        let runtime = self
            .runtimes
            .get(&binding.tool().uuid)
            .ok_or_else(failure)?;
        if runtime.descriptor().id.as_str() != binding.capability_id() {
            return Err(failure());
        }
        let operation = brassclaw_skills::completed_turn_review_components::OPERATIONS
            .iter()
            .find(|op| binding.capability_id() == format!("host.review_{op}"))
            .ok_or_else(failure)?;
        let bytes = serde_json::to_string(&arguments).map_err(|_| failure())?;
        let client = self.backend.pool.get().await.map_err(|_| failure())?;
        let inserted=client.execute("INSERT INTO brassclaw_monty_review_operations(attempt_id,operation,input_bytes,input_checksum)
            VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&self.backend.attempt,operation,&bytes,&checksum(&bytes)]).await.map_err(|_|failure())?;
        if inserted != 1 {
            return Err(failure());
        } // Never repeat an uncertain/completed effect.
        let context = &self.backend.context;
        let actor = context.actor.as_ref().ok_or_else(failure)?;
        let mut execution = ExecutionContext::local_default(
            actor.user_id.clone(),
            ExtensionId::new("host").map_err(|_| failure())?,
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .map_err(|_| failure())?;
        execution.tenant_id = context.scope.tenant_id.clone();
        execution.agent_id = context.scope.agent_id.clone();
        execution.project_id = context.scope.project_id.clone();
        execution.thread_id = Some(context.thread_id.clone());
        execution.resource_scope.tenant_id = execution.tenant_id.clone();
        execution.resource_scope.agent_id = execution.agent_id.clone();
        execution.resource_scope.project_id = execution.project_id.clone();
        execution.resource_scope.thread_id = execution.thread_id.clone();
        let trust = TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: runtime.descriptor().effects.clone(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::AdminConfig,
            evaluated_at: chrono::Utc::now(),
        };
        self.backend.check().await.map_err(|_| failure())?;
        let answer = match runtime
            .invoke(RuntimeCapabilityRequest::new(
                execution,
                runtime.descriptor().id.clone(),
                ResourceEstimate::default(),
                arguments,
                trust,
            ))
            .await
        {
            Ok(RuntimeCapabilityOutcome::Completed(value)) => PortAnswer::Return {
                value: value.output,
            },
            Ok(RuntimeCapabilityOutcome::Failed(value)) => PortAnswer::TerminalError {
                reason_kind: format!("review_{}", value.kind.as_str()),
            },
            _ => PortAnswer::TerminalError {
                reason_kind: "review_dispatch_failed".into(),
            },
        };
        let exact = serde_json::to_string(&answer).map_err(|_| failure())?;
        self.answers.lock().await.insert(
            operation.to_string(),
            serde_json::to_value(&answer).map_err(|_| failure())?,
        );
        let count=client.execute("UPDATE brassclaw_monty_review_operations SET output_bytes=$3,output_checksum=$4,answered_at=clock_timestamp()
            WHERE attempt_id=$1 AND operation=$2 AND output_bytes IS NULL",&[&self.backend.attempt,operation,&exact,&checksum(&exact)]).await.map_err(|_|failure())?;
        if count != 1 {
            return Err(failure());
        }
        Ok(answer)
    }
}
