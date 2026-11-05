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
use brassclaw_monty_host::service::TaskOutcome;
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    global_bootstrap_components::UsageComponentIds,
    revision_store::PgComponentRevisionStore,
    validator_components::source_inventory_drafts,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

pub(super) struct InventoryQualification {
    intent: Uuid,
    intent_bytes: String,
    behavior: Uuid,
    behavior_bytes: String,
}
impl InventoryQualification {
    async fn retain_observation(&self, pool: &PgPool) -> Result<(), RebornBuildError> {
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(invalid)?;
        tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING",
            &[&self.behavior,&self.behavior_bytes,&crate::bootstrap_evidence::checksum(&self.behavior_bytes)]).await.map_err(invalid)?;
        let _commit = tx.commit().await;
        drop(client);
        self.verify(pool).await
    }
    pub(super) async fn verify(&self, pool: &PgPool) -> Result<(), RebornBuildError> {
        let mut client = pool.get().await.map_err(invalid)?;
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
        tx.commit().await.map_err(invalid)
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

/// One closed qualification operation. Existing intent blocks a second run;
/// lost completion acknowledgement is resolved by the retained original proof.
/// The caller supplies the real retained reply only as factory context, never
/// as an executed part of this parser-only fixture.
pub(crate) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    candidate: &InspectedRetainedProgram,
    reply: &RetainedToolProgram,
    qualification_id: Uuid,
) -> Result<Arc<QualifiedSourceInventoryUsage>, RebornBuildError> {
    let preparation = candidate.preparation_limits();
    let worker = verified_worker(&worker, preparation.max_native_artifact_bytes).await?;
    let RetainedProgram::Tools(candidate) = candidate.program() else {
        return Err(invalid(
            "qualification requires the original prepared Tool program",
        ));
    };
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
    let recipe_id = Uuid::new_v5(&qualification_id, b"contained-inventory-fixture/1");
    let recipe = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":recipe_id,"class_code":21,"association":null,
        "dependencies":[ids.skill,ids.tool_skill,ids.python_code],
        "document":{"name":"contained-inventory-qualification","variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Invoke the exact parser usage","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare the retained inventory binding","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"source_inspection","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke the pinned inventory export","content":"","type":"component","include":[ids.python_code]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":graph[&ids.python_code].draft().document()["input_contract"],"steps":{"0:2":{"sources":{"kind":"task_input","reference":"{{vars.sources}}"}}}}},
            "invocations":{"selected":{"0:2":"inspect_python_inventory"}}}
    }).to_string()).map_err(invalid)?;
    let store = PgComponentRevisionStore::new(pool.clone());
    let recipe_ref = store
        .retain_packaged_draft(&recipe)
        .await
        .map_err(invalid)?;
    let refs: Vec<_> = std::iter::once(recipe_ref)
        .chain(binding.combination().iter().copied())
        .collect();
    let snapshot = Arc::new(
        store
            .read_exact(&[recipe_id], &refs)
            .await
            .map_err(invalid)?,
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
    let reference = format!("contained-parser-qualification:{recipe_id}");
    let admitted = crate::bootstrap_validation::admit(
        pool.clone(),
        "inventory-qualification",
        &reference,
        None,
    )
    .await?;
    let qualification = QualificationRequest {
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
    }
    .observe()
    .await;
    let closed = crate::bootstrap_validation::settle_closed(&admitted).await;
    let proof = match (qualification, closed) {
        (Ok(proof), Ok(())) => proof,
        (Err(operation), Err(cleanup)) => {
            return Err(invalid(format!(
                "{operation}; qualification cleanup unresolved: {cleanup}"
            )));
        }
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };
    proof.verify(&pool).await?;
    Ok(Arc::new(QualifiedSourceInventoryUsage {
        combination: binding.combination().to_vec(),
        association: binding.association().exact_bytes().to_owned(),
        worker,
        proof,
    }))
}

struct QualificationRequest<'a> {
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
}
impl QualificationRequest<'_> {
    async fn observe(self) -> Result<InventoryQualification, RebornBuildError> {
        let Self {
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
        } = self;
        let recipe_id = program.inputs().instruction().recipe().uuid;
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
        let intent = Uuid::new_v5(&recipe_id, b"inventory-qualification-intent/1");
        let behavior = Uuid::new_v5(&intent, b"actual-observations");
        let mut context = ExecutionContext::local_default(
            admitted
                .host
                .run_context()
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
        context.tenant_id = admitted.host.run_context().scope.tenant_id.clone();
        context.agent_id = admitted.host.run_context().scope.agent_id.clone();
        context.project_id = admitted.host.run_context().scope.project_id.clone();
        context.thread_id = Some(admitted.host.run_context().thread_id.clone());
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
        let over_capacity = preparation
            .bounds
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(|| invalid("inventory fixture source bound overflows"))?;
        let header = json!({"format":"contained-inventory-qualification/1","qualification_id":qualification_id,"recipe_selection":program.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
        "association":binding.association().exact_bytes(),"worker_checksum":hex::encode(worker.checksum()),"native":native,
        "run_id":admitted.host.run_context().run_id,"primitive_invocation":context.invocation_id,"sources":sources,
        "invalid_primitive_invocation":invalid_context.invocation_id,"invalid_arguments":{"sources":{"":""}},
        "denied_primitive_invocation":denied_context.invocation_id,
        "capacity_primitive_invocation":capacity_context.invocation_id,"capacity_fixture":{"sources":{"valid":"","over_capacity":{"repeat":"x","count":over_capacity}}},
        "preparation_bounds":preparation.bounds});
        let components: Vec<_> = binding.combination().iter().map(|r|json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect();
        let association_checksum =
            crate::bootstrap_evidence::checksum(binding.association().exact_bytes());
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
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(invalid)?;
        if !crate::bootstrap_evidence::retain_intent(&tx, intent, &intent_bytes).await? {
            return Err(invalid(
                "inventory original qualification requires reconciliation, no replay",
            ));
        }
        tx.commit().await.map_err(invalid)?;
        drop(client);
        // The native primitive is exercised independently, before its binding
        // or associated PythonCode can be considered qualified.
        let native_result = admitted
            .host
            .invoke_retained_tool(
                &primitive,
                RuntimeCapabilityRequest::new(
                    context,
                    primitive.descriptor().id.clone(),
                    ResourceEstimate::default(),
                    json!({"sources":sources}),
                    TrustDecision {
                        effective_trust: EffectiveTrustClass::user_trusted(),
                        authority_ceiling: AuthorityCeiling {
                            allowed_effects: primitive.descriptor().effects.clone(),
                            max_resource_ceiling: None,
                        },
                        provenance: TrustProvenance::AdminConfig,
                        evaluated_at: chrono::Utc::now(),
                    },
                ),
            )
            .await
            .map_err(invalid)?;
        let RuntimeCapabilityOutcome::Completed(native_result) = native_result else {
            return Err(invalid("native inventory prerequisite did not complete"));
        };
        let native_report = strict_json(
            native_result
                .output
                .as_str()
                .ok_or_else(|| invalid("native inventory report is not text"))?,
            REVISION_LIMITS,
        )
        .map_err(invalid)?;
        require_fixture(&native_report, &sources, worker.checksum())?;
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
        let invalid_outcome = match primitive
            .invoke(primitive_request(
                invalid_context,
                json!({"sources":{"":""}}),
            ))
            .await
        {
            Ok(RuntimeCapabilityOutcome::Failed(failed))
                if failed.kind == brassclaw_host_runtime::RuntimeFailureKind::InvalidInput =>
            {
                "invalid_input"
            }
            Err(brassclaw_host_runtime::HostRuntimeError::InvalidRequest { .. }) => {
                "invalid_request"
            }
            _ => {
                return Err(invalid(
                    "invalid inventory fixture was not rejected at native input boundary",
                ));
            }
        };
        match primitive
            .invoke(primitive_request(
                capacity_context,
                json!({"sources":{"valid":"","over_capacity":"x".repeat(over_capacity)}}),
            ))
            .await
            .map_err(invalid)?
        {
            RuntimeCapabilityOutcome::Failed(failed)
                if failed.kind == brassclaw_host_runtime::RuntimeFailureKind::Resource => {}
            _ => {
                return Err(invalid(
                    "over-capacity inventory must reject the whole operation without partial facts",
                ));
            }
        }
        let denied = crate::installed_monty_catalogue::bootstrap_builtin_runtime(
            &pool,
            &kernel,
            &admitted.host,
            &program,
            primitive_binding,
            true,
        )?;
        match denied
            .invoke(primitive_request(
                denied_context,
                json!({"sources":sources}),
            ))
            .await
            .map_err(invalid)?
        {
            RuntimeCapabilityOutcome::Failed(failed)
                if failed.kind == brassclaw_host_runtime::RuntimeFailureKind::Authorization => {}
            _ => {
                return Err(invalid(
                    "inventory restrictive policy did not deny the native fixture",
                ));
            }
        }
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
                prerequisites: json!({"header":header,"primitive":native_report,"invalid_primitive":invalid_outcome,"capacity_primitive":"resource","denied_primitive":"authorization","binding_native":native,"step":step}),
            },
        });
        let root = crate::global_root_seed::retain_packaged_global_root(
            pool.clone(),
            crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
        )
        .await?;
        let observed = crate::bootstrap_recipe_flow::execute(
            admitted,
            crate::bootstrap_recipe_flow::Request::contained(
                worker.path(),
                inspected.preparation_limits(),
                &root,
                source.clone(),
            )?,
        )
        .await?;
        if !matches!(
            observed.receipt.outcome,
            TaskOutcome::InternalCompleted { .. }
        ) || observed.exit.failure.is_some()
            || observed.exit.pending_admission.is_some()
            || !observed.exit.queued_admissions.is_empty()
            || admitted.host.finalized_reply_ref().is_some()
            || observed.report["withheld_root_answers"] != 0
            || observed.report["withheld_host_answers"] != false
            || observed.report["model_exchange_count"] != 0
            || !observed.report["reply"].is_null()
            || observed.report["semantic_approval"] != false
            || observed.report["catalogue_activation"] != false
        {
            return Err(invalid(
                "inventory usage qualification did not close cleanly",
            ));
        }
        let exit = observed
            .exit
            .transport
            .map_err(|error| invalid(format!("inventory containment: {error:?}")))?;
        if exit.exit_status.is_none()
            || exit.containment_error.is_some()
            || exit.reap_error.is_some()
            || exit.shutdown_failure.is_some()
        {
            return Err(invalid("inventory worker containment remains unresolved"));
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
    json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,"validation_mode":"system_seed",
        "association_checksum":association,"components":components,"succeeded":succeeded,"reviewed_evidence":[],"report":report}).to_string()
}
struct InventoryProofContext {
    intent: Uuid,
    intent_bytes: String,
    behavior: Uuid,
    association_checksum: String,
    components: Vec<Value>,
    prerequisites: Value,
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
    proof: Mutex<Option<InventoryQualification>>,
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
        let proof = InventoryQualification {
            intent: context.intent,
            intent_bytes: context.intent_bytes.clone(),
            behavior: context.behavior,
            behavior_bytes: evidence_record(
                context.behavior,
                "behavior",
                true,
                &context.association_checksum,
                &context.components,
                json!({"prerequisites":context.prerequisites,"usage":report,"task_handle":execution.task_handle(),"input_checksum":hex::encode(observed.input_checksum()),"arguments_checksum":observed.arguments_checksum().map(hex::encode),"answer_checksum":observed.answer_checksum().map(hex::encode),"result_checksum":observed.result_checksum().map(hex::encode),"preterminal_observation":true,"settled_approval":false}),
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
