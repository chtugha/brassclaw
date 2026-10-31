//! Data-only, unaccepted pair-construction proposals. Integrity and literal case
//! consistency are checked here; no template, instruction or provider is executed.
use super::prefix_model_designs::{PROPOSAL_LIMITS, bad, hash};
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixModelArtifactDetail,
};
use brassclaw_skills::value_contract::strict_json;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const FORMAT: &str = "brassclaw-prefix-pair-construction-proposal/1";
pub(crate) const WIRE_FORMAT: &str = "brassclaw-prefix-pair-construction-proposal/2";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    format: String,
    pub(crate) specification: String,
    pub(crate) artifact_set: String,
    rationale: String,
    evidence: Vec<Evidence>,
    template: Template,
    construction: Construction,
    sample_reference: String,
    worked_cases: Vec<Case>,
}
impl Document {
    pub(crate) fn request_output_tokens(&self) -> Option<u32> {
        self.construction.request_output_tokens
    }
    pub(crate) fn template_sha256(&self) -> &str {
        &self.template.template_sha256
    }
    pub(crate) fn fixed_prefix(&self, reference: &str) -> String {
        let reference = match self.construction.reference_normalization.as_deref() {
            Some("unicode-trim") => reference.trim(),
            _ => reference,
        };
        format!(
            "{}{}{}",
            self.construction.prefix.leading_literal,
            reference,
            self.construction.prefix.trailing_literal
        )
    }
    pub(crate) fn review_cases(&self) -> impl Iterator<Item = (&str, &str, &str, &str)> {
        self.worked_cases.iter().filter_map(|c| {
            c.wire_request_json.as_deref().map(|wire| {
                (
                    c.id.as_str(),
                    wire,
                    c.expected_fixed_prefix.as_str(),
                    c.expected_request.as_str(),
                )
            })
        })
    }
    pub(crate) fn sample_reference(&self) -> &str {
        &self.sample_reference
    }
    pub(crate) fn supports_mode(&self, mode: &str) -> bool {
        self.construction.supported_modes.iter().any(|m| m == mode)
    }
    pub(crate) fn case_count(&self) -> usize {
        self.worked_cases.len()
    }
    /// Data only, never executes prose or certifies a provider mapping.
    pub(crate) fn wire_mapping_json(&self) -> Option<&str> {
        self.construction.wire_mapping_json.as_deref()
    }

    /// Complete immutable case projection. This neither renders nor validates a
    /// model template, and omits no declared worked case.
    pub(crate) fn case_packet(&self, proposal: &str) -> Value {
        serde_json::json!({"format":"brassclaw-prefix-pair-case-packet/1",
            "proposal":proposal,"specification":self.specification,"artifact_set":self.artifact_set,
            "template_sha256":self.template.template_sha256,
            "cases":self.worked_cases.iter().map(|case|serde_json::json!({
                "id":case.id,"mode":case.mode,"context_json":case.render_context_json,
                "expected_prefix":case.expected_fixed_prefix,"expected_render":case.expected_request
            })).collect::<Vec<_>>(),"qualification":"unqualified-case-input"})
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    id: String,
    file: String,
    file_sha256: String,
    pointer: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    evidence: String,
    template_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instruction {
    instructions: String,
    evidence: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prefix {
    leading_literal: String,
    trailing_literal: String,
    edge_requirement: String,
    instructions: String,
    evidence: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Construction {
    prefix: Prefix,
    dynamic_prompt: Instruction,
    special_tokens: Instruction,
    memory: Instruction,
    supported_modes: Vec<String>,
    unsupported_modes: BTreeMap<String, String>,
    join: String,
    hard_limits_owner: String,
    runtime_cache: Value,
    #[serde(default)]
    wire_mapping_json: Option<String>,
    #[serde(default)]
    request_output_tokens: Option<u32>,
    #[serde(default)]
    reference_normalization: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    mode: String,
    render_context_json: String,
    expected_fixed_prefix: String,
    formatted_prompt: String,
    expected_request: String,
    evidence: Vec<String>,
    #[serde(default)]
    wire_request_json: Option<String>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn pointer_syntax(value: &str) -> bool {
    if !value.is_empty() && !value.starts_with('/') {
        return false;
    }
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}
fn references(values: &[String], known: &BTreeSet<&str>) -> bool {
    !values.is_empty()
        && values.iter().all(|value| known.contains(value.as_str()))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn instruction(value: &Instruction, known: &BTreeSet<&str>) -> bool {
    !value.instructions.trim().is_empty() && references(&value.evidence, known)
}

pub(crate) fn parse(specification: &str, bytes: &str) -> Result<Document, Error> {
    hash(specification)?;
    let value = strict_json(bytes, PROPOSAL_LIMITS).map_err(|_| bad())?;
    let mapping_present = value["construction"].get("wire_mapping_json").is_some();
    let output_present = value["construction"].get("request_output_tokens").is_some();
    let normalization_present = value["construction"]
        .get("reference_normalization")
        .is_some();
    if value["format"] == FORMAT
        && value["worked_cases"]
            .as_array()
            .is_some_and(|cases| cases.iter().any(|c| c.get("wire_request_json").is_some()))
    {
        return Err(bad());
    }
    let document: Document = serde_json::from_value(value).map_err(|_| bad())?;
    match document.format.as_str() {
        FORMAT if !mapping_present && !output_present && !normalization_present => {}
        WIRE_FORMAT
            if document
                .construction
                .request_output_tokens
                .is_some_and(|n| n > 0)
                && matches!(
                    document.construction.reference_normalization.as_deref(),
                    Some("preserve" | "unicode-trim")
                ) =>
        {
            brassclaw_model_encoding::wire::validate_mapping(
                document
                    .construction
                    .wire_mapping_json
                    .as_deref()
                    .ok_or_else(bad)?,
            )
            .map_err(|_| bad())?;
        }
        _ => return Err(bad()),
    }
    hash(&document.artifact_set)?;
    if document.specification != specification
        || document.rationale.trim().is_empty()
        || document.sample_reference.is_empty()
        || document.evidence.is_empty()
    {
        return Err(bad());
    }
    let known: BTreeSet<_> = document
        .evidence
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    if known.len() != document.evidence.len() || known.iter().any(|id| !identifier(id)) {
        return Err(bad());
    }
    hash(&document.template.template_sha256)?;
    if !known.contains(document.template.evidence.as_str()) {
        return Err(bad());
    }
    for entry in &document.evidence {
        hash(&entry.file_sha256)?;
        if !entry.pointer.is_null()
            && entry
                .pointer
                .as_str()
                .is_none_or(|pointer| !pointer_syntax(pointer))
        {
            return Err(bad());
        }
    }
    let rules = &document.construction;
    let prefix = &rules.prefix;
    if prefix.instructions.trim().is_empty()
        || !references(&prefix.evidence, &known)
        || !instruction(&rules.dynamic_prompt, &known)
        || !instruction(&rules.special_tokens, &known)
        || !instruction(&rules.memory, &known)
        || rules.join != "prepend-fixed-prefix"
        || rules.hard_limits_owner != "provider-definition-revision"
        || !rules.runtime_cache.is_null()
        || !matches!(
            prefix.edge_requirement.as_str(),
            "preserve-exact" | "non-whitespace-edges"
        )
        || (prefix.edge_requirement == "non-whitespace-edges"
            && document.sample_reference.trim() != document.sample_reference)
    {
        return Err(bad());
    }
    let modes: BTreeSet<_> = rules.supported_modes.iter().map(String::as_str).collect();
    if (document.format == WIRE_FORMAT
        && modes.iter().any(|mode| !matches!(*mode, "plain" | "tools")))
        || modes.is_empty()
        || modes.len() != rules.supported_modes.len()
        || modes.iter().any(|mode| !identifier(mode))
        || rules.unsupported_modes.iter().any(|(mode, reason)| {
            !identifier(mode) || modes.contains(mode.as_str()) || reason.trim().is_empty()
        })
    {
        return Err(bad());
    }
    let fixed = document.fixed_prefix(&document.sample_reference);
    let mut ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for case in &document.worked_cases {
        match document.format.as_str() {
            FORMAT if case.wire_request_json.is_none() => {}
            WIRE_FORMAT
                if case
                    .wire_request_json
                    .as_ref()
                    .is_some_and(|s| !s.is_empty()) => {}
            _ => return Err(bad()),
        }
        if !identifier(&case.id)
            || !ids.insert(&case.id)
            || !modes.contains(case.mode.as_str())
            || !references(&case.evidence, &known)
            || case.expected_fixed_prefix != fixed
            || case.expected_request != format!("{fixed}{}", case.formatted_prompt)
        {
            return Err(bad());
        }
        // Context is original ordered data, never executable source. Parsing here
        // proves syntax only; no claim that this expected render matches a template.
        let context = strict_json(&case.render_context_json, PROPOSAL_LIMITS).map_err(|_| bad())?;
        if !context.is_object() {
            return Err(bad());
        }
        covered.insert(case.mode.as_str());
    }
    if covered != modes {
        return Err(bad());
    }
    Ok(document)
}

pub(crate) fn verify_sources(
    document: &Document,
    detail: &PrefixModelArtifactDetail,
) -> Result<(), Error> {
    if document.specification != detail.summary.specification
        || document.artifact_set != detail.summary.id
    {
        return Err(bad());
    }
    let observation: Value =
        serde_json::from_str(detail.format_observation.as_ref().ok_or_else(bad)?)
            .map_err(|_| bad())?;
    if observation["format"] != "brassclaw-prefix-model-format-observation/1"
        || observation["qualification"] != "retained-format-sources-only"
        || observation["specification"] != document.specification
        || observation["artifact_set"] != document.artifact_set
        || observation["status"] != "observed-format-sources"
        || observation.get("effective_template") != Some(&Value::Null)
        || observation.get("serving_compatibility") != Some(&Value::Null)
    {
        return Err(bad());
    }
    let package: Value = serde_json::from_str(&detail.artifact_set_bytes).map_err(|_| bad())?;
    let mut decoded = BTreeMap::new();
    for entry in &document.evidence {
        if !matches!(
            entry.file.as_str(),
            "config.json"
                | "tokenizer_config.json"
                | "chat_template.jinja"
                | "special_tokens_map.json"
                | "added_tokens.json"
                | "README.md"
        ) {
            return Err(bad());
        }
        let original = detail
            .summary
            .files
            .iter()
            .find(|file| file.name == entry.file)
            .ok_or_else(bad)?;
        if original.sha256 != entry.file_sha256 {
            return Err(bad());
        }
        if !decoded.contains_key(&entry.file) {
            let encoded = package["artifacts"][&entry.file]["data"]
                .as_str()
                .ok_or_else(bad)?;
            let bytes = STANDARD.decode(encoded).map_err(|_| bad())?;
            if bytes.len() as u64 != original.bytes
                || hex::encode(Sha256::digest(&bytes)) != original.sha256
            {
                return Err(bad());
            }
            decoded.insert(entry.file.clone(), bytes);
        }
        match &entry.pointer {
            Value::Null => {}
            Value::String(pointer)
                if !matches!(entry.file.as_str(), "chat_template.jinja" | "README.md") =>
            {
                let text = std::str::from_utf8(&decoded[&entry.file]).map_err(|_| bad())?;
                let json = strict_json(text, PROPOSAL_LIMITS).map_err(|_| bad())?;
                if json.pointer(pointer).is_none() {
                    return Err(bad());
                }
            }
            _ => return Err(bad()),
        }
    }
    let source = document
        .evidence
        .iter()
        .find(|source| source.id == document.template.evidence)
        .ok_or_else(bad)?;
    if !observation["templates"]
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .any(|template| {
            template["file"] == source.file
                && template["pointer"] == source.pointer
                && template["template_sha256"] == document.template.template_sha256
        })
    {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    // A draft is an operator's data, not expected tokenizer or provider behavior.
    pub(crate) fn draft(
        specification: &str,
        artifact_set: &str,
        template_file: &str,
        template_file_sha: &str,
        pointer: Value,
        template_sha: &str,
        config_sha: &str,
    ) -> Value {
        let instruction = |text| json!({"instructions":text,"evidence":["template"]});
        json!({"format":FORMAT,"specification":specification,"artifact_set":artifact_set,
            "rationale":"Source-bound construction for independent review; no effective-serving claim",
            "evidence":[{"id":"template","file":template_file,"file_sha256":template_file_sha,"pointer":pointer},
                {"id":"architecture","file":"config.json","file_sha256":config_sha,"pointer":null}],
            "template":{"evidence":"template","template_sha256":template_sha},
            "construction":{
                "prefix":{"leading_literal":"<|im_start|>system\n","trailing_literal":"<|im_end|>\n",
                    "edge_requirement":"non-whitespace-edges","instructions":"Freeze the canonical reference inside the exact observed system frame; never edit it in Kohai.","evidence":["template"]},
                "dynamic_prompt":instruction("Optional connected Sempai first reviews the Orchestrator messages using its own generated reviewer prefix and accepted target-pair design. Kohai then formats only the reviewed dynamic prompt using the pinned target-pair design and adds the finished generated prefix unchanged exactly once. Generator, Kohai and same-pair Sempai share the same accepted design revision. Qualify the stable prefix boundary against the actual SDK system-preamble join, history and supported Tool serialization; unsupported modes remain blocked."),
                "special_tokens":instruction("Count the complete already framed text without adding a second BOS/EOS layer; qualify this against the exact tokenizer."),
                "memory":{"instructions":"Architecture declarations are not effective KV/recurrent storage. Runtime geometry remains unknown.","evidence":["architecture"]},
                "supported_modes":["plain"],"unsupported_modes":{"tools":"Default tool placement requires independent adapter qualification"},
                "join":"prepend-fixed-prefix","hard_limits_owner":"provider-definition-revision","runtime_cache":null},
            "sample_reference":"Reference ü",
            "worked_cases":[{"id":"plain_unicode","mode":"plain","render_context_json":"{\"messages\":[{\"role\":\"user\",\"content\":\"Frage ü\"}]}",
                "expected_fixed_prefix":"<|im_start|>system\nReference ü<|im_end|>\n",
                "formatted_prompt":"<|im_start|>user\nFrage ü<|im_end|>\n<|im_start|>assistant\n",
                "expected_request":"<|im_start|>system\nReference ü<|im_end|>\n<|im_start|>user\nFrage ü<|im_end|>\n<|im_start|>assistant\n","evidence":["template"]}]})
    }
    fn specimen() -> Value {
        draft(
            &"a".repeat(64),
            &"b".repeat(64),
            "chat_template.jinja",
            &"c".repeat(64),
            Value::Null,
            &"d".repeat(64),
            &"e".repeat(64),
        )
    }
    #[test]
    fn pair_drafts_require_literal_consistency_and_explicit_unknowns_without_limit_copies() {
        let value = specimen();
        parse(&"a".repeat(64), &value.to_string()).unwrap();
        for (pointer, replacement) in [
            ("/construction/runtime_cache", json!({"dtype":"int4"})),
            ("/construction/join", json!("server-injection")),
            (
                "/worked_cases/0/expected_request",
                json!("reformatted prefix"),
            ),
            ("/construction/supported_modes", json!(["plain", "tools"])),
            (
                "/construction/dynamic_prompt/evidence",
                json!(["unretained"]),
            ),
            ("/sample_reference", json!(" Reference ü ")),
            ("/evidence/0/pointer", json!("/~invalid")),
        ] {
            let mut invalid = value.clone();
            *invalid.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                parse(&"a".repeat(64), &invalid.to_string()).is_err(),
                "{pointer}"
            );
        }
        for key in [
            "max_prefix_tokens",
            "max_prompt_tokens",
            "profile",
            "accepted",
            "compiler_code",
        ] {
            let mut invalid = value.clone();
            invalid["construction"][key] = json!(100000);
            assert!(
                parse(&"a".repeat(64), &invalid.to_string()).is_err(),
                "{key}"
            );
        }
        let mut missing = value.clone();
        missing["construction"]
            .as_object_mut()
            .unwrap()
            .remove("runtime_cache");
        assert!(parse(&"a".repeat(64), &missing.to_string()).is_err());
        let duplicate = value
            .to_string()
            .replacen("{", "{\"format\":\"forged\",", 1);
        assert!(parse(&"a".repeat(64), &duplicate).is_err());
    }
    #[test]
    fn wire_mapping_successor_is_explicit_data_and_old_proposals_remain_exact() {
        let mut value = specimen();
        let old = value.to_string();
        assert!(
            parse(&"a".repeat(64), &old)
                .unwrap()
                .wire_mapping_json()
                .is_none()
        );
        value["construction"]["wire_mapping_json"] = Value::Null;
        assert!(
            parse(&"a".repeat(64), &value.to_string()).is_err(),
            "v1 cannot acquire new semantics through null or defaults"
        );
        value["format"] = json!(WIRE_FORMAT);
        value["construction"]["request_output_tokens"] = json!(1);
        value["construction"]["reference_normalization"] = json!("preserve");
        assert!(
            parse(&"a".repeat(64), &value.to_string()).is_err(),
            "v2 needs an explicit mapping"
        );
        let mapping=json!({"format":brassclaw_model_encoding::wire::WIRE_MAPPING,
            "render_defaults_json":r#"{"add_generation_prompt":true,"continue_final_message":false}"#,
            "missing_tools":"null","null_assistant_content":"preserve","tool_arguments":"preserve","text_parts":{"mode":"join","separator":"\n"},
            "forward_tool_choice":true,"forward_response_format":true,"allowed_tool_choices":["auto","none","required"],
            "allowed_chat_kwargs":[],"allowed_message_fields":["role","content"]}).to_string();
        value["construction"]["wire_mapping_json"] = json!(mapping);
        value["worked_cases"][0]["wire_request_json"] =
            json!("{\"model\":\"fixture\",\"messages\":[],\"max_tokens\":1}");
        let result = parse(&"a".repeat(64), &value.to_string()).unwrap();
        assert_eq!(result.wire_mapping_json(), Some(mapping.as_str()));
        value["construction"]["wire_mapping_json"] = json!("{\"format\":\"future-or-unknown\"}");
        assert!(parse(&"a".repeat(64), &value.to_string()).is_err());
        assert!(
            parse(&"a".repeat(64), &old)
                .unwrap()
                .wire_mapping_json()
                .is_none(),
            "original revision stays unchanged"
        );
    }
}
