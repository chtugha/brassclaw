//! Noncircular, contained parser qualification. Native primitive observations
//! precede binding verification and actual usage execution. No candidate runs.
use super::*;
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::prepare_retained_tool_program,
};
use brassclaw_host_api::{
    CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate, RuntimeKind,
    TrustClass,
};
use brassclaw_host_runtime::{RuntimeCapabilityOutcome, RuntimeCapabilityRequest};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS, global_bootstrap_components::UsageComponentIds,
    revision_store::PgComponentRevisionStore, validator_components::source_inventory_drafts,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use sha2::Digest;
use std::sync::OnceLock;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

#[path = "bootstrap_inventory_primitives.rs"]
mod primitives;
use primitives::{EvidenceSubject, ObservedPolicy, PrimitiveEvidence};
#[path = "bootstrap_inventory_completion.rs"]
mod completion;
pub(super) use completion::InventoryQualification;
#[cfg(all(test, unix))]
#[path = "bootstrap_inventory_fault_tests.rs"]
mod fault_tests;
#[cfg(all(test, unix))]
pub(crate) use fault_tests::assert_actual_ack_and_timeout_faults;

pub(super) struct InventoryObservation {
    intent: Uuid,
    intent_bytes: String,
    behavior: Uuid,
    behavior_bytes: String,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
}
impl InventoryObservation {
    async fn retain_observation(&self, pool: &PgPool) -> Result<(), RebornBuildError> {
        primitives::retain_record(
            pool,
            self.storage.as_ref(),
            self.behavior,
            &self.behavior_bytes,
        )
        .await?;
        self.verify(pool).await
    }
    pub(super) async fn verify(&self, pool: &PgPool) -> Result<(), RebornBuildError> {
        primitives::bounded(self.storage.as_ref(), async {
            let mut client = primitives::EvidenceClient::acquire(pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(invalid)?;
            if crate::bootstrap_evidence::read_evidence(&tx, self.intent)
                .await?
                .as_deref()
                != Some(self.intent_bytes.as_str())
                || crate::bootstrap_evidence::read_evidence(&tx, self.behavior)
                    .await?
                    .as_deref()
                    != Some(self.behavior_bytes.as_str())
            {
                return Err(invalid("original inventory qualification readback differs"));
            }
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
    }
}

fn usage_ids(binding: &RetainedToolBinding, recipe: Uuid) -> UsageComponentIds {
    UsageComponentIds {
        recipe,
        tool: binding.tool().uuid,
        tool_skill: binding.tool_skill().uuid,
        skill: binding.skill().uuid,
        python_code: binding.python().uuid,
        formatter: None,
    }
}

#[cfg(all(test, unix))]
#[derive(Clone, Copy)]
enum QualificationFault {
    WorkerExit,
    LateCancellation,
}

/// One closed qualification operation. Existing intent blocks a second run;
/// lost completion acknowledgement is resolved by the retained original proof.
/// The caller supplies the real retained reply only as factory context, never
/// as an executed part of this parser-only fixture.
#[cfg(test)]
pub(crate) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    candidate: &InspectedRetainedProgram,
    reply: &RetainedToolProgram,
    qualification_id: Uuid,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
) -> Result<Arc<QualifiedSourceInventoryUsage>, RebornBuildError> {
    let RetainedProgram::Tools(program) = candidate.program() else {
        return Err(invalid("qualification Tool program missing"));
    };
    qualify_packaged(
        pool,
        kernel,
        worker,
        program,
        candidate.preparation_limits(),
        reply,
        qualification_id,
        executor,
        storage,
        true,
        root,
    )
    .await?
    .ok_or_else(|| invalid("inventory qualification unavailable before execution"))
}

pub(super) async fn qualify_packaged(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    candidate: &RetainedToolProgram,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    reply: &RetainedToolProgram,
    qualification_id: Uuid,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    allow_new: bool,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
) -> Result<Option<Arc<QualifiedSourceInventoryUsage>>, RebornBuildError> {
    #[cfg(all(test, unix))]
    {
        qualify_inner(
            pool,
            kernel,
            worker,
            candidate,
            preparation,
            reply,
            qualification_id,
            executor,
            storage,
            allow_new,
            None,
            root,
        )
        .await
    }
    #[cfg(not(all(test, unix)))]
    {
        qualify_inner(
            pool,
            kernel,
            worker,
            candidate,
            preparation,
            reply,
            qualification_id,
            executor,
            storage,
            allow_new,
            root,
        )
        .await
    }
}

async fn qualify_inner(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    candidate: &RetainedToolProgram,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    reply: &RetainedToolProgram,
    qualification_id: Uuid,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    allow_new: bool,
    #[cfg(all(test, unix))] fault: Option<QualificationFault>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
) -> Result<Option<Arc<QualifiedSourceInventoryUsage>>, RebornBuildError> {
    let (_, binding) = require_inventory_binding(candidate)?;
    let reply_binding = reply
        .bindings()
        .values()
        .next()
        .filter(|b| reply.bindings().len() == 1 && b.capability_id() == "host.post_reply")
        .ok_or_else(|| invalid("packaged reply factory context missing"))?;
    let ids = crate::source_inspection_components::usage_ids(binding.tool().uuid, true);
    let expected = source_inventory_drafts(
        ids,
        crate::source_inspection_components::decoder_id(),
        usage_ids(reply_binding, reply.inputs().instruction().recipe().uuid),
    )
    .map_err(invalid)?;
    let graph = candidate.inputs().instruction().snapshot().revisions();
    for id in [ids.skill, ids.python_code, ids.tool_skill] {
        let draft = expected
            .iter()
            .find(|d| d.uuid() == id)
            .ok_or_else(|| invalid("packaged inventory usage definition missing"))?;
        if graph
            .get(&id)
            .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "inventory qualification requires actual packaged bytes",
            ));
        }
    }
    let recipe = brassclaw_skills::validator_components::source_inventory_qualification_draft(ids)
        .map_err(invalid)?;
    let recipe_id = recipe.uuid();
    let store = PgComponentRevisionStore::new(pool.clone());
    let recipe_ref = primitives::bounded(storage.as_ref(), async {
        store.retain_packaged_draft(&recipe).await.map_err(invalid)
    })
    .await?;
    let refs: Vec<_> = std::iter::once(recipe_ref)
        .chain(binding.combination().iter().copied())
        .collect();
    let snapshot = Arc::new(
        primitives::bounded(storage.as_ref(), async {
            store
                .read_exact(
                    &[recipe_id],
                    &refs,
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
                .await
                .map_err(invalid)
        })
        .await?,
    );
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                recipe_id,
                "selected",
                WorkflowClass::Deterministic,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?,
    );
    let subject = completion::Subject::new(
        program.clone(),
        worker.clone(),
        qualification_id,
        storage.clone(),
        &root,
    )?;
    if let Some(proof) = subject.read(&pool).await? {
        return Ok(Some(Arc::new(QualifiedSourceInventoryUsage {
            combination: binding.combination().to_vec(),
            association: binding.association().exact_bytes().to_owned(),
            worker,
            proof,
        })));
    }
    let reservation = Uuid::new_v5(&subject.intent, b"pre-admission-intent");
    if crate::bootstrap_evidence::io::read_record(&pool, storage.as_ref(), reservation)
        .await?
        .is_some()
    {
        return Err(invalid(
            "inventory original reservation incomplete; reconcile without admission",
        ));
    }
    if !allow_new {
        return Ok(None);
    }
    let worker = verified_worker(&worker, preparation.max_native_artifact_bytes).await?;
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(program.clone()),
            &worker.path(),
            preparation,
        )
        .await
        .map_err(invalid)?,
    );
    let sources = json!({"literal":"def probe(inputs):\n    raise RuntimeError('candidate stays data')\n", "empty":""});
    let reference = format!("contained-parser-qualification:{qualification_id}");
    let reservation_bytes = subject.record(
        reservation,
        json!({"format":"inventory-qualification-reservation/1",
        "manifest":subject.manifest,"executor_configuration":executor.exact_bytes(),
        "executor_configuration_checksum":executor.checksum(),"sources":sources}),
        false,
    );
    primitives::retain_new_intent(&pool, storage.as_ref(), reservation, &reservation_bytes).await?;
    let admitted = primitives::bounded(
        storage.as_ref(),
        crate::bootstrap_validation::admit(
            pool.clone(),
            "inventory-qualification",
            &reference,
            None,
        ),
    )
    .await?;
    let quiescence_proof = OnceLock::new();
    let qualification = QualificationRequest {
        quiescence_proof: &quiescence_proof,
        pool: pool.clone(),
        kernel,
        worker: worker.clone(),
        program,
        inspected,
        binding,
        admitted: &admitted,
        reference,
        sources,
        preparation,
        qualification_id,
        executor,
        storage: storage.clone(),
        subject: subject.clone(),
        reservation_bytes,
        root,
        #[cfg(all(test, unix))]
        fault,
    }
    .observe()
    .await;
    let closed = match quiescence_proof.get() {
        Some(proof) => {
            primitives::bounded(
                storage.as_ref(),
                crate::bootstrap_validation::settle_quiescent_closed(&pool, &admitted, proof),
            )
            .await
        }
        None => primitives::bounded(
            storage.as_ref(),
            crate::bootstrap_validation::settle_closed(&admitted),
        )
        .await
        .map(|_| brassclaw_turns::TurnStatus::Completed),
    };
    let closed = crate::bootstrap_validation::read_source_close(
        &pool,
        &admitted,
        quiescence_proof.get(),
        closed,
        storage.as_ref(),
    )
    .await;
    let observation = match qualification {
        Ok(observation) => observation,
        Err(operation) => {
            return Err(match closed {
                Ok(record) => invalid(format!(
                    "{operation}; original cleanup confirmed {:?}",
                    record.status
                )),
                Err(cleanup) => invalid(format!("{operation}; {cleanup}")),
            });
        }
    };
    let record = closed?;
    if record.status == brassclaw_turns::TurnStatus::Cancelled {
        return Err(invalid(
            "parser qualification cancelled; original cleanup confirmed Cancelled",
        ));
    }
    observation
        .verify(&pool)
        .await
        .map_err(|error| invalid(format!("{error}; original cleanup confirmed Completed")))?;
    // Recheck complete original evidence; metadata/ACK loss never replays effects.
    let proof = match subject.read(&pool).await {
        Ok(Some(proof)) => proof,
        Ok(None) => {
            return Err(invalid(
                "completed original qualification missing; original cleanup confirmed Completed",
            ));
        }
        Err(readback) => {
            return Err(invalid(format!(
                "{readback}; original cleanup confirmed Completed"
            )));
        }
    };
    Ok(Some(Arc::new(QualifiedSourceInventoryUsage {
        combination: binding.combination().to_vec(),
        association: binding.association().exact_bytes().to_owned(),
        worker,
        proof,
    })))
}

struct QualificationRequest<'a> {
    quiescence_proof: &'a OnceLock<crate::bootstrap_recipe_flow::ModelFreeQuiescence>,
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    program: Arc<RetainedToolProgram>,
    inspected: Arc<InspectedRetainedProgram>,
    binding: &'a RetainedToolBinding,
    admitted: &'a crate::bootstrap_validation::Admitted,
    reference: String,
    sources: Value,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    qualification_id: Uuid,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    subject: Arc<completion::Subject>,
    reservation_bytes: String,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
    #[cfg(all(test, unix))]
    fault: Option<QualificationFault>,
}
impl QualificationRequest<'_> {
    async fn observe(self) -> Result<InventoryObservation, RebornBuildError> {
        let Self {
            quiescence_proof,
            pool,
            kernel,
            worker,
            program,
            inspected,
            binding,
            admitted,
            reference,
            sources,
            preparation,
            qualification_id,
            executor,
            storage,
            subject,
            reservation_bytes,
            root,
            #[cfg(all(test, unix))]
            fault,
        } = self;
        let (step, primitive_binding) = require_inventory_binding(&program)?;
        let primitive = crate::installed_monty_catalogue::bootstrap_builtin_runtime(
            &pool,
            &kernel,
            &admitted.host,
            &program,
            primitive_binding,
            false,
        )?;
        primitive.verify_native_artifact().await.map_err(invalid)?;
        let native = primitive
            .native_identity()
            .ok_or_else(|| invalid("inventory native registration missing"))?;
        let intent = subject.intent;
        let behavior = subject.behavior;
        let mut context = ExecutionContext::local_default(
            admitted
                .chat_context()?
                .actor
                .as_ref()
                .ok_or_else(|| invalid("qualification actor missing"))?
                .user_id
                .clone(),
            ExtensionId::new("inventory-qualification").map_err(invalid)?,
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .map_err(invalid)?;
        context.tenant_id = admitted.chat_context()?.scope.tenant_id.clone();
        context.agent_id = admitted.chat_context()?.scope.agent_id.clone();
        context.project_id = admitted.chat_context()?.scope.project_id.clone();
        context.thread_id = Some(admitted.chat_context()?.thread_id.clone());
        context.resource_scope.tenant_id = context.tenant_id.clone();
        context.resource_scope.agent_id = context.agent_id.clone();
        context.resource_scope.project_id = context.project_id.clone();
        context.resource_scope.thread_id = context.thread_id.clone();
        let mut invalid_context = context.clone();
        invalid_context.invocation_id = brassclaw_host_api::InvocationId::new();
        invalid_context.resource_scope.invocation_id = invalid_context.invocation_id;
        let mut denied_context = context.clone();
        denied_context.invocation_id = brassclaw_host_api::InvocationId::new();
        denied_context.resource_scope.invocation_id = denied_context.invocation_id;
        let mut capacity_context = context.clone();
        capacity_context.invocation_id = brassclaw_host_api::InvocationId::new();
        capacity_context.resource_scope.invocation_id = capacity_context.invocation_id;
        let components: Vec<_> = binding.combination().iter().map(|r|json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect();
        let association_checksum =
            crate::bootstrap_evidence::checksum(binding.association().exact_bytes());
        let evidence = Arc::new(EvidenceSubject {
            pool: pool.clone(),
            storage: storage.clone(),
            intent,
            association: association_checksum.clone(),
            components: components.clone(),
        });
        let valid_evidence = PrimitiveEvidence::new(evidence.clone(), "primitive-valid");
        let invalid_evidence = PrimitiveEvidence::new(evidence.clone(), "primitive-invalid-input");
        let capacity_evidence = PrimitiveEvidence::new(evidence.clone(), "primitive-capacity");
        let denied_evidence = PrimitiveEvidence::new(evidence, "primitive-denial");
        let live_policy = |restrict| {
            ObservedPolicy::new(
                crate::monty_kernel::PgSelectedToolPolicy {
                    host: admitted.host.clone(),
                    pool: pool.clone(),
                    tool: primitive_binding.tool().uuid,
                    capability: primitive.descriptor().id.clone(),
                    effects: primitive.descriptor().effects.clone(),
                    mounts: MountView::default(),
                },
                restrict,
            )
        };
        let valid_policy = live_policy(false);
        let invalid_policy = live_policy(false);
        let capacity_policy = live_policy(false);
        let denied_policy = live_policy(true);
        let valid_cap = kernel
            .source_inspection_observed(
                native,
                context.resource_scope.clone(),
                json!({"sources":sources}),
                valid_evidence.clone(),
                valid_policy.clone(),
            )
            .map_err(invalid)?;
        let invalid_cap = kernel
            .source_inspection_observed(
                native,
                invalid_context.resource_scope.clone(),
                json!({"sources":{"":""}}),
                invalid_evidence.clone(),
                invalid_policy.clone(),
            )
            .map_err(invalid)?;
        let denied_cap = kernel
            .source_inspection_observed(
                native,
                denied_context.resource_scope.clone(),
                json!({"sources":sources}),
                denied_evidence.clone(),
                denied_policy.clone(),
            )
            .map_err(invalid)?;
        let capacity_fixture = kernel
            .inventory_capacity_fixture(
                native,
                capacity_context.resource_scope.clone(),
                capacity_evidence.clone(),
                capacity_policy.clone(),
            )
            .await
            .map_err(invalid)?;
        let header = json!({"format":"contained-inventory-qualification/1","qualification_id":qualification_id,"manifest":subject.manifest,"recipe_selection":program.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
        "association":binding.association().exact_bytes(),"worker_checksum":hex::encode(worker.checksum()),"native":native,
        "run_id":admitted.chat_context()?.run_id,"runner_id":admitted.attempt.runner_id,
        "claim_checksum":hex::encode(sha2::Sha256::digest(serde_json::to_vec(&admitted.attempt.lease_token).map_err(invalid)?)),
        "primitive_scope":context.resource_scope,"primitive_invocation":context.invocation_id,"sources":sources,
        "invalid_primitive_invocation":invalid_context.invocation_id,"invalid_arguments":{"sources":{"":""}},
        "denied_primitive_invocation":denied_context.invocation_id,
        "capacity_primitive_invocation":capacity_context.invocation_id,"capacity_fixture_profile":capacity_fixture.profile().exact_bytes(),"capacity_fixture_checksum":hex::encode(capacity_fixture.profile().checksum()),
        "preparation_bounds":preparation.bounds,"executor_configuration":executor.exact_bytes(),"executor_configuration_checksum":executor.checksum(),
        "reservation_checksum":crate::bootstrap_evidence::checksum(&reservation_bytes)});
        let record = |id, kind, succeeded, report| {
            evidence_record(
                id,
                kind,
                succeeded,
                &association_checksum,
                &components,
                report,
            )
        };
        // This is an incomplete intent, deliberately unusable as successful Q1.
        let intent_bytes = record(intent, "q1", false, json!({"header":header}));
        primitives::retain_new_intent(&pool, storage.as_ref(), intent, &intent_bytes).await?;
        // The native primitive is exercised independently, before its binding
        // or associated PythonCode can be considered qualified.
        let primitive_request = |context, arguments| {
            RuntimeCapabilityRequest::new(
                context,
                primitive.descriptor().id.clone(),
                ResourceEstimate::default(),
                arguments,
                TrustDecision {
                    effective_trust: EffectiveTrustClass::user_trusted(),
                    authority_ceiling: AuthorityCeiling {
                        allowed_effects: primitive.descriptor().effects.clone(),
                        max_resource_ceiling: None,
                    },
                    provenance: TrustProvenance::AdminConfig,
                    evaluated_at: chrono::Utc::now(),
                },
            )
        };
        let valid_request = primitive_request(context, json!({"sources":sources}));
        valid_evidence.prepare(&valid_request, None).await?;
        let native_result = admitted
            .host
            .invoke_retained_tool(&valid_cap, valid_request)
            .await;
        let valid_facts = valid_evidence
            .acknowledge(&native_result, valid_policy.facts()?)
            .await?;
        let native_result = native_result.map_err(invalid)?;
        let RuntimeCapabilityOutcome::Completed(native_result) = native_result else {
            return Err(invalid("native inventory prerequisite did not complete"));
        };
        if native_result.capability_id != primitive.descriptor().id
            || native_result.display_preview.is_some()
        {
            return Err(invalid(
                "native inventory prerequisite returned another capability or preview",
            ));
        }
        let native_report = strict_json(
            native_result
                .output
                .as_str()
                .ok_or_else(|| invalid("native inventory report is not text"))?,
            REVISION_LIMITS,
        )
        .map_err(invalid)?;
        require_fixture(&native_report, &sources, worker.checksum())?;
        let invalid_request = primitive_request(invalid_context, json!({"sources":{"":""}}));
        invalid_evidence.prepare(&invalid_request, None).await?;
        let invalid_result = admitted
            .host
            .invoke_retained_tool(&invalid_cap, invalid_request)
            .await;
        let invalid_facts = invalid_evidence
            .acknowledge(&invalid_result, invalid_policy.facts()?)
            .await?;
        let invalid_result = invalid_result.map_err(invalid)?;
        let RuntimeCapabilityOutcome::Failed(invalid_failure) = invalid_result else {
            return Err(invalid(
                "invalid inventory fixture lacks native input rejection",
            ));
        };
        if invalid_failure.kind != brassclaw_host_runtime::RuntimeFailureKind::InvalidInput
            || invalid_failure.capability_id != primitive.descriptor().id
            || invalid_failure.message.as_deref()
                != Some(brassclaw_host_runtime::SOURCE_INVENTORY_INPUT_INVALID_REASON)
        {
            return Err(invalid("invalid inventory fixture has unrelated failure"));
        }
        let capacity_request = primitive_request(
            capacity_context,
            capacity_fixture.profile().arguments().clone(),
        );
        capacity_evidence
            .prepare(
                &capacity_request,
                Some(capacity_fixture.profile().exact_bytes()),
            )
            .await?;
        let capacity_result = admitted
            .host
            .invoke_retained_tool(capacity_fixture.capability(), capacity_request)
            .await;
        let capacity_facts = capacity_evidence
            .acknowledge(&capacity_result, capacity_policy.facts()?)
            .await?;
        let capacity_result = capacity_result.map_err(invalid)?;
        let RuntimeCapabilityOutcome::Failed(capacity_failure) = capacity_result else {
            return Err(invalid(
                "inventory capacity fixture lacks complete native rejection",
            ));
        };
        let capacity_reason = capacity_fixture.profile().guard_reason();
        if capacity_failure.kind != brassclaw_host_runtime::RuntimeFailureKind::Resource
            || capacity_failure.capability_id != primitive.descriptor().id
            || capacity_failure.message.as_deref() != Some(capacity_reason.as_str())
        {
            return Err(invalid(
                "inventory capacity fixture has unrelated infrastructure or guard failure",
            ));
        }
        let denied_request = primitive_request(denied_context, json!({"sources":sources}));
        denied_evidence.prepare(&denied_request, None).await?;
        let denied_result = admitted
            .host
            .invoke_retained_tool(&denied_cap, denied_request)
            .await;
        let denied_facts = denied_evidence
            .acknowledge(&denied_result, denied_policy.facts()?)
            .await?;
        let denied_result = denied_result.map_err(invalid)?;
        let RuntimeCapabilityOutcome::Failed(denied_failure) = denied_result else {
            return Err(invalid(
                "inventory restrictive policy did not deny the native fixture",
            ));
        };
        if !denied_policy.genuine_restriction()?
            || denied_failure.kind != brassclaw_host_runtime::RuntimeFailureKind::Authorization
            || denied_failure.capability_id != primitive.descriptor().id
        {
            return Err(invalid(
                "inventory restrictive policy has unrelated failure",
            ));
        }
        let negative_facts = json!({
            "invalid_input":{"kind":invalid_failure.kind.as_str(),"capability_id":invalid_failure.capability_id,"message":invalid_failure.message},
            "capacity":{"kind":capacity_failure.kind.as_str(),"capability_id":capacity_failure.capability_id,"message":capacity_failure.message},
            "denied":{"kind":denied_failure.kind.as_str(),"capability_id":denied_failure.capability_id,"message":denied_failure.message}
        });
        // Binding preparation uses exactly the same registered native identity;
        // native_input_schema and actual metadata are checked before usage.
        let bound = crate::installed_monty_catalogue::bootstrap_builtin_runtime(
            &pool,
            &kernel,
            &admitted.host,
            &program,
            primitive_binding,
            false,
        )?;
        if bound.native_identity() != Some(native)
            || bound.native_input_schema() != primitive.native_input_schema()
        {
            return Err(invalid(
                "inventory binding prerequisite differs from native",
            ));
        }
        let tools = crate::installed_monty_catalogue::inventory_qualification_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )?;
        let source = Arc::new(QualificationSource {
            reference,
            inputs: json!({"sources":sources}),
            inspected: inspected.clone(),
            tools,
            pool: pool.clone(),
            host: admitted.host.clone(),
            worker: worker.checksum(),
            report: Mutex::new(None),
            proof: Mutex::new(None),
            proof_context: InventoryProofContext {
                intent,
                intent_bytes: intent_bytes.clone(),
                behavior,
                association_checksum: association_checksum.clone(),
                components: components.clone(),
                storage: storage.clone(),
                prerequisites: json!({"header":header,"primitive":{"capability_id":native_result.capability_id,"output":native_result.output,"usage":native_result.usage,"display_preview":null,"parsed_inventory":native_report},"negative_primitives":negative_facts,"primitive_records":[valid_facts,invalid_facts,capacity_facts,denied_facts],"binding_native":native,"step":step}),
            },
        });
        let request = crate::bootstrap_recipe_flow::Request::contained(
            worker.path(),
            executor.clone(),
            &root,
            source.clone(),
        )?;
        #[cfg(all(test, unix))]
        let request = if matches!(fault, Some(QualificationFault::WorkerExit)) {
            request.with_forced_worker_exit()
        } else {
            request
        };
        let observed = crate::bootstrap_recipe_flow::execute(admitted, request).await?;
        #[cfg(all(test, unix))]
        if matches!(fault, Some(QualificationFault::WorkerExit)) {
            use std::os::unix::process::ExitStatusExt;
            let status = observed
                .exit
                .transport
                .as_ref()
                .expect("actual contained exit")
                .exit_status
                .as_ref()
                .expect("failed worker reaped");
            assert!(!status.success());
            assert_eq!(status.signal(), Some(9));
        }
        let (actual_quiescence, proof) = observed.quiescence_proof()?;
        quiescence_proof
            .set(proof)
            .map_err(|_| invalid("original parser quiescence already observed"))?;
        if admitted.host.finalized_reply_ref().is_some() {
            return Err(invalid("inventory fixture published an unexpected reply"));
        }
        let report = source
            .report
            .lock()
            .await
            .take()
            .ok_or_else(|| invalid("inventory usage observation missing"))?;
        require_fixture(&report, &sources, worker.checksum())?;
        let proof = source
            .proof
            .lock()
            .await
            .take()
            .ok_or_else(|| invalid("inventory usage proof missing"))?;
        let quiescence=subject.record(subject.quiescence,json!({"format":"inventory-quiescence-observation/1","manifest":subject.manifest,
            "behavior_checksum":crate::bootstrap_evidence::checksum(&proof.behavior_bytes),"turn_scope":admitted.chat_context()?.scope,"native":actual_quiescence}),false);
        primitives::retain_record(&pool, storage.as_ref(), subject.quiescence, &quiescence).await?;
        #[cfg(all(test, unix))]
        if matches!(fault, Some(QualificationFault::LateCancellation)) {
            primitives::bounded(storage.as_ref(), async {
                brassclaw_turns::TurnStateStore::request_cancel(
                    admitted.state.as_ref(),
                    brassclaw_turns::CancelRunRequest {
                        scope: admitted.chat_context()?.scope.clone(),
                        actor: admitted
                            .chat_context()?
                            .actor
                            .clone()
                            .ok_or_else(|| invalid("original cancel actor missing"))?,
                        run_id: admitted.attempt.run_id,
                        reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
                        idempotency_key: brassclaw_turns::IdempotencyKey::new(format!(
                            "inventory-late-cancel:{qualification_id}"
                        ))
                        .map_err(invalid)?,
                    },
                )
                .await
                .map_err(invalid)?;
                Ok(())
            })
            .await?;
        }

        Ok(proof)
    }
}

fn require_fixture(
    report: &Value,
    sources: &Value,
    worker: [u8; 32],
) -> Result<(), RebornBuildError> {
    if report["format"] != "python-source-inventory/1"
        || report["worker_checksum"] != hex::encode(worker)
        || report["sources"].as_object().is_none_or(|o| o.len() != 2)
        || sources.as_object().is_none_or(|o| o.len() != 2)
    {
        return Err(invalid("inventory fixture identity differs"));
    }
    for name in ["literal", "empty"] {
        let source = sources[name]
            .as_str()
            .ok_or_else(|| invalid("fixture source missing"))?;
        if report["sources"][name]["kind"] != "inspected"
            || report["sources"][name]["structure"]["source_checksum"]
                != crate::bootstrap_evidence::checksum(source)
        {
            return Err(invalid("inventory fixture was not completely inspected"));
        }
    }
    if report["sources"]["literal"]["structure"]["preload"]["functions"]["probe"]
        != json!(["inputs"])
        || report["sources"]["empty"]["structure"]["direct_host_calls"] != json!([])
    {
        return Err(invalid("native inventory facts differ from fixture"));
    }
    Ok(())
}
fn evidence_record(
    id: Uuid,
    kind: &str,
    succeeded: bool,
    association: &str,
    components: &[Value],
    report: Value,
) -> String {
    crate::bootstrap_evidence::system_record(id, kind, succeeded, association, components, report)
}
struct InventoryProofContext {
    intent: Uuid,
    intent_bytes: String,
    behavior: Uuid,
    association_checksum: String,
    components: Vec<Value>,
    prerequisites: Value,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
}
struct QualificationSource {
    reference: String,
    inputs: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    pool: Arc<PgPool>,
    host: Arc<MontyTaskHost>,
    worker: [u8; 32],
    report: Mutex<Option<Value>>,
    proof_context: InventoryProofContext,
    proof: Mutex<Option<InventoryObservation>>,
}
#[async_trait]
impl InternalRecipeSource for QualificationSource {
    fn name(&self) -> &str {
        "contained-parser-qualification"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
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
        let RetainedProgram::Tools(program) = selected.program() else {
            return Err(failure());
        };
        let (step, _) = require_inventory_binding(program).map_err(|_| failure())?;
        if !execution.is_complete()
            || execution.transport_failure().is_some()
            || program
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(|_| failure())?
                .exact_bytes()
                != self
                    .inspected
                    .program()
                    .inputs()
                    .instruction()
                    .retained_selection()
                    .map_err(|_| failure())?
                    .exact_bytes()
        {
            return Err(failure());
        }
        let result = json!(receipt);
        let report = NativeInventoryObservation {
            pool: &self.pool,
            host: &self.host,
            program,
            step,
            worker: self.worker,
            limits: captured_limits(selected),
        }
        .verify(execution, &result)
        .await
        .map_err(|_| failure())?;
        let observed = execution.observations().get(step).ok_or_else(failure)?;
        if observed.input_checksum()
            != typed_value_checksum_with_limits(&self.inputs, observed.data_limits())
                .map_err(|_| failure())?
        {
            return Err(failure());
        }
        require_fixture(&report, &self.inputs["sources"], self.worker).map_err(|_| failure())?;
        let context = &self.proof_context;
        let proof = InventoryObservation {
            intent: context.intent,
            intent_bytes: context.intent_bytes.clone(),
            behavior: context.behavior,
            storage: context.storage.clone(),
            behavior_bytes: evidence_record(
                context.behavior,
                "behavior",
                false,
                &context.association_checksum,
                &context.components,
                json!({"prerequisites":context.prerequisites,"usage":report,"task_handle":execution.task_handle(),"input_checksum":hex::encode(observed.input_checksum()),"arguments_checksum":observed.arguments_checksum().map(hex::encode),"answer_checksum":observed.answer_checksum().map(hex::encode),"result_checksum":observed.result_checksum().map(hex::encode),"native_answer":receipt,"preterminal_observation":true,"settled_approval":false}),
            ),
        };
        // Write complete native facts before receipt acknowledgement/worker
        // shutdown. Failure of either cannot erase this original observation.
        proof
            .retain_observation(&self.pool)
            .await
            .map_err(|_| failure())?;
        *self.proof.lock().await = Some(proof);
        *self.report.lock().await = Some(report);
        Ok(())
    }
}
