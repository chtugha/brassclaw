//! Read-only projection of exact retained configuration data. This is neither
//! model compatibility nor effective serving/cache evidence. No inference,
//! provider discovery, defaults, source execution or storage mutation occurs.
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_product_workflow::{InterceptorConfigServiceError as Error, PrefixModelArtifactFile};
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// A bounded metadata view, independent of the original artifact storage limit.
// Oversized/unsupported configuration stays retained and visibly uninspected.
const LIMITS: ContractLimits = ContractLimits {
    max_bytes: 1024 * 1024,
    max_nodes: 16384,
    max_depth: 32,
};
const FIELDS: &[&str] = &[
    "model_type",
    "bos_token_id",
    "eos_token_id",
    "pad_token_id",
    "vocab_size",
    "architectures",
    "hidden_size",
    "num_hidden_layers",
    "num_attention_heads",
    "num_key_value_heads",
    "head_dim",
    "max_position_embeddings",
    "layer_types",
    "full_attention_interval",
    "linear_conv_kernel_dim",
    "linear_key_head_dim",
    "linear_value_head_dim",
    "linear_num_key_heads",
    "linear_num_value_heads",
    "torch_dtype",
    "dtype",
    "ssm_dtype",
    "mamba_ssm_dtype",
    "mamba_conv_dtype",
    "sliding_window",
    "attention_chunk_size",
    "use_cache",
];

#[derive(Deserialize)]
struct File<'a> {
    #[serde(borrow)]
    data: &'a str,
    #[serde(borrow)]
    sha256: &'a str,
}
#[derive(Deserialize)]
struct Package<'a> {
    #[serde(borrow)]
    artifacts: BTreeMap<&'a str, File<'a>>,
}

pub(crate) fn observe(
    specification: &str,
    artifact_set: &str,
    bytes: &str,
    inventory: &[PrefixModelArtifactFile],
) -> Result<String, Error> {
    let entry = inventory
        .iter()
        .find(|file| file.name == "config.json")
        .ok_or(Error::Unavailable)?;
    let mut report = json!({"format":"brassclaw-prefix-model-config-observation/1",
        "specification":specification,"artifact_set":artifact_set,
        "config_sha256":entry.sha256,"qualification":"declared-config-only",
        "status":"uninspected","fields":{},"runtime_cache":null,
        "weights_identity":null,"serving_context_limit":null,
        "attention_kv_cache":null,"recurrent_state":null});
    if entry.bytes > LIMITS.max_bytes as u64 {
        report["status"] = json!("inspection-capacity-exceeded");
    } else {
        // Borrow the canonical base64 strings; never copy tokenizer/model files
        // merely to display a configuration. The owning store verifies the full
        // package first. This projection checks its selected original as well.
        let package: Package<'_> = serde_json::from_str(bytes).map_err(|_| Error::Unavailable)?;
        let original = retained_original(&package, entry)?;
        let config = std::str::from_utf8(&original)
            .ok()
            .and_then(|text| strict_json(text, LIMITS).ok())
            .filter(Value::is_object);
        if let Some(config) = config {
            let mut fields = serde_json::Map::new();
            for prefix in ["", "/text_config"] {
                for name in FIELDS {
                    let pointer = format!("{prefix}/{name}");
                    if let Some(value) = config.pointer(&pointer) {
                        fields.insert(pointer, value.clone());
                    }
                }
            }
            // Observe weight quantization declarations without inferring any
            // attention/recurrent dtype or exposing arbitrary config fields.
            observe_quantization(&config, &mut fields);
            report["status"] = json!("observed-declarations");
            report["fields"] = Value::Object(fields);
        } else {
            report["status"] = json!("unsupported-or-invalid-config");
        }
    }
    serde_json::to_string(&report).map_err(|_| Error::Unavailable)
}

// These remain literal declarations. In particular, weight bit-width and a
// declared cache scheme are not the effective serving KV/recurrent-state dtype.
fn observe_quantization(config: &Value, fields: &mut serde_json::Map<String, Value>) {
    for prefix in ["/quantization_config", "/text_config/quantization_config"] {
        for name in [
            "quant_method",
            "bits",
            "group_size",
            "desc_act",
            "sym",
            "format",
            "quantization_status",
            "version",
            "ignore",
        ] {
            let pointer = format!("{prefix}/{name}");
            if let Some(value) = config.pointer(&pointer) {
                fields.insert(pointer, value.clone());
            }
        }
        let Some(groups) = config
            .pointer(&format!("{prefix}/config_groups"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (name, group) in groups {
            // Keep pointers resolvable even for nonstandard group identifiers.
            let name = name.replace('~', "~0").replace('/', "~1");
            let base = format!("{prefix}/config_groups/{name}");
            for key in ["format", "targets"] {
                if let Some(value) = group.get(key) {
                    fields.insert(format!("{base}/{key}"), value.clone());
                }
            }
            for kind in ["weights", "input_activations", "output_activations"] {
                let Some(parameters) = group.get(kind) else {
                    continue;
                };
                if parameters.is_null() {
                    fields.insert(format!("{base}/{kind}"), Value::Null);
                    continue;
                }
                for key in [
                    "num_bits",
                    "type",
                    "strategy",
                    "group_size",
                    "symmetric",
                    "dynamic",
                    "actorder",
                    "block_structure",
                    "scale_dtype",
                    "zp_dtype",
                ] {
                    if let Some(value) = parameters.get(key) {
                        fields.insert(format!("{base}/{kind}/{key}"), value.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(text: &str) -> Value {
        let digest = hex::encode(Sha256::digest(text.as_bytes()));
        let file = PrefixModelArtifactFile {
            name: "config.json".into(),
            sha256: digest.clone(),
            bytes: text.len() as u64,
        };
        let package =
            json!({"artifacts":{"config.json":{"data":STANDARD.encode(text),"sha256":digest}}});
        serde_json::from_str(&observe("spec", "assets", &package.to_string(), &[file]).unwrap())
            .unwrap()
    }
    #[test]
    fn compressed_tensor_groups_preserve_exact_source_pointers_without_runtime_inference() {
        let config = json!({"quantization_config": {
            "quant_method":"compressed-tensors", "format":"pack-quantized",
            "quantization_status":"compressed", "ignore":["linear_attn.in_proj_a"],
            "config_groups":{"group/~0": {
                "targets":["Linear"], "input_activations":null,
                "weights":{"num_bits":4,"group_size":32,"symmetric":false,"type":"int","token":"private"},
                "token":"private"
            }},"token":"private"
        }});
        let report = observation(&config.to_string());
        let fields = report["fields"].as_object().unwrap();
        assert_eq!(
            fields["/quantization_config/quant_method"],
            "compressed-tensors"
        );
        assert_eq!(
            fields["/quantization_config/config_groups/group~1~00/weights/num_bits"],
            4
        );
        assert_eq!(
            fields["/quantization_config/config_groups/group~1~00/weights/symmetric"],
            false
        );
        assert!(
            fields["/quantization_config/config_groups/group~1~00/input_activations"].is_null()
        );
        for (pointer, value) in fields {
            assert_eq!(config.pointer(pointer), Some(value));
        }
        assert!(!report.to_string().contains("private"));
        assert_eq!(report["qualification"], "declared-config-only");
        assert!(report["attention_kv_cache"].is_null());
        assert!(report["recurrent_state"].is_null());
        let mut nested = json!({"text_config":config});
        nested["text_config"]["quantization_config"]["config_groups"]["group/~0"]["weights"]["num_bits"] =
            json!(8);
        let nested_report = observation(&nested.to_string());
        assert_eq!(
            nested_report["fields"]["/text_config/quantization_config/config_groups/group~1~00/weights/num_bits"],
            8
        );
        assert!(
            nested_report["fields"]
                .get("/quantization_config/bits")
                .is_none()
        );
    }

    #[test]
    fn configuration_observation_preserves_declarations_without_inventing_runtime_facts() {
        let report = observation(
            r#"{"model_type":"unfamiliar","text_config":{"num_hidden_layers":4,"layer_types":["linear_attention","linear_attention","linear_attention","full_attention"],"max_position_embeddings":262144,"ssm_dtype":null,"mamba_ssm_dtype":"float32"},"quantization_config":{"quant_method":"awq","bits":4,"token":"private"},"token":"private","auto_map":{"AutoModel":"arbitrary.program"}}"#,
        );
        assert_eq!(report["fields"]["/model_type"], "unfamiliar");
        assert_eq!(report["fields"]["/text_config/num_hidden_layers"], 4);
        assert_eq!(report["fields"]["/quantization_config/bits"], 4);
        assert_eq!(
            report["fields"]["/text_config/layer_types"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert!(
            report["fields"]
                .as_object()
                .unwrap()
                .contains_key("/text_config/ssm_dtype")
        );
        assert!(
            !report["fields"]
                .as_object()
                .unwrap()
                .contains_key("/torch_dtype")
        );
        assert_eq!(report["fields"]["/text_config/mamba_ssm_dtype"], "float32");
        assert_eq!(report["qualification"], "declared-config-only");
        for name in [
            "runtime_cache",
            "weights_identity",
            "serving_context_limit",
            "attention_kv_cache",
            "recurrent_state",
        ] {
            assert!(report[name].is_null());
        }
        assert!(!report.to_string().contains("private"));
        assert!(!report.to_string().contains("arbitrary.program"));
        for invalid in [
            r#"{"num_hidden_layers":4,"num_hidden_layers":8}"#,
            "null",
            "not JSON",
        ] {
            let report = observation(invalid);
            assert_eq!(report["status"], "unsupported-or-invalid-config");
            assert_eq!(report["fields"], json!({}));
        }
        let oversized = observation(&" ".repeat(LIMITS.max_bytes + 1));
        assert_eq!(oversized["status"], "inspection-capacity-exceeded");
    }
}

// Exact template/tokenizer sources are data for design construction, not an
// inferred effective template or an executable/default-selection instruction.
const FORMAT_FILES: &[&str] = &[
    "tokenizer_config.json",
    "special_tokens_map.json",
    "added_tokens.json",
    "chat_template.jinja",
];
const TOKENIZER_FIELDS: &[&str] = &[
    "bos_token",
    "eos_token",
    "pad_token",
    "unk_token",
    "additional_special_tokens",
    "added_tokens_decoder",
    "added_tokens_encoder",
    "add_bos_token",
    "add_eos_token",
    "add_prefix_space",
    "clean_up_tokenization_spaces",
    "split_special_tokens",
    "model_max_length",
    "tokenizer_class",
];

fn retained_original(
    package: &Package<'_>,
    entry: &PrefixModelArtifactFile,
) -> Result<Vec<u8>, Error> {
    let file = package
        .artifacts
        .get(entry.name.as_str())
        .ok_or(Error::Unavailable)?;
    let original = STANDARD.decode(file.data).map_err(|_| Error::Unavailable)?;
    if original.len() as u64 != entry.bytes
        || file.sha256 != entry.sha256
        || hex::encode(Sha256::digest(&original)) != entry.sha256
    {
        return Err(Error::Unavailable);
    }
    Ok(original)
}

fn template_candidate(
    file: &str,
    pointer: Option<String>,
    name: Option<&str>,
    text: &str,
) -> Value {
    json!({"file":file,"pointer":pointer,"name":name,"template":text,
        "template_sha256":hex::encode(Sha256::digest(text.as_bytes()))})
}

fn declared_templates(value: Option<&Value>, file: &str) -> Option<(String, Vec<Value>)> {
    let mut templates = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    let mut add = |name: Option<&str>, text: &Value, pointer: String| {
        let text = text.as_str()?;
        if text.trim().is_empty()
            || name.is_some_and(|name| name.is_empty() || !names.insert(name.to_owned()))
        {
            return None;
        }
        templates.push(template_candidate(file, Some(pointer), name, text));
        Some(())
    };
    let shape = match value {
        None => "missing",
        Some(Value::Null) => "null",
        Some(value @ Value::String(_)) => {
            add(None, value, "/chat_template".into())?;
            "single"
        }
        Some(Value::Object(values)) => {
            for (name, text) in values {
                let key = name.replace('~', "~0").replace('/', "~1");
                add(Some(name), text, format!("/chat_template/{key}"))?;
            }
            "named-map"
        }
        Some(Value::Array(values)) => {
            for (index, value) in values.iter().enumerate() {
                let object = value.as_object()?;
                if object.len() != 2 {
                    return None;
                }
                let name = object.get("name")?.as_str()?;
                add(
                    Some(name),
                    object.get("template")?,
                    format!("/chat_template/{index}/template"),
                )?;
            }
            "named-list"
        }
        _ => return None,
    };
    Some((shape.into(), templates))
}

pub(crate) fn observe_format_sources(
    specification: &str,
    artifact_set: &str,
    bytes: &str,
    inventory: &[PrefixModelArtifactFile],
) -> Result<String, Error> {
    let selected: Vec<_> = inventory
        .iter()
        .filter(|file| FORMAT_FILES.contains(&file.name.as_str()))
        .collect();
    let mut names = std::collections::BTreeSet::new();
    if selected
        .iter()
        .any(|file| !names.insert(file.name.as_str()))
        || !names.contains("tokenizer_config.json")
    {
        return Err(Error::Unavailable);
    }
    let mut report = json!({"format":"brassclaw-prefix-model-format-observation/1",
        "specification":specification,"artifact_set":artifact_set,
        "qualification":"retained-format-sources-only","status":"no-declared-template",
        "sources":selected.iter().map(|file| json!({"file":file.name,"sha256":file.sha256,"bytes":file.bytes})).collect::<Vec<_>>(),
        "source_shapes":{},"fields":{},"templates":[],"effective_template":null,"serving_compatibility":null});
    let total = selected
        .iter()
        .try_fold(0u64, |sum, file| sum.checked_add(file.bytes));
    if total.is_none_or(|total| total > LIMITS.max_bytes as u64) {
        report["status"] = json!("inspection-capacity-exceeded");
    } else {
        let package: Package<'_> = serde_json::from_str(bytes).map_err(|_| Error::Unavailable)?;
        let mut templates = Vec::new();
        let mut fields = serde_json::Map::new();
        let mut shapes = serde_json::Map::new();
        let mut invalid = false;
        for entry in selected {
            let original = retained_original(&package, entry)?;
            let text = std::str::from_utf8(&original).map_err(|_| Error::Unavailable)?;
            if entry.name == "chat_template.jinja" {
                if text.trim().is_empty() {
                    invalid = true;
                    break;
                }
                templates.push(template_candidate(&entry.name, None, None, text));
                shapes.insert(entry.name.clone(), json!("standalone-source"));
                continue;
            }
            let Some(value) = strict_json(text, LIMITS).ok().filter(Value::is_object) else {
                invalid = true;
                break;
            };
            let mut declarations = serde_json::Map::new();
            if matches!(
                entry.name.as_str(),
                "added_tokens.json" | "special_tokens_map.json"
            ) {
                // Preserve the complete designated token maps, including custom
                // token categories and escaped spellings. No effective-ID inference.
                for (key, value) in value.as_object().expect("checked object") {
                    let pointer = key.replace('~', "~0").replace('/', "~1");
                    declarations.insert(format!("/{pointer}"), value.clone());
                }
            } else {
                for key in TOKENIZER_FIELDS {
                    if let Some(value) = value.get(*key) {
                        declarations.insert(format!("/{key}"), value.clone());
                    }
                }
            }
            fields.insert(entry.name.clone(), Value::Object(declarations));
            if entry.name == "tokenizer_config.json" {
                let Some((shape, declared)) =
                    declared_templates(value.get("chat_template"), &entry.name)
                else {
                    invalid = true;
                    break;
                };
                shapes.insert(entry.name.clone(), json!(shape));
                templates.extend(declared);
            }
        }
        if invalid {
            // Reject the complete view; never publish a partial apparently usable
            // template selection or silently prefer another declaration.
            report["status"] = json!("unsupported-or-invalid-source");
        } else {
            report["status"] = json!(if templates.is_empty() {
                "no-declared-template"
            } else {
                "observed-format-sources"
            });
            report["source_shapes"] = Value::Object(shapes);
            report["fields"] = Value::Object(fields);
            report["templates"] = Value::Array(templates);
        }
    }
    let mut result = serde_json::to_string(&report).map_err(|_| Error::Unavailable)?;
    if result.len() > LIMITS.max_bytes {
        report["status"] = json!("inspection-capacity-exceeded");
        report["source_shapes"] = json!({});
        report["fields"] = json!({});
        report["templates"] = json!([]);
        result = serde_json::to_string(&report).map_err(|_| Error::Unavailable)?;
    }
    Ok(result)
}

#[cfg(test)]
mod format_tests {
    use super::*;
    fn package(files: &[(&str, &str)]) -> (String, Vec<PrefixModelArtifactFile>) {
        let mut artifacts = serde_json::Map::new();
        let mut inventory = Vec::new();
        for (name, text) in files {
            let digest = hex::encode(Sha256::digest(text.as_bytes()));
            artifacts.insert(
                (*name).into(),
                json!({"data":STANDARD.encode(text),"sha256":digest}),
            );
            inventory.push(PrefixModelArtifactFile {
                name: (*name).into(),
                sha256: digest,
                bytes: text.len() as u64,
            });
        }
        (json!({"artifacts":artifacts}).to_string(), inventory)
    }
    fn report(files: &[(&str, &str)]) -> Value {
        let (bytes, inventory) = package(files);
        serde_json::from_str(&observe_format_sources("spec", "assets", &bytes, &inventory).unwrap())
            .unwrap()
    }
    #[test]
    fn format_sources_preserve_exact_template_and_tokenizer_declarations_without_execution() {
        let template = "\r\n{{ arbitrary_program('never execute') }} <script>data</script> ä\r\n";
        let report = report(&[
            (
                "tokenizer_config.json",
                r#"{"bos_token":null,"eos_token":{"content":"<eos>"},"added_tokens_decoder":{"42":{"content":"<eos>","special":true,"lstrip":false}},"add_bos_token":false,"model_max_length":18446744073709551615,"api_key":"private-canary","auto_map":"private-program"}"#,
            ),
            ("chat_template.jinja", template),
            ("added_tokens.json", r#"{"<extra/token>":42}"#),
            (
                "special_tokens_map.json",
                r#"{"eos_token":{"content":"<eos>","special":true},"image_token":"<image>"}"#,
            ),
        ]);
        assert_eq!(report["status"], "observed-format-sources");
        assert_eq!(report["templates"][0]["template"], template);
        assert_eq!(
            report["templates"][0]["template_sha256"],
            hex::encode(Sha256::digest(template.as_bytes()))
        );
        assert_eq!(
            report["fields"]["tokenizer_config.json"]["/model_max_length"].as_u64(),
            Some(u64::MAX)
        );
        assert!(
            report["fields"]["tokenizer_config.json"]
                .as_object()
                .unwrap()
                .contains_key("/bos_token")
        );
        assert!(
            !report["fields"]["tokenizer_config.json"]
                .as_object()
                .unwrap()
                .contains_key("/pad_token")
        );
        assert_eq!(
            report["fields"]["tokenizer_config.json"]["/added_tokens_decoder"]["42"]["content"],
            "<eos>"
        );
        assert_eq!(
            report["fields"]["tokenizer_config.json"]["/add_bos_token"],
            false
        );
        assert_eq!(report["fields"]["added_tokens.json"]["/<extra~1token>"], 42);
        assert_eq!(
            report["fields"]["special_tokens_map.json"]["/eos_token"]["content"],
            "<eos>"
        );
        assert_eq!(
            report["fields"]["special_tokens_map.json"]["/image_token"],
            "<image>"
        );
        assert_eq!(report["effective_template"], Value::Null);
        assert_eq!(report["serving_compatibility"], Value::Null);
        assert!(!report.to_string().contains("private-canary"));
        assert!(!report.to_string().contains("private-program"));
    }
    #[test]
    fn named_templates_retain_all_sources_without_selecting_a_default() {
        for tokenizer in [
            r#"{"chat_template":{"default":"D","tool/use":"T"}}"#,
            r#"{"chat_template":[{"name":"default","template":"D"},{"name":"tool_use","template":"T"}]}"#,
        ] {
            let report = report(&[
                ("tokenizer_config.json", tokenizer),
                ("chat_template.jinja", "S"),
            ]);
            assert_eq!(report["status"], "observed-format-sources");
            assert_eq!(report["templates"].as_array().unwrap().len(), 3);
            assert_eq!(report["effective_template"], Value::Null);
            assert!(
                report["templates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["template"] == "D")
            );
            assert!(
                report["templates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["template"] == "T")
            );
        }
        let report = report(&[(
            "tokenizer_config.json",
            r#"{"chat_template":{"tool/use":"T"}}"#,
        )]);
        assert_eq!(
            report["templates"][0]["pointer"],
            "/chat_template/tool~1use"
        );
    }
    #[test]
    fn malformed_or_ambiguous_sources_reject_the_whole_view_without_fallback() {
        for tokenizer in [
            "not JSON",
            "null",
            r#"{"chat_template":"A","chat_template":"B"}"#,
            r#"{"chat_template":2}"#,
            r#"{"chat_template":[{"name":"default","template":"A"},{"name":"default","template":"B"}]}"#,
            r#"{"chat_template":[{"name":"default","template":"A","execute":true}]}"#,
            r#"{"chat_template":"   "}"#,
        ] {
            let report = report(&[
                ("tokenizer_config.json", tokenizer),
                ("chat_template.jinja", "valid-other-source"),
            ]);
            assert_eq!(
                report["status"], "unsupported-or-invalid-source",
                "{tokenizer}"
            );
            assert_eq!(report["templates"], json!([]));
            assert_eq!(report["fields"], json!({}));
        }
    }
    #[test]
    fn missing_null_and_empty_template_collections_are_distinct_declarations() {
        for (source, shape) in [
            ("{}", "missing"),
            (r#"{"chat_template":null}"#, "null"),
            (r#"{"chat_template":{}}"#, "named-map"),
            (r#"{"chat_template":[]}"#, "named-list"),
        ] {
            let report = report(&[("tokenizer_config.json", source)]);
            assert_eq!(report["status"], "no-declared-template");
            assert_eq!(report["source_shapes"]["tokenizer_config.json"], shape);
        }
    }
    #[test]
    fn format_source_integrity_is_rechecked_and_duplicate_inventory_is_rejected() {
        let (bytes, mut inventory) = package(&[
            ("tokenizer_config.json", "{}"),
            ("chat_template.jinja", "T"),
        ]);
        inventory[0].sha256 = "0".repeat(64);
        assert!(observe_format_sources("spec", "assets", &bytes, &inventory).is_err());
        let (bytes, mut inventory) = package(&[("tokenizer_config.json", "{}")]);
        inventory[0].bytes += 1;
        assert!(observe_format_sources("spec", "assets", &bytes, &inventory).is_err());
        inventory[0].bytes -= 1;
        inventory.push(inventory[0].clone());
        assert!(observe_format_sources("spec", "assets", &bytes, &inventory).is_err());
    }
    #[test]
    fn format_inspection_limits_reject_complete_view_never_clip_sources() {
        for template in [
            "x".repeat(LIMITS.max_bytes),
            "\u{0001}".repeat(LIMITS.max_bytes / 2),
        ] {
            let report = report(&[
                ("tokenizer_config.json", "{}"),
                ("chat_template.jinja", &template),
            ]);
            assert_eq!(report["status"], "inspection-capacity-exceeded");
            assert_eq!(report["templates"], json!([]));
            assert_eq!(report["fields"], json!({}));
        }
    }

    #[tokio::test]
    async fn retained_native_model_format_sources_are_exact_scoped_and_read_only() {
        use crate::prefix_model_artifacts::{ArtifactImportOrigin, PgPrefixModelArtifacts};
        use brassclaw_product_workflow::{
            CapturePrefixModelRequest, PrefixModelTarget, RetainPrefixModelArtifactsRequest,
        };
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let namespace = "native-format-source-owner";
        let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), namespace);
        repo.initialize().await.unwrap();
        let definition=serde_json::from_value(json!({"id":"retained-format-source","aliases":[],"protocol":"open_ai_completions",
            "default_base_url":"http://127.0.0.1:1/v1","base_url_required":true,"api_key_required":false,
            "model_env":"UNUSED_RETAINED_FORMAT_FIXTURE","default_model":"retained-ornith-source","description":"Exact retained format sources; no serving claims"})).unwrap();
        let provider = repo.replace_revision(definition, None).await.unwrap();
        let designs = crate::prefix_model_designs::PgPrefixModelDesigns::new(
            rig.pool.clone(),
            namespace.into(),
        );
        let specification = designs
            .capture(
                CapturePrefixModelRequest {
                    provider_id: "retained-format-source".into(),
                    expected_provider_revision: provider.revision,
                    model: "retained-ornith-source".into(),
                },
                vec![PrefixModelTarget {
                    provider_id: "retained-format-source".into(),
                    provider_revision: provider.revision,
                    model: "retained-ornith-source".into(),
                    consumer: "configured".into(),
                    configured_context_window: None,
                }],
            )
            .await
            .unwrap();
        let directory = std::path::PathBuf::from(
            std::env::var("BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT")
                .expect("actual frozen model sources required"),
        );
        let mut originals = BTreeMap::new();
        let mut artifacts = serde_json::Map::new();
        for name in [
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "chat_template.jinja",
            "vocab.json",
            "merges.txt",
        ] {
            let bytes = std::fs::read(directory.join(name)).unwrap();
            artifacts.insert(name.into(),json!({"encoding":"base64","data":STANDARD.encode(&bytes),"sha256":hex::encode(Sha256::digest(&bytes))}));
            originals.insert(name, bytes);
        }
        let store = PgPrefixModelArtifacts::new(rig.pool.clone(), namespace.into());
        let request = RetainPrefixModelArtifactsRequest {
            specification: specification.id.clone(),
            artifact_set_bytes:
                json!({"format":"brassclaw-prefix-model-artifact-set/1","artifacts":artifacts})
                    .to_string(),
        };
        let retained = store
            .retain_with_origin(
                request,
                ArtifactImportOrigin::Operator {
                    actor: "native-format-operator".into(),
                },
            )
            .await
            .unwrap();
        let before = store.list().await.unwrap();
        let detail = store.get(&specification.id, &retained.id).await.unwrap();
        let report: Value =
            serde_json::from_str(detail.format_observation.as_ref().unwrap()).unwrap();
        assert_eq!(report["qualification"], "retained-format-sources-only");
        assert_eq!(report["status"], "observed-format-sources");
        assert_eq!(report["specification"], specification.id);
        assert_eq!(report["artifact_set"], retained.id);
        let template = report["templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["file"] == "chat_template.jinja")
            .unwrap();
        assert_eq!(
            template["template"],
            std::str::from_utf8(&originals["chat_template.jinja"]).unwrap()
        );
        assert_eq!(
            template["template_sha256"],
            hex::encode(Sha256::digest(&originals["chat_template.jinja"]))
        );
        let config: Value =
            serde_json::from_str(detail.configuration_observation.as_ref().unwrap()).unwrap();
        let original_config: Value = serde_json::from_slice(&originals["config.json"]).unwrap();
        for (pointer, value) in config["fields"].as_object().unwrap() {
            assert_eq!(original_config.pointer(pointer), Some(value), "{pointer}");
        }
        let groups = original_config["quantization_config"]["config_groups"]
            .as_object()
            .expect("frozen grouped quantization original required");
        assert!(!groups.is_empty());
        for (name, group) in groups {
            let escaped = name.replace('~', "~0").replace('/', "~1");
            let pointer = format!("/quantization_config/config_groups/{escaped}/weights/num_bits");
            assert_eq!(config["fields"][&pointer], group["weights"]["num_bits"]);
            assert!(config["fields"].as_object().unwrap().contains_key(&pointer));
        }
        assert_eq!(
            config["fields"]["/quantization_config/ignore"],
            original_config["quantization_config"]["ignore"]
        );
        assert!(config["attention_kv_cache"].is_null());
        assert!(config["recurrent_state"].is_null());
        assert_eq!(
            config["fields"]["/text_config/mamba_ssm_dtype"],
            original_config["text_config"]["mamba_ssm_dtype"]
        );
        assert!(
            config["fields"]
                .as_object()
                .unwrap()
                .contains_key("/text_config/mamba_ssm_dtype")
        );
        assert!(report["effective_template"].is_null());
        assert!(report["serving_compatibility"].is_null());
        let wire = serde_json::to_value(&detail).unwrap();
        assert!(wire["format_observation"].is_string());
        assert_eq!(
            store
                .get(&specification.id, &retained.id)
                .await
                .unwrap()
                .format_observation,
            detail.format_observation
        );
        assert_eq!(
            serde_json::to_value(store.list().await.unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(
            PgPrefixModelArtifacts::new(rig.pool.clone(), "other-format-owner".into())
                .get(&specification.id, &retained.id)
                .await
                .is_err()
        );
        let client = rig.pool.get().await.unwrap();
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM reborn_prefix_generations", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
}
