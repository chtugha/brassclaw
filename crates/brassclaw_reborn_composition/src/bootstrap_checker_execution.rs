//! Actual fixed Recipe execution and original terminal custody, not a Rust
//! checking workflow. Monty runs the fixture step order from immutable Recipes.
use super::*;
use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use async_trait::async_trait;
use brassclaw_engine::memory::retained_validator::ValidatorInputFingerprint;

impl Context {
    pub(super) fn verify_observed(
        &self,
        suite: &Suite,
        observed: &Observed,
    ) -> Result<(), RebornBuildError> {
        if suite.python_stop != suite.expected_python_line.is_some() {
            return Err(invalid("checker failure expectation is incomplete"));
        }
        if observed.format != "structural-checker-observed/1"
            || observed.task.is_nil()
            || observed.steps.len() != suite.steps.len()
        {
            return Err(invalid("checker observation incomplete"));
        }
        let program = self.program(&suite.variant)?;
        let results = if suite.python_stop {
            if observed.receipt.is_some() || suite.steps.len() != 1 {
                return Err(invalid("Python stop cannot have a successful receipt"));
            }
            BTreeMap::new()
        } else {
            let receipt = observed
                .receipt
                .as_deref()
                .ok_or_else(|| invalid("checker receipt absent"))?;
            if let Some(expected) = &suite.expected_receipt {
                let actual = strict_json(receipt, self.limits()?).map_err(invalid)?;
                if &actual != expected {
                    return Err(invalid("independent receipt-helper oracle differs"));
                }
                BTreeMap::from([(suite.steps[0].id.clone(), json!(receipt))])
            } else {
                let expected = suite.steps[..suite.steps.len() - 1]
                    .iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>();
                crate::pg_review_admission_source::recorded_receipt_results(
                    crate::pg_review_admission_source::ValidatorReceiptProfile::Inventory,
                    receipt,
                    &expected,
                    &suite
                        .steps
                        .last()
                        .ok_or_else(|| invalid("fixture has no final step"))?
                        .id,
                    self.limits()?,
                )
                .map_err(invalid)?
            }
        };
        let mut fingerprints = BTreeMap::new();
        let limits = self.limits()?;
        for (fact, expected) in observed.steps.iter().zip(&suite.steps) {
            let captured = ContractLimits {
                max_depth: fact.max_depth,
                max_nodes: fact.max_nodes,
                max_bytes: fact.max_bytes,
            };
            if fact.id != expected.id
                || captured.max_depth != limits.max_depth
                || captured.max_nodes != limits.max_nodes
                || captured.max_bytes != limits.max_bytes
            {
                return Err(invalid("checker step order/original capacity differs"));
            }
            let checksum: [u8; 32] = hex::decode(&fact.input_checksum)
                .map_err(invalid)?
                .try_into()
                .map_err(|_| invalid("checker input checksum invalid"))?;
            fingerprints.insert(
                fact.id.clone(),
                ValidatorInputFingerprint {
                    checksum,
                    limits: captured,
                },
            );
            if suite.python_stop {
                if fact.result_checksum.is_some() {
                    return Err(invalid("failed checker has a successful result"));
                }
                // The pinned worker emits MontyException::Display verbatim in
                // ProcessError.diagnostic. Require its exact terminal exception
                // line as well as VmFailure::Python and the actual Start frame;
                // a NameError or unrelated Python failure is not this fixture.
                let actual_final_exception = fact
                    .python_diagnostic
                    .as_deref()
                    .and_then(|d| d.lines().last());
                if actual_final_exception != suite.expected_python_line.as_deref() {
                    // These are closed shipped fixtures with fixed inputs, no
                    // host calls or secrets. Expose only their final exception
                    // line, never the native traceback or executable source.
                    return Err(invalid(format!(
                        "checker native Python exception differs from expected stop: {}",
                        json!({
                            "root": self.claim.root.uuid,
                            "variant": suite.variant,
                            "expected_final_exception": suite.expected_python_line,
                            "actual_final_exception": actual_final_exception,
                        })
                    )));
                }
                let bytes = fact
                    .python_failure_command
                    .as_deref()
                    .ok_or_else(|| invalid("expected Python failure lacks original command"))?;
                let command: WorkerCommand =
                    serde_json::from_value(strict_json(bytes, REVISION_LIMITS).map_err(invalid)?)
                        .map_err(invalid)?;
                let WorkerCommand::Recipe {
                    command:
                        RecipeCommand::Start {
                            selected, inputs, ..
                        },
                } = command
                else {
                    return Err(invalid(
                        "expected Python failure was not actual export invocation",
                    ));
                };
                let invocation =
                    brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
                        program.inputs().instruction().snapshot().revisions()[&expected.component]
                            .draft()
                            .document(),
                        program.inputs().instruction().snapshot().revisions()[&expected.component]
                            .draft()
                            .dependencies(),
                    )
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("failed fixture preload absent"))?
                    .invocation(
                        &expected.export,
                        &program.inputs().instruction().snapshot().revisions()[&expected.component]
                            .draft()
                            .document()["input_contract"],
                    )
                    .map_err(invalid)?;
                let checksum: [u8; 32] = Sha256::digest(invocation.as_bytes()).into();
                if selected.source != invocation
                    || selected.checksum != checksum
                    || !selected.aliases.is_empty()
                    || program
                        .inputs()
                        .bind_step_inputs_with_limits(&fact.id, &inputs, captured)
                        .map_err(invalid)?
                        != inputs
                    || hex::encode(
                        typed_value_checksum_with_limits(&inputs, captured).map_err(invalid)?,
                    ) != fact.input_checksum
                {
                    return Err(invalid("Python failure belongs to different export/input"));
                }
            } else {
                if fact.python_failure_command.is_some() || fact.python_diagnostic.is_some() {
                    return Err(invalid("completed checker has failed-command evidence"));
                }
                let result = results
                    .get(&fact.id)
                    .ok_or_else(|| invalid("checker whole step result missing"))?;
                program
                    .inputs()
                    .validate_step_result_with_limits(&fact.id, result, captured)
                    .map_err(invalid)?;
                if fact.result_checksum
                    != Some(hex::encode(
                        typed_value_checksum_with_limits(result, captured).map_err(invalid)?,
                    ))
                {
                    return Err(invalid("checker actual result fingerprint differs"));
                }
            }
        }
        let order = suite
            .steps
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>();
        program
            .inputs()
            .verify_recorded_input_fingerprints(&json!({}), &order, &results, &fingerprints)
            .map_err(invalid)?;
        Ok(())
    }
    pub(super) async fn execute(self: &Arc<Self>) -> Result<(), RebornBuildError> {
        let preparation = self.preparation()?;
        let worker = crate::pg_review_admission_source::verified_worker(
            &self.worker,
            preparation.limits().max_native_artifact_bytes,
        )
        .await?;
        let first = self.program(&self.claim.suites[0].variant)?;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(first.clone()),
                &worker.path(),
                preparation.limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let pure = inspected
            .pure_component(self.claim.root.uuid)
            .map_err(invalid)?;
        let source = SourceProof {
            format: "structural-checker-source/1".into(),
            operation: self.op(),
            root: pure.component().into(),
            closure: pure
                .components()
                .values()
                .copied()
                .map(Into::into)
                .collect(),
            exports: pure.export_invocations().clone(),
            sources: inspected
                .source_checks()
                .iter()
                .map(|(id, s)| {
                    Ok((
                        *id,
                        serde_json::to_value(s.observations()).map_err(invalid)?,
                    ))
                })
                .collect::<Result<_, RebornBuildError>>()?,
        };
        // Inspect the receipt prerequisite's entire pure interface as well; it
        // never inherits independent approval from this candidate's closure.
        for id in first
            .inputs()
            .components()
            .values()
            .map(|r| r.uuid)
            .collect::<BTreeSet<_>>()
        {
            inspected.pure_component(id).map_err(invalid)?;
        }
        let bytes = self.retain(self.id("source"), &source).await?;
        let source_ref = reference(self.id("source"), &bytes);
        for suite in &self.claim.suites {
            self.execute_suite(suite, &source_ref).await?;
        }
        Ok(())
    }
    async fn execute_suite(
        self: &Arc<Self>,
        suite: &Suite,
        source_ref: &RecordRef,
    ) -> Result<(), RebornBuildError> {
        let stage = |name: &str| self.id(&format!("{}:{name}", suite.variant));
        let reference_string = format!(
            "structural-checker-fixture:{}:{}",
            self.operation_id, suite.variant
        );
        let admitted = io::bounded(
            self.storage.as_ref(),
            crate::bootstrap_validation::admit(
                self.pool.clone(),
                "structural-checker-own-function",
                &reference_string,
                None,
            ),
        )
        .await?;
        let binding = Admission {
            format: "structural-checker-admission/1".into(),
            operation: self.op(),
            variant: suite.variant.clone(),
            run_id: admitted.attempt.run_id.as_uuid(),
            scope: admitted.chat_context()?.scope.clone(),
            runner_id: admitted.attempt.runner_id.as_uuid(),
            claim_checksum: hex::encode(Sha256::digest(
                serde_json::to_vec(&admitted.attempt.lease_token).map_err(invalid)?,
            )),
        };
        let proof = OnceLock::new();
        let run = async {
            let binding_bytes = self.retain(stage("admission"), &binding).await?;
            let program = self.program(&suite.variant)?;
            let preparation = self.preparation()?;
            let worker = crate::pg_review_admission_source::verified_worker(
                &self.worker,
                preparation.limits().max_native_artifact_bytes,
            )
            .await?;
            let inspected = Arc::new(
                InspectedRetainedProgram::inspect_with_limits(
                    RetainedProgram::Tools(program.clone()),
                    &worker.path(),
                    preparation.limits(),
                )
                .await
                .map_err(invalid)?,
            );
            inspected
                .pure_component(self.claim.root.uuid)
                .map_err(invalid)?;
            let source = Arc::new(CheckerSource {
                context: self.clone(),
                suite: suite.clone(),
                reference: reference_string,
                inspected,
                admission: reference(stage("admission"), &binding_bytes),
                source: source_ref.clone(),
                received: OnceLock::new(),
            });
            let request = crate::bootstrap_recipe_flow::Request::contained(
                worker.path(),
                self.executor()?,
                &self.root,
                source.clone(),
            )?;
            #[cfg(all(test, unix))]
            let request = if super::tests::force_exit(self, suite) {
                request.with_forced_worker_exit()
            } else {
                request
            };
            let observation = crate::bootstrap_recipe_flow::execute(&admitted, request).await?;
            let observed = if suite.python_stop {
                observation
                    .ports
                    .observe_internal_execution(|execution| source.capture(None, execution))
                    .await
                    .map_err(|_| {
                        invalid("checker retained native execution observation unavailable")
                    })??
            } else {
                source
                    .received
                    .get()
                    .ok_or_else(|| invalid("checker preterminal observation absent"))?
                    .clone()
            };
            let actual = if suite.python_stop {
                observation.ended_failure_projection()?
            } else {
                let (actual, native) = observation.quiescence_proof()?;
                proof
                    .set(native)
                    .map_err(|_| invalid("duplicate checker quiescence"))?;
                actual
            };
            let observed_bytes = self.retain(stage("observed"), &observed).await?;
            let quiescence = Quiescence {
                format: "structural-checker-quiescence/1".into(),
                operation: self.op(),
                admission: source.admission.clone(),
                observed: reference(stage("observed"), &observed_bytes),
                actual,
            };
            let quiescence_bytes = self.retain(stage("quiescence"), &quiescence).await?;
            Ok::<_, RebornBuildError>((binding_bytes, observed_bytes, quiescence_bytes))
        }
        .await;
        // Always close original custody after the contained owner has joined.
        // An expected Python stop is never a successful quiescence capability.
        #[cfg(test)]
        let run = match run {
            Ok(records) => super::tests::before_close(self, suite, &admitted)
                .await
                .map(|_| records),
            Err(error) => Err(error),
        };
        let close = match proof.get() {
            Some(proof) => {
                io::bounded(
                    self.storage.as_ref(),
                    crate::bootstrap_validation::settle_quiescent_closed(
                        &self.pool, &admitted, proof,
                    ),
                )
                .await
            }
            None => io::bounded(
                self.storage.as_ref(),
                crate::bootstrap_validation::settle_closed(&admitted),
            )
            .await
            .map(|_| brassclaw_turns::TurnStatus::Completed),
        };
        let close = crate::bootstrap_validation::read_source_close(
            &self.pool,
            &admitted,
            proof.get(),
            close,
            self.storage.as_ref(),
        )
        .await;
        let (binding_bytes, observed_bytes, quiescence_bytes) = match run {
            Ok(records) => records,
            Err(error) => {
                return Err(match close {
                    Ok(_) => error,
                    Err(close) => invalid(format!(
                        "checker fixture failed: {error}; original cleanup: {close}"
                    )),
                });
            }
        };
        let turn = close?;
        if turn.status != brassclaw_turns::TurnStatus::Completed {
            return Err(invalid("cancelled checker fixture cannot qualify"));
        }
        let completed = Completed {
            format: "structural-checker-completed/1".into(),
            operation: self.op(),
            admission: reference(stage("admission"), &binding_bytes),
            observed: reference(stage("observed"), &observed_bytes),
            quiescence: reference(stage("quiescence"), &quiescence_bytes),
            turn: serde_json::to_value(turn).map_err(invalid)?,
        };
        self.retain(stage("completed"), &completed).await?;
        Ok(())
    }
}
struct CheckerSource {
    context: Arc<Context>,
    suite: Suite,
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    admission: RecordRef,
    source: RecordRef,
    received: OnceLock<Observed>,
}
impl CheckerSource {
    fn capture(
        &self,
        receipt: Option<&str>,
        execution: &RetainedRecipeExecution,
    ) -> Result<Observed, RebornBuildError> {
        if execution
            .inspected_program()
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            != self.context.claim.selection_bytes[&self.suite.variant]
        {
            return Err(invalid("checker actual selection differs"));
        }
        let order = execution
            .observation_order()
            .ok_or_else(|| invalid("checker step observations disabled"))?;
        if order.iter().map(String::as_str).collect::<Vec<_>>()
            != self
                .suite
                .steps
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
        {
            return Err(invalid("checker actual step order incomplete"));
        }
        let mut steps = Vec::new();
        for id in order {
            let fact = execution
                .observations()
                .get(id)
                .ok_or_else(|| invalid("checker native step missing"))?;
            if !fact.settled()
                || fact.observation_error().is_some()
                || fact.arguments_checksum().is_some()
                || fact.answer_checksum().is_some()
            {
                return Err(invalid(
                    "checker step is unsettled or has unexpected native dispatch",
                ));
            }
            let (command, diagnostic) = if self.suite.python_stop {
                if fact.failure() != Some(RetainedStepFailure::Process) {
                    return Err(invalid("expected checker stop has unrelated failure"));
                }
                match execution.transport_failure() {
                    Some(RetainedTransportEvidence::Process {
                        error,
                        command: Ok(command),
                        ..
                    }) if matches!(&error.kind, ProcessFailure::Vm(VmFailure::Python)) => (
                        Some(serde_json::to_string(command).map_err(invalid)?),
                        Some(error.diagnostic.clone().ok_or_else(|| {
                            invalid("checker native Python exception diagnostic absent")
                        })?),
                    ),
                    _ => {
                        return Err(invalid(
                            "expected checker Python exception lacks native command; no infrastructure substitution",
                        ));
                    }
                }
            } else {
                if fact.failure().is_some() || !execution.is_complete() {
                    return Err(invalid("checker did not complete"));
                }
                (None, None)
            };
            steps.push(StepFact {
                id: id.clone(),
                input_checksum: hex::encode(fact.input_checksum()),
                result_checksum: fact.result_checksum().map(hex::encode),
                max_depth: fact.data_limits().max_depth,
                max_nodes: fact.data_limits().max_nodes,
                max_bytes: fact.data_limits().max_bytes,
                python_failure_command: command,
                python_diagnostic: diagnostic,
            });
        }
        let observed = Observed {
            format: "structural-checker-observed/1".into(),
            operation: self.context.op(),
            admission: self.admission.clone(),
            source: self.source.clone(),
            variant: self.suite.variant.clone(),
            task: serde_json::from_value(
                serde_json::to_value(execution.task_handle()).map_err(invalid)?,
            )
            .map_err(invalid)?,
            receipt: receipt.map(str::to_owned),
            steps,
        };
        self.context.verify_observed(&self.suite, &observed)?;
        Ok(observed)
    }
}
#[async_trait]
impl InternalRecipeSource for CheckerSource {
    fn name(&self) -> &str {
        "contained-structural-checker-own-function"
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
            tools: None,
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let run = async {
            if selected
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?
                .exact_bytes()
                != self.context.claim.selection_bytes[&self.suite.variant]
            {
                return Err(invalid("checker receipt selection differs"));
            }
            let observed = self.capture(Some(receipt), execution)?;
            self.received
                .set(observed.clone())
                .map_err(|_| invalid("checker receipt repeated"))?;
            self.context
                .retain(
                    self.context.id(&format!("{}:observed", self.suite.variant)),
                    &observed,
                )
                .await?;
            Ok::<_, RebornBuildError>(())
        }
        .await;
        run.map_err(|_| {
            PortFailure::new("checker_native_observation_invalid").expect("constant classification")
        })
    }
}
