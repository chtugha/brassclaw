//! One existing parser Tool usage, plus its explicit pure command decoder.
//! These immutable drafts supply no Q1, activation or trusted subject custody.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const SOURCE_ENTRIES_SOURCE: &str = include_str!("validator_source_entries.py");

/// A canonical command accepts a JSON list of [ID, source] pairs because the
/// existing intent capture returns strings. A separate pure component rejects
/// duplicate IDs before constructing the map. The Skill itself takes that typed
/// map, so a private validator can reuse it without serializing/decoding again.
/// Resolve the actual Tool UUID/implementation separately; never synthesize it.
pub fn source_inventory_drafts(
    ids: UsageComponentIds,
    decoder: Uuid,
    reply: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let unique = std::collections::BTreeSet::from([
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
        decoder,
        reply.recipe,
        reply.python_code,
        reply.tool_skill,
        reply.tool,
        reply.skill,
    ]);
    if ids.formatter.is_some()
        || reply.formatter.is_some()
        || unique.len() != 11
        || unique.contains(&Uuid::nil())
    {
        return Err(RevisionError::Invalid(
            "distinct source-inventory usage and decoder identities required",
        ));
    }
    let text = json!({"type":"string","checks":[]});
    let sources = json!({"type":"object","checks":[],"fields":{},"allow_extra_fields":true,"extra_fields":text});
    let mut required_sources = sources.clone();
    required_sources["required"] = json!(true);
    let inputs = json!({"sources":required_sources});
    let command_inputs = json!({"entries_json":{"type":"string","required":true,"checks":[]}});
    let export = "inspect_python_inventory";
    let interface = |symbol: &str, imports: Vec<String>| {
        json!({"format":"python-preload/2","exports":{symbol:{"symbol":symbol,"parameters":["inputs"],"mapping":true}},
            "private_functions":{},"constants":[],"imports":imports,"dependencies":[],"default_export":symbol})
    };
    let preload = interface(export, Vec::<String>::new());
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,
        "python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.source_inspection","inputs":inputs,"arguments":{"sources":"sources"},
        "code_arguments":{},"result":text,"failure":failure})
    .to_string();
    let make = |id: Uuid,
                class_code: i32,
                document: Value,
                dependencies: Vec<Uuid>,
                association: Value| {
        ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,
            "class_code":class_code,"document":document,"dependencies":dependencies,"association":association}).to_string())
    };
    let drafts = vec![
        make(
            decoder,
            22,
            json!({"name":"decode-python-source-entries",
                "description":"Decode the complete JSON list of ID/source pairs into typed data; reject duplicate or empty IDs, nonstring sources and malformed entries before any Tool call. Never execute source or read files.",
                "content":SOURCE_ENTRIES_SOURCE,"includes":[],"dependency_registry":null,
                "input_contract":command_inputs,"result_contract":sources,
                "preload":interface("decode_python_source_entries", vec!["json".to_owned()])}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,
                "callable":"host.source_inspection","capability_id":"builtin.source_inspection"}}),
            vec![ids.tool],
            Value::Null,
        )?,
        make(
            ids.python_code,
            22,
            json!({"name":"inspect-python-inventory",
                "description":"Forward one complete typed ID/source map to the existing nonexecuting parser primitive once, returning exact native inventory JSON bytes without a verdict or partial result.",
                "content":"def inspect_python_inventory(inputs):\n    return host.source_inspection(sources=inputs['sources'])\n",
                "includes":[],"dependency_registry":null,
                "input_contract":inputs,"result_contract":text,"preload":preload}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.skill,
            1,
            json!({"body":format!(
                "Purpose\nInspect one complete named Python source inventory without executing candidate code. One Tool usage, not a class validator.\n\nComponents and interface\nTool {}; ToolSkill {}; associated PythonCode {}. Pinned export inspect_python_inventory(inputs) calls host.source_inspection(sources=inputs['sources']) exactly once. No helpers, constants or code dependencies. Loading is effect-free. The binding prepares availability and grants no permission.\n\nInput\nsources is a required non-null object mapping nonempty opaque IDs to non-null source strings. IDs are not paths or Python identifiers. Empty source text and an explicitly empty map are accepted mechanical inputs. Quotes, Unicode, newlines and template-looking text remain data. The native boundary rejects empty IDs; it does not read files.\n\nResult\nReturn the exact python-source-inventory/1 JSON string with actual preparation revision, retained worker checksum and the complete original ID set. Each entry carries source-checksummed parser facts or a mechanical compiler-profile rejection. A syntax category may reflect the selected nesting guard; categories are not semantic invalidity or Q1. One shared native process, deadline, allocator, source/AST/output/frame accounting covers all entries. Capacity, resource, infrastructure and transport failures reject the whole operation without a successful partial inventory.\n\nPrerequisites and failure\nRetain the actual registered native Tool and worker image. Current instance Tool policy, cancellation, attempt fencing and acknowledged preparation limits apply before dispatch. Stop on invalid input, policy denial, worker failure or rejected output. Exactly one initial dispatch; no automatic retry or inferred idempotency. Never reinterpret failure as No-Match or approval. This usage writes no reply, review evidence or component state.\n\nCanonical Recipe and command\nRecipe {}, variant selected: inspect Python inventory entries %. Position zero entries_json:string is required, non-null and verbatim JSON text containing an ordered list of [ID, source] pairs. Pure decoder {} converts it to the Skill's typed sources map in a separate step; duplicate/empty IDs and wrong entry types fail before the Tool. Examples: inspect Python inventory entries [[\"module Ü\",\"x = 1\\n\"]]; inspect Python inventory entries [[\"empty\",\"\"],[\"never executed\",\"raise RuntimeError('data')\"]]; inspect Python inventory entries []. Matching does not execute source. Consumers with a typed map bind sources directly and do not run the command decoder. The canonical command Recipe then binds the existing reply usage and publishes the exact native JSON string once, without parsing or rewriting its facts; a private validator uses the inventory usage without this reply step.\n\nTier and qualification\nThis deterministic operation is Tier 0 only after its exact graph and native implementation are qualified and activated. Parsing alone proves no behavioral or semantic agreement. The consuming Recipe owns subsequent checks and completion; trusted review/approval remains separate.",
                ids.tool,ids.tool_skill,ids.python_code,ids.recipe,decoder),
                "interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,
                    "exports":preload["exports"],"inputs":inputs,"result":text,"failure":failure,
                    "dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
        make(
            ids.recipe,
            21,
            json!({"name":"inspect-python-inventory",
                "description":"Decode explicit JSON source pairs, invoke the single complete-inventory parser usage, then publish its exact JSON text through the existing reply usage. Never execute candidates or approve components.",
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[
                    "inspect Python inventory entries %","inspect named Python sources %",
                    "inspect Python source pairs %","parse Python inventory entries %",
                    "parse named Python sources %","inspect these Python source pairs %",
                    "report facts for Python inventory %","get parser facts for Python entries %",
                    "inspect Python modules as pairs %","inspect source inventory entries %"
                ],"variable_patterns":[{"name":"entries_json","pattern":null,"description":"Complete JSON list of [nonempty ID, literal source] pairs; duplicates are rejected"}]}],
                "step_descriptions":[{"desc_idx":0,"label":"Inspect a complete source inventory","yaml_source":"","steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Decode all source pairs as data","content":"","type":"component","include":[decoder]},
                    {"stepnumber":2,"knowledge":"rust","goal":"Prepare the native parser binding","content":"","type":"component","include":[ids.tool_skill],
                        "tool_bindings":[{"tool_id":ids.tool,"tool_name":"source_inspection","params":{},"error_policy":{"policy":"fail"}}]},
                    {"stepnumber":3,"knowledge":"orchestrator","goal":"Invoke the pinned complete-inventory usage","content":"","type":"component","include":[ids.python_code]},
                    {"stepnumber":4,"knowledge":"rust","goal":"Prepare the existing reply binding","content":"","type":"component","include":[reply.tool_skill],
                        "tool_bindings":[{"tool_id":reply.tool,"tool_name":"post_reply","params":{},"error_policy":{"policy":"fail"}}]},
                    {"stepnumber":5,"knowledge":"orchestrator","goal":"Publish the exact parser report","content":"","type":"component","include":[reply.python_code]}
                ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":command_inputs,"steps":{
                    "0:1":{"entries_json":{"kind":"task_input","reference":"{{vars.entries_json}}"}},
                    "0:3":{"sources":{"kind":"result","step_id":"0:1","path":[]}},
                    "0:5":{"answer":{"kind":"result","step_id":"0:3","path":[]}}
                }}},"invocations":{"selected":{"0:1":"decode_python_source_entries","0:3":export,"0:5":"publish_turn_reply"}}}),
            vec![
                decoder,
                ids.tool_skill,
                ids.python_code,
                ids.skill,
                reply.tool_skill,
                reply.python_code,
                reply.skill,
            ],
            Value::Null,
        )?,
    ];
    for draft in drafts.iter().filter(|draft| draft.class_code() == 22) {
        crate::value_contract::InputContract::from_value(
            &draft.document()["input_contract"],
            crate::component_revision::REVISION_LIMITS,
        )?;
        crate::value_contract::ValueContract::result_from_value(
            &draft.document()["result_contract"],
            crate::component_revision::REVISION_LIMITS,
        )?;
    }
    Ok(drafts)
}
