//! Exact W/selected source-workflow qualification. Reuses the installation
//! observer's original native custody; no candidate export is invoked here.
use super::*;
use crate::bootstrap_validator_approval::CheckedPureCheckerClosure;
use brassclaw_skills::revision_store::PgComponentRevisionStore;

struct CheckedCase {
    context: Arc<Context>,
    claim: Value,
    expected_rules: Vec<Value>,
    completion: String,
    report: String,
}
pub(crate) struct CheckedPureSourceWorkflow {
    cases: Vec<CheckedCase>,
    report: String,
}
impl CheckedCase {
    fn encode(&self, id: Uuid, success: bool, value: &Value) -> String {
        let components = self
            .context
            .program
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|r| revision_reference_value(r.reference()))
            .collect::<Vec<_>>();
        evidence::system_record(
            id,
            "behavior",
            success,
            &evidence::checksum(&self.claim.to_string()),
            &components,
            value.clone(),
        )
    }
    fn claim_id(&self) -> Uuid {
        self.context.id("pure-source-workflow-claim")
    }
    fn report_id(&self) -> Uuid {
        self.context.id("pure-source-workflow-report")
    }
    async fn report_value(&self, tx: &Transaction<'_>) -> Result<Value, RebornBuildError> {
        let context = &self.context;
        let (completion, _) = context
            .read_completed(tx)
            .await?
            .ok_or_else(|| invalid("pure source workflow original completion missing"))?;
        if completion != self.completion {
            return Err(invalid("pure workflow completion changed"));
        }
        let (pre_bytes, pre) = context
            .read::<Preterminal>(tx, context.id("preterminal"))
            .await?
            .ok_or_else(|| invalid("pure workflow original observations missing"))?;
        let receipt = strict_json(&pre.receipt, REVISION_LIMITS).map_err(invalid)?;
        let results = receipt["results"]
            .as_object()
            .ok_or_else(|| invalid("pure workflow results missing"))?;
        if results.keys().map(String::as_str).collect::<Vec<_>>()
            != ["0:1", "0:3", "0:4", "0:5", "0:6", "0:7", "0:8", "0:9"]
        {
            return Err(invalid("pure workflow exact result coverage differs"));
        }
        let mut sources = serde_json::Map::new();
        let subject = context.subject.value();
        for entry in std::iter::once(&subject["candidate"]).chain(
            subject["dependencies"]
                .as_array()
                .ok_or_else(|| invalid("pure workflow subject dependencies missing"))?,
        ) {
            sources.insert(
                entry["reference"]["uuid"]
                    .as_str()
                    .ok_or_else(|| invalid("pure workflow source identity missing"))?
                    .to_owned(),
                entry["revision"]["document"]["content"].clone(),
            );
        }
        if results["0:1"] != Value::Object(sources) || !results["0:3"].is_string() {
            return Err(invalid("pure workflow original source/inventory differs"));
        }
        for (rule, step) in self.expected_rules.iter().zip(["0:4", "0:5", "0:6", "0:7"]) {
            if results[step] != *rule {
                return Err(invalid(format!(
                    "pure workflow {} differs from fixed oracle",
                    step
                )));
            }
        }
        let mut collected = self.expected_rules.clone();
        collected.sort_by(|a, b| a["rule_id"].as_str().cmp(&b["rule_id"].as_str()));
        let errors = collected
            .iter()
            .filter(|r| r["ok"] == false)
            .map(|r| json!({"rule_id":r["rule_id"],"errors":r["errors"]}))
            .collect::<Vec<_>>();
        let aggregation = json!({"pass":errors.is_empty(),"errors":errors});
        if results["0:8"] != json!(collected) || results["0:9"] != aggregation {
            return Err(invalid(
                "pure workflow collect/aggregate differs from fixed oracle",
            ));
        }
        // read_completed authenticates every original typed input/return digest,
        // actual native Tool journal, quiescence and closed Turn before this oracle.
        Ok(
            json!({"format":"pure-checker-workflow-fixture-observation/1","scope":"installation-pure-source-workflow-only",
            "claim":reference(self.claim_id(),&self.encode(self.claim_id(),false,&self.claim)),
            "source":context.subject.reference(),"recipe":revision_reference_value(context.program.inputs().instruction().recipe()),
            "variant":"selected","selection":context.selection.reference(),"executions":[reference(context.id("completed"),&completion)],
            "coverage":self.claim["coverage"],"native_inventory_observations":[reference(context.id("preterminal"),&pre_bytes)],
            "outcome":"observed_expected","diagnostics":[]}),
        )
    }
    async fn verify(
        &self,
        tx: &Transaction<'_>,
        prerequisites: &[Value],
    ) -> Result<(), RebornBuildError> {
        let claim = evidence::read_evidence(tx, self.claim_id())
            .await?
            .ok_or_else(|| invalid("pure workflow claim missing"))?;
        if claim != self.encode(self.claim_id(), false, &self.claim)
            || self.claim["prerequisites"] != json!(prerequisites)
        {
            return Err(invalid("pure workflow original claim/prerequisites differ"));
        }
        let expected = self.report_value(tx).await?;
        let bytes = evidence::read_evidence(tx, self.report_id())
            .await?
            .ok_or_else(|| invalid("pure workflow report missing"))?;
        if bytes != self.report || bytes != self.encode(self.report_id(), false, &expected) {
            return Err(invalid("pure workflow original report differs"));
        }
        Ok(())
    }
}
impl CheckedPureSourceWorkflow {
    pub(crate) fn program(&self) -> Result<Arc<RetainedToolProgram>, RebornBuildError> {
        Ok(self
            .cases
            .first()
            .ok_or_else(|| invalid("qualified W missing"))?
            .context
            .program
            .clone())
    }
    pub(crate) fn parser(&self) -> Result<Arc<QualifiedSourceInventoryUsage>, RebornBuildError> {
        Ok(self
            .cases
            .first()
            .ok_or_else(|| invalid("qualified parser missing"))?
            .context
            .parser
            .clone())
    }
    /// Passive pins derived only after re-reading the complete original F01
    /// proof and every independently qualified prerequisite in this snapshot.
    pub(crate) async fn source_qualification(
        &self,
        tx: &Transaction<'_>,
        checkers: &CheckedPureCheckerClosure,
    ) -> Result<
        brassclaw_skills::review_submission_store::selection::SourceQualification,
        RebornBuildError,
    > {
        let prerequisites = self.verify_with_prerequisites(tx, checkers).await?;
        let first = self
            .cases
            .first()
            .ok_or_else(|| invalid("qualified W missing"))?;
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_phase("parser_proof", None);
        first.context.parser.proof.verify_in_transaction(tx).await?;
        let program = self.program()?;
        let recipe = &program.inputs().instruction().snapshot().revisions()
            [&program.inputs().instruction().recipe().uuid];
        let mut prerequisite_proofs = prerequisites
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(invalid))
            .collect::<Result<Vec<_>, _>>()?;
        let parser = first.context.parser.proof.record_reference();
        prerequisite_proofs.push(
            brassclaw_skills::review_submission_store::selection::SourceProofReference {
                id: parser.id,
                checksum: parser.checksum,
            },
        );
        let qualification =
            brassclaw_skills::review_submission_store::selection::SourceQualification {
                profile: "pure_source_structure".into(),
                workflow_proof:
                    brassclaw_skills::review_submission_store::selection::SourceProofReference {
                        id: self.report_id()?,
                        checksum: evidence::checksum(&self.report),
                    },
                prerequisite_proofs,
                required_checks_checksum: evidence::checksum(
                    &recipe.draft().document()["validator_coverage"].to_string(),
                ),
                worker_checksum: hex::encode(first.context.worker.checksum()),
                package_checksum: evidence::checksum(&first.claim["base"]["package"].to_string()),
            };
        qualification.validate().map_err(invalid)?;
        Ok(qualification)
    }
    fn claim_value(&self) -> Result<Value, RebornBuildError> {
        let first = self
            .cases
            .first()
            .ok_or_else(|| invalid("complete workflow fixture plan required"))?;
        Ok(
            json!({"format":"pure-checker-workflow-claim/1","scope":"installation-pure-source-workflow-only",
            "base":{"fixture_claims":self.cases.iter().map(|c|reference(c.claim_id(),&c.encode(c.claim_id(),false,&c.claim))).collect::<Vec<_>>()},
            "recipe":first.claim["recipe"],"variant":"selected","selection":first.context.selection.reference(),
            "subject_closure":first.claim["subject_closure"],"coverage":self.cases.iter().map(|c|c.claim["coverage"].clone()).collect::<Vec<_>>(),
            "profile":first.claim["profile"],"prerequisites":first.claim["prerequisites"]}),
        )
    }
    fn claim_id(&self) -> Result<Uuid, RebornBuildError> {
        Ok(Uuid::new_v5(
            &packaged_component_id("validator:python-pure-structure:recipe"),
            self.claim_value()?.to_string().as_bytes(),
        ))
    }
    fn report_id(&self) -> Result<Uuid, RebornBuildError> {
        Ok(Uuid::new_v5(
            &self.claim_id()?,
            b"qualified-workflow-report",
        ))
    }
    fn record(&self, id: Uuid, success: bool, value: Value) -> Result<String, RebornBuildError> {
        let first = self
            .cases
            .first()
            .ok_or_else(|| invalid("workflow plan missing"))?;
        let components = first
            .context
            .program
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|r| revision_reference_value(r.reference()))
            .collect::<Vec<_>>();
        Ok(evidence::system_record(
            id,
            "behavior",
            success,
            &evidence::checksum(&self.claim_value()?.to_string()),
            &components,
            value,
        ))
    }
    fn report_value(&self) -> Result<Value, RebornBuildError> {
        let claim = self.claim_value()?;
        let first = self
            .cases
            .first()
            .ok_or_else(|| invalid("workflow plan missing"))?;
        let native_observations = self
            .cases
            .iter()
            .map(|case| {
                let value = strict_json(&case.report, REVISION_LIMITS).map_err(invalid)?;
                value["report"]["native_inventory_observations"]
                    .as_array()
                    .and_then(|a| a.first())
                    .cloned()
                    .ok_or_else(|| {
                        invalid("original workflow native inventory observation missing")
                    })
            })
            .collect::<Result<Vec<_>, RebornBuildError>>()?;
        Ok(
            json!({"format":"pure-checker-workflow-report/1","scope":"installation-pure-source-workflow-only",
            "claim":reference(self.claim_id()?,&self.record(self.claim_id()?,false,claim.clone())?),
            "source":first.context.selection.reference(),"executions":self.cases.iter().map(|c|reference(c.context.id("completed"),&c.completion)).collect::<Vec<_>>(),
            "outcome":"qualified","diagnostics":[],"recipe":claim["recipe"],"variant":"selected","selection":claim["selection"],
            "coverage":claim["coverage"],"native_inventory_observations":native_observations}),
        )
    }
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
        checkers: &CheckedPureCheckerClosure,
    ) -> Result<(), RebornBuildError> {
        self.verify_with_prerequisites(tx, checkers)
            .await
            .map(|_| ())
    }
    /// Reuse only facts authenticated by this exact invocation/snapshot. These
    /// references are not a cache or an authority usable by another transaction.
    async fn verify_with_prerequisites(
        &self,
        tx: &Transaction<'_>,
        checkers: &CheckedPureCheckerClosure,
    ) -> Result<Vec<Value>, RebornBuildError> {
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_phase("checker_prerequisites", None);
        let prerequisites = checkers.proof_references(tx).await?;
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_phase("workflow_plan", None);
        for case in &self.cases {
            checkers.require_program(&case.context.program)?;
        }
        let gallery = crate::bootstrap_validator_approval::pure_source_cases()?;
        if gallery.len() != self.cases.len() {
            return Err(invalid("complete pure workflow gallery required"));
        }
        for (case, fixture) in self.cases.iter().zip(gallery) {
            #[cfg(all(test, feature = "skills-db"))]
            crate::pg_review_submission_io::projection_phase(
                "workflow_case",
                self.cases
                    .iter()
                    .position(|selected| std::ptr::eq(selected, case)),
            );
            if case.context.subject.candidate().uuid != fixture.root.uuid()
                || case.expected_rules != fixture.rules
            {
                return Err(invalid("pure workflow fixture subject/oracle differs"));
            }
            case.verify(tx, &prerequisites).await?;
        }
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_phase("workflow_report", None);
        let claim_id = self.claim_id()?;
        let claim = evidence::read_evidence(tx, claim_id)
            .await?
            .ok_or_else(|| invalid("whole workflow original claim missing"))?;
        let report = evidence::read_evidence(tx, self.report_id()?)
            .await?
            .ok_or_else(|| invalid("whole workflow original report missing"))?;
        if claim != self.record(claim_id, false, self.claim_value()?)?
            || report != self.report
            || report != self.record(self.report_id()?, true, self.report_value()?)?
        {
            return Err(invalid("whole workflow original qualification differs"));
        }
        Ok(prerequisites)
    }
}
pub(crate) async fn qualify_pure_workflow(
    observations: &InstallationSourceObservations,
    checkers: &CheckedPureCheckerClosure,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
) -> Result<CheckedPureSourceWorkflow, RebornBuildError> {
    observations.require_positive_for_qualification()?;
    let InstallationSourceObservations::ObservedPositive(profiles) = observations else {
        return Err(invalid("pure workflow actual parser prerequisite missing"));
    };
    let base = profiles
        .iter()
        .find(|p| p.context.selection.profile() == store::InstallationProfile::Pure)
        .ok_or_else(|| invalid("pure workflow original W selection missing"))?
        .context
        .clone();
    checkers.require_program(&base.program)?;
    // Complete the owner's two-phase custody before acquiring W's RR client.
    base.parser.proof.verify(&base.pool).await?;
    let prerequisites = io::bounded_for(
        base.storage.as_ref(),
        "pure_workflow_prerequisites",
        base.program.inputs().instruction().recipe().uuid,
        async {
            let mut client = io::acquire(&base.pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            observations.verify_in_transaction(&tx).await?;
            let proofs = checkers.proof_references(&tx).await?;
            base.parser.proof.verify_in_transaction(&tx).await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(proofs)
        },
    )
    .await?;
    let revisions = PgComponentRevisionStore::new(base.pool.clone());
    let mut cases = Vec::new();
    for fixture in crate::bootstrap_validator_approval::pure_source_cases()? {
        let mut refs = Vec::new();
        for draft in std::iter::once(&fixture.root).chain(&fixture.dependencies) {
            refs.push(
                io::bounded_for(
                    base.storage.as_ref(),
                    "pure_workflow_fixture_retain",
                    draft.uuid(),
                    async {
                        revisions
                            .retain_packaged_draft(draft)
                            .await
                            .map_err(invalid)
                    },
                )
                .await?,
            );
        }
        let graph = io::bounded_for(
            base.storage.as_ref(),
            "pure_workflow_fixture_snapshot",
            fixture.root.uuid(),
            async {
                revisions
                    .read_exact(
                        &[fixture.root.uuid()],
                        &refs,
                        brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                    )
                    .await
                    .map_err(invalid)
            },
        )
        .await?;
        let candidate = graph.revisions()[&fixture.root.uuid()].reference();
        let mut package = base.subject.package_context().clone();
        package.factory_checksum=evidence::checksum(&json!({"owner":evidence::checksum(include_str!("bootstrap_pure_workflow.rs")),
            "gallery":evidence::checksum(include_str!("bootstrap_pure_source_cases.rs")),"original":package.factory_checksum,"checker_prerequisites":prerequisites}).to_string());
        let subject_draft =
            store::pure_fixture_subject_record(&graph, candidate, package, REVISION_LIMITS)
                .map_err(invalid)?;
        retain_draft(&base.pool, base.storage.as_ref(), &subject_draft).await?;
        let (subject, selection) = io::bounded_for(
            base.storage.as_ref(),
            "pure_workflow_subject_read",
            subject_draft.reference().id,
            async {
                let mut client = io::acquire(&base.pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let subject = store::read_subject_in_transaction(
                    &tx,
                    subject_draft.reference().id,
                    REVISION_LIMITS,
                )
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("pure fixture original subject missing"))?;
                let selection = store::selection_record(
                    &subject,
                    base.selection.validator_bytes(),
                    store::InstallationProfile::Pure,
                    REVISION_LIMITS,
                )
                .map_err(invalid)?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok((Arc::new(subject), selection))
            },
        )
        .await?;
        retain_draft(&base.pool, base.storage.as_ref(), &selection).await?;
        let selection = io::bounded_for(
            base.storage.as_ref(),
            "pure_workflow_selection_read",
            selection.reference().id,
            async {
                let mut client = io::acquire(&base.pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let saved = store::read_selection_in_transaction(
                    &tx,
                    selection.reference().id,
                    REVISION_LIMITS,
                )
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("pure fixture original selection missing"))?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok(Arc::new(saved))
            },
        )
        .await?;
        let operation = Uuid::new_v5(&selection.reference().id, b"pure-checker-workflow-original");
        let context = Arc::new(Context {
            pool: base.pool.clone(),
            storage: base.storage.clone(),
            kernel: base.kernel.clone(),
            worker: base.worker.clone(),
            parser: base.parser.clone(),
            subject,
            selection,
            program: base.program.clone(),
            root: base.root.clone(),
            operation,
        });
        let claim = json!({"format":"pure-checker-workflow-fixture/1","scope":"installation-pure-source-workflow-only",
            "base":{"package":context.subject.package_context(),"worker_checksum":hex::encode(context.worker.checksum()),"global_root":revision_reference_value(context.root.reference()),
                "fixture_subject":context.subject.reference(),"original_operation_id":operation},
            "recipe":revision_reference_value(context.program.inputs().instruction().recipe()),"variant":"selected","selection":context.selection.reference(),
            "subject_closure":context.program.inputs().instruction().snapshot().revisions().values().map(|r|revision_reference_value(r.reference())).collect::<Vec<_>>(),
            "coverage":{"case_id":fixture.name,"rules":fixture.rules,"native_result_steps":["0:1","0:3","0:4","0:5","0:6","0:7","0:8","0:9","0:10"]},
            "profile":brassclaw_engine::executor::retained_profile::profile(),"prerequisites":prerequisites});
        let checked = CheckedCase {
            context,
            claim,
            expected_rules: fixture.rules,
            completion: String::new(),
            report: String::new(),
        };
        let claim_bytes = checked.encode(checked.claim_id(), false, &checked.claim);
        io::retain_record(
            &base.pool,
            base.storage.as_ref(),
            checked.claim_id(),
            &claim_bytes,
        )
        .await?;
        cases.push(checked);
    }
    let mut package = CheckedPureSourceWorkflow {
        cases,
        report: String::new(),
    };
    let claim_id = package.claim_id()?;
    io::retain_record(
        &base.pool,
        base.storage.as_ref(),
        claim_id,
        &package.record(claim_id, false, package.claim_value()?)?,
    )
    .await?;
    // Every subject/selection/oracle and the complete W fixture plan are durable
    // before the first W invocation. Lost replies read these same originals.
    for checked in &mut package.cases {
        checked.completion = match checked.context.read_original().await? {
            Some((bytes, _)) => bytes,
            None => {
                checked
                    .context
                    .execute(executor.clone(), preparation)
                    .await?
                    .0
            }
        };
        let report = io::bounded_for(
            base.storage.as_ref(),
            "pure_workflow_case_report",
            checked.context.subject.candidate().uuid,
            async {
                let mut client = io::acquire(&base.pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let report = checked.report_value(&tx).await?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok(report)
            },
        )
        .await?;
        checked.report = checked.encode(checked.report_id(), false, &report);
        io::retain_record(
            &base.pool,
            base.storage.as_ref(),
            checked.report_id(),
            &checked.report,
        )
        .await?;
    }
    package.report = package.record(package.report_id()?, true, package.report_value()?)?;
    io::retain_record(
        &base.pool,
        base.storage.as_ref(),
        package.report_id()?,
        &package.report,
    )
    .await?;
    Ok(package)
}

#[cfg(test)]
pub(crate) async fn assert_original_pure_workflow_readback(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    pool: Arc<PgPool>,
    checkers: &CheckedPureCheckerClosure,
) {
    let RetainedProgram::Tools(reply) = catalogue.reply.program() else {
        panic!("actual reply context")
    };
    let mut package = crate::source_inspection_components::retain(
        pool.clone(),
        catalogue.kernel.as_ref(),
        &catalogue.qualification_scope.tenant_id,
        reply,
    )
    .await
    .unwrap();
    package
        .definitions
        .extend(
            crate::builtin_bootstrap::retain_validator_structure_drafts(&pool, package.usage)
                .await
                .unwrap(),
        )
        .unwrap();
    let preparation = catalogue.capture_task_preparation().await.unwrap();
    let original = observe_installation(
        pool.clone(),
        catalogue.kernel.clone(),
        InventoryExecutor {
            worker: catalogue.worker_image.clone(),
            configuration: catalogue.startup_executor.clone(),
            root: catalogue.root_definition.clone(),
        },
        package.definitions,
        reply,
        preparation.limits(),
        catalogue.resource_store_settings.clone(),
    )
    .await
    .unwrap();
    let mut proof = qualify_pure_workflow(
        &original,
        checkers,
        catalogue.startup_executor.clone(),
        preparation.limits(),
    )
    .await
    .unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    proof.verify_in_transaction(&tx, checkers).await.unwrap();
    assert_eq!(proof.cases.len(), 29);
    // A caller cannot qualify W by retaining only a positive subset, relabeling
    // a scope, or replacing a full rule result with the same pass boolean.
    let first = proof.cases[0].expected_rules[0].clone();
    proof.cases[0].expected_rules[0]["errors"] = json!(["substituted diagnostic"]);
    assert!(proof.verify_in_transaction(&tx, checkers).await.is_err());
    proof.cases[0].expected_rules[0] = first;
    let scope = proof.cases[0].claim["scope"].clone();
    proof.cases[0].claim["scope"] = json!("general-pure-approval");
    assert!(proof.verify_in_transaction(&tx, checkers).await.is_err());
    proof.cases[0].claim["scope"] = scope;
    proof.cases.pop();
    assert!(proof.verify_in_transaction(&tx, checkers).await.is_err());
    tx.commit().await.unwrap();
}
