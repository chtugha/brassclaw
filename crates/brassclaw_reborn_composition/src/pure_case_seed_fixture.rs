//! Sealed shipped instrument for the native primitive's controlled bootstrap.
//! This constructor accepts no authored source, Source-positive label or mode
//! flag. Its target is the exact retained fixed installation fixture below.
use super::*;
use crate::pure_case_execution::CaseCause;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex as StdMutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    bootstrap_evidence::{self as evidence, io},
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    pure_case_execution::OriginalCaseOperation,
};
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_pure_invocation::NativePureReceipt,
    retained_recipe::{RetainedRecipeExecution, RetainedToolInvocation},
    retained_source::{InspectedPureSelection, InspectedRetainedProgram},
};
use brassclaw_host_api::{ResourceScope, ResourceUsage};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityResult, RuntimePureCasePort,
};
use brassclaw_monty_host::{process::PortAnswer, service::PortFailure};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use serde_json::Value;
use tokio::sync::Mutex;
use uuid::Uuid;

const SEED_EVIDENCE_INSERT: &str = "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING";
#[derive(serde::Serialize)]
struct OriginalReceipt<'a> {
    bytes: &'a str,
    checksum: String,
    ordinal: u64,
    request_id: Uuid,
    stage: &'a str,
}
fn original_receipts<'a>(
    originals: &'a BTreeMap<&'static str, Arc<NativePureReceipt>>,
) -> Vec<OriginalReceipt<'a>> {
    originals
        .iter()
        .map(|(stage, r)| OriginalReceipt {
            bytes: r.bytes(),
            checksum: hex::encode(r.checksum()),
            ordinal: r.ordinal(),
            request_id: r.request_id(),
            stage,
        })
        .collect()
}
#[derive(serde::Serialize)]
struct NativeIntentRef {
    checksum: String,
    id: Uuid,
}
#[derive(serde::Serialize)]
struct NativeObservationReport<'a> {
    format: &'static str,
    native_intent: NativeIntentRef,
    native_manifest: &'a Value,
    native_receipts: Vec<OriginalReceipt<'a>>,
    operation: Uuid,
    output: &'a str,
    scope: &'a ResourceScope,
    stage: Uuid,
}
#[derive(serde::Serialize)]
struct NativeFailureReport<'a> {
    cause: &'a Value,
    format: &'static str,
    native_manifest: &'a Value,
    native_output: Option<&'a str>,
    native_receipts: Vec<OriginalReceipt<'a>>,
    native_scope: &'a ResourceScope,
    operation: Uuid,
    phase: &'a str,
    run_id: Uuid,
    scope: &'static str,
    stage: Uuid,
    stage_intent_checksum: String,
}
fn encode_record<T: serde::Serialize + ?Sized>(
    id: Uuid,
    association: &str,
    components: &[Value],
    report: &T,
    original_max_bytes: u64,
) -> Result<String, crate::pure_case_evidence::CaseEncodingError> {
    crate::pure_case_evidence::encode_original_for(
        &evidence::system_record_view(id, "behavior", false, association, components, report),
        original_max_bytes,
        SEED_EVIDENCE_INSERT,
        &[16, 64],
    )
}

const FIXTURE_SOURCE: &str = "def native_case_return(inputs):\n    return 'closed original Ü'\n\ndef native_case_exception(inputs):\n    raise ValueError('closed original exception')\n\ndef native_case_unexpected(inputs):\n    raise TypeError('closed unexpected exception')\n\ndef native_case_output_defect(inputs):\n    return 7\n";

pub(super) fn fixture_id() -> Uuid {
    brassclaw_skills::global_bootstrap_components::packaged_component_id(
        "validator:pure-case:native-seed-instrument",
    )
}
pub(super) fn fixture_draft() -> Result<ComponentRevisionDraft, RebornBuildError> {
    let exports = [
        "native_case_return",
        "native_case_exception",
        "native_case_unexpected",
        "native_case_output_defect",
    ]
    .into_iter()
    .map(|name| {
        (
            name.to_owned(),
            json!({"symbol":name,"parameters":["inputs"],"mapping":true}),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":fixture_id(),"class_code":22,
        "association":null,"dependencies":[],"document":{"name":"closed-native-original-pure-instrument",
            "description":"Controlled installation fixture, not an authored Source prerequisite or approved library candidate.",
            "content":FIXTURE_SOURCE,"includes":[],"dependency_registry":null,"input_contract":{},
            "result_contract":{"type":"string","checks":[]},"preload":{"format":"python-preload/2","exports":exports,
                "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"native_case_return"}}})
        .to_string()).map_err(invalid)
}

pub(super) struct Received {
    pub(super) output: String,
    pub(super) manifest: Value,
    pub(super) originals: BTreeMap<&'static str, Arc<NativePureReceipt>>,
    pub(super) invocation_id: Uuid,
    pub(super) scope: ResourceScope,
}
/// Issued solely for a real immutable bootstrap reservation/stage. All fields
/// remain private to this fixed seed owner; ordinary cases use the A constructor.
pub(super) struct SeedOperation {
    operation: Uuid,
    stage: Uuid,
    intent_bytes: String,
    base: crate::bootstrap_validator_approval::SessionInput,
    host: Arc<MontyTaskHost>,
    target: Arc<InspectedPureSelection>,
    export: &'static str,
    bound: OnceLock<Arc<BoundSeed>>,
    components: Vec<Value>,
}
impl SeedOperation {
    /// Passive bounded projection of already-owned evidence. This never reads
    /// raw receipt bodies, reconciles an operation or proves worker closure.
    pub(super) fn diagnostic(&self) -> String {
        let Some(bound) = self.bound.get() else {
            return "bound=false".into();
        };
        let failure = bound
            .failures
            .try_lock()
            .map(|failures| {
                failures
                    .get("operation")
                    .or_else(|| failures.values().next())
                    .map(|failure| {
                        let last = failure
                            .last_error
                            .try_lock()
                            .map(|error| {
                                error
                                    .as_ref()
                                    .map(seed_cause_kind)
                                    .unwrap_or_else(|| "none".into())
                            })
                            .unwrap_or_else(|_| "custody_unavailable".into());
                        format!(
                            "{} acknowledged={} last={last}",
                            seed_cause_kind(&failure.cause),
                            failure.acknowledged.load(Ordering::Acquire)
                        )
                    })
                    .unwrap_or_else(|| "none".into())
            })
            .unwrap_or_else(|_| "custody_unavailable".into());
        let reconciliation = bound
            .reconciliation_error
            .try_lock()
            .map(|error| {
                error
                    .as_ref()
                    .map(seed_cause_kind)
                    .unwrap_or_else(|| "none".into())
            })
            .unwrap_or_else(|_| "custody_unavailable".into());
        format!(
            "bound=true begun={} finished={} transport_pending={} supervised_reconciliation={} received={} failure=[{failure}] reconciliation={reconciliation}",
            bound.begun.load(Ordering::Acquire),
            bound.finished.load(Ordering::Acquire),
            bound.transport_pending.load(Ordering::Acquire),
            bound.context.requires_supervised_root_reconciliation(),
            bound.received.get().is_some()
        )
    }
    pub(super) fn from_reserved_stage(
        operation: Uuid,
        stage: Uuid,
        intent_bytes: String,
        base: crate::bootstrap_validator_approval::SessionInput,
        host: Arc<MontyTaskHost>,
        target: Arc<InspectedPureSelection>,
        export: &'static str,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let selected = target.inspected().map_err(invalid)?;
        let expected = fixture_draft()?
            .at_version(selected.component().version)
            .map_err(invalid)?
            .reference();
        if selected.component() != expected || selected.components().len() != 1 {
            return Err(invalid(
                "closed original seed bytes are not the shipped instrument",
            ));
        }
        let components = selected
            .components()
            .values()
            .map(|r| crate::installed_monty_catalogue::reference_value(*r))
            .collect();
        if operation.is_nil()
            || stage.is_nil()
            || target.inspected().map_err(invalid)?.component().uuid != fixture_id()
            || !target
                .inspected()
                .map_err(invalid)?
                .export_invocations()
                .contains_key(export)
        {
            return Err(invalid("closed original Pure seed instrument differs"));
        }
        Ok(Arc::new(Self {
            operation,
            stage,
            intent_bytes,
            base,
            host,
            target,
            export,
            bound: OnceLock::new(),
            components,
        }))
    }
    pub(super) fn received(&self) -> Result<&Received, RebornBuildError> {
        self.bound
            .get()
            .and_then(|b| b.received.get())
            .ok_or_else(|| invalid("closed native original observation absent"))
    }
}

// Closed typed error families only. In particular Dispatch JSON, host reasons,
// source/SQL errors and native commands/answers are never formatted here.
fn seed_cause_kind(cause: &CaseCause) -> String {
    use crate::pure_case_evidence::CaseEncodingError as Capacity;
    use brassclaw_engine::executor::retained_recipe::RetainedExecutionError as Engine;
    match cause {
        CaseCause::Engine(error) => match error {
            Engine::Invalid(_) => "engine/invalid".into(),
            Engine::Inputs(_) => "engine/inputs".into(),
            Engine::Tool(_) => "engine/tool".into(),
            Engine::Submission(error) => format!("engine/submission/{:?}", error.kind),
            Engine::Receipt { kind, .. } => format!("engine/receipt/{kind:?}"),
            Engine::Process(error) => format!("engine/process/{:?}", error.kind),
        },
        CaseCause::Metadata(_) => "metadata".into(),
        CaseCause::Host(error) => format!("host/{}", error.kind.as_str()),
        CaseCause::Capacity(error) => match error {
            Capacity::PolicyCapacity => "capacity/policy",
            Capacity::Representation => "capacity/representation",
            Capacity::Allocation(_) => "capacity/allocation",
            Capacity::Encoding(_) => "capacity/encoding",
        }
        .into(),
        CaseCause::Bootstrap { .. } => "bootstrap_metadata".into(),
        CaseCause::Boundary(_) => "boundary".into(),
        CaseCause::Dispatch(_) => "native_dispatch".into(),
    }
}

#[async_trait]
impl OriginalCaseOperation for SeedOperation {
    fn has_uncommitted_evidence(&self) -> bool {
        self.bound.get().is_some_and(|bound| bound.has_pending())
    }
    async fn reconcile_evidence(
        &self,
    ) -> Result<(), brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError>
    {
        if let Some(bound) = self.bound.get() {
            bound.reconcile().await
        } else {
            Ok(())
        }
    }
    async fn dispatch_failure(&self, phase: &'static str, facts: Value) {
        if let Some(bound) = self.bound.get() {
            bound
                .retain_failure(phase, CaseCause::Dispatch(facts))
                .await;
        }
    }
    async fn dispatch_host_failure(&self, error: brassclaw_turns::run_profile::AgentLoopHostError) {
        if let Some(bound) = self.bound.get() {
            bound
                .retain_failure("kernel_result", CaseCause::Host(error))
                .await;
        }
    }

    fn bind(
        &self,
        invocation: &RetainedToolInvocation<'_>,
        scope: &ResourceScope,
    ) -> Result<Arc<dyn RuntimePureCasePort>, RebornBuildError> {
        self.host.check_dispatch_open().map_err(invalid)?;
        let context = self.host.work_context();
        let thread = self.host.run_context().map(|chat| &chat.thread_id);
        if invocation.step_id() != "0:2"
            || self.host.attempt().run_id.as_uuid().is_nil()
            || scope.tenant_id != context.tenant_id
            || scope.agent_id != context.agent_id
            || scope.project_id != context.project_id
            || scope.thread_id.as_ref() != thread
            || context.actor.as_ref().map(|actor| &actor.user_id) != Some(&scope.user_id)
            || scope.user_id.as_str() != crate::bootstrap_validation::INSTALLATION_ACTOR
        {
            return Err(invalid("closed original seed dispatch differs"));
        }
        let selected = self
            .target
            .invocation(self.export, &json!({}), invocation.data_limits())
            .map_err(invalid)?;
        let bound = Arc::new(BoundSeed {
            operation: self.operation,
            stage: self.stage,
            intent_bytes: self.intent_bytes.clone(),
            base: self.base.clone(),
            host: self.host.clone(),
            context: invocation
                .pure_context()
                .ok_or_else(|| {
                    invalid("original Pure invocation is not bound to its native capability")
                })?
                .clone(),
            selected: Mutex::new(Some(selected)),
            scope: scope.clone(),
            run_id: self.host.attempt().run_id.as_uuid(),
            components: self.components.clone(),
            received: OnceLock::new(),
            failures: StdMutex::new(BTreeMap::new()),
            reconciliation_error: StdMutex::new(None),
            begun: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            transport_pending: AtomicBool::new(false),
        });
        self.bound
            .set(bound.clone())
            .map_err(|_| invalid("closed original seed binding repeated"))?;
        Ok(bound)
    }
}
struct BoundSeed {
    operation: Uuid,
    stage: Uuid,
    intent_bytes: String,
    base: crate::bootstrap_validator_approval::SessionInput,
    host: Arc<MontyTaskHost>,
    context: brassclaw_engine::executor::retained_pure_invocation::BoundPureContext,
    selected: Mutex<Option<brassclaw_engine::executor::retained_source::SealedPureInvocation>>,
    scope: ResourceScope,
    run_id: Uuid,
    components: Vec<Value>,
    received: OnceLock<Received>,
    failures: StdMutex<BTreeMap<&'static str, Arc<SeedFailure>>>,
    reconciliation_error: StdMutex<Option<CaseCause>>,
    begun: AtomicBool,
    finished: AtomicBool,
    transport_pending: AtomicBool,
}
struct SeedFailure {
    cause: CaseCause,
    bytes: OnceLock<String>,
    acknowledged: AtomicBool,
    last_error: StdMutex<Option<CaseCause>>,
}
impl BoundSeed {
    fn has_pending(&self) -> bool {
        self.context.requires_supervised_root_reconciliation()
            || (self.begun.load(Ordering::Acquire) && !self.finished.load(Ordering::Acquire))
            || self.transport_pending.load(Ordering::Acquire)
            || self
                .failures
                .lock()
                .map(|failures| {
                    failures
                        .values()
                        .any(|f| !f.acknowledged.load(Ordering::Acquire))
                })
                .unwrap_or(true)
    }
    async fn retain_failure(&self, phase: &'static str, cause: CaseCause) {
        let failure = Arc::new(SeedFailure {
            cause,
            bytes: OnceLock::new(),
            acknowledged: AtomicBool::new(false),
            last_error: StdMutex::new(None),
        });
        let failure = match self.failures.lock() {
            Ok(mut failures) => failures.entry(phase).or_insert(failure).clone(),
            Err(_) => return,
        };
        let _ = self.persist_failure(phase, &failure).await;
    }
    async fn persist_failure(
        &self,
        phase: &'static str,
        failure: &SeedFailure,
    ) -> Result<(), brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError>
    {
        use brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError as Error;
        if failure.acknowledged.load(Ordering::Acquire) {
            return Ok(());
        }
        if failure.bytes.get().is_none() {
            let cause = failure.cause.facts(&self.context).await;
            let manifest = self.context.native_receipt_facts().await.map_err(|error| {
                if let Ok(mut retained) = failure.last_error.lock() {
                    *retained = Some(CaseCause::Engine(error));
                }
                Error
            })?;
            if !manifest["pending_request"].is_null() {
                self.transport_pending.store(true, Ordering::Release);
            }
            let originals = self
                .context
                .failure_native_receipts()
                .await
                .map_err(|error| {
                    if let Ok(mut retained) = failure.last_error.lock() {
                        *retained = Some(CaseCause::Engine(error));
                    }
                    Error
                })?;
            let report = NativeFailureReport {
                cause: &cause,
                format: "original-pure-seed-native-technical-failure/1",
                native_manifest: &manifest,
                native_output: self.received.get().map(|r| r.output.as_str()),
                native_receipts: original_receipts(&originals),
                native_scope: &self.scope,
                operation: self.operation,
                phase,
                run_id: self.run_id,
                scope: "technical-observation-only",
                stage: self.stage,
                stage_intent_checksum: evidence::checksum(&self.intent_bytes),
            };
            let id = Uuid::new_v5(
                &self.stage,
                format!("original-native-technical-failure:{phase}").as_bytes(),
            );
            let bytes = encode_record(
                id,
                &evidence::checksum(&self.intent_bytes),
                &self.components,
                &report,
                self.base.preparation.policy().max_value_bytes,
            )
            .map_err(|error| {
                if let Ok(mut retained) = failure.last_error.lock() {
                    *retained = Some(CaseCause::Capacity(error));
                }
                Error
            })?;
            let _ = failure.bytes.set(bytes);
        }
        let bytes = failure.bytes.get().ok_or(Error)?;
        let id = Uuid::new_v5(
            &self.stage,
            format!("original-native-technical-failure:{phase}").as_bytes(),
        );
        match io::retain_record(&self.base.pool, self.base.storage.as_ref(), id, bytes).await {
            Ok(()) => {
                failure.acknowledged.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                if let Ok(mut retained) = failure.last_error.lock() {
                    *retained = Some(CaseCause::Bootstrap {
                        phase: "technical_failure",
                        error,
                    });
                }
                Err(Error)
            }
        }
    }
    async fn reconcile(
        &self,
    ) -> Result<(), brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError>
    {
        use brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError as Error;
        if self.begun.load(Ordering::Acquire) && !self.finished.load(Ordering::Acquire) {
            self.retain_failure(
                "operation",
                CaseCause::Boundary("original seed future interrupted"),
            )
            .await;
            self.finished.store(true, Ordering::Release);
        }
        let failures = self
            .failures
            .lock()
            .map_err(|_| Error)?
            .iter()
            .map(|(stage, f)| (*stage, f.clone()))
            .collect::<Vec<_>>();
        for (stage, failure) in failures {
            self.persist_failure(stage, &failure).await?;
        }
        if self.transport_pending.load(Ordering::Acquire) {
            self.context
                .collect_original_receipt()
                .await
                .map_err(|error| {
                    if let Ok(mut retained) = self.reconciliation_error.lock() {
                        *retained = Some(CaseCause::Engine(error));
                    }
                    Error
                })?;
            self.retain_failure(
                "transport_readback",
                CaseCause::Boundary("collected original seed receipt after interruption"),
            )
            .await;
            let failure = self
                .failures
                .lock()
                .map_err(|_| Error)?
                .get("transport_readback")
                .cloned()
                .ok_or(Error)?;
            if !failure.acknowledged.load(Ordering::Acquire) {
                return Err(Error);
            }
            self.transport_pending.store(false, Ordering::Release);
        }
        if self.has_pending() {
            Err(Error)
        } else {
            Ok(())
        }
    }
    async fn execute(&self) -> Result<FirstPartyCapabilityResult, CaseCause> {
        let selected = self
            .selected
            .lock()
            .await
            .take()
            .ok_or(CaseCause::Boundary("seed original is one-shot"))?;
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        let intent_id = Uuid::new_v5(&self.stage, b"original-native-intent");
        let report = json!({"format":"original-pure-seed-native-intent/1","operation":self.operation,
            "stage":self.stage,"stage_intent_checksum":evidence::checksum(&self.intent_bytes),
            "run_id":self.run_id,"scope":self.scope,"arguments":{}});
        let bytes = evidence::system_record(
            intent_id,
            "behavior",
            false,
            &evidence::checksum(&self.intent_bytes),
            &self.components,
            report,
        );
        io::retain_new_intent(
            &self.base.pool,
            self.base.storage.as_ref(),
            intent_id,
            &bytes,
        )
        .await
        .map_err(|error| CaseCause::Bootstrap {
            phase: "native_intent",
            error,
        })?;
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        let observation = self
            .context
            .invoke(selected)
            .await
            .map_err(CaseCause::Engine)?;
        let output = crate::pure_case_evidence::encode_original(
            &observation,
            self.base.preparation.policy().max_value_bytes,
        )
        .map_err(CaseCause::Capacity)?;
        let manifest = self
            .context
            .native_receipt_facts()
            .await
            .map_err(CaseCause::Engine)?;
        self.received
            .set(Received {
                output,
                manifest,
                originals: self.context.retained_native_receipts().await,
                invocation_id: self.scope.invocation_id.as_uuid(),
                scope: self.scope.clone(),
            })
            .map_err(|_| CaseCause::Boundary("native observation repeated"))?;
        let received = self
            .received
            .get()
            .ok_or(CaseCause::Boundary("retained native seed answer missing"))?;
        let id = Uuid::new_v5(&self.stage, b"original-native-observed");
        let report = NativeObservationReport {
            format: "original-pure-seed-native-observation/1",
            native_intent: NativeIntentRef {
                id: intent_id,
                checksum: evidence::checksum(&bytes),
            },
            native_manifest: &received.manifest,
            native_receipts: original_receipts(&received.originals),
            operation: self.operation,
            output: &received.output,
            scope: &received.scope,
            stage: self.stage,
        };
        let bytes = encode_record(
            id,
            &evidence::checksum(&self.intent_bytes),
            &self.components,
            &report,
            self.base.preparation.policy().max_value_bytes,
        )
        .map_err(CaseCause::Capacity)?;
        io::retain_record(&self.base.pool, self.base.storage.as_ref(), id, &bytes)
            .await
            .map_err(|error| CaseCause::Bootstrap {
                phase: "native_observation",
                error,
            })?;
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        Ok(FirstPartyCapabilityResult::new(
            Value::String(
                crate::pure_case_evidence::copy_original_text(
                    &received.output,
                    self.base.preparation.policy().max_value_bytes,
                )
                .map_err(CaseCause::Capacity)?,
            ),
            ResourceUsage {
                output_bytes: received.output.len() as u64,
                ..Default::default()
            },
        ))
    }
}

#[async_trait]
impl RuntimePureCasePort for BoundSeed {
    async fn invoke_original(
        &self,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        if self.begun.swap(true, Ordering::AcqRel) {
            return Err(CaseCause::Boundary("seed original is one-shot").public_error());
        }
        let result = match self.execute().await {
            Ok(result) => Ok(result),
            Err(cause) => {
                let public = cause.public_error();
                self.retain_failure("operation", cause).await;
                Err(public)
            }
        };
        self.finished.store(true, Ordering::Release);
        result
    }
}

pub(super) struct SeedSource {
    pub(super) reference: String,
    pub(super) inspected: Arc<InspectedRetainedProgram>,
    pub(super) operation: Arc<SeedOperation>,
    pub(super) tools: Arc<dyn brassclaw_engine::executor::retained_recipe::RetainedToolPort>,
    pub(super) raw: bool,
    pub(super) expected: Value,
    pub(super) received_receipt: OnceLock<String>,
    pub(super) receipt_failure: PortFailure,
}
#[async_trait]
impl InternalRecipeSource for SeedSource {
    fn name(&self) -> &str {
        "contained-original-pure-seed-fixture"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(json!({}).into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let verify = || -> Result<(), RebornBuildError> {
            if !std::ptr::eq(selected, self.inspected.as_ref())
                || !execution.is_complete()
                || execution.transport_failure().is_some()
                || execution.host_answers().len() != 1
                || execution.host_answers()[0].0 != "0:2"
            {
                return Err(invalid("seed native execution differs"));
            }
            let received = self.operation.received()?;
            if !matches!(&execution.host_answers()[0].1,PortAnswer::Return {value} if value.as_value().as_str()==Some(received.output.as_str()))
            {
                return Err(invalid("seed native answer differs"));
            }
            let value: Value = serde_json::from_str(&received.output).map_err(invalid)?;
            if value["outcome"] != self.expected
                || value["task"]
                    != serde_json::to_value(execution.task_handle()).map_err(invalid)?
                || value["component"]
                    != serde_json::to_value(
                        self.operation
                            .target
                            .inspected()
                            .map_err(invalid)?
                            .component(),
                    )
                    .map_err(invalid)?
            {
                return Err(invalid(
                    "seed actual typed original outcome differs from independent oracle",
                ));
            }
            if self.raw {
                if receipt != received.output
                    || execution.completed_step_ids().collect::<Vec<_>>() != ["0:2"]
                {
                    return Err(invalid("seed raw usage receipt differs"));
                }
            } else if serde_json::from_str::<Value>(receipt).map_err(invalid)?
                != json!({"format":"original-pure-case-receipt/1","observation":value})
                || execution.completed_step_ids().collect::<Vec<_>>() != ["0:2", "0:3"]
            {
                return Err(invalid("seed final private harness receipt differs"));
            }
            self.received_receipt
                .set(receipt.to_owned())
                .map_err(|_| invalid("seed receipt repeated"))?;
            Ok(())
        };
        verify().map_err(|_| self.receipt_failure.clone())
    }
}

/// Read only originals issued by this sealed fixed instrument. Callers already
/// hold one coherent qualification transaction; no pool reacquisition occurs.
pub(super) async fn verify_originals(
    tx: &tokio_postgres::Transaction<'_>,
    context: &Context,
    spec: &Spec,
    observed: &Value,
    native: &Value,
) -> Result<(), RebornBuildError> {
    let stage = context.id(spec.name);
    let stage_intent = context.stage_intent(spec)?;
    let association = evidence::checksum(&stage_intent);
    let components = context
        .instrument
        .inspected()
        .map_err(invalid)?
        .components()
        .values()
        .map(|r| crate::installed_monty_catalogue::reference_value(*r))
        .collect::<Vec<_>>();
    let intent_id = Uuid::new_v5(&stage, b"original-native-intent");
    let intent = evidence::read_evidence(tx, intent_id)
        .await?
        .ok_or_else(|| invalid("seed original native intent missing"))?;
    let intent_value: Value = serde_json::from_str(&intent).map_err(invalid)?;
    let scope: ResourceScope =
        serde_json::from_value(intent_value["report"]["scope"].clone()).map_err(invalid)?;
    let scope_value = serde_json::to_value(&scope).map_err(invalid)?;
    for field in ["tenant_id", "agent_id", "project_id", "thread_id"] {
        if scope_value[field] != observed["scope"][field] {
            return Err(invalid("seed original native actor scope differs"));
        }
    }
    if scope.user_id.as_str() != crate::bootstrap_validation::INSTALLATION_ACTOR {
        return Err(invalid(
            "seed original native actor differs from its fixed installation owner",
        ));
    }
    if json!(scope.invocation_id.as_uuid()) != observed["native_invocation_id"] {
        return Err(invalid("seed original native invocation differs"));
    }
    let expected = json!({"format":"original-pure-seed-native-intent/1","operation":context.operation,
        "stage":stage,"stage_intent_checksum":association,"run_id":observed["run_id"],"scope":scope,"arguments":{}});
    if intent
        != evidence::system_record(
            intent_id,
            "behavior",
            false,
            &association,
            &components,
            expected,
        )
    {
        return Err(invalid("seed original native intent bytes differ"));
    }
    let observed_id = Uuid::new_v5(&stage, b"original-native-observed");
    let bytes = evidence::read_evidence(tx, observed_id)
        .await?
        .ok_or_else(|| invalid("seed original native observation missing"))?;
    let envelope: Value = serde_json::from_str(&bytes).map_err(invalid)?;
    let report = &envelope["report"];
    let stored_originals = report["native_receipts"]
        .as_array()
        .ok_or_else(|| invalid("seed original native receipts absent"))?;
    let borrowed_receipts = stored_originals
        .iter()
        .map(|original| {
            let bytes = original["bytes"]
                .as_str()
                .ok_or_else(|| invalid("seed native original bytes absent"))?;
            Ok(OriginalReceipt {
                bytes,
                checksum: evidence::checksum(bytes),
                ordinal: original["ordinal"]
                    .as_u64()
                    .ok_or_else(|| invalid("seed original ordinal absent"))?,
                request_id: serde_json::from_value(original["request_id"].clone())
                    .map_err(invalid)?,
                stage: original["stage"]
                    .as_str()
                    .ok_or_else(|| invalid("seed original stage absent"))?,
            })
        })
        .collect::<Result<Vec<_>, RebornBuildError>>()?;
    let expected = NativeObservationReport {
        format: "original-pure-seed-native-observation/1",
        native_intent: NativeIntentRef {
            id: intent_id,
            checksum: evidence::checksum(&intent),
        },
        native_manifest: &report["native_manifest"],
        native_receipts: borrowed_receipts,
        operation: context.operation,
        output: observed["native_output"]
            .as_str()
            .ok_or_else(|| invalid("seed original native output absent"))?,
        scope: &scope,
        stage,
    };
    if bytes
        != encode_record(
            observed_id,
            &association,
            &components,
            &expected,
            context.base.preparation.policy().max_value_bytes,
        )
        .map_err(invalid)?
    {
        return Err(invalid("seed original native observation bytes differ"));
    }
    let manifest = &report["native_manifest"];
    let refs = manifest["retained_receipts"]
        .as_array()
        .ok_or_else(|| invalid("seed original native references absent"))?;
    let originals = report["native_receipts"]
        .as_array()
        .ok_or_else(|| invalid("seed original native receipts absent"))?;
    if manifest["root"] != observed["quiescence"]["receipt_root"] {
        return Err(invalid(
            "seed original native root differs from its actual quiescent service receipt",
        ));
    }
    if manifest["format"] != "original-pure-native-receipts/2"
        || manifest["task"] != native["task"]
        || manifest["parent"] != native["parent"]
        || manifest["host_key"] != native["host_key"]
        || manifest["started"] != true
        || !manifest["pending_request"].is_null()
        || !manifest["received_request"].is_null()
        || originals.len() != refs.len()
    {
        return Err(invalid("seed original native manifest differs"));
    }
    let mut stages = BTreeMap::new();
    for original in originals {
        let stage = original["stage"]
            .as_str()
            .ok_or_else(|| invalid("seed native receipt stage absent"))?;
        if ![
            "open",
            "preload",
            "preload_terminal",
            "start",
            "terminal",
            "release",
        ]
        .contains(&stage)
            || stages.contains_key(stage)
        {
            return Err(invalid("seed native receipt stage repeated or unsupported"));
        }
        let bytes = original["bytes"]
            .as_str()
            .ok_or_else(|| invalid("seed native receipt bytes absent"))?;
        let digest = evidence::checksum(bytes);
        let record: Value = serde_json::from_str(bytes).map_err(invalid)?;
        if original.as_object().is_none_or(|object| object.len() != 5)
            || original["stage"] != stage
            || original["ordinal"] != record["ordinal"]
            || original["request_id"] != record["request_id"]
            || original["checksum"] != digest
            || record["format"] != "original-pure-native-receipt/1"
            || record["transport_started"] != true
            || record["command"]["operation"] != "recipe"
            || record["snapshot"]["root"] != manifest["root"]
            || record["snapshot"]["lifecycle"] != "Ready"
            || !refs.contains(
                &json!({"stage":stage,"ordinal":record["ordinal"],"request_id":record["request_id"],
                "checksum":hex::decode(&digest).map_err(invalid)?}),
            )
        {
            return Err(invalid("seed native receipt exact bytes/reference differ"));
        }
        stages.insert(stage, record);
    }
    for stage in ["open", "preload", "start", "release"] {
        if !stages.contains_key(stage) {
            return Err(invalid("seed original native boundary missing"));
        }
    }
    if !manifest["terminal_stage"]
        .as_str()
        .is_some_and(|stage| stages.contains_key(stage))
    {
        return Err(invalid("seed original native terminal missing"));
    }
    let start = &stages["start"];
    let selected = &start["command"]["command"]["selected"];
    let source = context
        .instrument
        .inspected()
        .map_err(invalid)?
        .export_invocations()
        .get(spec.export)
        .cloned()
        .ok_or_else(|| invalid("seed original export binding missing"))?;
    let source_checksum = hex::decode(evidence::checksum(&source)).map_err(invalid)?;
    let empty_inputs_checksum = hex::decode(evidence::checksum("{}")).map_err(invalid)?;
    let terminal = manifest["terminal_stage"]
        .as_str()
        .and_then(|stage| stages.get(stage))
        .ok_or_else(|| invalid("seed original native terminal missing"))?;
    if start["command"]["command"]["operation"] != "start"
        || start["command"]["command"]["context"] != native["context"]
        || start["command"]["command"]["inputs"] != json!({})
        || selected != &json!({"source":source,"checksum":source_checksum,"aliases":[]})
        || native["invocation_checksum"] != json!(source_checksum)
        || native["inputs_checksum"] != json!(empty_inputs_checksum)
        || terminal["command"]["command"]["context"] != native["context"]
        || !matches!(
            terminal["command"]["command"]["operation"].as_str(),
            Some("start" | "resume_control")
        )
        || match native["outcome"]["kind"].as_str() {
            Some("return" | "result_contract_violation") => {
                !terminal["failure"].is_null()
                    || terminal["snapshot"]["recipe"]["boundary"]["kind"] != "complete"
                    || terminal["snapshot"]["recipe"]["boundary"]["value"]
                        != native["outcome"]["value"]
            }
            Some("python_exception") => {
                terminal["failure"] != "Vm(Python)"
                    || terminal["snapshot"]["recipe"]["kind"] != "failed"
                    || terminal["python_exception"] != native["outcome"]["exception"]
            }
            _ => true,
        }
    {
        return Err(invalid("seed original Start/terminal binding differs"));
    }
    let release = &stages["release"];
    let recipe = &release["snapshot"]["recipe"];
    let count = manifest["receipt_count"]
        .as_u64()
        .ok_or_else(|| invalid("seed original receipt count invalid"))?;
    if release["ordinal"].as_u64().and_then(|n| n.checked_add(1)) != Some(count)
        || manifest["receipt_chain"]
            != json!(
                hex::decode(evidence::checksum(
                    originals
                        .iter()
                        .find(|r| r["stage"] == "release")
                        .and_then(|r| r["bytes"].as_str())
                        .ok_or_else(|| invalid("release original missing"))?
                ))
                .map_err(invalid)?
            )
        || !release["failure"].is_null()
        || recipe["kind"] != "released"
        || !recipe["task"].is_null()
        || recipe["contexts"].as_array().map(Vec::len) != Some(1)
        || recipe["contexts"][0]["context"] != native["context"]
        || !recipe["contexts"][0]["pending_host"].is_null()
        || recipe["parent_host"]
            != json!({"task":native["task"],"context":native["parent"],"key":native["host_key"]})
    {
        return Err(invalid(
            "seed original released child/paused parent differs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn native_dispatch_diagnostic_never_formats_retained_payload() {
        let cause = CaseCause::Dispatch(json!({"answer":"private receipt/source marker"}));
        assert_eq!(seed_cause_kind(&cause), "native_dispatch");
    }
}
