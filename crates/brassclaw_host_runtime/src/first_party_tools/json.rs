use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::FirstPartyCapabilityError;

use super::{first_party_capability_manifest, input_error, operation_error, resource_profile};

pub const JSON_CAPABILITY_ID: &str = "builtin.json";

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    first_party_capability_manifest(
        JSON_CAPABILITY_ID,
        "Parse, query, stringify and validate JSON; hash a bounded ordered list of UTF-8 strings.",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Allow,
        resource_profile(),
    )
}

pub(super) fn dispatch(input: &Value) -> Result<Value, FirstPartyCapabilityError> {
    if input.get("source_tool_call_id").is_some() {
        return Err(input_error());
    }
    let operation = input
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(input_error)?;
    match operation {
        "sha256_utf8" => {
            if input.as_object().is_none_or(|map| map.len() != 2) {
                return Err(input_error());
            }
            let data = input.get("data").ok_or_else(input_error)?;
            // JSON text keeps one typed string transport compatible with the
            // other JSON operations; direct structured callers remain supported.
            let decoded = data
                .as_str()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .map_err(|_| input_error())?;
            let values = decoded
                .as_ref()
                .unwrap_or(data)
                .as_array()
                .ok_or_else(input_error)?;
            // One deterministic vector operation, with no I/O or workflow
            // decisions. Bound its result under the existing JSON output limit.
            if values.len() > 8192 || values.iter().any(|value| !value.is_string()) {
                return Err(input_error());
            }
            Ok(
                json!({"digests":values.iter().map(|value| hex::encode(Sha256::digest(value.as_str().expect("validated text").as_bytes()))).collect::<Vec<_>>()}),
            )
        }
        "parse" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let text = data.as_str().ok_or_else(input_error)?;
            serde_json::from_str::<Value>(text).map_err(|_| input_error())
        }
        "stringify" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let value = if let Some(text) = data.as_str() {
                serde_json::from_str::<Value>(text).map_err(|_| input_error())?
            } else {
                data.clone()
            };
            serde_json::to_string_pretty(&value)
                .map(Value::String)
                .map_err(|_| operation_error())
        }
        "query" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(input_error)?;
            let value = if let Some(text) = data.as_str() {
                serde_json::from_str::<Value>(text).map_err(|_| input_error())?
            } else {
                data.clone()
            };
            query_json(&value, path).cloned()
        }
        "validate" => {
            let valid = input
                .get("data")
                .and_then(Value::as_str)
                .map(|text| serde_json::from_str::<Value>(text).is_ok())
                .unwrap_or(false);
            Ok(json!({ "valid": valid }))
        }
        _ => Err(input_error()),
    }
}

fn query_json<'a>(value: &'a Value, path: &str) -> Result<&'a Value, FirstPartyCapabilityError> {
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        if let Some((field, rest)) = segment.split_once('[') {
            if !field.is_empty() {
                current = current.get(field).ok_or_else(input_error)?;
            }
            let index_text = rest.strip_suffix(']').ok_or_else(input_error)?;
            let index = index_text.parse::<usize>().map_err(|_| input_error())?;
            current = current.get(index).ok_or_else(input_error)?;
        } else {
            current = current.get(segment).ok_or_else(input_error)?;
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_sha256_utf8_preserves_order_exact_bytes_and_rejects_other_values() {
        let output = dispatch(&json!({"operation":"sha256_utf8","data":["","abc","abc"]})).unwrap();
        assert_eq!(
            output,
            json!({"digests":["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad","ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"]})
        );
        assert_eq!(
            dispatch(&json!({"operation":"sha256_utf8","data":"[\"\",\"abc\",\"abc\"]"})).unwrap(),
            output
        );
        let bytes =
            dispatch(&json!({"operation":"sha256_utf8","data":["é","e\u{301}","x\r\n","x\n"]}))
                .unwrap();
        assert_ne!(
            bytes["digests"][0], bytes["digests"][1],
            "no Unicode normalization"
        );
        assert_ne!(
            bytes["digests"][2], bytes["digests"][3],
            "no newline normalization"
        );
        for input in [
            json!({"operation":"sha256_utf8","data":"abc"}),
            json!({"operation":"sha256_utf8","data":"[true]"}),
            json!({"operation":"sha256_utf8","data":"{\"not\":\"a list\"}"}),
            json!({"operation":"sha256_utf8","data":[true]}),
            json!({"operation":"sha256_utf8","data":["abc"],"path":"private"}),
            json!({"operation":"sha256_utf8","data":vec!["";8193]}),
        ] {
            assert!(dispatch(&input).is_err());
        }
    }
}
