//! Executable post-turn consumer Recipe and its one-operation usages.
//!
//! The installation owner supplies real, registered primitive identities. This
//! package does not invent host callables, qualify native implementations, grant
//! approval, activate a route or claim that a delivery owner has been installed.
use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot, RevisionError},
};

/// In this order: read, qualify, claim, model, decode, submit, acknowledge.
pub const OPERATIONS: [&str; 7] = [
    "read_event",
    "qualify_evidence",
    "claim_review",
    "ask_sempai",
    "decode_proposals",
    "submit_candidates",
    "acknowledge_review",
];

/// Source construction is installation seeding only; runtime reads the retained row.
pub fn request_formatter_source() -> String {
    crate::builtin_bootstrap::post_turn_request_source()
}

#[derive(Clone)]
pub struct ReviewUsage {
    pub tool: Uuid,
    pub tool_skill: Uuid,
    pub skill: Uuid,
    pub python_code: Uuid,
    /// An actual registered host callable, not a suggested API name.
    pub callable: String,
    pub capability_id: String,
}

#[derive(Clone)]
pub struct ReviewComponents {
    pub recipe: Uuid,
    pub prepare_request: Uuid,
    pub usages: [ReviewUsage; 7],
}

/// Package integrity is evidence about source bytes only. Native implementation
/// registration, exact-combination qualification and activation remain the
/// installation owner's distinct responsibilities.
pub fn verify_package(
    ids: &ReviewComponents,
    snapshot: &RetainedComponentSnapshot,
) -> Result<Vec<ComponentRevisionRef>, RevisionError> {
    if snapshot.roots() != &BTreeSet::from([ids.recipe]) {
        return Err(RevisionError::Invalid("exact review Recipe root required"));
    }
    for expected in drafts(ids)? {
        let actual = snapshot
            .revisions()
            .get(&expected.uuid())
            .ok_or(RevisionError::Invalid("packaged review component missing"))?;
        if actual.draft().exact_bytes() != expected.exact_bytes() {
            return Err(RevisionError::Invalid(
                "review component differs from package",
            ));
        }
    }
    for usage in &ids.usages {
        let tool = snapshot
            .revisions()
            .get(&usage.tool)
            .filter(|r| r.reference().class_code == 0)
            .ok_or(RevisionError::Invalid(
                "actual review Tool definition required",
            ))?;
        if tool.draft().document()["callable"] != usage.callable
            || tool.draft().document()["capability_id"] != usage.capability_id
        {
            return Err(RevisionError::Invalid(
                "review Tool identity differs from binding",
            ));
        }
    }
    Ok(snapshot
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect())
}

fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: Vec<Uuid>,
    association: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1", "uuid":id,
        "class_code":class,"document":document,"dependencies":dependencies,
        "association":association})
        .to_string(),
    )
}

fn preload(export: &str, imports: Vec<&str>) -> Value {
    json!({"format":"python-preload/2","exports":{export:{"symbol":export,
        "parameters":["inputs"],"mapping":true}},"private_functions":{},
        "constants":[],"imports":imports,"dependencies":[],"default_export":export})
}

/// Every primitive receives one exact, bounded record as data and returns one
/// exact record. Their operation-specific host parsers must validate the inner
/// contract; opaque record bytes never become Python source or an approval.
/// Strings also preserve response bytes for duplicate-key-aware host decoding.
pub fn drafts(ids: &ReviewComponents) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut unique = BTreeSet::from([ids.recipe, ids.prepare_request]);
    if unique.len() != 2 || unique.contains(&Uuid::nil()) {
        return Err(RevisionError::Invalid(
            "distinct non-nil review identities required",
        ));
    }
    let tools: BTreeSet<_> = ids.usages.iter().map(|usage| usage.tool).collect();
    if tools.contains(&Uuid::nil()) || tools.iter().any(|tool| unique.contains(tool)) {
        return Err(RevisionError::Invalid("invalid review Tool identity"));
    }
    for usage in &ids.usages {
        let Some(name) = usage.callable.strip_prefix("host.") else {
            return Err(RevisionError::Invalid("registered host callable required"));
        };
        if name.is_empty()
            || !name
                .bytes()
                .enumerate()
                .all(|(i, c)| c == b'_' || c.is_ascii_lowercase() || (i > 0 && c.is_ascii_digit()))
            || usage.capability_id.is_empty()
        {
            return Err(RevisionError::Invalid("invalid review primitive identity"));
        }
        for id in [usage.tool_skill, usage.skill, usage.python_code] {
            if id.is_nil() || tools.contains(&id) || !unique.insert(id) {
                return Err(RevisionError::Invalid(
                    "distinct non-nil review identities required",
                ));
            }
        }
    }
    let string = json!({"type":"string","required":true,"checks":[]});
    let input = json!({"record_bytes":string});
    let result = json!({"type":"string"});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let mut components = Vec::new();
    let mut dependencies = vec![ids.prepare_request];
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    let mut invocations = serde_json::Map::new();
    let mut variants = vec![json!({"variant_key":"selected","step_link":"0:1-0:E",
        "intent_examples":[],"variable_patterns":[{"name":"review_event_ref","pattern":null,"description":null}]})];
    let mut input_layouts = serde_json::Map::new();
    let mut invocation_layouts = serde_json::Map::new();
    let mut previous = None::<String>;
    let mut number = 0;
    for (operation, usage) in OPERATIONS.iter().zip(&ids.usages) {
        // Request construction is pure logic between claim and model dispatch.
        if *operation == "ask_sempai" {
            number += 1;
            let step = format!("0:{number}");
            steps.push(json!({"stepnumber":number,"knowledge":"orchestrator",
                "goal":"Build isolated Sempai analysis request","content":"",
                "type":"component","include":[ids.prepare_request]}));
            layouts.insert(
                step.clone(),
                json!({"claim_bytes":{"kind":"result",
                "step_id":previous.as_ref().expect("claim precedes model"),"path":[]}}),
            );
            invocations.insert(step.clone(), json!("prepare_post_turn_request"));
            previous = Some(step);
        }
        let export = format!("review_{operation}");
        let source = format!(
            "def {export}(inputs):\n    return {}(record_bytes=inputs['record_bytes'])",
            usage.callable
        );
        let metadata = preload(&export, vec![]);
        let mut prose = format!(
            "Purpose\n{}\n\nInputs\nrecord_bytes is a required non-null string containing the exact preceding receipt. Validate the operation-specific record before effects; reject unknown or duplicate keys. Never evaluate it as source.\n\nComponents\nTool {}; ToolSkill {}; associated PythonCode {}. Export {export}(inputs) calls {} exactly once.\n\nPrerequisites\nThe delivery owner admits an isolated internal task into the existing global Monty service. Pin this complete Recipe, exact native implementations, Sempai provider and prefix, and bundle before model work. Current instance Tool policy, cancellation, task fencing and technical limits apply before every dispatch. Metadata grants no authority.\n\nResults and failures\nReturn exact receipt bytes. Stop on invalid data, missing adapter support, policy denial, cancellation, stale claim or uncertain effect. One initial invocation; no automatic retry or lease-expiry model redispatch. Storage reconciliation uses the original IDs and identical bytes under its durable owner, never a rerun of this Recipe. No original reply, Tool or history replay. Candidate receipts remain unreviewed and unactivated; summaries are not behavioral evidence.\n\nAcceptance\nVerify the retained effect and receipt in the actual store. A model assertion, status label or nonempty receipt is not proof. No implicit host callable is created by this Skill.",
            purpose(operation),
            usage.tool,
            usage.tool_skill,
            usage.python_code,
            usage.callable
        );
        prose.push_str(&format!("\n\nExecution Recipe and command\nRecipe {}, variant {operation}, command: post-turn {operation} %. Position 0 is record_bytes:string, required, non-null, with verbatim encoding and no default. Insert the exact host-issued record unchanged, including quotes, Unicode, newlines and percent signs; do not evaluate or escape it as source. The wrong verb or a missing record is invalid. These are private internal usages, not MCP entries or ordinary incoming intent routes. The catalogue must exclude all these variants from public matching/discovery. Invoking one usage alone never completes the consumer workflow. {export}(inputs) is the sole public export; private symbols and code dependencies: none.", ids.recipe));
        let association = json!({"format":"skill-association/1","skill_uuid":usage.skill,
            "python_code_uuid":usage.python_code,"tool_skill_uuid":usage.tool_skill,
            "tool_uuid":usage.tool,"callable":usage.callable,"inputs":input,
            "arguments":{"record_bytes":"record_bytes"},"code_arguments":{},
            "result":result,"failure":failure})
        .to_string();
        components.push(draft(
            usage.tool_skill,
            13,
            json!({"binding":{
            "format":"tool-skill-binding/1","tool_uuid":usage.tool,
            "callable":usage.callable,"capability_id":usage.capability_id}}),
            vec![usage.tool],
            Value::Null,
        )?);
        components.push(draft(
            usage.python_code,
            22,
            json!({"content":source,"preload":metadata,
            "input_contract":input,"result_contract":result}),
            vec![],
            Value::Null,
        )?);
        components.push(draft(
            usage.skill,
            1,
            json!({"body":prose,"interface":{
            "format":"skill-interface/1","python_code_uuid":usage.python_code,
            "exports":metadata["exports"],"inputs":input,"result":result,"failure":failure,
            "dependencies":[],"private_symbols":[]}}),
            vec![usage.python_code, usage.tool_skill, usage.tool],
            json!(association),
        )?);
        dependencies.extend([usage.tool_skill, usage.python_code, usage.skill]);
        number += 1;
        steps.push(json!({"stepnumber":number,"knowledge":"rust","goal":format!("Bind {operation}"),
            "content":"","type":"component","include":[usage.tool_skill],"tool_bindings":[{
                "tool_id":usage.tool,"tool_name":usage.callable.strip_prefix("host.").expect("checked callable"),
                "params":{},"error_policy":{"policy":"fail"}}]}));
        number += 1;
        let step = format!("0:{number}");
        steps.push(
            json!({"stepnumber":number,"knowledge":"orchestrator","goal":purpose(operation),
            "content":"","type":"component","include":[usage.python_code]}),
        );
        let reference = match previous {
            Some(ref step) => json!({"kind":"result","step_id":step,"path":[]}),
            None => json!({"kind":"task_input","reference":"{{vars.review_event_ref}}"}),
        };
        layouts.insert(step.clone(), json!({"record_bytes":reference}));
        invocations.insert(step.clone(), json!(export));
        variants.push(json!({"variant_key":operation,
            "step_link":format!("0:{}-0:{number}",number-1),
            "intent_examples":[format!("post-turn {operation} %")],
            "variable_patterns":[{"name":"record_bytes","pattern":null,"description":null}]}));
        input_layouts.insert(
            (*operation).into(),
            json!({"format":"recipe-input-layout/1",
            "task_inputs":input,"steps":{&step:{"record_bytes":{"kind":"task_input",
                "reference":"{{vars.record_bytes}}"}}}}),
        );
        invocation_layouts.insert((*operation).into(), json!({&step:export}));
        previous = Some(step);
    }
    input_layouts.insert(
        "selected".into(),
        json!({"format":"recipe-input-layout/1",
        "task_inputs":{"review_event_ref":string},"steps":layouts}),
    );
    invocation_layouts.insert("selected".into(), json!(invocations));
    components.push(draft(ids.prepare_request,22,json!({
        "content":request_formatter_source(),"analysis_instructions":crate::builtin_bootstrap::POST_TURN_INSTRUCTIONS.trim(),"preload":preload("prepare_post_turn_request",vec!["json"]),
        "input_contract":{"claim_bytes":string},"result_contract":result}),vec![],Value::Null)?);
    components.push(draft(ids.recipe,21,json!({"name":"host-review-completed-turn",
        "description":"Private post-turn Sempai consumer: read immutable evidence, qualify it, claim once, construct an isolated analysis request, ask the pinned Sempai, decode exact response bytes, retain unreviewed candidates and acknowledge receipts. Incomplete evidence stops before model dispatch. Unknown effects require reconciliation. No task replay or activation.",
        "variants":variants,
        "step_descriptions":[{"desc_idx":0,"label":"Post-turn review","yaml_source":"","steps":steps}],
        "input_layouts":input_layouts,
        "invocations":invocation_layouts}),dependencies,Value::Null)?);
    Ok(components)
}

fn purpose(operation: &str) -> &'static str {
    match operation {
        "read_event" => {
            "Read the immutable settlement event by exact identity and verify its checksum; do not read latest forensic packets."
        }
        "qualify_evidence" => {
            "Resolve bounded same-conversation evidence and retain the exact bundle plus explicit completeness diagnostics. Missing evidence must stop this workflow before claim/model dispatch; record an incomplete-evidence receipt without replaying the turn."
        }
        "claim_review" => {
            "Claim a durable deduplicated attempt over the complete bundle and exact Recipe/model/prefix selection. An already claimed or uncertain attempt is not eligible for another model dispatch."
        }
        "ask_sempai" => {
            "Dispatch one isolated Sempai analysis request with no task Tools, independently pinned route/prefix and actual policy/accounting; retain original response bytes, usage and unresolved outcomes before downstream processing."
        }
        "decode_proposals" => {
            "Strictly decode the original model response, rejecting duplicate/unknown keys and unsupported contracts. Empty proposals mean zero candidates, never a claimed export. Validate exact dependency references without consulting mutable latest heads."
        }
        "submit_candidates" => {
            "Retain immutable unreviewed subjects using host-owned stable submission IDs and exact pinned dependency graphs. Return actual receipts; no legacy draft sink, forged Q1 evidence or automatic activation."
        }
        "acknowledge_review" => {
            "Acknowledge only the exact bundle, model response and submission receipts in a durable deduplicated completion record. Preserve original terminal outcome and incomplete/rejected diagnostics."
        }
        _ => unreachable!("packaged operation"),
    }
}

#[cfg(test)]
mod tests;
