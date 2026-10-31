//! Pure bootstrap protocol stages and fixed instructions. Draft construction is
//! not workflow qualification, model dispatch, import, approval or activation.
//! Reuse value_contract::strict_json at the native response boundary and the
//! existing JSON digest usage; never hand unparsed model output to this stage.
//! The ordinary JSON Tool's legacy parse operation uses last-wins semantics and
//! is not sufficient for this response contract without a qualified successor.
use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    model_completion_components::model_prompt_result_schema,
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const ANSWER_SOURCE: &str = include_str!("../../../scripts/prefix/bootstrap_answer.py");
pub const SYSTEM: &str = include_str!("../../../scripts/prefix/bootstrap/v1/system.txt");
pub const EXTRACT: &str =
    include_str!("../../../scripts/prefix/bootstrap/v1/extract_bootstrap_facts.txt");
pub const SELECT: &str =
    include_str!("../../../scripts/prefix/bootstrap/v1/select_bootstrap_prefill.txt");
pub const RESPONSE_SCHEMA: &str =
    include_str!("../../../scripts/prefix/bootstrap/v1/answer.schema.json");
pub const PROMPT_MANIFEST: &str =
    include_str!("../../../scripts/prefix/bootstrap/v1/manifest.json");

fn string() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}

fn pure(
    id: Uuid,
    name: &str,
    source: &str,
    inputs: Value,
    result: Value,
    private: Value,
    dependencies: Vec<Uuid>,
) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil()
        || dependencies
            .iter()
            .any(|dependency| dependency.is_nil() || *dependency == id)
        || dependencies
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != dependencies.len()
    {
        return Err(RevisionError::Invalid(
            "bootstrap component identity required",
        ));
    }
    InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":22,
            "document":{"name":name,"description":"Pure bootstrap protocol stage. Native correlation, source provenance, rule eligibility, technical bounds, CAS, dispatch and activation are separate mandatory owner responsibilities.",
                "content":source,"input_contract":inputs,"result_contract":result,
                "preload":{"format":"python-preload/2",
                    "exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},
                    "private_functions":private,"constants":[],"imports":["json"],
                    "dependencies":dependencies,"default_export":name}},
            "dependencies":dependencies,"association":null})
        .to_string(),
    )
}

/// One shared answer/evidence implementation. The finite canonical string
/// envelope follows the existing design stage pattern; it does not add an
/// unsupported `any` or union kind to the retained recursive contract schema.
pub fn bootstrap_answer_draft(
    id: Uuid,
    budget: Uuid,
) -> Result<ComponentRevisionDraft, RevisionError> {
    pure(
        id,
        "bootstrap_answer_stage",
        ANSWER_SOURCE,
        json!({"phase":string(),"data_bytes":string()}),
        json!({"type":"string","checks":[]}),
        json!({
            "bootstrap_fields":["value","names"],
            "bootstrap_string":["value"],
            "bootstrap_fact_key":["value"],
            "validate_bootstrap_answer":["inputs"],
            "resolve_bootstrap_pointer":["source_view","pointer"],
            "check_bootstrap_evidence":["inputs"],
            "bootstrap_prefill_value":["field","value"],
            "bootstrap_prefill_option":["option","observations","existing","policy_ref","context"],
            "check_bootstrap_prefill":["inputs"]
        }),
        vec![budget],
    )
}

/// Render fixed instructions and one canonical captured packet into the
/// existing typed model-prompt profile. This performs no LLM call and selects
/// no generated prefix. The retained qualified workflow supplies that selection.
pub fn bootstrap_prompt_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let extract = EXTRACT
        .strip_suffix("{{INPUT_PACKET_JSON}}\n")
        .ok_or(RevisionError::Invalid(
            "bootstrap extract prompt marker differs",
        ))?;
    let select = SELECT
        .strip_suffix("{{INPUT_PACKET_JSON}}\n")
        .ok_or(RevisionError::Invalid(
            "bootstrap select prompt marker differs",
        ))?;
    let source = format!(
        "def prepare_bootstrap_prompt(inputs):\n    import json\n    output_tokens = inputs['max_output_tokens']\n    if type(output_tokens) is not int or output_tokens <= 0 or output_tokens > {output_max}:\n        raise ValueError('explicit positive output reservation required')\n    packet = json.loads(inputs['packet_bytes'])\n    canonical = json.dumps(packet, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)\n    if canonical != inputs['packet_bytes'] or not isinstance(packet, dict) or sorted(packet) != ['input_sha256', 'payload', 'request_id']:\n        raise ValueError('canonical captured bootstrap packet required')\n    if not isinstance(packet['request_id'], str) or not packet['request_id'].strip():\n        raise ValueError('original bootstrap request identity required')\n    digest = packet['input_sha256']\n    if not isinstance(digest, str) or len(digest) != 64 or any(ch not in '0123456789abcdef' for ch in digest):\n        raise ValueError('exact bootstrap payload hash required')\n    payload = packet['payload']\n    if not isinstance(payload, dict) or sorted(payload) != ['checked_observations', 'eligible_options', 'executing_pair', 'existing_values', 'required_facts', 'response_contract', 'sources', 'target', 'task']:\n        raise ValueError('exact bootstrap payload fields required')\n    if payload['response_contract'] != json.loads({schema}):\n        raise ValueError('bootstrap response contract differs from packaged schema')\n    if payload['task'] == 'extract_bootstrap_facts':\n        instruction = {extract}\n    elif payload['task'] == 'select_bootstrap_prefill':\n        instruction = {select}\n    else:\n        raise ValueError('unknown bootstrap prompt task')\n    return {{'user_query':instruction + canonical, 'chat_history':[{{'role':'system','content':{system}}}], 'max_output_tokens':output_tokens}}\n",
        schema = json!(RESPONSE_SCHEMA),
        extract = json!(extract),
        select = json!(select),
        system = json!(SYSTEM),
        output_max = u32::MAX,
    );
    pure(
        id,
        "prepare_bootstrap_prompt",
        &source,
        json!({"packet_bytes":string(),"max_output_tokens":{
            "type":"integer","required":true,"checks":[
                {"kind":"min","value":1},{"kind":"max","value":u32::MAX}
            ]
        }}),
        model_prompt_result_schema(),
        json!({}),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_contract::strict_json;
    use sha2::{Digest, Sha256};

    #[test]
    fn bootstrap_drafts_use_existing_contract_and_packaged_integrity() {
        let budget = Uuid::new_v4();
        let answer = bootstrap_answer_draft(Uuid::new_v4(), budget).unwrap();
        let prompt = bootstrap_prompt_draft(Uuid::new_v4()).unwrap();
        for draft in [&answer, &prompt] {
            let document = draft.document();
            InputContract::from_value(&document["input_contract"], REVISION_LIMITS).unwrap();
            ValueContract::result_from_value(&document["result_contract"], REVISION_LIMITS)
                .unwrap();
            assert_eq!(
                document["preload"]["dependencies"],
                json!(draft.dependencies())
            );
        }
        assert_eq!(
            prompt.document()["input_contract"]["max_output_tokens"]["required"],
            true
        );
        assert_eq!(
            prompt.document()["result_contract"]["fields"]["max_output_tokens"]["required"],
            false
        );
        let manifest = strict_json(PROMPT_MANIFEST, REVISION_LIMITS).unwrap();
        for (name, bytes) in [
            ("system.txt", SYSTEM),
            ("extract_bootstrap_facts.txt", EXTRACT),
            ("select_bootstrap_prefill.txt", SELECT),
            ("answer.schema.json", RESPONSE_SCHEMA),
        ] {
            let checksum = format!("{:x}", Sha256::digest(bytes.as_bytes()));
            assert_eq!(
                manifest["artifacts"][name].as_str(),
                Some(checksum.as_str())
            );
        }
        assert_eq!(
            answer.dependencies(),
            &std::collections::BTreeSet::from([budget])
        );
        assert_eq!(
            answer.document()["preload"]["dependencies"],
            json!([budget])
        );
        let id = Uuid::new_v4();
        assert!(bootstrap_answer_draft(Uuid::nil(), budget).is_err());
        assert!(bootstrap_answer_draft(id, Uuid::nil()).is_err());
        assert!(bootstrap_answer_draft(id, id).is_err());
        assert!(bootstrap_prompt_draft(Uuid::nil()).is_err());
    }

    #[test]
    fn bootstrap_raw_response_requires_existing_strict_reader() {
        for invalid in [
            r#"{"result":{"value":1,"value":2}}"#,
            r#"{"value":NaN}"#,
            r#"{"value":Infinity}"#,
            r#"{} {}"#,
            "```json\n{}\n```",
        ] {
            assert!(strict_json(invalid, REVISION_LIMITS).is_err());
        }
        assert!(strict_json(r#"{ "value" : 1 }"#, REVISION_LIMITS).is_ok());
    }
}
