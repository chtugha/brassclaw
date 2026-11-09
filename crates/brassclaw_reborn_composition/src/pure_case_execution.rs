//! Single-case invocation binding at the existing native dispatcher. Case
//! provenance and settlement stay with the validator/private-delivery owners.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex as StdMutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use brassclaw_authorization::InstanceToolPolicySource;
use brassclaw_engine::executor::retained_recipe::RetainedToolInvocation;
use brassclaw_host_api::ResourceScope;
use brassclaw_host_runtime::RuntimePureCasePort;

use crate::RebornBuildError;
use crate::{
    bootstrap_reply_validation::invalid, pure_case_evidence::EvidenceStore,
    pure_case_original::CheckedOriginalPureCase,
};
use brassclaw_engine::executor::{
    retained_pure_invocation::BoundPureContext,
    retained_source::{InspectedPureSelection, SealedPureInvocation},
};
use brassclaw_host_runtime::{FirstPartyCapabilityError, FirstPartyCapabilityResult};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use tokio::sync::Mutex;

/// Implemented by the sealed case owner, never by JSON input or a Tool handler.
/// Binding receives the actual executor HostCall and the already journalled
/// native invocation scope. It may not open another task or select latest.
#[async_trait::async_trait]
pub(crate) trait OriginalCaseOperation: Send + Sync {
    fn has_uncommitted_evidence(&self) -> bool {
        false
    }
    async fn reconcile_evidence(
        &self,
    ) -> Result<(), brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError>
    {
        Ok(())
    }
    async fn dispatch_failure(&self, _phase: &'static str, _facts: serde_json::Value) {}
    async fn dispatch_host_failure(
        &self,
        _error: brassclaw_turns::run_profile::AgentLoopHostError,
    ) {
    }

    fn bind(
        &self,
        invocation: &RetainedToolInvocation<'_>,
        scope: &ResourceScope,
    ) -> Result<Arc<dyn RuntimePureCasePort>, RebornBuildError>;
}

pub(crate) struct NativeCaseBinding {
    pub(crate) operation: Arc<dyn OriginalCaseOperation>,
    pub(crate) policy: Arc<dyn InstanceToolPolicySource>,
}

/// Holds the checked authored original, its exact all-export preflight and the
/// actual private task host. No request JSON can manufacture this binding.
pub(crate) struct AuthoredCaseOperation {
    case: Arc<CheckedOriginalPureCase>,
    inspected: Arc<InspectedPureSelection>,
    evidence: Arc<EvidenceStore>,
    host: Arc<MontyTaskHost>,
    bound: OnceLock<Arc<BoundAuthoredCase>>,
}
impl AuthoredCaseOperation {
    pub(crate) fn new(
        case: Arc<CheckedOriginalPureCase>,
        inspected: Arc<InspectedPureSelection>,
        evidence: Arc<EvidenceStore>,
        host: Arc<MontyTaskHost>,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let actual = inspected.inspected().map_err(invalid)?;
        let expected = case.executable_components()?;
        if !host.is_result_only_completion()
            || host.attempt().run_id.as_uuid() != case.id()
            || actual.component() != case.candidate()
            || actual.components() != &expected
        {
            return Err(invalid(
                "original Pure case host or inspected candidate differs",
            ));
        }
        Ok(Arc::new(Self {
            case,
            inspected,
            evidence,
            host,
            bound: OnceLock::new(),
        }))
    }
    pub(crate) fn observed_output(&self) -> Result<&str, RebornBuildError> {
        self.bound
            .get()
            .and_then(|bound| bound.received.get())
            .map(|r| r.output.as_str())
            .ok_or_else(|| invalid("original Pure case native observation missing"))
    }
}
#[async_trait::async_trait]
impl OriginalCaseOperation for AuthoredCaseOperation {
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
    async fn dispatch_failure(&self, phase: &'static str, facts: serde_json::Value) {
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
        let context = self.case.context();
        if invocation.step_id() != "0:2"
            || scope.tenant_id != context.tenant_id
            || scope.agent_id != context.agent_id
            || scope.project_id != context.project_id
            || scope.thread_id.is_some()
            || context.actor.as_ref().map(|a| &a.user_id) != Some(&scope.user_id)
        {
            return Err(invalid("original Pure case native scope differs"));
        }
        // Binding only retains definitions/data; dispatch freshness remains
        // checked by the kernel/host and again before the first case intent.
        let selected = self
            .inspected
            .invocation(
                self.case.export(),
                self.case.inputs(),
                invocation.data_limits(),
            )
            .map_err(invalid)?;
        #[derive(serde::Serialize)]
        struct Execution<'a> {
            case_checksum: [u8; 32],
            case_id: uuid::Uuid,
            format: &'static str,
            invocation_id: uuid::Uuid,
            run_id: uuid::Uuid,
            scope: &'a ResourceScope,
            task: brassclaw_monty_host::process::TaskHandle,
            worker_checksum: [u8; 32],
        }
        let evidence_max_bytes =
            u64::try_from(invocation.data_limits().max_bytes).map_err(invalid)?;
        let bytes = crate::pure_case_evidence::encode_original_for(
            &Execution {
                case_checksum: self.case.checksum(),
                case_id: self.case.id(),
                format: "original-pure-case-execution/1",
                invocation_id: scope.invocation_id.as_uuid(),
                run_id: self.host.attempt().run_id.as_uuid(),
                scope,
                task: invocation.task(),
                worker_checksum: self.case.worker_checksum(),
            },
            evidence_max_bytes,
            crate::pure_case_evidence::EXECUTION_INSERT,
            &[16, 16, 32],
        )
        .map_err(invalid)?;
        let bound = Arc::new(BoundAuthoredCase {
            case: self.case.clone(),
            evidence: self.evidence.clone(),
            host: self.host.clone(),
            context: invocation
                .pure_context()
                .ok_or_else(|| {
                    invalid("original Pure invocation is not bound to its native capability")
                })?
                .clone(),
            selected: Mutex::new(Some(selected)),
            invocation_id: scope.invocation_id.as_uuid(),
            execution_bytes: bytes,
            received: OnceLock::new(),
            evidence_max_bytes,
            failures: StdMutex::new(BTreeMap::new()),
            reconciliation_error: StdMutex::new(None),
            transport_pending: AtomicBool::new(false),
            begun: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        });
        self.bound
            .set(bound.clone())
            .map_err(|_| invalid("original Pure case native binding already used"))?;
        Ok(bound)
    }
}
struct Received {
    execution: [u8; 32],
    observed: AtomicBool,
    output: String,
    native: String,
    originals: std::collections::BTreeMap<
        &'static str,
        Arc<brassclaw_engine::executor::retained_pure_invocation::NativePureReceipt>,
    >,
}
struct BoundAuthoredCase {
    case: Arc<CheckedOriginalPureCase>,
    evidence: Arc<EvidenceStore>,
    host: Arc<MontyTaskHost>,
    context: BoundPureContext,
    selected: Mutex<Option<SealedPureInvocation>>,
    invocation_id: uuid::Uuid,
    execution_bytes: String,
    received: OnceLock<Received>,
    evidence_max_bytes: u64,
    failures: StdMutex<BTreeMap<&'static str, Arc<Failed>>>,
    reconciliation_error: StdMutex<Option<CaseCause>>,
    transport_pending: AtomicBool,
    begun: AtomicBool,
    finished: AtomicBool,
}
/// Private sources survive the public sanitized error conversion. No Debug or
/// Display implementation can accidentally print candidate/SQL diagnostics.
pub(crate) enum CaseCause {
    Engine(brassclaw_engine::executor::retained_recipe::RetainedExecutionError),
    Metadata(crate::pure_case_evidence::EvidenceError),
    Host(brassclaw_turns::run_profile::AgentLoopHostError),
    Capacity(crate::pure_case_evidence::CaseEncodingError),
    Boundary(&'static str),
    Bootstrap {
        phase: &'static str,
        error: RebornBuildError,
    },
    Dispatch(serde_json::Value),
}
impl CaseCause {
    pub(crate) async fn facts(&self, context: &BoundPureContext) -> serde_json::Value {
        match self {
            Self::Engine(error) => context.failure_facts(error).await,
            Self::Metadata(error) => serde_json::json!({"kind":"metadata","cause":error.facts()}),
            Self::Host(error) => {
                serde_json::json!({"kind":"host","cause":error.kind.as_str(),"reason":error.reason_kind})
            }
            Self::Capacity(error) => {
                let kind = match error {
                    crate::pure_case_evidence::CaseEncodingError::PolicyCapacity => {
                        "original_policy"
                    }
                    crate::pure_case_evidence::CaseEncodingError::Representation => {
                        "postgres_representation"
                    }
                    crate::pure_case_evidence::CaseEncodingError::Allocation(_) => {
                        "host_allocation"
                    }
                    crate::pure_case_evidence::CaseEncodingError::Encoding(_) => "json_encoding",
                };
                serde_json::json!({"kind":"evidence_capacity","cause":kind})
            }
            Self::Bootstrap { phase, error } => {
                let _ = error;
                serde_json::json!({"kind":"bootstrap_metadata","phase":phase})
            }
            Self::Boundary(reason) => serde_json::json!({"kind":"boundary","cause":reason}),
            Self::Dispatch(facts) => serde_json::json!({"kind":"native_dispatch","cause":facts}),
        }
    }
    pub(crate) fn public_error(&self) -> FirstPartyCapabilityError {
        use brassclaw_host_api::RuntimeDispatchErrorKind as Kind;
        use brassclaw_turns::run_profile::AgentLoopHostErrorKind as HostKind;
        let (kind, summary) = match self {
            Self::Engine(_) => (
                Kind::Executor,
                "original Pure worker boundary failed; original evidence retained",
            ),
            Self::Metadata(_) | Self::Bootstrap { .. } => (
                Kind::Backend,
                "original Pure metadata acknowledgement failed; original evidence retained",
            ),
            Self::Host(error) if error.kind == HostKind::BudgetExceeded => (
                Kind::Resource,
                "original Pure task resource limit reached; original evidence retained",
            ),
            Self::Host(error) if error.kind == HostKind::PolicyDenied => (
                Kind::PolicyDenied,
                "original Pure task dispatch is fenced; original evidence retained",
            ),
            Self::Host(_) => (
                Kind::OperationFailed,
                "original Pure task is no longer dispatchable; original evidence retained",
            ),
            Self::Capacity(_) => (
                Kind::Resource,
                "original Pure evidence capacity unavailable; original evidence retained",
            ),
            Self::Boundary(_) => (
                Kind::InvalidResult,
                "original Pure case boundary failed; original evidence retained",
            ),
            Self::Dispatch(_) => (
                Kind::OperationFailed,
                "original Pure native delivery failed; original evidence retained",
            ),
        };
        FirstPartyCapabilityError::with_safe_summary(kind, summary)
            .with_usage(brassclaw_host_api::ResourceUsage::default())
    }
}
struct Failed {
    cause: CaseCause,
    payload: OnceLock<String>,
    acknowledged: AtomicBool,
    // Failed metadata/capture attempts keep their actual source privately.
    metadata_error: StdMutex<Option<crate::pure_case_evidence::EvidenceError>>,
    capture_error: StdMutex<Option<CaseCause>>,
}
impl BoundAuthoredCase {
    fn has_pending(&self) -> bool {
        self.context.requires_supervised_root_reconciliation()
            || (self.begun.load(Ordering::Acquire) && !self.finished.load(Ordering::Acquire))
            || self.transport_pending.load(Ordering::Acquire)
            || self
                .received
                .get()
                .is_some_and(|received| !received.observed.load(Ordering::Acquire))
            || self
                .failures
                .lock()
                .map(|failures| {
                    failures
                        .values()
                        .any(|failure| !failure.acknowledged.load(Ordering::Acquire))
                })
                .unwrap_or(true)
    }
    async fn retain_failure(&self, phase: &'static str, cause: CaseCause) {
        let failed = Arc::new(Failed {
            cause,
            payload: OnceLock::new(),
            acknowledged: AtomicBool::new(false),
            metadata_error: StdMutex::new(None),
            capture_error: StdMutex::new(None),
        });
        // Anchor actual sources before any capture or database await. The same
        // native operation is kept by the existing Factory reconciliation owner.
        let retained = match self.failures.lock() {
            Ok(mut failures) => failures.entry(phase).or_insert(failed).clone(),
            Err(_) => return,
        };
        let _ = self.persist_failure(phase, &retained).await;
    }
    async fn persist_failure(
        &self,
        phase: &'static str,
        failed: &Failed,
    ) -> Result<(), brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError>
    {
        use brassclaw_engine::executor::retained_recipe::NativeResultReconciliationError as Error;
        if failed.acknowledged.load(Ordering::Acquire) {
            return Ok(());
        }
        if failed.payload.get().is_none() {
            let facts = failed.cause.facts(&self.context).await;
            let native = self.context.native_receipt_facts().await.map_err(|error| {
                if let Ok(mut retained) = failed.capture_error.lock() {
                    *retained = Some(CaseCause::Engine(error));
                }
                Error
            })?;
            if !native["pending_request"].is_null() {
                self.transport_pending.store(true, Ordering::Release);
            }
            let originals = self
                .context
                .failure_native_receipts()
                .await
                .map_err(|error| {
                    if let Ok(mut retained) = failed.capture_error.lock() {
                        *retained = Some(CaseCause::Engine(error));
                    }
                    Error
                })?;
            #[derive(serde::Serialize)]
            struct Original<'a> {
                ordinal: u64,
                request_id: uuid::Uuid,
                checksum: [u8; 32],
                bytes: &'a str,
            }
            #[derive(serde::Serialize)]
            struct Payload<'a> {
                format: &'static str,
                case_id: uuid::Uuid,
                invocation_id: uuid::Uuid,
                phase: &'static str,
                execution_bytes: &'a str,
                failure: serde_json::Value,
                native: serde_json::Value,
                originals: BTreeMap<&'static str, Original<'a>>,
            }
            let payload = Payload {
                format: "original-pure-case-technical-failure/1",
                case_id: self.case.id(),
                invocation_id: self.invocation_id,
                phase,
                execution_bytes: &self.execution_bytes,
                failure: facts,
                native,
                originals: originals
                    .iter()
                    .map(|(stage, receipt)| {
                        (
                            *stage,
                            Original {
                                ordinal: receipt.ordinal(),
                                request_id: receipt.request_id(),
                                checksum: receipt.checksum(),
                                bytes: receipt.bytes(),
                            },
                        )
                    })
                    .collect(),
            };
            let bytes = crate::pure_case_evidence::encode_original_for(
                &payload,
                self.evidence_max_bytes,
                crate::pure_case_evidence::FAILURE_INSERT,
                &[16, 16, phase.len(), 32],
            )
            .map_err(|error| {
                if let Ok(mut retained) = failed.capture_error.lock() {
                    *retained = Some(CaseCause::Capacity(error));
                }
                Error
            })?;
            // Once set, a retry writes these exact bytes, even if later receipt
            // collection changes the carrier's current state.
            let _ = failed.payload.set(bytes);
        }
        let payload = failed.payload.get().ok_or(Error)?;
        match self
            .evidence
            .technical_failure(self.case.id(), self.invocation_id, phase, payload)
            .await
        {
            Ok(()) => {
                failed.acknowledged.store(true, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                if let Ok(mut retained) = failed.metadata_error.lock() {
                    *retained = Some(error);
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
                CaseCause::Boundary("original native operation future interrupted"),
            )
            .await;
            // This does not complete the original operation. It marks only that
            // its interruption now has a retained immutable failure anchor.
            self.finished.store(true, Ordering::Release);
        }
        // Only original actor receipt collection and exact metadata writes.
        // Never open/start/resume/release a worker context from reconciliation.
        if let Some(received) = self.received.get()
            && !received.observed.load(Ordering::Acquire)
        {
            self.evidence
                .observe(
                    self.case.id(),
                    received.execution,
                    &received.output,
                    &received.native,
                    &received.originals,
                )
                .await
                .map_err(|error| {
                    if let Ok(mut retained) = self.reconciliation_error.lock() {
                        *retained = Some(CaseCause::Metadata(error));
                    }
                    Error
                })?;
            received.observed.store(true, Ordering::Release);
        }
        let failures = self
            .failures
            .lock()
            .map_err(|_| Error)?
            .iter()
            .map(|(phase, failed)| (*phase, failed.clone()))
            .collect::<Vec<_>>();
        for (phase, failed) in failures {
            self.persist_failure(phase, &failed).await?;
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
                CaseCause::Boundary("collected original receipt after interrupted case"),
            )
            .await;
            let failed = self
                .failures
                .lock()
                .map_err(|_| Error)?
                .get("transport_readback")
                .cloned()
                .ok_or(Error)?;
            if !failed.acknowledged.load(Ordering::Acquire) {
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
            .ok_or(CaseCause::Boundary("already used"))?;
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        let execution = self
            .evidence
            .begin(self.case.id(), self.invocation_id, &self.execution_bytes)
            .await
            .map_err(CaseCause::Metadata)?;
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        let observation = self
            .context
            .invoke(selected)
            .await
            .map_err(CaseCause::Engine)?;
        let native = crate::pure_case_evidence::encode_original(
            &self
                .context
                .native_receipt_facts()
                .await
                .map_err(CaseCause::Engine)?,
            self.evidence_max_bytes,
        )
        .map_err(CaseCause::Capacity)?;
        let output = crate::pure_case_evidence::encode_original_for(
            &observation,
            self.evidence_max_bytes,
            crate::pure_case_evidence::OBSERVATION_INSERT,
            &[16, 32, native.len(), 32],
        )
        .map_err(CaseCause::Capacity)?;
        self.received
            .set(Received {
                execution,
                observed: AtomicBool::new(false),
                output,
                native,
                originals: self.context.retained_native_receipts().await,
            })
            .map_err(|_| CaseCause::Boundary("observation already received"))?;
        let received = self
            .received
            .get()
            .ok_or(CaseCause::Boundary("stored observation missing"))?;
        self.evidence
            .observe(
                self.case.id(),
                execution,
                &received.output,
                &received.native,
                &received.originals,
            )
            .await
            .map_err(CaseCause::Metadata)?;
        received.observed.store(true, Ordering::Release);
        self.host.check_dispatch_open().map_err(CaseCause::Host)?;
        Ok(FirstPartyCapabilityResult::new(
            serde_json::Value::String(
                crate::pure_case_evidence::copy_original_text(
                    &received.output,
                    self.evidence_max_bytes,
                )
                .map_err(CaseCause::Capacity)?,
            ),
            brassclaw_host_api::ResourceUsage {
                output_bytes: received.output.len() as u64,
                ..Default::default()
            },
        ))
    }
}
#[async_trait::async_trait]
impl RuntimePureCasePort for BoundAuthoredCase {
    async fn invoke_original(
        &self,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        if self.begun.swap(true, Ordering::AcqRel) {
            return Err(CaseCause::Boundary("original case is one-shot").public_error());
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
