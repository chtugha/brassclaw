//! Closed own-checker subjects. This module supplies fixtures and checked scopes
//! to the existing installation session; it is not an ordinary Pure reader.
use super::*;
use brassclaw_skills::component_revision::ComponentRevisionDraft;

#[path = "bootstrap_case_encoder.rs"]
pub(super) mod case_encoder;
#[path = "bootstrap_pure_checker_fixtures.rs"]
mod cases;

#[path = "bootstrap_pure_source_cases.rs"]
mod source_gallery;
#[cfg(test)]
#[path = "bootstrap_pure_checker_tests.rs"]
mod tests;
pub(crate) use source_gallery::source_cases as pure_source_cases;

pub(super) const ENCODER_SLOTS: [&str; 5] = ["text", "sources", "rule", "checks", "aggregation"];
pub(super) fn encoder_id() -> Uuid {
    packaged_component_id("validator:pure-qualification:fixture-encoder")
}
fn witness_id() -> Uuid {
    packaged_component_id("validator:pure-qualification:reader-witness")
}
fn reader_id() -> Uuid {
    packaged_component_id("validator:review-subject-reader")
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Function,
    Support,
    Instrument,
}
impl Scope {
    fn name(self) -> &'static str {
        match self {
            Self::Function => "installation-own-checker-function",
            Self::Support => "installation-private-reader-support",
            Self::Instrument => "installation-pure-fixture-instrument",
        }
    }
    fn format(self, stage: &str) -> String {
        let kind = match self {
            Self::Function => "function",
            Self::Support => "support",
            Self::Instrument => "fixture",
        };
        format!("pure-checker-{kind}-{stage}/1")
    }
}

#[derive(Clone)]
pub(super) struct Profile {
    scope: Scope,
    subject: Uuid,
    drafts: BTreeMap<Uuid, ComponentRevisionDraft>,
    expected: BTreeMap<String, BTreeMap<String, Value>>,
    pub(super) prerequisites: Vec<RecordRef>,
    native_inputs: Vec<(RecordRef, String)>,
}
impl Profile {
    pub(super) async fn verify_native_inputs(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        for (original, bytes) in &self.native_inputs {
            let stored = evidence::read_evidence(tx, original.id)
                .await?
                .ok_or_else(|| invalid("original native fixture facts missing"))?;
            if stored != *bytes || reference(original.id, &stored) != *original {
                return Err(invalid("original native fixture facts changed"));
            }
        }
        Ok(())
    }
    pub(super) fn subject_id(&self) -> Uuid {
        self.subject
    }
    pub(super) fn requires_subject(&self, subject: Uuid) -> bool {
        self.drafts.contains_key(&subject)
    }

    pub(super) fn components(&self, claim: &Claim) -> Result<Vec<Reference>, RebornBuildError> {
        let mut pending = vec![self.subject];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if seen.insert(id) {
                pending.extend(
                    self.drafts
                        .get(&id)
                        .ok_or_else(|| invalid("pure subject dependency missing"))?
                        .dependencies(),
                );
            }
        }
        seen.into_iter()
            .map(|id| {
                claim
                    .components
                    .iter()
                    .find(|r| r.uuid == id)
                    .cloned()
                    .ok_or_else(|| invalid("pure subject revision missing"))
            })
            .collect()
    }
    pub(super) fn subject(&self, claim: &Claim) -> Result<Reference, RebornBuildError> {
        self.components(claim)?
            .into_iter()
            .find(|r| r.uuid == self.subject)
            .ok_or_else(|| invalid("pure qualification subject missing"))
    }
    pub(super) fn claim_bytes(&self, claim: &Claim) -> Result<String, RebornBuildError> {
        let subject = &self.drafts[&self.subject];
        let document = subject.document();
        let interface = json!({"preload":document["preload"],
            "inputs":document.get("input_contract"),"result":document.get("result_contract"),
            "dependencies":subject.dependencies()});
        // Native owners expose exact bytes/digests, not fictitious record UUIDs.
        // The winning reservation supplies execution_request_id separately.
        let pin = |value: Value| {
            let bytes = value.to_string();
            json!({"checksum":evidence::checksum(&bytes),"bytes":bytes})
        };
        let base = json!({"execution_context":claim,
            "instruments":claim.components.iter().filter(|r| r.uuid == encoder_id() || r.uuid == witness_id()).collect::<Vec<_>>(),
            "suites":pin(json!({"variants":claim.suites,"expected_returns":self.expected})),
            "original_native_fixture_inputs":self.native_inputs.iter().map(|(r,_)|r).collect::<Vec<_>>(),
            "factory_checksum":evidence::checksum(include_str!("bootstrap_pure_checker.rs")),
            "fixtures_checksum":evidence::checksum(include_str!("bootstrap_pure_checker_fixtures.rs")),
            "source_gallery_checksum":evidence::checksum(include_str!("bootstrap_pure_source_cases.rs"))});
        let mut value = json!({"format":self.scope.format("claim"),"scope":self.scope.name(),"base":base,
            "subject_closure":self.components(claim)?,"private_signatures":pin(document["preload"]["private_functions"].clone()),
            "prerequisites":self.prerequisites});
        if matches!(self.scope, Scope::Support) {
            let witness = &self.drafts[&claim.root.uuid];
            let doc = witness.document();
            let interface = json!({"preload":doc["preload"],"inputs":doc["input_contract"],"result":doc["result_contract"],"dependencies":witness.dependencies()});
            value["support"] = json!(self.subject(claim)?);
            value["imports"] = document["preload"]["imports"].clone();
            value["constants"] = document["preload"]["constants"].clone();
            value["witness"] = json!(claim.root);
            value["witness_interface_checksum"] = json!(evidence::checksum(&interface.to_string()));
        } else {
            value["root"] = json!(self.subject(claim)?);
            value["interface_checksum"] = json!(evidence::checksum(&interface.to_string()));
            value["exports"] = pin(document["preload"]["exports"].clone());
        }
        brassclaw_skills::value_contract::validate_data_bounds(&value, REVISION_LIMITS)
            .map_err(invalid)?;
        Ok(value.to_string())
    }
    pub(super) fn verify_source(
        &self,
        context: &Context,
        proof: &SourceProof,
        bytes: &str,
    ) -> Result<(RecordRef, BTreeMap<String, String>), RebornBuildError> {
        if proof.format != context.format("source")
            || proof.operation != context.op()
            || proof.root != context.claim.root
        {
            return Err(invalid("pure qualification original source differs"));
        }
        let program = context.program(&context.claim.suites[0].variant)?;
        let snapshot = program.inputs().instruction().snapshot();
        let rooted = snapshot
            .select_roots(&[context.claim.root.uuid])
            .map_err(invalid)?;
        let closure: Vec<Reference> = rooted
            .revisions()
            .values()
            .map(|r| r.reference().into())
            .collect();
        if closure != proof.closure
            || proof.sources.len()
                != context
                    .claim
                    .components
                    .iter()
                    .filter(|r| r.class_code == 22)
                    .count()
        {
            return Err(invalid(
                "pure qualification complete source closure differs",
            ));
        }
        for revision in snapshot
            .revisions()
            .values()
            .filter(|r| r.reference().class_code == 22)
        {
            let id = revision.reference().uuid;
            let expected = self
                .drafts
                .get(&id)
                .ok_or_else(|| invalid("undeclared pure fixture component"))?;
            if expected.exact_bytes() != revision.draft().exact_bytes() {
                return Err(invalid("pure fixture artifact bytes differ"));
            }
            let facts: SourceStructure = serde_json::from_value(
                proof
                    .sources
                    .get(&id)
                    .ok_or_else(|| invalid("pure fixture native source missing"))?
                    .clone(),
            )
            .map_err(invalid)?;
            let doc = revision.draft().document();
            let declaration =
                brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
                    doc,
                    revision.draft().dependencies(),
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("pure fixture preload missing"))?;
            let functions = declaration
                .exports
                .values()
                .map(|e| (e.symbol.clone(), e.parameters.clone()))
                .chain(declaration.private_functions.clone())
                .collect::<BTreeMap<_, _>>();
            let preload = facts
                .preload
                .as_ref()
                .ok_or_else(|| invalid("pure fixture load has effects"))?;
            if !facts.direct_host_calls.is_empty()
                || facts.relative_imports
                || facts.host_value_references != 0
                || !facts.reserved_name_references.is_empty()
                || facts.source_checksum
                    != evidence::checksum(
                        doc["content"]
                            .as_str()
                            .ok_or_else(|| invalid("pure fixture source absent"))?,
                    )
                || functions != preload.functions
                || declaration
                    .constants
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    != preload.constants
                || declaration.imports.iter().cloned().collect::<BTreeSet<_>>() != facts.imports
            {
                return Err(invalid("pure fixture actual definition facts differ"));
            }
        }
        let root = &snapshot.revisions()[&context.claim.root.uuid];
        let preload = brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
            root.draft().document(),
            root.draft().dependencies(),
        )
        .map_err(invalid)?
        .ok_or_else(|| invalid("pure fixture root exports missing"))?;
        let exports = preload
            .exports
            .keys()
            .map(|name| {
                Ok((
                    name.clone(),
                    preload
                        .invocation(name, &root.draft().document()["input_contract"])
                        .map_err(invalid)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RebornBuildError>>()?;
        if proof.exports != exports {
            return Err(invalid("pure entire export selection differs"));
        }
        Ok((reference(context.id("source"), bytes), exports))
    }
    pub(super) fn verify_results(
        &self,
        suite: &Suite,
        results: &BTreeMap<String, Value>,
    ) -> Result<(), RebornBuildError> {
        let expected = self
            .expected
            .get(&suite.variant)
            .ok_or_else(|| invalid("pure fixture oracle missing"))?;
        for (step, wanted) in expected {
            let (_, ordinal) = step
                .split_once(':')
                .ok_or_else(|| invalid("pure fixture oracle step invalid"))?;
            let (description, _) = suite.steps[0]
                .id
                .split_once(':')
                .ok_or_else(|| invalid("pure fixture step invalid"))?;
            let actual = results
                .get(&format!("{description}:{ordinal}"))
                .ok_or_else(|| invalid("pure fixture actual result missing"))?;
            // Compare canonical typed fingerprints too: JSON's numeric equality
            // is not permission to erase integer/number representation.
            if let Some(expected) = wanted.get("native_grammar").and_then(Value::as_bool) {
                if actual["rule_id"] != "recursive_contract"
                    || actual["ok"] != expected
                    || actual["errors"]
                        .as_array()
                        .is_none_or(|errors| errors.is_empty() != expected)
                {
                    return Err(invalid(
                        "actual recursive grammar differs from independent native reader",
                    ));
                }
                continue;
            }
            if actual != wanted
                || typed_value_checksum_with_limits(actual, REVISION_LIMITS).map_err(invalid)?
                    != typed_value_checksum_with_limits(wanted, REVISION_LIMITS).map_err(invalid)?
            {
                return Err(invalid(format!(
                    "pure fixture {} result differs at {step}",
                    suite.variant
                )));
            }
        }
        Ok(())
    }
    pub(super) fn report(
        &self,
        context: &Context,
        qualified: &Qualified,
    ) -> Result<Value, RebornBuildError> {
        let subject = &self.drafts[&self.subject];
        let document = subject.document();
        let interface = json!({"preload":document["preload"],"inputs":document.get("input_contract"),
            "result":document.get("result_contract"),"dependencies":subject.dependencies()});
        let mut report = json!({"format":self.scope.format("report"),"scope":self.scope.name(),
            "claim":context.reservation,
            "source":qualified.source,"executions":qualified.completed,"outcome":"qualified","diagnostics":[]});
        if matches!(self.scope, Scope::Support) {
            report["support"] =
                serde_json::to_value(self.subject(&context.claim)?).map_err(invalid)?;
            report["witness"] = serde_json::to_value(&context.claim.root).map_err(invalid)?;
            report["private_signatures"] = document["preload"]["private_functions"].clone();
            let observations = context.claim.suites.iter().zip(&qualified.completed).filter_map(|(suite,execution)| {
                let input=&suite.steps[0].constants;
                let operation=input["operation"].as_str()?;
                let symbol=match operation { "entry"=>"_validator_review_entry", "base"=>"_validator_review_base", "subject"=>"_validator_review_subject", _=>return None };
                Some(json!({"private_symbol":symbol,"parameters":document["preload"]["private_functions"].get(symbol),
                    "input":input,"return_observation":if suite.python_stop {Value::Null} else {json!(execution)},
                    "failure_observation":if suite.python_stop {json!(execution)} else {Value::Null},
                    "instrument":context.claim.root,"execution":execution}))
            }).collect::<Vec<_>>();
            report["private_observations"] = json!(observations);
        } else {
            report["root"] =
                serde_json::to_value(self.subject(&context.claim)?).map_err(invalid)?;
            report["interface_checksum"] = json!(evidence::checksum(&interface.to_string()));
            report["export_coverage"] = json!(context.claim.suites);
            report["helper_coverage"] = json!({"private_signatures":document["preload"]["private_functions"],"expected_returns":self.expected});
        }
        Ok(report)
    }

    pub(super) fn approves_function(&self) -> bool {
        matches!(self.scope, Scope::Function)
    }
}

fn draft(
    id: Uuid,
    export: &str,
    source: &str,
    inputs: Value,
    dependencies: Vec<Uuid>,
) -> Result<ComponentRevisionDraft, RebornBuildError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":export,"description":"Private closed installation fixture instrument, no product approval or command.",
        "content":source,"input_contract":inputs,"result_contract":{"type":"string","checks":[]},
        "preload":{"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":["json"],"dependencies":dependencies,"default_export":export}},
        "dependencies":dependencies,"association":null}).to_string()).map_err(invalid)
}
pub(super) fn encoder_draft() -> Result<ComponentRevisionDraft, RebornBuildError> {
    let text = json!({"type":"string","checks":[]});
    let rule = cases::rule_contract();
    let diagnostic = json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "rule_id":{"type":"string","required":true,"checks":[]},
        "errors":{"type":"list","required":true,"checks":[],"items":text}}});
    let mut inputs = json!({"kind":{"type":"string","required":true,"checks":[]},"step_id":{"type":"string","required":true,"checks":[]}});
    for (name, mut schema) in [
        ("text", text.clone()),
        (
            "sources",
            json!({"type":"object","checks":[],"fields":{},"allow_extra_fields":true,"extra_fields":text}),
        ),
        ("rule", rule.clone()),
        ("checks", json!({"type":"list","checks":[],"items":rule})),
        (
            "aggregation",
            json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
            "pass":{"type":"boolean","required":true,"checks":[]},"errors":{"type":"list","required":true,"checks":[],"items":diagnostic}}}),
        ),
    ] {
        schema["required"] = json!(false);
        schema["nullable"] = json!(true);
        schema["default"] = Value::Null;
        inputs[name] = schema;
    }
    draft(
        encoder_id(),
        "encode_pure_fixture_result",
        crate::builtin_bootstrap::PURE_FIXTURE_ENCODER_SEED,
        inputs,
        vec![],
    )
}
fn witness_draft() -> Result<ComponentRevisionDraft, RebornBuildError> {
    let input = json!({"type":"string","required":true,"checks":[]});
    draft(
        witness_id(),
        "observe_private_review_reader",
        crate::builtin_bootstrap::PURE_READER_WITNESS_SEED,
        json!({"operation":input,"subject_json":input,"entry_json":input,"base_json":input}),
        vec![reader_id()],
    )
}

pub(crate) struct CheckedPureCheckerClosure {
    contexts: Vec<Arc<Context>>,
    structural: CheckedStructuralHelpers,
}
impl CheckedPureCheckerClosure {
    pub(crate) fn require_program(
        &self,
        program: &RetainedToolProgram,
    ) -> Result<(), RebornBuildError> {
        let graph = program.inputs().instruction().snapshot();
        for context in &self.contexts {
            let profile = context
                .profile
                .as_ref()
                .ok_or_else(|| invalid("pure checker subject scope missing"))?;
            if matches!(profile.scope, Scope::Instrument) {
                continue;
            }
            for expected in profile.components(&context.claim)? {
                let actual = graph.revisions().get(&expected.uuid).ok_or_else(|| {
                    invalid("W lacks an independently qualified checker/support revision")
                })?;
                if Reference::from(actual.reference()) != expected {
                    return Err(invalid(
                        "W checker/support revision differs from own qualification",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) async fn proof_references(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<Vec<Value>, RebornBuildError> {
        self.verified_references(tx).await
    }
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        self.verified_references(tx).await.map(|_| ())
    }
    // References escape only after every predicate in this complete invocation
    // has passed under the supplied snapshot. Nothing is cached between calls.
    async fn verified_references(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<Vec<Value>, RebornBuildError> {
        evidence::require_coherent(tx).await?;
        #[cfg(all(test, feature = "skills-db"))]
        crate::pg_review_submission_io::projection_phase("structural_prerequisites", None);
        let original = self.structural.verify_with_receipt(tx).await?;
        let expected = [
            encoder_id(),
            reader_id(),
            packaged_component_id("validator:preload-definition"),
            packaged_component_id("validator:recursive-contract"),
            packaged_component_id("validator:python-pure-structure:collect"),
            packaged_component_id("validator:python-pure-structure:aggregate"),
            packaged_component_id("validator:python-pure-structure:receipt"),
            packaged_component_id("validator:subject-sources"),
            packaged_component_id("validator:python-definitions"),
            packaged_component_id("validator:python-contracts"),
            packaged_component_id("validator:python-graph"),
            packaged_component_id("validator:python-pure-calls"),
        ];
        if self.contexts.len() != expected.len() {
            return Err(invalid("complete own pure checker closure required"));
        }
        let mut previous: Vec<(&Profile, RecordRef)> = Vec::new();
        for (context, subject) in self.contexts.iter().zip(expected) {
            #[cfg(all(test, feature = "skills-db"))]
            crate::pg_review_submission_io::projection_phase(
                "own_checker_context",
                self.contexts
                    .iter()
                    .position(|selected| Arc::ptr_eq(selected, context)),
            );
            let profile = context
                .profile
                .as_ref()
                .ok_or_else(|| invalid("pure subject scope missing"))?;
            let root = if subject == reader_id() {
                witness_id()
            } else {
                subject
            };
            let scope = if subject == reader_id() {
                Scope::Support
            } else if subject == encoder_id() {
                Scope::Instrument
            } else {
                Scope::Function
            };
            if profile.subject != subject
                || context.claim.root.uuid != root
                || profile.scope.name() != scope.name()
            {
                return Err(invalid("pure function/support/instrument scopes differ"));
            }
            let mut required = vec![original.clone()];
            required.extend(
                previous
                    .iter()
                    .filter(|(p, _)| profile.drafts.contains_key(&p.subject))
                    .map(|(_, r)| r.clone()),
            );
            if required != profile.prerequisites {
                return Err(invalid("pure exact prerequisite evidence differs"));
            }
            previous.push((profile, context.verify_final(tx).await?));
        }
        previous
            .into_iter()
            .map(|(_, proof)| serde_json::to_value(proof).map_err(invalid))
            .collect()
    }
}

pub(crate) async fn qualify(
    base: SessionInput,
    prerequisites: &CheckedStructuralHelpers,
) -> Result<CheckedPureCheckerClosure, RebornBuildError> {
    // E01 remains an actual prerequisite. Source presence or a caller flag cannot
    // manufacture its checked completion.
    let receipt = io::bounded_for(
        base.storage.as_ref(),
        "pure_checker_prerequisites",
        fixtures::receipt_id(),
        async {
            let mut client = io::acquire(&base.pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            let receipt = prerequisites.verify_with_receipt(&tx).await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(receipt)
        },
    )
    .await?;
    let mut specs = cases::specs(&base).await?;
    for spec in &mut specs {
        let profile = spec
            .profile
            .as_mut()
            .ok_or_else(|| invalid("pure fixture profile missing"))?;
        Arc::make_mut(profile).prerequisites.push(receipt.clone());
    }
    Ok(CheckedPureCheckerClosure {
        contexts: qualify_specs(base, specs).await?,
        structural: CheckedStructuralHelpers {
            contexts: prerequisites.contexts.clone(),
        },
    })
}
