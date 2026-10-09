//! Packaged typed draft workflows for global reply/history bootstrap review.
//! They reuse existing Tool identities; they create no primitive, approval,
//! activated route or production fallback. The bootstrap owner must verify the
//! selected Tool/artifact, actual evidence and coherent activation independently.
use std::collections::BTreeSet;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot, RevisionError},
};
use serde_json::{Value, json};
use uuid::Uuid;

/// Stable identities allocated/resolved by the component-library owner.
/// Versions and checksums belong to retained selection, never Recipe includes.
#[derive(Clone, Copy)]
pub struct UsageComponentIds {
    pub recipe: Uuid,
    pub python_code: Uuid,
    pub tool_skill: Uuid,
    pub tool: Uuid,
    pub skill: Uuid,
    pub formatter: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub enum GlobalBootstrapUsage {
    PostReply,
    SaveHistory,
}

/// Exact packaged source integrity, distinct from Tool artifact verification,
/// behavioral evidence, Q1/Q2, catalogue activation and invocation permission.
/// Construction compares actual retained immutable documents, not row labels.
pub struct PackagedUsageIntegrity {
    components: Vec<ComponentRevisionRef>,
    tool: ComponentRevisionRef,
}
impl PackagedUsageIntegrity {
    /// Actual full selection, including the Tool's separately verified metadata
    /// and dependencies. Only the packaged usage drafts were compared above.
    pub fn components(&self) -> &[ComponentRevisionRef] {
        &self.components
    }
    pub fn tool(&self) -> ComponentRevisionRef {
        self.tool
    }
}

/// A changed draft requires its own review; it cannot inherit package provenance
/// merely by retaining a UUID/name or claiming source=system/validated.
pub fn verify_packaged_usage(
    ids: UsageComponentIds,
    usage: GlobalBootstrapUsage,
    snapshot: &RetainedComponentSnapshot,
) -> Result<PackagedUsageIntegrity, RevisionError> {
    if snapshot.roots() != &BTreeSet::from([ids.recipe]) {
        return Err(RevisionError::Invalid(
            "packaged usage requires its exact Recipe root",
        ));
    }
    let expected = usage_drafts(ids, usage)?;
    for draft in expected {
        let actual = snapshot
            .revisions()
            .get(&draft.uuid())
            .ok_or(RevisionError::Invalid(
                "packaged usage component is missing",
            ))?;
        if actual.draft().exact_bytes() != draft.exact_bytes() {
            return Err(RevisionError::Invalid(
                "component differs from packaged usage",
            ));
        }
    }
    let tool = snapshot
        .revisions()
        .get(&ids.tool)
        .filter(|tool| tool.reference().class_code == 0)
        .ok_or(RevisionError::Invalid(
            "existing Tool definition is missing",
        ))?;
    let (capability, callable) = match usage {
        GlobalBootstrapUsage::PostReply => ("host.post_reply", "host.post_reply"),
        GlobalBootstrapUsage::SaveHistory => ("builtin.memory_write", "host.memory_write"),
    };
    if tool
        .draft()
        .document()
        .get("capability_id")
        .and_then(Value::as_str)
        != Some(capability)
        || tool
            .draft()
            .document()
            .get("callable")
            .and_then(Value::as_str)
            != Some(callable)
    {
        return Err(RevisionError::Invalid(
            "Tool identity differs from packaged binding",
        ));
    }
    Ok(PackagedUsageIntegrity {
        components: snapshot
            .revisions()
            .values()
            .map(|revision| revision.reference())
            .collect(),
        tool: tool.reference(),
    })
}

/// Construct immutable authoring drafts using the supported revision envelope.
/// The Tool row is deliberately absent: reuse its real immutable definition and
/// implementation, rather than inventing a primitive from a usage contract.
pub fn usage_drafts(
    ids: UsageComponentIds,
    usage: GlobalBootstrapUsage,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let history = matches!(usage, GlobalBootstrapUsage::SaveHistory);
    if ids.formatter.is_some() != history {
        return Err(RevisionError::Invalid(
            "history requires exactly one formatter identity",
        ));
    }
    let identities = [
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
    ];
    let mut unique = BTreeSet::new();
    for id in identities.into_iter().chain(ids.formatter) {
        if id.is_nil() || !unique.insert(id) {
            return Err(RevisionError::Invalid(
                "bootstrap usage identities must be distinct and non-nil",
            ));
        }
    }
    let (root, code, descriptor, tool, skill) = (
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
    );
    let formatter = ids.formatter;
    let string = json!({"type":"string","required":true,"checks":[]});
    let (callable, capability, name, body, inputs, arguments, fixed, result, purpose) = if history {
        (
            "host.memory_write",
            "builtin.memory_write",
            "memory_write",
            "def save_turn_history(inputs):\n    return host.memory_write(content=inputs['content'], target='daily_log')",
            json!({"content":string}),
            json!({"content":"content"}),
            json!({"target":{"type":"string","checks":[],"depends_on":[],"meaning":"Append to the daily log"}}),
            json!({"type":"object","allow_extra_fields":false,"fields":{
             "status":{"type":"string","required":true},"path":{"type":"string","required":true},
             "append":{"type":"boolean","required":true},"content_length":{"type":"integer","required":true}}}),
            "Append the supplied completed-turn record to the daily memory log.",
        )
    } else {
        (
            "host.post_reply",
            "host.post_reply",
            "post_reply",
            "def publish_turn_reply(inputs):\n    return host.post_reply(answer=inputs['answer'])",
            json!({"answer":string}),
            json!({"answer":"answer"}),
            json!({}),
            json!({"type":"string"}),
            "Publish the supplied answer once to this admitted task's transcript.",
        )
    };
    let export = if history {
        "save_turn_history"
    } else {
        "publish_turn_reply"
    };
    let preload = json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let interface = json!({"format":"skill-interface/1","python_code_uuid":code,
        "exports":preload["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]});
    let prose = format!(
        r#"Purpose
{purpose}

Use when
The owning Monty task supplies the final typed data for this one usage.

Required components
Tool {tool}; ToolSkill {descriptor}; associated PythonCode {code}. No internal code dependencies.

Inputs
{input_description} All inputs are required strings without defaults; missing, null and non-string values stop the usage.

Prerequisites
The exact admitted task owns the transcript and memory address. The selected Tool implementation and adapter must be retained; current global policy and technical constraints apply at dispatch. No additional invocation approval is granted.

Execution
PythonCode {code} defines the pinned {export}(inputs) export without effects. The invocation boundary validates typed inputs and captures its returned value. The export calls {callable} once. {argument_description}

Result
{result_description} This usage invokes no downstream Tool.

Failures
Stop on invalid input, policy denial, stale/cancelled attempt, auth failure, Tool error or malformed output. One initial dispatch only; no retry or inferred idempotency. A timeout does not prove absence of effects. Never replay a completed write or reply after a later failure.

Limits and tier
Deterministic forwarding of already prepared data is Tier 0. Producing new answer content belongs to the consuming Recipe's explicit Tier-1/2 work. Transport, memory and executing-time limits remain independent of token-budget settings.

Examples and acceptance
{acceptance} Prose is documentation; the association alone identifies executable PythonCode."#,
        input_description = if history {
            "content is the prepared completed-turn record."
        } else {
            "answer is the final response; the reply Tool rejects empty/whitespace-only answers."
        },
        argument_description = if history {
            "Pass content unchanged and fix target to daily_log; omit append so the primitive uses its actual append default."
        } else {
            "Pass answer unchanged."
        },
        result_description = if history {
            "Return exactly status:string, path:string, append:boolean and content_length:integer. The primitive's acknowledged write is the result; the Recipe decides completion."
        } else {
            "Return the actual nonempty msg: reference published for this task. Its format alone does not prove finalization; the task host resolves that reference."
        },
        acceptance = if history {
            "Preserve quotes, Unicode and marker-looking text literally. Verify the actual daily log and returned write metadata. A live Tool block stops before another effect."
        } else {
            "Publish quotes, Unicode and marker-looking text literally exactly once. Verify the actual transcript/reference. Wrong-task, extra-field, blank-answer and fenced calls fail without a new publication."
        },
    );
    let association =
        json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,
        "tool_skill_uuid":descriptor,"tool_uuid":tool,"callable":callable,"inputs":inputs,
        "arguments":arguments,"code_arguments":fixed,"result":result,
        "failure":failure})
        .to_string();
    let task_inputs = if history {
        json!({"user_input":string,"answer":string,"reply_ref":string})
    } else {
        inputs.clone()
    };
    let offset = i32::from(history);
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    if history {
        steps.push(
            json!({"stepnumber":1,"knowledge":"orchestrator","goal":"Format the completed turn",
            "content":"","type":"component","include":[formatter.expect("validated history formatter")]}),
        );
        layouts.insert(
            "0:1".into(),
            json!({
            "user_input":{"kind":"task_input","reference":"{{vars.user_input}}"},
            "answer":{"kind":"task_input","reference":"{{vars.answer}}"},
            "reply_ref":{"kind":"task_input","reference":"{{vars.reply_ref}}"}}),
        );
        layouts.insert(
            "0:3".into(),
            json!({"content":{"kind":"result","step_id":"0:1","path":[]}}),
        );
    } else {
        layouts.insert(
            "0:2".into(),
            json!({"answer":{"kind":"task_input","reference":"{{vars.answer}}"}}),
        );
    }
    steps.push(
        json!({"stepnumber":1+offset,"knowledge":"rust","goal":"Bind the usage",
        "content":"","type":"component","include":[descriptor],"tool_bindings":[{
            "tool_id":tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]}),
    );
    steps.push(
        json!({"stepnumber":2+offset,"knowledge":"orchestrator","goal":"Execute the usage",
        "content":"","type":"component","include":[code]}),
    );
    let (recipe_name, examples, variables, description) = if history {
        (
            "host-save-history",
            vec![
                "save completed turn user=%; answer=%; reply=%",
                "record completed turn user=%; answer=%; reply=%",
                "append completed turn user=%; answer=%; reply=%",
                "log completed turn user=%; answer=%; reply=%",
                "save turn history user=%; answer=%; reply=%",
                "record turn history user=%; answer=%; reply=%",
                "append turn history user=%; answer=%; reply=%",
                "log turn history user=%; answer=%; reply=%",
                "please save completed turn user=%; answer=%; reply=%",
                "please record completed turn user=%; answer=%; reply=%",
            ],
            vec!["user_input", "answer", "reply_ref"],
            "Named internal workflow: format the owning task's finalized user/answer/reply data, bind memory_write, then append once to daily_log. Return the acknowledged write metadata. No reply publication, automatic retry or Tier-2 replay. The catalogue owner must keep this helper out of incoming intent routes.",
        )
    } else {
        (
            "host-post-reply",
            vec![
                "reply %",
                "please reply %",
                "respond with %",
                "please respond with %",
                "say %",
                "please say %",
                "answer with %",
                "please answer with %",
                "post the reply %",
                "send the response %",
            ],
            vec!["answer"],
            "Forward the supplied literal final answer: bind post_reply, then publish once and return its actual message reference. Stop on failure/cancellation; never retry publication or replay as Tier 2. The task host independently verifies finalization.",
        )
    };
    let variables: Vec<_> = variables
        .into_iter()
        .map(|name| json!({"name":name,"pattern":null,"description":null}))
        .collect();
    let recipe = json!({"name":recipe_name,"description":description,
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E",
        "intent_examples":examples,"variable_patterns":variables}],
        "step_descriptions":[{"desc_idx":0,"label":"Explicit typed usage","yaml_source":"","steps":steps}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":layouts}},
        "invocations":{"selected":if history {
            json!({"0:1":"format_turn_history","0:3":export})
        } else {json!({"0:2":export})}}});
    let mut drafts = vec![
        (
            descriptor,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,
            "callable":callable,"capability_id":capability}}),
            vec![tool],
            Value::Null,
        ),
        (
            code,
            22,
            json!({"content":body,"preload":preload,"input_contract":inputs,"result_contract":result}),
            vec![],
            Value::Null,
        ),
        (
            skill,
            1,
            json!({"body":prose,"interface":interface}),
            vec![code, descriptor, tool],
            json!(association),
        ),
    ];
    let mut dependencies = vec![descriptor, code, skill];
    if history {
        dependencies.push(formatter.expect("validated history formatter"));
        drafts.push((formatter.expect("validated history formatter"),22,json!({"content":
            "def format_turn_history(inputs):\n    import json\n    return json.dumps({'format': 'completed-turn/1', 'user_input': inputs['user_input'], 'answer': inputs['answer'], 'reply_ref': inputs['reply_ref']}) + '\\n'",
            "preload":{"format":"python-preload/2","exports":{"format_turn_history":{"symbol":"format_turn_history","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"format_turn_history"},
            "input_contract":task_inputs,"result_contract":{"type":"string"}}),vec![],Value::Null));
    }
    drafts.push((root, 21, recipe, dependencies, Value::Null));
    drafts
        .into_iter()
        .map(|(id, class, document, dependencies, association)| {
            ComponentRevisionDraft::from_json(
                &json!({
                    "format":"component-revision/1", "uuid":id, "class_code":class,
                    "document":document,"dependencies":dependencies,"association":association,
                })
                .to_string(),
            )
        })
        .collect()
}
