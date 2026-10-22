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
        let file = package
            .artifacts
            .get("config.json")
            .ok_or(Error::Unavailable)?;
        let original = STANDARD.decode(file.data).map_err(|_| Error::Unavailable)?;
        if original.len() as u64 != entry.bytes
            || file.sha256 != entry.sha256
            || hex::encode(Sha256::digest(&original)) != entry.sha256
        {
            return Err(Error::Unavailable);
        }
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
            for prefix in ["/quantization_config", "/text_config/quantization_config"] {
                for name in ["quant_method", "bits", "group_size", "desc_act", "sym"] {
                    let pointer = format!("{prefix}/{name}");
                    if let Some(value) = config.pointer(&pointer) {
                        fields.insert(pointer, value.clone());
                    }
                }
            }
            report["status"] = json!("observed-declarations");
            report["fields"] = Value::Object(fields);
        } else {
            report["status"] = json!("unsupported-or-invalid-config");
        }
    }
    serde_json::to_string(&report).map_err(|_| Error::Unavailable)
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
    fn configuration_observation_preserves_declarations_without_inventing_runtime_facts() {
        let report = observation(
            r#"{"model_type":"unfamiliar","text_config":{"num_hidden_layers":4,"layer_types":["linear_attention","linear_attention","linear_attention","full_attention"],"max_position_embeddings":262144,"ssm_dtype":null},"quantization_config":{"quant_method":"awq","bits":4,"token":"private"},"token":"private","auto_map":{"AutoModel":"arbitrary.program"}}"#,
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
