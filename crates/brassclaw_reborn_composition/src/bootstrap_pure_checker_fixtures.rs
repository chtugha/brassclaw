//! Independent concrete fixture expectations. Sources remain data for the native
//! inspector; none of these candidate source strings is executed.
use super::super::fixtures::{FixtureStep, Suite};
use super::*;
use brassclaw_monty_host::utility::{self, UtilityOutput, UtilityRequest};

pub(super) fn rule_contract() -> Value {
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "rule_id":{"type":"string","required":true,"checks":[]},
        "ok":{"type":"boolean","required":true,"checks":[]},
        "errors":{"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}}}})
}
fn rule(name: &str, errors: &[&str]) -> Value {
    json!({"rule_id":name,"ok":errors.is_empty(),"errors":errors})
}
struct Case {
    name: String,
    input: Value,
    expected: Value,
    failure: Option<String>,
}
fn case(name: impl Into<String>, input: Value, expected: Value) -> Case {
    Case {
        name: name.into(),
        input,
        expected,
        failure: None,
    }
}
fn stop(name: &str, input: Value, message: &str) -> Case {
    Case {
        name: name.into(),
        input,
        expected: Value::Null,
        failure: Some(format!("ValueError: {message}")),
    }
}
fn id(name: &str) -> Uuid {
    packaged_component_id(&format!("validator:{name}"))
}

fn subject(source: &str) -> Value {
    let uuid = packaged_component_id("validator:pure-qualification:fixture-data");
    let document = json!({"content":source,"input_contract":{"text":{"type":"string","required":true,"checks":[]}},
        "result_contract":{"type":"string"},"preload":{"format":"python-preload/2",
        "exports":{"candidate":{"symbol":"candidate","parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"candidate"}});
    let revision = json!({"format":"component-revision/1","uuid":uuid,"class_code":22,"document":document,"dependencies":[],"association":null});
    let bytes = revision.to_string();
    json!({"format":"installation-validator-subject/1","subject_id":uuid,"subject_bytes":"fixture original data",
        "subject_checksum":evidence::checksum("fixture original data"),"candidate":{
            "reference":{"uuid":uuid,"class_code":22,"version":1,"checksum":evidence::checksum(&bytes)},
            "revision_bytes":bytes,"revision":revision},"dependencies":[],"base_comparison":null})
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeFixtureInventory {
    format: String,
    source_identity: String,
    worker_checksum: String,
    preparation_bytes: String,
    preparation_checksum: String,
    sources: Value,
}
async fn inventory(
    base: &SessionInput,
    subject: &Value,
) -> Result<(Value, (RecordRef, String)), RebornBuildError> {
    let mut sources = BTreeMap::new();
    for entry in std::iter::once(&subject["candidate"]).chain(
        subject["dependencies"]
            .as_array()
            .ok_or_else(|| invalid("fixture dependency data absent"))?,
    ) {
        if entry["reference"]["class_code"] == 22 {
            sources.insert(
                entry["reference"]["uuid"]
                    .as_str()
                    .ok_or_else(|| invalid("fixture source identity absent"))?
                    .to_owned(),
                entry["revision"]["document"]["content"]
                    .as_str()
                    .ok_or_else(|| invalid("fixture source absent"))?
                    .to_owned(),
            );
        }
    }
    // Stable fixture identity excludes today's preparation. The original native
    // inspector output/capture is retained as data before any checker invocation.
    let identity=json!({"sources":sources,"worker":hex::encode(base.worker.checksum()),"application":base.application_checksum,
        "owner":evidence::checksum(include_str!("bootstrap_pure_checker_fixtures.rs")),
        "gallery":evidence::checksum(include_str!("bootstrap_pure_source_cases.rs"))}).to_string();
    let digest = evidence::checksum(&identity);
    let id = Uuid::new_v5(&encoder_id(), identity.as_bytes());
    let encode = |record: &NativeFixtureInventory| -> Result<String, RebornBuildError> {
        Ok(evidence::system_record(
            id,
            "behavior",
            false,
            &digest,
            &[],
            serde_json::to_value(record).map_err(invalid)?,
        ))
    };
    let bytes = match io::read_record(&base.pool, base.storage.as_ref(), id).await? {
        Some(bytes) => bytes,
        None => {
            let worker = crate::pg_review_admission_source::verified_worker(
                &base.worker,
                base.preparation.limits().max_native_artifact_bytes,
            )
            .await?;
            let output = utility::execute(
                &worker.path(),
                UtilityRequest::InspectSources {
                    sources: sources.clone(),
                    bounds: base.preparation.limits().bounds,
                },
                base.preparation.limits().process,
            )
            .await
            .map_err(invalid)?;
            let UtilityOutput::InspectedSources { sources: actual } = output else {
                return Err(invalid("actual native fixture inventory required"));
            };
            let record = NativeFixtureInventory {
                format: "pure-checker-native-fixture-inventory/1".into(),
                source_identity: digest.clone(),
                worker_checksum: hex::encode(worker.checksum()),
                preparation_bytes: base.preparation.bytes().to_owned(),
                preparation_checksum: base.preparation.checksum().to_owned(),
                sources: serde_json::to_value(actual).map_err(invalid)?,
            };
            io::retain_record(&base.pool, base.storage.as_ref(), id, &encode(&record)?).await?;
            io::read_record(&base.pool, base.storage.as_ref(), id)
                .await?
                .ok_or_else(|| invalid("original fixture inventory missing"))?
        }
    };
    let envelope: Envelope =
        serde_json::from_value(strict_json(&bytes, REVISION_LIMITS).map_err(invalid)?)
            .map_err(invalid)?;
    let record: NativeFixtureInventory =
        serde_json::from_value(envelope.report).map_err(invalid)?;
    let preparation = crate::installed_monty_catalogue::PreparationCapture::restore_original(
        &record.preparation_bytes,
        &record.preparation_checksum,
    )
    .map_err(invalid)?;
    if record.format != "pure-checker-native-fixture-inventory/1"
        || record.source_identity != digest
        || record.worker_checksum != hex::encode(base.worker.checksum())
        || encode(&record)? != bytes
    {
        return Err(invalid("original native fixture identity/capture differs"));
    }
    let records = record
        .sources
        .as_object()
        .ok_or_else(|| invalid("native fixture inventory incomplete"))?;
    if records.len() != sources.len() {
        return Err(invalid("native fixture source coverage differs"));
    }
    for (id, source) in sources {
        let fact = records
            .get(&id)
            .ok_or_else(|| invalid("native fixture source missing"))?;
        let checksum = if fact["kind"] == "inspected" {
            &fact["structure"]["source_checksum"]
        } else if fact["kind"] == "compile_rejected" {
            &fact["source_checksum"]
        } else {
            return Err(invalid("native fixture source outcome unsupported"));
        };
        if checksum != &json!(evidence::checksum(&source)) {
            return Err(invalid("native fixture original source checksum differs"));
        }
    }
    let value = json!({"format":"python-source-inventory/1","preparation_revision":preparation.revision(),"worker_checksum":record.worker_checksum,"sources":record.sources});
    Ok((value, (reference(id, &bytes), bytes)))
}
fn source_subject(fixture: &source_gallery::SourceCase) -> Result<Value, RebornBuildError> {
    let entry = |draft: &ComponentRevisionDraft| -> Result<Value, RebornBuildError> {
        Ok(
            json!({"reference":{"uuid":draft.uuid(),"class_code":22,"version":1,
            "checksum":evidence::checksum(draft.exact_bytes())},"revision_bytes":draft.exact_bytes(),
            "revision":strict_json(draft.exact_bytes(),REVISION_LIMITS).map_err(invalid)?}),
        )
    };
    Ok(
        json!({"format":"installation-validator-subject/1","subject_id":fixture.root.uuid(),
        "subject_bytes":"closed fixture data, not a stored candidate approval",
        "subject_checksum":evidence::checksum("closed fixture data, not a stored candidate approval"),
        "candidate":entry(&fixture.root)?,"dependencies":fixture.dependencies.iter().map(entry).collect::<Result<Vec<_>,_>>()?,"base_comparison":null}),
    )
}
fn sources_input(subject: &Value) -> Value {
    json!({"subject_json":subject.to_string()})
}
fn expected_sources(subject: &Value) -> Result<Value, RebornBuildError> {
    let mut sources = serde_json::Map::new();
    sources.insert(
        subject["candidate"]["reference"]["uuid"]
            .as_str()
            .ok_or_else(|| invalid("fixture source identity missing"))?
            .to_owned(),
        subject["candidate"]["revision"]["document"]["content"].clone(),
    );
    Ok(Value::Object(sources))
}
fn encoder_inputs(kind: &str, value: Value) -> Value {
    let mut inputs = json!({"kind":kind,"step_id":"fixture:1"});
    for slot in ENCODER_SLOTS {
        inputs[slot] = Value::Null;
    }
    inputs[kind] = value;
    inputs
}

fn encoder_cases() -> Vec<Case> {
    let values = [
        ("text", json!("Ü\n{{vars.x}}")),
        ("sources", json!({"empty":"","Ü":"a\nb"})),
        ("rule", rule("r", &["one", "two"])),
        ("checks", json!([rule("r", &[])])),
        (
            "aggregation",
            json!({"pass":false,"errors":[{"rule_id":"r","errors":["one"]}]}),
        ),
    ];
    let mut result = values
        .into_iter()
        .map(|(kind, value)| {
            case(
                kind,
                encoder_inputs(kind, value.clone()),
                json!({"format":"validator-step-results/1","results":{"fixture:1":value}}),
            )
        })
        .collect::<Vec<_>>();
    let mut extra = encoder_inputs("text", json!("x"));
    extra["sources"] = json!({});
    result.push(stop("extra", extra, "pure_fixture_extra_result"));
    result.push(stop(
        "missing",
        encoder_inputs("text", Value::Null),
        "pure_fixture_result_missing",
    ));
    let mut unknown = encoder_inputs("text", json!("x"));
    unknown["kind"] = json!("unknown");
    result.push(stop("unknown", unknown, "pure_fixture_identity_invalid"));
    result
}
fn witness_inputs(operation: &str, subject: &Value) -> Value {
    json!({"operation":operation,"subject_json":subject.to_string(),"entry_json":subject["candidate"].to_string(),"base_json":"null"})
}
fn witness_cases() -> Vec<Case> {
    let s = subject("def candidate(inputs):\n    return inputs['text']\n");
    let mut cases = vec![
        case(
            "entry",
            witness_inputs("entry", &s),
            json!({"format":"private-review-reader-observation/1","operation":"entry","return":{"native_kind":"string","value":s["candidate"]["reference"]["uuid"]}}),
        ),
        case(
            "base_null",
            witness_inputs("base", &s),
            json!({"format":"private-review-reader-observation/1","operation":"base","return":{"native_kind":"none"}}),
        ),
        case(
            "subject_tuple",
            witness_inputs("subject", &s),
            json!({"format":"private-review-reader-observation/1","operation":"subject","return":{
            "native_kind":"tuple2","first_kind":"dict","second_kind":"list","subject":s,"entries":[s["candidate"]]}}),
        ),
        stop(
            "unknown",
            witness_inputs("unknown", &s),
            "private_reader_operation_invalid",
        ),
    ];
    let mut base = witness_inputs("base", &s);
    base["base_json"] = json!(s["candidate"].to_string());
    cases.push(stop(
        "same_version_base",
        base,
        "comparison base differs from retained candidate lineage",
    ));
    let mut newer = s.clone();
    newer["candidate"]["reference"]["version"] = json!(2);
    let mut older = witness_inputs("base", &newer);
    older["base_json"] = json!(s["candidate"].to_string());
    cases.push(case("older_base",older,json!({"format":"private-review-reader-observation/1","operation":"base","return":{"native_kind":"none"}})));
    let mut installed_base = s.clone();
    installed_base["base_comparison"] = s["candidate"].clone();
    cases.push(stop(
        "installation_base",
        witness_inputs("subject", &installed_base),
        "installation subject has no authored comparison base",
    ));
    let mut authored = s.clone();
    if let Some(object) = authored.as_object_mut() {
        object.remove("subject_id");
    }
    authored["format"] = json!("component-validator-subject/1");
    authored["submission_id"] = json!(packaged_component_id(
        "validator:pure-qualification:authored-data"
    ));
    authored["actor"] = json!("fixture data");
    cases.push(case("authored_subject",witness_inputs("subject",&authored),json!({"format":"private-review-reader-observation/1","operation":"subject","return":{
        "native_kind":"tuple2","first_kind":"dict","second_kind":"list","subject":authored,"entries":[authored["candidate"]]}})));
    let mut joint = authored.clone();
    joint["format"] = json!("component-validator-subject/2");
    let mut skill = s["candidate"].clone();
    let skill_id = packaged_component_id("validator:pure-qualification:skill-partner-data");
    skill["reference"]["uuid"] = json!(skill_id);
    skill["reference"]["class_code"] = json!(1);
    skill["revision"]["uuid"] = json!(skill_id);
    skill["revision"]["class_code"] = json!(1);
    skill["revision"]["association"] = json!(
        json!({"format":"skill-association/1","skill_uuid":skill_id,
        "python_code_uuid":s["candidate"]["reference"]["uuid"]})
        .to_string()
    );
    skill["revision_bytes"] = json!(skill["revision"].to_string());
    joint["function_partner"] = json!({"role":"context","component":skill,"base_comparison":null});
    cases.push(case("joint_subject",witness_inputs("subject",&joint),json!({"format":"private-review-reader-observation/1","operation":"subject","return":{
        "native_kind":"tuple2","first_kind":"dict","second_kind":"list","subject":joint,"entries":[joint["candidate"],skill]}})));
    let mut wrong_partner = joint.clone();
    wrong_partner["function_partner"]["role"] = json!("approved");
    cases.push(stop(
        "partner_role",
        witness_inputs("subject", &wrong_partner),
        "complete function partner required",
    ));
    let mut wrong_association = joint.clone();
    wrong_association["function_partner"]["component"]["revision"]["association"] = Value::Null;
    cases.push(stop(
        "partner_association",
        witness_inputs("subject", &wrong_association),
        "retained Skill association required",
    ));
    let mut context_base = joint.clone();
    context_base["function_partner"]["base_comparison"] = skill;
    cases.push(stop(
        "partner_context_base",
        witness_inputs("subject", &context_base),
        "unchanged context is not a changed review root",
    ));
    for (name, field, value, message) in [
        (
            "version_bool",
            "version",
            json!(true),
            "positive retained version required",
        ),
        (
            "version_zero",
            "version",
            json!(0),
            "positive retained version required",
        ),
        (
            "version_overflow",
            "version",
            json!(9223372036854775808_u64),
            "positive retained version required",
        ),
        (
            "bad_digest",
            "checksum",
            json!("FF".repeat(32)),
            "retained checksum required",
        ),
        (
            "class_bool",
            "class_code",
            json!(true),
            "retained identity and supported integer class required",
        ),
    ] {
        let mut changed = s.clone();
        changed["candidate"]["reference"][field] = value;
        cases.push(stop(name, witness_inputs("entry", &changed), message));
    }
    let mut duplicate = s.clone();
    duplicate["dependencies"] = json!([s["candidate"]]);
    cases.push(stop(
        "duplicate",
        witness_inputs("subject", &duplicate),
        "distinct retained component identities required",
    ));
    let mut extra = s.clone();
    extra["approved"] = json!(true);
    cases.push(stop(
        "extra",
        witness_inputs("subject", &extra),
        "complete retained validator subject required",
    ));
    cases
}

fn contract_cases() -> Vec<Case> {
    use brassclaw_skills::value_contract::{InputContract, ValueContract};
    let mut schemas = vec![
        json!({"type":"string","required":true,"checks":[]}),
        json!({"type":"string","required":false,"checks":[]}),
        json!({"type":"integer","required":false,"checks":[],"default":null}),
        json!({"type":"integer","required":false,"checks":[],"nullable":true,"default":null}),
        json!({"type":"integer","required":true,"checks":[],"default":2}),
        json!({"type":"integer","required":"true","checks":[]}),
        json!({"type":"integer","required":true,"checks":[],"nullable":0}),
        json!({"type":"any","required":true,"checks":[]}),
        json!({"type":"string","required":true,"checks":[],"enum":["x"]}),
        json!({"type":"integer","required":true}),
        json!({"type":"integer","required":true,"checks":{}}),
        json!({"type":"integer","required":true,"checks":[{"kind":"min","value":true}]}),
        json!({"type":"integer","required":true,"checks":[{"kind":"min","value":0.0}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min","value":0}]}),
        json!({"type":"boolean","required":true,"checks":[{"kind":"min","value":0}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":-1}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1.0}]}),
        json!({"type":"string","required":false,"checks":[{"kind":"min_length","value":2}],"default":"Ü🙂"}),
        json!({"type":"string","required":false,"checks":[{"kind":"min_length","value":2}],"default":"Ü"}),
        json!({"type":"integer","required":true,"checks":[],"depends_on":[],"meaning":"computed is a separate profile"}),
        json!({"type":"list","required":true,"checks":[]}),
        json!({"type":"list","required":true,"checks":[],"items":{"type":"string"}}),
        json!({"type":"list","required":true,"checks":[],"items":{"type":"string","required":true}}),
        json!({"type":"object","required":true,"checks":[],"allow_extra_fields":false}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":false,"extra_fields":{"type":"string"}}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":true}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":true,"extra_fields":{"type":"string"}}),
        json!({"type":"object","required":true,"checks":[],"fields":{"":{"type":"string","required":true}},"allow_extra_fields":false}),
        json!({"type":"object","required":false,"checks":[],"fields":{"name":{"type":"string","required":true}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{"name":{"type":"string","required":false,"default":"exact"}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{"parent":{"type":"object","required":false,"fields":{"child":{"type":"integer","required":true}},"allow_extra_fields":false}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{},"allow_extra_fields":false,"default":{"unknown":"data"}}),
        json!({"type":"null","required":false,"checks":[],"default":null}),
        json!({"type":"null","required":true,"checks":[],"nullable":true}),
    ];
    for (lower, upper) in [
        (json!(0), json!(0.1)),
        (json!(0.1), json!(0)),
        (json!(-0.1), json!(0)),
        (json!(0), json!(-0.1)),
        (json!(-0.0), json!(0.0)),
        (json!(1e-100), json!(0)),
        (
            json!(9_007_199_254_740_993_u64),
            json!(9_007_199_254_740_992.0),
        ),
        (
            json!(9_007_199_254_740_992.0),
            json!(9_007_199_254_740_993_u64),
        ),
        (json!(u64::MAX), json!(u64::MAX - 1)),
        (json!(i64::MIN), json!(i64::MIN + 1)),
        (json!(1e20), json!(u64::MAX)),
        (json!(-1e100), json!(-1e20)),
    ] {
        schemas.push(json!({"type":"number","required":true,"checks":[
            {"kind":"min","value":lower},{"kind":"max","value":upper}
        ]}));
    }
    let mut cases: Vec<(String, Value)> =
        vec![("input".into(), json!({})), ("input".into(), Value::Null)];
    for schema in &schemas {
        for mode in ["input", "parameters"] {
            cases.push((mode.into(), json!({"value":schema})));
        }
        // A result has no top-level presence fields; nested placement is still
        // checked and defaults are forbidden throughout the output tree.
        let mut result = schema.clone();
        if let Some(object) = result.as_object_mut() {
            object.remove("required");
        }
        cases.push(("result".into(), result));
    }
    for target in [
        json!({"type":"integer","required":true,"checks":[]}),
        json!({"type":"number","required":true,"checks":[],"nullable":true}),
        json!({"type":"string","required":true,"checks":[]}),
    ] {
        cases.push(("input".into(),json!({
            "value":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"ceiling"}]},
            "ceiling":target
        })));
    }
    cases.extend([
        ("input".into(),json!({"Value":{"type":"string","required":true,"checks":[]}})),
        ("input".into(),json!({"value":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"absent"}]}})),
        ("computed".into(),json!({"type":"integer"})),
    ]);
    cases.push(cases[0].clone());
    let entries = cases.into_iter().map(|(position, contract)| {
        let valid = match position.as_str() {
            "input" => InputContract::from_value(&contract, REVISION_LIMITS).is_ok(),
            "parameters" => {
                InputContract::tool_parameters_from_value(&contract, REVISION_LIMITS).is_ok()
            }
            "result" => ValueContract::result_from_value(&contract, REVISION_LIMITS).is_ok(),
            _ => false,
        };
        (position, contract, valid)
    });
    // Native grammar is an independent implementation; invalid diagnostics are
    // pinned separately by the concrete golden fixtures below.
    entries
        .enumerate()
        .map(|(i, (position, contract, valid))| {
            case(
                format!("grammar_{i}"),
                json!({"position":position,"contract_json":contract.to_string()}),
                json!({"native_grammar":valid}),
            )
        })
        .collect()
}

fn definition_cases(declaration: &Value, native: &Value) -> Vec<Case> {
    let mut result = vec![
        case(
            "valid",
            json!({"declaration":declaration,"observations":native}),
            rule("preload_definition", &[]),
        ),
        case(
            "missing",
            json!({"declaration":null,"observations":native}),
            rule("preload_definition", &["preload: declaration is missing"]),
        ),
    ];
    for (name, path, value, errors) in [
        (
            "format",
            vec!["format"],
            json!("python-preload/unknown"),
            vec!["preload.format: unsupported declaration"],
        ),
        (
            "symbol",
            vec!["exports", "candidate", "symbol"],
            json!("host"),
            vec![
                "preload.exports: invalid or conflicting implementation symbol",
                "source.preload.functions: declared signatures differ from parsed definitions",
            ],
        ),
        (
            "mapping",
            vec!["exports", "candidate", "parameters"],
            json!(["wrong"]),
            vec![
                "preload.exports: mapping export requires exactly inputs",
                "source.preload.functions: declared signatures differ from parsed definitions",
            ],
        ),
        (
            "parameters",
            vec!["exports", "candidate", "parameters"],
            json!(["inputs", "inputs"]),
            vec![
                "preload.exports: invalid parameter names",
                "preload.exports: mapping export requires exactly inputs",
                "source.preload.functions: declared signatures differ from parsed definitions",
            ],
        ),
        (
            "private_conflict",
            vec!["private_functions"],
            json!({"candidate":["inputs"]}),
            vec!["preload.private_functions: invalid or conflicting symbol"],
        ),
        (
            "private_parameters",
            vec!["private_functions"],
            json!({"helper":["open"]}),
            vec![
                "preload.private_functions: invalid parameter names",
                "source.preload.functions: declared signatures differ from parsed definitions",
            ],
        ),
        (
            "constant",
            vec!["constants"],
            json!(["host"]),
            vec![
                "preload.constants: invalid or conflicting symbol",
                "source.preload.constants: declarations differ from parsed definitions",
            ],
        ),
        (
            "imports",
            vec!["imports"],
            json!(["json", "json"]),
            vec![
                "preload.imports: duplicate module",
                "source.imports: declarations differ from parsed modules",
            ],
        ),
        (
            "default",
            vec!["default_export"],
            json!("missing"),
            vec!["preload.default_export: unknown public export"],
        ),
    ] {
        let mut changed = declaration.clone();
        let mut target = &mut changed;
        for field in path {
            target = &mut target[field];
        }
        *target = value;
        result.push(case(
            name,
            json!({"declaration":changed,"observations":native}),
            rule("preload_definition", &errors),
        ));
    }
    let mut hidden = declaration.clone();
    let entry = hidden["exports"]["candidate"].clone();
    hidden["exports"] = json!({"_private":entry});
    hidden["default_export"] = Value::Null;
    result.push(case(
        "public_name",
        json!({"declaration":hidden,"observations":native}),
        rule(
            "preload_definition",
            &["preload.exports: invalid public name"],
        ),
    ));
    result
}

type FixtureExpectedResults = BTreeMap<String, BTreeMap<String, Value>>;

fn suites(
    root: &ComponentRevisionDraft,
    values: Vec<Case>,
    kind: &str,
) -> Result<(Vec<Suite>, FixtureExpectedResults), RebornBuildError> {
    let export = root.document()["preload"]["default_export"]
        .as_str()
        .ok_or_else(|| invalid("fixture root export absent"))?;
    let mut suites = Vec::new();
    let mut expected = BTreeMap::new();
    for value in values {
        let mut steps = vec![FixtureStep {
            id: "0:1".into(),
            component: root.uuid(),
            export: export.into(),
            constants: value.input,
            results: BTreeMap::new(),
        }];
        let direct = kind == "text";
        if value.failure.is_none() && !direct {
            steps.push(FixtureStep {
                id: "0:2".into(),
                component: encoder_id(),
                export: "encode_pure_fixture_result".into(),
                constants: json!({"kind":kind,"step_id":"0:1"}),
                results: BTreeMap::from([(kind.into(), "0:1".into())]),
            });
        }
        if value.failure.is_none() && !direct {
            expected.insert(
                value.name.clone(),
                BTreeMap::from([("0:1".into(), value.expected.clone())]),
            );
        } else {
            expected.insert(value.name.clone(), BTreeMap::new());
        }
        suites.push(Suite {
            variant: value.name,
            steps,
            expected_receipt: (direct && value.failure.is_none()).then_some(value.expected),
            python_stop: value.failure.is_some(),
            expected_python_line: value.failure,
        });
    }
    Ok((suites, expected))
}

pub(super) async fn specs(base: &SessionInput) -> Result<Vec<Spec>, RebornBuildError> {
    let mut drafts = crate::builtin_bootstrap::validator_check_drafts()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let encoder = encoder_draft()?;
    let witness = witness_draft()?;
    drafts.extend([encoder.clone(), witness.clone()]);
    let all = drafts
        .into_iter()
        .map(|d| (d.uuid(), d))
        .collect::<BTreeMap<_, _>>();
    let baseline = subject("def candidate(inputs):\n    return inputs['text']\n");
    let (facts, baseline_native) = inventory(base, &baseline).await?;
    let mut native_inputs = vec![baseline_native];
    let profile = serde_json::to_value(brassclaw_engine::executor::retained_profile::profile())
        .map_err(invalid)?;
    let required = [
        "python_definitions",
        "python_contracts",
        "python_graph",
        "python_pure_calls",
    ];
    // Keep the factory's exact declaration bytes. Invocation binding orders its
    // named schemas with BTreeMap, so the collector's native iteration is sorted.
    let mut bound_names = required;
    bound_names.sort();
    let checks = bound_names
        .iter()
        .map(|name| rule(name, &[]))
        .collect::<Vec<_>>();
    let collect = brassclaw_skills::validator_components::collect_draft(
        id("python-pure-structure:collect"),
        &required.map(str::to_owned),
    )
    .map_err(invalid)?;
    let aggregate = brassclaw_skills::validator_components::aggregate_draft(id(
        "python-pure-structure:aggregate",
    ))
    .map_err(invalid)?;
    let receipt = brassclaw_skills::validator_components::python_pure_structure_drafts(
        crate::source_inspection_components::usage_ids(id("inspection:tool"), true),
        crate::builtin_bootstrap::VALIDATOR_PURE_STRUCTURE_RECEIPT_SEED,
        serde_json::to_value(brassclaw_engine::executor::retained_profile::profile())
            .map_err(invalid)?,
    )
    .map_err(invalid)?
    .into_iter()
    .find(|d| d.uuid() == id("python-pure-structure:receipt"))
    .ok_or_else(|| invalid("pure receipt factory missing"))?;
    let mut all = all;
    for draft in [collect.clone(), aggregate.clone(), receipt.clone()] {
        all.insert(draft.uuid(), draft);
    }
    let mut definitions = Vec::new();
    let mut contracts = Vec::new();
    let mut graph = Vec::new();
    let mut pure = Vec::new();
    for fixture in source_gallery::source_cases()? {
        let data = source_subject(&fixture)?;
        let (native, original) = inventory(base, &data).await?;
        native_inputs.push(original);
        let mut input = sources_input(&data);
        input["inventory_json"] = json!(native.to_string());
        definitions.push(case(fixture.name, input.clone(), fixture.rules[0].clone()));
        contracts.push(case(
            fixture.name,
            sources_input(&data),
            fixture.rules[1].clone(),
        ));
        input["profile"] = profile.clone();
        input["definitions"] = fixture.rules[0].clone();
        input["contracts"] = fixture.rules[1].clone();
        graph.push(case(fixture.name, input.clone(), fixture.rules[2].clone()));
        input
            .as_object_mut()
            .ok_or_else(|| invalid("fixture input absent"))?
            .remove("definitions");
        input
            .as_object_mut()
            .ok_or_else(|| invalid("fixture input absent"))?
            .remove("contracts");
        input["graph"] = fixture.rules[2].clone();
        pure.push(case(fixture.name, input, fixture.rules[3].clone()));
    }
    let mut collect_input = serde_json::Map::new();
    for check in &checks {
        collect_input.insert(
            check["rule_id"]
                .as_str()
                .ok_or_else(|| invalid("fixture rule absent"))?
                .into(),
            check.clone(),
        );
    }
    let mut wrong = Value::Object(collect_input.clone());
    wrong["python_graph"]["rule_id"] = json!("foreign");
    let mut aggregates = vec![case(
        "complete",
        json!({"required_rule_ids":required,"checks":checks}),
        json!({"pass":true,"errors":[]}),
    )];
    let mut rejected = checks.clone();
    rejected[2] = rule("python_graph", &["g1", "g2"]);
    aggregates.push(case(
        "negative",
        json!({"required_rule_ids":required,"checks":rejected}),
        json!({"pass":false,"errors":[{"rule_id":"python_graph","errors":["g1","g2"]}]}),
    ));
    aggregates.push(stop(
        "missing",
        json!({"required_rule_ids":required,"checks":[]}),
        "validator_rule_coverage_incomplete",
    ));
    for (name, required_ids, values, message) in [
        (
            "empty_required",
            json!([]),
            json!([]),
            "validator_rule_coverage_invalid",
        ),
        (
            "duplicate_required",
            json!(["r", "r"]),
            json!([]),
            "validator_required_rule_invalid",
        ),
        (
            "duplicate_result",
            json!(["r"]),
            json!([rule("r", &[]), rule("r", &[])]),
            "validator_rule_identity_invalid",
        ),
        (
            "foreign_result",
            json!(["r"]),
            json!([rule("other", &[])]),
            "validator_rule_identity_invalid",
        ),
        (
            "true_diagnostics",
            json!(["r"]),
            json!([{"rule_id":"r","ok":true,"errors":["unexpected"]}]),
            "validator_rule_result_contradictory",
        ),
        (
            "false_empty",
            json!(["r"]),
            json!([{"rule_id":"r","ok":false,"errors":[]}]),
            "validator_rule_result_contradictory",
        ),
    ] {
        aggregates.push(stop(
            name,
            json!({"required_rule_ids":required_ids,"checks":values}),
            message,
        ));
    }
    let receipt_input = json!({"sources":expected_sources(&baseline)?,"inventory_json":facts.to_string(),"definitions":checks[1],"contracts":checks[0],
        "graph":checks[2],"pure_calls":checks[3],"checks":checks,"aggregation":{"pass":true,"errors":[]}});
    let receipt_expected = json!({"format":"validator-step-results/1","results":{"0:1":receipt_input["sources"],"0:3":receipt_input["inventory_json"],
        "0:4":receipt_input["definitions"],"0:5":receipt_input["contracts"],"0:6":receipt_input["graph"],"0:7":receipt_input["pure_calls"],"0:8":receipt_input["checks"],"0:9":receipt_input["aggregation"]}});
    let declaration = baseline["candidate"]["revision"]["document"]["preload"].clone();
    let native = facts["sources"][baseline["candidate"]["reference"]["uuid"]
        .as_str()
        .ok_or_else(|| invalid("fixture uuid absent"))?]["structure"]
        .clone();
    let definition_cases = definition_cases(&declaration, &native);
    let mut contract_values = contract_cases();
    contract_values.push(case("optional_missing_default",json!({"position":"input","contract_json":json!({"value":{"type":"string","required":false,"checks":[]}}).to_string()}),rule("recursive_contract",&["schema: optional top-level input needs default"])));
    contract_values.push(case(
        "unsupported_position",
        json!({"position":"computed","contract_json":"{}"}),
        rule(
            "recursive_contract",
            &["contract: unsupported review position"],
        ),
    ));
    let mut source_values = vec![case(
        "complete",
        sources_input(&baseline),
        expected_sources(&baseline)?,
    )];
    for (name, source) in [
        ("empty", ""),
        (
            "verbatim",
            "Ü🙂\n{{vars.never_source}}\nraise RuntimeError('data')\n",
        ),
    ] {
        let value = subject(source);
        source_values.push(case(name, sources_input(&value), expected_sources(&value)?));
    }
    let mut missing_source = baseline.clone();
    if let Some(document) = missing_source["candidate"]["revision"]["document"].as_object_mut() {
        document.remove("content");
    }
    source_values.push(stop(
        "missing_source",
        sources_input(&missing_source),
        "complete PythonCode source string required",
    ));
    let mut with_metadata = baseline.clone();
    let mut metadata = baseline["candidate"].clone();
    let metadata_id = packaged_component_id("validator:pure-qualification:metadata-data");
    metadata["reference"]["uuid"] = json!(metadata_id);
    metadata["reference"]["class_code"] = json!(13);
    metadata["revision"]["uuid"] = json!(metadata_id);
    metadata["revision"]["class_code"] = json!(13);
    metadata["revision"]["document"] = json!({"content":"metadata is not PythonCode"});
    metadata["revision_bytes"] = json!(metadata["revision"].to_string());
    metadata["reference"]["checksum"] = json!(evidence::checksum(
        metadata["revision_bytes"]
            .as_str()
            .ok_or_else(|| invalid("metadata fixture bytes missing"))?
    ));
    with_metadata["dependencies"] = json!([metadata]);
    source_values.push(case(
        "metadata_filtered",
        sources_input(&with_metadata),
        expected_sources(&baseline)?,
    ));
    let subjects = vec![
        (
            encoder.clone(),
            Scope::Instrument,
            encoder_id(),
            encoder_cases(),
            "text",
        ),
        (
            witness,
            Scope::Support,
            reader_id(),
            witness_cases(),
            "text",
        ),
        (
            all[&id("preload-definition")].clone(),
            Scope::Function,
            id("preload-definition"),
            definition_cases,
            "rule",
        ),
        (
            all[&id("recursive-contract")].clone(),
            Scope::Function,
            id("recursive-contract"),
            contract_values,
            "rule",
        ),
        (
            collect,
            Scope::Function,
            id("python-pure-structure:collect"),
            vec![
                case("complete", Value::Object(collect_input), json!(checks)),
                stop("foreign", wrong, "validator_rule_identity_invalid"),
            ],
            "checks",
        ),
        (
            aggregate,
            Scope::Function,
            id("python-pure-structure:aggregate"),
            aggregates,
            "aggregation",
        ),
        (
            receipt,
            Scope::Function,
            id("python-pure-structure:receipt"),
            vec![case("all_results", receipt_input, receipt_expected)],
            "text",
        ),
        (
            all[&id("subject-sources")].clone(),
            Scope::Function,
            id("subject-sources"),
            source_values,
            "sources",
        ),
        (
            all[&id("python-definitions")].clone(),
            Scope::Function,
            id("python-definitions"),
            definitions,
            "rule",
        ),
        (
            all[&id("python-contracts")].clone(),
            Scope::Function,
            id("python-contracts"),
            contracts,
            "rule",
        ),
        (
            all[&id("python-graph")].clone(),
            Scope::Function,
            id("python-graph"),
            graph,
            "rule",
        ),
        (
            all[&id("python-pure-calls")].clone(),
            Scope::Function,
            id("python-pure-calls"),
            pure,
            "rule",
        ),
    ];
    let mut result = Vec::new();
    for (root, scope, subject, values, kind) in subjects {
        let (suites, expected) = suites(&root, values, kind)?;
        let mut needed = BTreeSet::new();
        let mut pending = vec![root.uuid()];
        if kind != "text" {
            pending.push(encoder_id());
        }
        while let Some(id) = pending.pop() {
            if needed.insert(id) {
                pending.extend(
                    all.get(&id)
                        .ok_or_else(|| invalid("fixture dependency absent"))?
                        .dependencies(),
                );
            }
        }
        let selected = needed
            .into_iter()
            .map(|id| {
                Ok((
                    id,
                    all.get(&id)
                        .ok_or_else(|| invalid("fixture draft absent"))?
                        .clone(),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RebornBuildError>>()?;
        let recipe = fixtures::recipe_at(
            packaged_component_id(&format!("validator:pure-qualification:recipe:{subject}")),
            root.uuid(),
            &suites,
        )?;
        let profile = Arc::new(Profile {
            scope,
            subject,
            drafts: selected.clone(),
            expected,
            prerequisites: vec![],
            native_inputs: native_inputs.clone(),
        });
        result.push(Spec {
            candidate: root.clone(),
            recipe,
            suites,
            dependencies: selected
                .into_values()
                .filter(|d| d.uuid() != root.uuid())
                .collect(),
            profile: Some(profile),
        });
    }
    Ok(result)
}
