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
    let contexts = qualify_specs(
        base,
        vec![Spec {
            candidate: draft,
            recipe,
            suites,
            dependencies: vec![],
            profile: Some(profile),
        }],
    )
    .await?;
    let context = contexts
        .into_iter()
        .next()
        .ok_or_else(|| invalid("original Pure encoder qualification missing"))?;
    Ok(CheckedCaseEncoder { context })
}
