//! One reusable strict JSON-Tool usage. Drafts are not review, activation or
//! provenance evidence. A consuming Recipe owns later semantic/effect decisions.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use serde_json::{Value, json};
use uuid::Uuid;

/// Existing Tool identity plus independently allocated usage components.
/// A Skill does not require a dedicated Recipe or matching command identity.
#[derive(Clone, Copy)]
pub struct StrictJsonUsageIds {
    pub tool: Uuid,
    pub tool_skill: Uuid,
    pub python_code: Uuid,
    pub skill: Uuid,
}

pub const STRICT_JSON_TEXT_SOURCE: &str = "def parse_strict_json_text(inputs):\n    import json\n    parsed = host.json(operation='parse_strict', data=inputs['json_text'])\n    return json.dumps(parsed, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)\n";

/// The same native primitive parses once. The export returns canonical text so
/// heterogeneous JSON stays data without an unsupported retained `any`/union.
/// Original lexical bytes remain with the caller/native argument custodian.
pub fn strict_json_usage_drafts(
    ids: StrictJsonUsageIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let identities = [ids.tool, ids.tool_skill, ids.python_code, ids.skill];
    if identities.iter().any(Uuid::is_nil)
        || identities
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != identities.len()
    {
        return Err(RevisionError::Invalid(
            "strict JSON usage identities must be distinct and non-nil",
        ));
    }
    let inputs = json!({"json_text":{"type":"string","required":true,"checks":[]}});
    let result = json!({"type":"string","checks":[{"kind":"min_length","value":1}]});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let exports = json!({"parse_strict_json_text":{"symbol":"parse_strict_json_text","parameters":["inputs"],"mapping":true}});
    let preload = json!({"format":"python-preload/2","exports":exports,
        "private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"parse_strict_json_text"});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,
        "python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.json","inputs":inputs,"arguments":{"data":"json_text"},
        "code_arguments":{"operation":{"type":"string","checks":[],"depends_on":[],
            "meaning":"Fixed parse_strict selector over the complete original json_text; reject duplicate keys at every object depth"}},
        "result":result,"failure":failure}).to_string();
    let prose = format!(
        r#"Purpose
Parse one complete JSON text with builtin.json parse_strict and return its canonical JSON text representation. This is one deterministic Tool usage, not a workflow or model request.

Interface and components
Tool {}; ToolSkill {}; associated PythonCode {}. Preload parse_strict_json_text(inputs) without effects. Its sole public mapping export accepts inputs; declared import json, no private functions/constants/dependencies. Invocation inputs, temporary parsed values and results remain isolated. Loading does not invoke the Tool or grant permission.

Input
Required non-null json_text:string, no default or extra inputs. Preserve the full supplied text as Tool data, including quotes, Unicode and instruction-looking content. Empty/whitespace-only or malformed documents fail in the native parser. No code/source substitution, source_tool_call_id or caller operation selector is accepted.

Operation and result
Call host.json once with fixed operation parse_strict and data equal to json_text. The native bounded common reader rejects duplicate keys at every object depth, including escaped equivalent keys, malformed/trailing data and unsupported native representations. Current captured physical bounds still apply with discretionary token budgets disabled. The primitive returns a parsed JSON payload; the export deterministically serializes that successful payload with ensure_ascii false, sorted keys, compact separators and allow_nan false. The nonempty canonical string is the declared Skill/step result; it is not the primitive payload or the original lexical text. Native numeric/runtime representation limits and failures remain in force, without coercion/repair. The caller/native custody retains original input and actual Tool outcome separately; canonicalization is not byte-preserving archival storage.

Failures and authority
Exactly one initial dispatch, stop on error; no retry, default object, repaired answer, downstream Tool or Tier-2 replay. A serialization/result failure does not replay the completed parse. Current instance Tool policy, exact retained implementation/binding, cancellation, original claim and resource constraints remain mandatory. Review/selection grants no Tool permission.

Use in a Recipe
Bind this ToolSkill immediately before invoking its pinned Python export. Later deterministic schema, correlation, source/evidence/option and import checks are separate Recipe steps. Successful parsing grants no provenance, prefill permission, model-design acceptance or component/workflow approval. This Skill needs no dedicated Recipe or matching command; intended MCP exposure requires its own qualified MCP-call Recipe/variant. These are unapproved authoring drafts until the existing exact association/behavior/bootstrap owner qualifies them.
"#,
        ids.tool, ids.tool_skill, ids.python_code
    );
    let row = |id, class, document: Value, dependencies: Vec<Uuid>, association: Value| {
        ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,
            "class_code":class,"document":document,"dependencies":dependencies,"association":association}).to_string())
    };
    Ok(vec![
        row(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,"callable":"host.json","capability_id":"builtin.json"}}),
            vec![ids.tool],
            Value::Null,
        )?,
        row(
            ids.python_code,
            22,
            json!({"name":"parse-strict-json-text","content":STRICT_JSON_TEXT_SOURCE,
                "input_contract":inputs,"result_contract":result,"preload":preload}),
            vec![],
            Value::Null,
        )?,
        row(
            ids.skill,
            1,
            json!({"body":prose,"interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,
                "exports":exports,"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_strict_usage_declares_real_association_and_text_boundaries_without_recipe() {
        let ids = StrictJsonUsageIds {
            tool: Uuid::new_v4(),
            tool_skill: Uuid::new_v4(),
            python_code: Uuid::new_v4(),
            skill: Uuid::new_v4(),
        };
        let drafts = strict_json_usage_drafts(ids).unwrap();
        assert_eq!(
            drafts
                .iter()
                .map(ComponentRevisionDraft::class_code)
                .collect::<Vec<_>>(),
            [13, 22, 1]
        );
        let association = drafts[2].association().unwrap();
        assert_eq!(
            association.inputs().names().collect::<Vec<_>>(),
            ["json_text"]
        );
        for bad in [
            json!({}),
            json!({"json_text":null}),
            json!({"json_text":{}}),
            json!({"json_text":"{}","operation":"parse"}),
        ] {
            assert!(association.inputs().bind(&bad).is_err());
        }
        association
            .validate_arguments(
                &json!({"json_text":"{}"}),
                &json!({"operation":"parse_strict","data":"{}"}),
            )
            .unwrap();
        assert!(
            association
                .validate_arguments(
                    &json!({"json_text":"{}"}),
                    &json!({"operation":"parse_strict","data":"changed"})
                )
                .is_err()
        );
        association.result().validate(&json!("{}")).unwrap();
        assert!(association.result().validate(&json!({})).is_err());
        assert!(association.result().validate(&json!("")).is_err());
        assert_eq!(drafts[1].document()["preload"]["imports"], json!(["json"]));
        assert!(
            strict_json_usage_drafts(StrictJsonUsageIds {
                tool: Uuid::nil(),
                ..ids
            })
            .is_err()
        );
        assert!(
            strict_json_usage_drafts(StrictJsonUsageIds {
                skill: ids.python_code,
                ..ids
            })
            .is_err()
        );
    }
}
