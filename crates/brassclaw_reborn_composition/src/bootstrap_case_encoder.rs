//! Exact optional private receipt instrument, using the existing controlled
//! pure Spec/Suite/Context owner. It is not part of W's checker prerequisites.
use super::*;

pub(crate) struct CheckedCaseEncoder {
    context: Arc<Context>,
}
impl CheckedCaseEncoder {
    pub(crate) async fn verify(
        &self,
        tx: &Transaction<'_>,
        program: &RetainedToolProgram,
    ) -> Result<
        brassclaw_skills::review_submission_store::selection::SourceProofReference,
        RebornBuildError,
    > {
        let id = brassclaw_skills::pure_case_components::encoder_id();
        if self.context.claim.root.uuid != id
            || program
                .inputs()
                .instruction()
                .snapshot()
                .revisions()
                .get(&id)
                .is_none_or(|r| Reference::from(r.reference()) != self.context.claim.root)
        {
            return Err(invalid(
                "original Pure receipt encoder differs from its independent qualification",
            ));
        }
        let record = self.context.verify_final(tx).await?;
        Ok(
            brassclaw_skills::review_submission_store::selection::SourceProofReference {
                id: record.id,
                checksum: record.checksum,
            },
        )
    }
}

pub(crate) async fn qualify(base: SessionInput) -> Result<CheckedCaseEncoder, RebornBuildError> {
    let trace = QualificationTrace::new();
    qualify_inner(base, &trace)
        .await
        .map_err(|original| qualification_error(original, &trace))
}

async fn qualify_inner(
    base: SessionInput,
    trace: &QualificationTrace,
) -> Result<CheckedCaseEncoder, RebornBuildError> {
    let id = brassclaw_skills::pure_case_components::encoder_id();
    let draft = brassclaw_skills::pure_case_components::drafts()
        .map_err(invalid)?
        .into_iter()
        .find(|d| d.uuid() == id)
        .ok_or_else(|| invalid("original Pure receipt encoder draft missing"))?;
    let mut suites = Vec::new();
    let mut expected = BTreeMap::new();
    for (variant, input, wanted, failure) in [
        (
            "whole_return",
            json!({"format":"original-pure-invocation/1","outcome":{"kind":"return","value":{"nested":[null,true,7,"Ü {{vars.x}}"]}},"extra":{"keep":"all fields"}}),
            None,
            None,
        ),
        (
            "whole_exception",
            json!({"format":"original-pure-invocation/1","outcome":{"kind":"python_exception","exception":{"class":"ValueError","message":"Ω\nquotes: \""}}}),
            None,
            None,
        ),
        (
            "whole_output_defect",
            json!({"format":"original-pure-invocation/1","outcome":{"kind":"result_contract_violation","value":[7]}}),
            None,
            None,
        ),
        (
            "reject_format",
            json!({"format":"other/1"}),
            None,
            Some("ValueError: unsupported original Pure observation"),
        ),
        (
            "reject_nonobject",
            json!([]),
            None,
            Some("ValueError: original Pure observation must be an object"),
        ),
    ] {
        let wanted: Option<Value> = wanted;
        let actual_expected = wanted.unwrap_or_else(
            || json!({"format":"original-pure-case-receipt/1","observation":input}),
        );
        suites.push(Suite {
            variant: variant.into(),
            steps: vec![fixtures::FixtureStep {
                id: "0:1".into(),
                component: id,
                export: "encode_original_pure_case_receipt".into(),
                constants: json!({"case_outcome_json":input.to_string()}),
                results: BTreeMap::new(),
            }],
            expected_receipt: failure.is_none().then_some(actual_expected),
            python_stop: failure.is_some(),
            expected_python_line: failure.map(str::to_owned),
        });
        expected.insert(variant.into(), BTreeMap::new());
    }
    let recipe = fixtures::recipe_at(
        packaged_component_id("validator:pure-case:encoder-qualification"),
        id,
        &suites,
    )?;
    let profile = Arc::new(Profile {
        scope: Scope::Instrument,
        subject: id,
        drafts: BTreeMap::from([(id, draft.clone())]),
        expected,
        prerequisites: vec![],
        native_inputs: vec![],
    });
    let contexts = qualify_specs_traced(
        base,
        vec![Spec {
            candidate: draft,
            recipe,
            suites,
            dependencies: vec![],
            profile: Some(profile),
        }],
        Some(trace),
    )
    .await?;
    let context = contexts
        .into_iter()
        .next()
        .ok_or_else(|| invalid("original Pure encoder qualification missing"))?;
    Ok(CheckedCaseEncoder { context })
}

// The shared owner returns its exact error unchanged. Keep it privately even
// when an older adapter has already converted the cause to InvalidConfig text.
// Display/Debug of this wrapper never recursively format that private error.
struct PrivateQualificationCause(RebornBuildError);
impl std::fmt::Debug for PrivateQualificationCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateQualificationCause { .. }")
    }
}
impl std::fmt::Display for PrivateQualificationCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("private original encoder qualification cause")
    }
}
impl std::error::Error for PrivateQualificationCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
fn qualification_error(original: RebornBuildError, trace: &QualificationTrace) -> RebornBuildError {
    let diagnostic = classify_error(&original);
    RebornBuildError::OriginalPureEncoderQualification {
        diagnostic: format!("{} cause=[{diagnostic}]", trace.snapshot()),
        source: Box::new(PrivateQualificationCause(original)),
    }
}

// One bounded, passive diagnostic scope for this invocation only. The common
// owner supplies fixed activity labels; arbitrary suite text never escapes.
#[derive(Clone)]
pub(in crate::bootstrap_validator_approval) struct QualificationTrace {
    state: Arc<std::sync::Mutex<TraceState>>,
}
struct TraceState {
    activity: &'static str,
    suite: &'static str,
    primary: Option<&'static str>,
    primary_suite: Option<&'static str>,
    primary_cause: Option<String>,
    cleanup_cause: Option<String>,
    contained_cause: Option<String>,
    receipt_cause: Option<String>,
    receipt_rejection: Option<&'static str>,
    python_class: Option<&'static str>,
}
impl QualificationTrace {
    fn new() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(TraceState {
                activity: "construct_encoder_spec",
                suite: "none",
                primary: None,
                primary_suite: None,
                primary_cause: None,
                cleanup_cause: None,
                contained_cause: None,
                receipt_cause: None,
                receipt_rejection: None,
                python_class: None,
            })),
        }
    }
    pub(in crate::bootstrap_validator_approval) fn note(
        &self,
        activity: &'static str,
        suite: Option<&str>,
    ) {
        if let Ok(mut state) = self.state.try_lock() {
            state.activity = activity;
            state.suite = match suite {
                None => "none",
                Some("whole_return") => "whole_return",
                Some("whole_exception") => "whole_exception",
                Some("whole_output_defect") => "whole_output_defect",
                Some("reject_format") => "reject_format",
                Some("reject_nonobject") => "reject_nonobject",
                Some(_) => "private_suite",
            };
        }
    }
    pub(in crate::bootstrap_validator_approval) fn primary_failure(
        &self,
        error: &RebornBuildError,
    ) {
        if let Ok(mut state) = self.state.try_lock() {
            if state.primary.is_none() {
                state.primary = Some(state.activity);
                state.primary_suite = Some(state.suite);
            }
            if state.primary_cause.is_none() {
                state.primary_cause = Some(classify_error(error));
            }
        }
    }
    pub(in crate::bootstrap_validator_approval) fn contained_failure(
        &self,
        error: &RebornBuildError,
        rejection: Option<&'static str>,
    ) {
        if let Ok(mut state) = self.state.try_lock() {
            let cause = classify_error(error);
            state.contained_cause = Some(cause.clone());
            if state.primary_cause.is_none() {
                state.primary_cause = Some(cause);
            }
            state.receipt_rejection = rejection;
        }
    }
    pub(in crate::bootstrap_validator_approval) fn receipt_failure(
        &self,
        error: &RebornBuildError,
        rejection: &'static str,
    ) {
        if let Ok(mut state) = self.state.try_lock() {
            let cause = classify_error(error);
            state.receipt_cause = Some(cause.clone());
            if state.primary_cause.is_none() {
                state.primary_cause = Some(cause);
            }
            state.receipt_rejection = Some(rejection);
        }
    }
    pub(in crate::bootstrap_validator_approval) fn cleanup_failure(
        &self,
        error: &RebornBuildError,
    ) {
        if let Ok(mut state) = self.state.try_lock() {
            state.cleanup_cause = Some(classify_error(error));
        }
    }
    pub(in crate::bootstrap_validator_approval) fn python_stop_mismatch(&self, line: Option<&str>) {
        let class = match line
            .and_then(|line| line.split_once(':'))
            .map(|(class, _)| class)
        {
            Some("ValueError") => "ValueError",
            Some("TypeError") => "TypeError",
            Some("NameError") => "NameError",
            Some("AttributeError") => "AttributeError",
            Some("AssertionError") => "AssertionError",
            Some("KeyError") => "KeyError",
            Some("IndexError") => "IndexError",
            Some("RuntimeError") => "RuntimeError",
            Some("NotImplementedError") => "NotImplementedError",
            Some("MemoryError") => "MemoryError",
            Some("TimeoutError") => "TimeoutError",
            Some("RecursionError") => "RecursionError",
            Some("SyntaxError") => "SyntaxError",
            _ => "unavailable_or_private",
        };
        if let Ok(mut state) = self.state.try_lock() {
            state.python_class = Some(class);
            if state.primary_cause.is_none() {
                state.primary_cause = Some("native_exception_mismatch".into());
            }
        }
    }
    fn snapshot(&self) -> String {
        self.state
            .try_lock()
            .map(|state| {
                format!(
                    "activity={} primary_activity={} suite={} primary_suite={} primary_cause=[{}] cleanup_cause=[{}] contained_cause=[{}] receipt_cause=[{}] receipt_rejection={} python_class={}",
                    state.activity,
                    state.primary.unwrap_or(state.activity),
                    state.suite,
                    state.primary_suite.unwrap_or(state.suite),
                    state.primary_cause.as_deref().unwrap_or("not_recorded"),
                    state.cleanup_cause.as_deref().unwrap_or("not_recorded"),
                    state.contained_cause.as_deref().unwrap_or("not_recorded"),
                    state.receipt_cause.as_deref().unwrap_or("not_recorded"),
                    state.receipt_rejection.unwrap_or("not_recorded"),
                    state.python_class.unwrap_or("not_recorded"),
                )
            })
            .unwrap_or_else(|_| {
                "activity=unavailable primary_activity=unavailable suite=unavailable".into()
            })
    }
}

// Exact known source literals and closed fixture names are a redaction
// vocabulary ONLY. No match here affects qualification, gates, retries, joins,
// evidence, publication or optional support. Unknown text remains private.
const FIXED_CAUSES: &[(&str, &str)] = &[
    ("fixture", "original Pure encoder qualification missing"),
    (
        "fixture",
        "original Pure receipt encoder differs from its independent qualification",
    ),
    ("fixture", "original Pure receipt encoder draft missing"),
    (
        "execution",
        "Python failure belongs to different export/input",
    ),
    ("execution", "Python stop cannot have a successful receipt"),
    ("execution", "cancelled checker fixture cannot qualify"),
    ("execution", "checker actual result fingerprint differs"),
    ("execution", "checker actual selection differs"),
    ("execution", "checker actual step order incomplete"),
    ("execution", "checker did not complete"),
    ("execution", "checker failure expectation is incomplete"),
    ("execution", "checker input checksum invalid"),
    (
        "execution",
        "checker native Python exception diagnostic absent",
    ),
    ("execution", "checker native step missing"),
    ("execution", "checker observation incomplete"),
    ("execution", "checker preterminal observation absent"),
    ("execution", "checker receipt absent"),
    ("execution", "checker receipt repeated"),
    ("execution", "checker receipt selection differs"),
    (
        "execution",
        "checker retained native execution observation unavailable",
    ),
    (
        "execution",
        "checker step is unsettled or has unexpected native dispatch",
    ),
    ("execution", "checker step observations disabled"),
    ("execution", "checker step order/original capacity differs"),
    ("execution", "checker whole step result missing"),
    ("execution", "completed checker has failed-command evidence"),
    ("execution", "duplicate checker quiescence"),
    (
        "execution",
        "expected Python failure lacks original command",
    ),
    (
        "execution",
        "expected Python failure was not actual export invocation",
    ),
    (
        "execution",
        "expected checker Python exception lacks native command; no infrastructure substitution",
    ),
    ("execution", "expected checker stop has unrelated failure"),
    ("execution", "failed checker has a successful result"),
    ("execution", "failed fixture preload absent"),
    ("execution", "fixture has no final step"),
    ("execution", "independent receipt-helper oracle differs"),
    ("readback", "checker durable native close differs"),
    ("readback", "checker historical executor differs"),
    (
        "readback",
        "checker immutable claim/envelope identity differs",
    ),
    ("readback", "checker original admission missing"),
    (
        "readback",
        "checker original completion missing; do not replay",
    ),
    ("readback", "checker original custody graph differs"),
    ("readback", "checker original envelope differs"),
    ("readback", "checker original executor checksum differs"),
    ("readback", "checker original intent differs"),
    ("readback", "checker original intent missing"),
    ("readback", "checker original quiescence missing"),
    ("readback", "checker original reservation missing"),
    ("readback", "checker original result missing"),
    ("readback", "checker program missing"),
    ("readback", "checker qualification reservation changed"),
    ("readback", "checker receipt missing"),
    ("readback", "checker terminal original differs"),
    ("readback", "private fixture behavior missing"),
    ("readback", "private fixture original qualification differs"),
    ("readback", "private fixture original qualification missing"),
    (
        "profile",
        "actual recursive grammar differs from independent native reader",
    ),
    ("profile", "original native fixture facts changed"),
    ("profile", "original native fixture facts missing"),
    ("profile", "pure entire export selection differs"),
    ("profile", "pure fixture actual definition facts differ"),
    ("profile", "pure fixture actual result missing"),
    ("profile", "pure fixture artifact bytes differ"),
    ("profile", "pure fixture load has effects"),
    ("profile", "pure fixture native source missing"),
    ("profile", "pure fixture oracle missing"),
    ("profile", "pure fixture oracle step invalid"),
    ("profile", "pure fixture preload missing"),
    ("profile", "pure fixture root exports missing"),
    ("profile", "pure fixture source absent"),
    ("profile", "pure fixture step invalid"),
    (
        "profile",
        "pure qualification complete source closure differs",
    ),
    ("profile", "pure qualification original source differs"),
    ("profile", "pure qualification subject missing"),
    ("profile", "pure subject dependency missing"),
    ("profile", "pure subject revision missing"),
    ("profile", "undeclared pure fixture component"),
    ("quiescence", "actual compute duration malformed"),
    ("quiescence", "actual contained Recipe receipt differs"),
    ("quiescence", "actual contained receipt/accounting differs"),
    (
        "quiescence",
        "actual contained task or worker did not complete successfully",
    ),
    ("quiescence", "actual recipe accounting missing"),
    (
        "quiescence",
        "expected stop requires one fixed fixture step",
    ),
    (
        "quiescence",
        "failed fixture does not match original occurrence",
    ),
    (
        "quiescence",
        "original executor configuration checksum differs",
    ),
    ("quiescence", "quiescence object required"),
    ("quiescence", "quiescence required fields differ"),
    ("contained", "bootstrap conversation missing"),
    ("contained", "bootstrap message missing"),
    ("contained", "bootstrap model runtime binding failed"),
    (
        "contained",
        "bootstrap model source has no original program",
    ),
    ("contained", "bootstrap run missing"),
    (
        "contained",
        "bootstrap source reference differs from admission",
    ),
    ("contained", "bootstrap turn missing"),
    ("contained", "contained expected-stop lifecycle incomplete"),
    ("contained", "contained native worker failed"),
    ("contained", "contained quiescence belongs to another run"),
    ("contained", "contained transport unresolved"),
    ("contained", "contained worker closure unacknowledged"),
    ("contained", "contained worker has not been reaped"),
    ("contained", "controlled worker fault injection failed"),
    ("contained", "controlled worker process missing"),
    (
        "contained",
        "model-free contained receipt or quiescence incomplete",
    ),
    (
        "metadata",
        "original intent already retained; reconcile without replay",
    ),
    (
        "metadata",
        "original observation write unresolved; reconcile without replay",
    ),
    ("metadata", "original primitive record readback differs"),
];

fn classify_error(error: &RebornBuildError) -> String {
    match error {
        RebornBuildError::InvalidConfig { reason } => classify_reason(reason),
        RebornBuildError::Turn(_) => "turn_private".into(),
        RebornBuildError::Resource(_) => "resource_private".into(),
        RebornBuildError::HostRuntime(_) => "host_runtime_private".into(),
        RebornBuildError::Filesystem(_) => "filesystem_private".into(),
        RebornBuildError::ContentIntegrity(_) => "artifact_integrity_private".into(),
        _ => "typed_build_error_private".into(),
    }
}
fn classify_reason(reason: &str) -> String {
    if let Some((family, fixed)) = FIXED_CAUSES.iter().find(|(_, fixed)| reason == *fixed) {
        return format!("{family}/{fixed}");
    }
    for phase in ["admission", "completion", "settlement", "shutdown"] {
        for kind in [
            "InvalidInput",
            "InvalidLimits",
            "InvalidPortResult",
            "Backpressure",
            "Closed",
            "Transport",
            "Protocol",
            "Deadline",
            "Join",
            "SettingsConflict",
            "UnsafeHeapReduction",
        ] {
            if reason == format!("bootstrap Recipe {phase}: {kind}") {
                return format!("contained/{phase}/{kind}");
            }
        }
    }
    "invalid_config_private".into()
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn encoder_failure_redacts_private_cause_and_retains_original() {
        let private = "private source/receipt/SQL marker";
        let trace = QualificationTrace::new();
        trace.note("execute_encoder_suite", Some("whole_return"));
        trace.primary_failure(&invalid(private));
        trace.note("read_encoder_suite_close", Some("whole_return"));
        let error = qualification_error(invalid(private), &trace);
        assert!(error.to_string().contains("invalid_config_private"));
        assert!(
            error
                .to_string()
                .contains("primary_activity=execute_encoder_suite suite=whole_return")
        );
        assert!(!error.to_string().contains(private));
        assert!(!format!("{error:?}").contains(private));
        let original = error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<RebornBuildError>()
            .unwrap();
        assert!(
            matches!(original, RebornBuildError::InvalidConfig { reason } if reason == private)
        );
    }

    #[test]
    fn encoder_fixture_projects_only_closed_suite_rejection_and_cause() {
        let trace = QualificationTrace::new();
        trace.note("execute_encoder_suite", Some("whole_return"));
        trace.receipt_failure(
            &invalid("checker actual result fingerprint differs"),
            "result_fingerprint",
        );
        let contained = invalid("bootstrap Recipe completion: Deadline");
        trace.contained_failure(&contained, Some("result_fingerprint"));
        trace.primary_failure(&invalid("private nested error"));
        trace.cleanup_failure(&invalid("private cleanup error"));
        let projected = trace.snapshot();
        assert!(projected.contains("suite=whole_return"));
        assert!(projected.contains("primary_suite=whole_return"));
        assert!(projected.contains("receipt_rejection=result_fingerprint"));
        assert!(
            projected
                .contains("primary_cause=[execution/checker actual result fingerprint differs]")
        );
        assert!(projected.contains("contained_cause=[contained/completion/Deadline]"));
        assert!(projected.contains("cleanup_cause=[invalid_config_private]"));
        assert!(!projected.contains("private nested error"));
        assert!(!projected.contains("private cleanup error"));
        assert_eq!(
            classify_reason("bootstrap Recipe completion: Deadline private marker"),
            "invalid_config_private"
        );
        trace.note("execute_encoder_suite", Some("private suite marker"));
        assert!(!trace.snapshot().contains("private suite marker"));
    }

    #[test]
    fn encoder_stop_mismatch_never_formats_actual_exception_message() {
        let trace = QualificationTrace::new();
        trace.note("capture_encoder_suite_observation", Some("reject_format"));
        trace.python_stop_mismatch(Some("NameError: private source marker"));
        let projected = trace.snapshot();
        assert!(projected.contains("suite=reject_format"));
        assert!(projected.contains("primary_cause=[native_exception_mismatch]"));
        assert!(projected.contains("python_class=NameError"));
        assert!(!projected.contains("private source marker"));
        assert_eq!(
            classify_reason("checker actual result fingerprint differs"),
            "execution/checker actual result fingerprint differs"
        );
    }
}
