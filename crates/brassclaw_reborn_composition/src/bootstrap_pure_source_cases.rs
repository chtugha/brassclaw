//! Finite source-data gallery. These sources are inspected, never invoked.
//! Expected diagnostics are fixed independently of the checking functions.
use super::*;

pub(crate) struct SourceCase {
    pub name: &'static str,
    pub root: ComponentRevisionDraft,
    pub dependencies: Vec<ComponentRevisionDraft>,
    pub rules: Vec<Value>,
}
fn component(id: Uuid, source: &str, dependencies: &[Uuid], public: bool) -> Value {
    let mut document = json!({"name":"Private source qualification data","content":source,
        "preload":{"format":"python-preload/2","exports":{},"private_functions":{"helper":["value"]},
        "constants":[],"imports":[],"dependencies":dependencies,"default_export":null}});
    if public {
        document["preload"]["exports"] =
            json!({"candidate":{"symbol":"candidate","parameters":["inputs"],"mapping":true}});
        document["preload"]["private_functions"] = json!({});
        document["preload"]["default_export"] = json!("candidate");
        document["input_contract"] = json!({"text":{"type":"string","required":true,"checks":[]}});
        document["result_contract"] = json!({"type":"string"});
    }
    json!({"format":"component-revision/1","uuid":id,"class_code":22,"document":document,"dependencies":dependencies,"association":null})
}
pub(crate) fn source_cases() -> Result<Vec<SourceCase>, RebornBuildError> {
    let names = [
        "valid",
        "mapping_signature",
        "private_signature",
        "closed_declaration",
        "syntax",
        "load_effect",
        "optional_default",
        "output_default",
        "dependency_declaration",
        "missing_symbol",
        "symbol_conflict",
        "export_conflict",
        "positional_signature",
        "undeclared_owner",
        "transitive_owner",
        "retired_intrinsic",
        "public_host",
        "private_host",
        "public_recursion",
        "helper_recursion",
        "other_export_recursion",
        "other_export_helper",
        "unused_recursion",
        "constant_call",
        "function_value",
        "other_export_host",
        "value_recursion",
        "no_public_export",
        "literal",
    ];
    let source = "def candidate(inputs):\n    return helper(inputs['text'])\n";
    let mut cases = Vec::new();
    for name in names {
        let identity = |role| {
            packaged_component_id(&format!(
                "validator:pure-qualification:source-case:{name}:{role}"
            ))
        };
        let root = identity("root");
        let helper = identity("helper");
        let other = identity("other");
        let mut candidate = component(root, source, &[helper], true);
        let mut dependency =
            component(helper, "def helper(value):\n    return value\n", &[], false);
        let mut additional = Vec::new();
        let mut definitions: Vec<String> = vec![];
        let mut contracts: Vec<String> = vec![];
        let mut graph: Vec<String> = vec![];
        let mut pure: Vec<String> = vec![];
        match name {
            "mapping_signature" => {
                candidate["document"]["preload"]["exports"]["candidate"]["parameters"] =
                    json!(["wrong"]);
                definitions.extend(["component 0: preload.exports: mapping export requires exactly inputs".into(),
                    "component 0: source.preload.functions: declared signatures differ from parsed definitions".into()]);
            }
            "private_signature" => {
                dependency["document"]["preload"]["private_functions"]["helper"] = json!(["other"]);
                definitions.push("component 1: source.preload.functions: declared signatures differ from parsed definitions".into());
            }
            "closed_declaration" => {
                candidate["document"]["preload"]["unexpected"] = json!(true);
                definitions.push("component 0: preload: closed typed declaration required".into());
            }
            "syntax" => {
                candidate["document"]["content"] = json!("def candidate(:\n");
                definitions.push(
                    "component 0: source rejected by the selected compiler profile: syntax".into(),
                );
            }
            "load_effect" => {
                candidate["document"]["content"] = json!(format!(
                    "raise RuntimeError('source remains data')\n{source}"
                ));
                definitions.push("component 0: source.preload: definitions cannot be loaded without effects under the selected profile".into());
            }
            "optional_default" => {
                candidate["document"]["input_contract"]["text"]["required"] = json!(false);
                contracts.push(
                    "component 0: input_contract: schema: optional top-level input needs default"
                        .into(),
                );
            }
            "output_default" => {
                dependency["document"]["result_contract"] =
                    json!({"type":"string","default":"invalid"});
                contracts.push(
                    "component 1: result_contract: schema: unknown or misplaced field".into(),
                );
            }
            "dependency_declaration" => {
                candidate["document"]["preload"]["dependencies"] = json!([]);
                definitions.push(
                    "component 0: preload.dependencies: retained dependency declaration differs"
                        .into(),
                );
            }
            "missing_symbol" => {
                candidate["document"]["content"] =
                    json!("def candidate(inputs):\n    return missing(inputs['text'])\n");
                graph.push("component 0: source.references: undeclared retained symbol".into());
            }
            "symbol_conflict" => {
                candidate["document"]["content"] =
                    json!("def candidate(inputs):\n    return inputs['text']\n");
                dependency["document"]["content"] =
                    json!("def candidate(value):\n    return value\n");
                dependency["document"]["preload"]["private_functions"] =
                    json!({"candidate":["value"]});
                graph.push("component 1: preload: conflicting implementation symbol".into());
            }
            "export_conflict" => {
                dependency["document"]["preload"]["exports"] =
                    json!({"candidate":{"symbol":"helper","parameters":["value"],"mapping":false}});
                dependency["document"]["preload"]["private_functions"] = json!({});
                dependency["document"]["preload"]["default_export"] = json!("candidate");
                dependency["document"]["input_contract"] =
                    json!({"value":{"type":"string","required":true,"checks":[]}});
                dependency["document"]["result_contract"] = json!({"type":"string"});
                graph.push("component 1: preload.exports: conflicting public export".into());
            }
            "positional_signature" => {
                candidate["document"]["preload"]["exports"]["candidate"]["mapping"] = json!(false);
                graph.push("component 0: preload.exports: positional signature differs from consumer inputs".into());
            }
            "undeclared_owner" | "transitive_owner" => {
                let mut third =
                    component(other, "def other(value):\n    return value\n", &[], false);
                third["document"]["preload"]["private_functions"] = json!({"other":["value"]});
                additional
                    .push(ComponentRevisionDraft::from_json(&third.to_string()).map_err(invalid)?);
                dependency["document"]["content"] =
                    json!("def helper(value):\n    return other(value)\n");
                if name == "undeclared_owner" {
                    candidate["dependencies"] = json!([helper, other]);
                    candidate["document"]["preload"]["dependencies"] = json!([helper, other]);
                    let index = if helper < other { 1 } else { 2 };
                    graph.push(format!("component {index}: source.references: symbol belongs to an undeclared dependency"));
                } else {
                    dependency["dependencies"] = json!([other]);
                    dependency["document"]["preload"]["dependencies"] = json!([other]);
                }
            }
            "retired_intrinsic" => {
                candidate["document"]["content"] = json!(
                    "def candidate(inputs):\n    return __execute_action__('not executed')\n"
                );
                graph.push(
                    "component 0: source: forbidden intrinsic or dynamic host reference".into(),
                );
            }
            "public_host" | "private_host" | "other_export_host" => {
                if name == "private_host" {
                    candidate["document"]["content"] =
                        json!("def candidate(inputs):\n    return inputs['text']\n");
                    dependency["document"]["content"] =
                        json!("def helper(value):\n    return host.post_reply(text=value)\n");
                } else if name == "other_export_host" {
                    candidate["document"]["content"] = json!(format!(
                        "{source}def another(inputs):\n    return host.post_reply(text=inputs['text'])\n"
                    ));
                    candidate["document"]["preload"]["exports"]["another"] =
                        json!({"symbol":"another","parameters":["inputs"],"mapping":true});
                } else {
                    candidate["document"]["content"] = json!(
                        "def candidate(inputs):\n    return host.post_reply(text=inputs['text'])\n"
                    );
                }
                let index = if name == "private_host" { 1 } else { 0 };
                pure.push(format!("component {index}: pure closure contains a host call, including private or unselected code"));
            }
            "public_recursion"
            | "helper_recursion"
            | "other_export_recursion"
            | "unused_recursion"
            | "value_recursion" => {
                match name {
                    "public_recursion" => {
                        candidate["document"]["content"] =
                            json!("def candidate(inputs):\n    return candidate(inputs)\n")
                    }
                    "other_export_recursion" => {
                        candidate["document"]["content"] = json!(format!(
                            "{source}def another(inputs):\n    return another(inputs)\n"
                        ));
                        candidate["document"]["preload"]["exports"]["another"] =
                            json!({"symbol":"another","parameters":["inputs"],"mapping":true});
                    }
                    "value_recursion" => {
                        dependency["document"]["content"] =
                            json!("def helper(value):\n    return list(map(helper, value))\n")
                    }
                    _ => {
                        dependency["document"]["content"] =
                            json!("def helper(value):\n    return helper(value)\n");
                        if name == "unused_recursion" {
                            candidate["document"]["content"] =
                                json!("def candidate(inputs):\n    return inputs['text']\n");
                        }
                    }
                }
                if name != "unused_recursion" {
                    pure.push("pure_calls: recursive function graph requires unsupported retained invocation qualification".into());
                }
            }
            "other_export_helper" => {
                candidate["document"]["content"] = json!(format!(
                    "{source}def another(inputs):\n    return helper(inputs['text'])\n"
                ));
                candidate["document"]["preload"]["exports"]["another"] =
                    json!({"symbol":"another","parameters":["inputs"],"mapping":true});
            }
            "constant_call" => {
                candidate["document"]["content"] =
                    json!("VALUE = 1\ndef candidate(inputs):\n    return VALUE(inputs['text'])\n");
                candidate["document"]["preload"]["constants"] = json!(["VALUE"]);
                pure.push("function candidate: unsupported retained global callable VALUE".into());
            }
            "function_value" => {
                candidate["document"]["content"] = json!(
                    "def candidate(inputs):\n    return ''.join(map(helper, inputs['text']))\n"
                )
            }
            "no_public_export" => {
                candidate["document"]["preload"]["exports"] = json!({});
                candidate["document"]["preload"]["private_functions"] =
                    json!({"candidate":["inputs"]});
                candidate["document"]["preload"]["default_export"] = Value::Null;
                pure.push(
                    "candidate.preload.exports: pure approval requires a public entry point".into(),
                );
            }
            "literal" => {
                candidate["document"]["content"] =
                    json!("def candidate(inputs):\n    return 'host.post_reply(text=inputs)'\n")
            }
            "valid" => {}
            _ => return Err(invalid("unknown closed source fixture")),
        }
        if !definitions.is_empty() || !contracts.is_empty() {
            graph = vec!["graph: matching definitions and valid contracts required".into()];
        }
        if !graph.is_empty() {
            pure = vec!["pure_calls: valid original graph required".into()];
        }
        let rules = [
            ("python_definitions", definitions),
            ("python_contracts", contracts),
            ("python_graph", graph),
            ("python_pure_calls", pure),
        ]
        .into_iter()
        .map(|(rule, errors)| json!({"rule_id":rule,"ok":errors.is_empty(),"errors":errors}))
        .collect();
        additional
            .push(ComponentRevisionDraft::from_json(&dependency.to_string()).map_err(invalid)?);
        additional.sort_by_key(ComponentRevisionDraft::uuid);
        cases.push(SourceCase {
            name,
            root: ComponentRevisionDraft::from_json(&candidate.to_string()).map_err(invalid)?,
            dependencies: additional,
            rules,
        });
    }
    Ok(cases)
}
