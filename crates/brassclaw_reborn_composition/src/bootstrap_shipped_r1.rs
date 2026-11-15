//! First independent shipped business function. The existing installation
//! Spec/Suite/Context owner runs every case and verifies original native close.
//! No authored approval, catalogue publication or ordinary task admission.
use super::*;
use brassclaw_skills::component_revision::ComponentRevisionDraft;
use fixtures::FixtureStep;

pub(super) fn integer_instrument_id() -> Uuid {
    packaged_component_id("shipped-pure:typed-receipt:integer")
}
fn root_id() -> Uuid {
    packaged_component_id("prefix-generation:revision-formatter")
}

fn instrument() -> Result<ComponentRevisionDraft, RebornBuildError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":integer_instrument_id(),"class_code":22,
        "document":{"name":"shipped-pure-typed-receipt-integer","description":"Private typed installation observation instrument; no public function approval.",
        "content":crate::builtin_bootstrap::SHIPPED_PURE_TYPED_RECEIPT_SOURCE,
        "input_contract":{"step_id":{"type":"string","required":true,"checks":[]},"value":{"type":"integer","required":true,
            "checks":[{"kind":"min","value":1},{"kind":"max","value":9007199254740991i64}]}},"result_contract":{"type":"string","checks":[]},
        "preload":{"format":"python-preload/2","exports":{"encode_shipped_result":{"symbol":"encode_shipped_result","parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"encode_shipped_result"}},
        "dependencies":[],"association":null}).to_string()).map_err(invalid)
}
fn direct_case(name: &str, step_id: &str, value: i64, failure: Option<&str>) -> Suite {
    Suite {
        variant: name.into(),
        steps: vec![FixtureStep {
            id: "0:1".into(),
            component: integer_instrument_id(),
            export: "encode_shipped_result".into(),
            constants: json!({"step_id":step_id,"value":value}),
            results: BTreeMap::new(),
        }],
        expected_receipt: failure
            .is_none()
            .then(|| json!({"format":"validator-step-results/1","results":{step_id:value}})),
        python_stop: failure.is_some(),
        expected_python_line: failure.map(str::to_owned),
    }
}
fn specs(root: ComponentRevisionDraft) -> Result<Vec<Spec>, RebornBuildError> {
    // The source is the actual already-retained product definition. Never
    // reconstruct it or assume revision 1 from an audit's illustrative UUID.
    let packaged =
        brassclaw_skills::prefix_generation_components::generation_revision_draft(root_id())
            .map_err(invalid)?;
    if root.exact_bytes() != packaged.exact_bytes()
        || root.uuid() != root_id()
        || root.class_code() != 22
        || !root.dependencies().is_empty()
        || root.document()["preload"]["default_export"] != "prepare_generation_revision"
        || root.document()["preload"]["exports"]
            .as_object()
            .is_none_or(|exports| exports.len() != 1)
    {
        return Err(invalid("shipped generation revision subject differs"));
    }
    let instrument = instrument()?;
    let instrument_suites = vec![
        direct_case("literal_one", "2:7", 1, None),
        direct_case("literal_maximum", "2:7", 9007199254740991, None),
        direct_case(
            "empty_step",
            "",
            1,
            Some("ValueError: literal producer step required"),
        ),
        direct_case(
            "no_step_separator",
            "27",
            1,
            Some("ValueError: literal producer step required"),
        ),
    ];
    let instrument_profile = pure::Profile::shipped(
        instrument.uuid(),
        BTreeMap::from([(instrument.uuid(), instrument.clone())]),
        instrument_suites
            .iter()
            .map(|suite| (suite.variant.clone(), BTreeMap::new()))
            .collect(),
        true,
    );
    let instrument_spec = Spec {
        candidate: instrument.clone(),
        recipe: fixtures::recipe_at(
            packaged_component_id("shipped-pure-r1:integer-instrument:qualification"),
            instrument.uuid(),
            &instrument_suites,
        )?,
        suites: instrument_suites,
        dependencies: vec![],
        profile: Some(Arc::new(instrument_profile)),
    };
    let mut suites = Vec::new();
    let mut expected = BTreeMap::new();
    // Literal independent boundary/neighbor oracles, never candidate outputs.
    for (name, input, wanted, failure) in [
        ("one", "1", Some(1), None),
        ("ten", "10", Some(10), None),
        (
            "below_maximum",
            "9007199254740990",
            Some(9007199254740990i64),
            None,
        ),
        (
            "maximum",
            "9007199254740991",
            Some(9007199254740991i64),
            None,
        ),
        (
            "zero",
            "0",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "leading_zero",
            "01",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "negative",
            "-1",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "leading_space",
            " 1",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "trailing_space",
            "1 ",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "fraction",
            "1.0",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "exponent",
            "1e1",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "unicode_digit",
            "١",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "overlong",
            "99999999999999999",
            None,
            Some("ValueError: canonical positive decimal revision required"),
        ),
        (
            "above_maximum",
            "9007199254740992",
            None,
            Some("ValueError: revision exceeds portable integer range"),
        ),
    ] {
        let mut steps = vec![FixtureStep {
            id: "0:1".into(),
            component: root.uuid(),
            export: "prepare_generation_revision".into(),
            constants: json!({"revision":input}),
            results: BTreeMap::new(),
        }];
        let mut returns = BTreeMap::new();
        if let Some(value) = wanted {
            returns.insert("0:1".into(), json!(value));
            steps.push(FixtureStep {
                id: "0:2".into(),
                component: instrument.uuid(),
                export: "encode_shipped_result".into(),
                constants: json!({"step_id":"0:1"}),
                results: BTreeMap::from([("value".into(), "0:1".into())]),
            });
        }
        expected.insert(name.into(), returns);
        suites.push(Suite {
            variant: name.into(),
            steps,
            expected_receipt: None,
            python_stop: failure.is_some(),
            expected_python_line: failure.map(str::to_owned),
        });
    }
    let profile = pure::Profile::shipped(
        root.uuid(),
        BTreeMap::from([
            (root.uuid(), root.clone()),
            (instrument.uuid(), instrument.clone()),
        ]),
        expected,
        false,
    );
    let root_spec = Spec {
        recipe: fixtures::recipe_at(
            packaged_component_id("shipped-pure-r1:generation-revision:qualification"),
            root.uuid(),
            &suites,
        )?,
        candidate: root,
        suites,
        dependencies: vec![instrument],
        profile: Some(Arc::new(profile)),
    };
    Ok(vec![instrument_spec, root_spec])
}

/// Opaque installation evidence. This does not extend the ordinary selection
/// reader's independent-Pure coverage; that remains the connected B2 owner.
pub(crate) struct CheckedGenerationRevision {
    contexts: Vec<Arc<Context>>,
}
impl CheckedGenerationRevision {
    pub(crate) async fn verify(
        &self,
        tx: &Transaction<'_>,
        selected: &InspectedRetainedProgram,
    ) -> Result<(), RebornBuildError> {
        evidence::require_coherent(tx).await?;
        if self.contexts.len() != 2 {
            return Err(invalid("shipped R1 instrument/root pair missing"));
        }
        let instrument = &self.contexts[0];
        let function = &self.contexts[1];
        let instrument_profile = instrument
            .profile
            .as_ref()
            .ok_or_else(|| invalid("shipped instrument scope missing"))?;
        let profile = function
            .profile
            .as_ref()
            .ok_or_else(|| invalid("shipped function scope missing"))?;
        let prerequisite = instrument.verify_final(tx).await?;
        if instrument.claim.root.uuid != integer_instrument_id()
            || instrument_profile.approves_function()
            || !instrument_profile.is_shipped()
            || !instrument_profile.prerequisites.is_empty()
            || function.claim.root.uuid != root_id()
            || !profile.is_shipped()
            || !profile.approves_function()
            || profile.prerequisites != [prerequisite]
        {
            return Err(invalid("shipped R1 exact independent prerequisite differs"));
        }
        let actual = selected
            .program()
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .get(&root_id())
            .ok_or_else(|| invalid("generation consumer lacks original formatter"))?;
        if Reference::from(actual.reference()) != function.claim.root
            || profile.components(&function.claim)?.len() != 1
        {
            return Err(invalid(
                "generation consumer differs from qualified own root",
            ));
        }
        function.verify_approval(tx).await?;
        Ok(())
    }
}
pub(crate) async fn qualify(
    base: SessionInput,
    selected: &InspectedRetainedProgram,
) -> Result<CheckedGenerationRevision, RebornBuildError> {
    let root = selected
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .get(&root_id())
        .ok_or_else(|| invalid("retained shipped generation formatter missing"))?
        .draft()
        .clone();
    Ok(CheckedGenerationRevision {
        contexts: qualify_specs(base, specs(root)?).await?,
    })
}

#[cfg(test)]
#[path = "bootstrap_shipped_r1_tests.rs"]
mod tests;
