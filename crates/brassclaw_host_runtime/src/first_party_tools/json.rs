use brassclaw_common::json::{
    DuplicateKeys, JsonDocument, JsonReadError, JsonReadLimits, check_json_value, clone_json,
    read_json, write_json_pretty,
};
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{
    EffectKind, MontyExecutionLimits, PermissionMode, ResourceEstimate, ResourceProfile,
    ResourceUsage, RuntimeDispatchErrorKind,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    time::Instant,
};

use crate::{FirstPartyCapabilityError, FirstPartyCapabilityResult};

use super::{
    HostPrimitiveSettings, HostPrimitiveSettingsSource, bounded_output_bytes,
    first_party_capability_manifest, input_error, operation_error,
};

pub const JSON_CAPABILITY_ID: &str = "builtin.json";

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    first_party_capability_manifest(
        JSON_CAPABILITY_ID,
        "Parse, query, stringify and validate JSON; hash a bounded ordered list of UTF-8 strings.",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Allow,
        Some(ResourceProfile {
            default_estimate: ResourceEstimate::default(),
            hard_ceiling: None,
        }),
    )
}

// Capture once before CPU work. Later settings edits affect later operations;
// they cannot turn a completed parse into partial/truncated success.
pub(super) async fn dispatch(
    input: Value,
    source: Option<&dyn HostPrimitiveSettingsSource>,
) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
    let settings = match source {
        Some(source) => source.snapshot().await?,
        // Direct host consumers use canonical defaults. Production installs a
        // required bootstrap/live source; an unbound source never falls back.
        None => HostPrimitiveSettings {
            revision: 0,
            limits: MontyExecutionLimits::default(),
        },
    };
    settings.limits.validate().map_err(|_| resource_error())?;
    let limits = JsonReadLimits {
        max_depth: usize::try_from(settings.limits.max_value_depth)
            .map_err(|_| resource_error())?,
        max_nodes: usize::try_from(settings.limits.max_value_nodes)
            .map_err(|_| resource_error())?,
        max_bytes: usize::try_from(settings.limits.max_value_bytes)
            .map_err(|_| resource_error())?,
    };
    let start = Instant::now();
    let result = tokio::task::spawn_blocking(move || dispatch_with_limits(&input, limits))
        .await
        .map_err(|_| operation_error())?;
    let wall_clock_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    match result {
        Ok((output, output_bytes)) => Ok(FirstPartyCapabilityResult::new(
            output,
            ResourceUsage {
                wall_clock_ms,
                output_bytes,
                ..ResourceUsage::default()
            },
        )),
        Err(error) => Err(error.with_usage(ResourceUsage {
            wall_clock_ms,
            ..ResourceUsage::default()
        })),
    }
}
fn resource_error() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::Resource)
}
fn read_error(error: JsonReadError) -> FirstPartyCapabilityError {
    match error {
        JsonReadError::InvalidLimits | JsonReadError::DocumentBytes | JsonReadError::Resource => {
            resource_error()
        }
        _ => input_error(),
    }
}
fn read(text: &str, limits: JsonReadLimits) -> Result<JsonDocument, FirstPartyCapabilityError> {
    read_json(text, limits, DuplicateKeys::LastWins).map_err(read_error)
}
fn finish(value: Value, limits: JsonReadLimits) -> Result<(Value, u64), FirstPartyCapabilityError> {
    let bytes = bounded_output_bytes(&value, limits.max_bytes as u64)?;
    check_json_value(&value, limits).map_err(read_error)?;
    Ok((value, bytes))
}
fn finish_document(
    document: JsonDocument,
    limits: JsonReadLimits,
) -> Result<(Value, u64), FirstPartyCapabilityError> {
    // Keep iterative custody through the last fallible output check.
    let bytes = bounded_output_bytes(document.as_value(), limits.max_bytes as u64)?;
    Ok((document.into_value(), bytes))
}
fn dispatch_with_limits(
    input: &Value,
    limits: JsonReadLimits,
) -> Result<(Value, u64), FirstPartyCapabilityError> {
    check_json_value(input, limits).map_err(read_error)?;
    brassclaw_common::json::count_json_bytes(input, Some(limits.max_bytes as u64))
        .map_err(|_| resource_error())?;
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
            let decoded = data.as_str().map(|text| read(text, limits)).transpose()?;
            let values = decoded
                .as_ref()
                .map(JsonDocument::as_value)
                .unwrap_or(data)
                .as_array()
                .ok_or_else(input_error)?;
            // One deterministic vector operation, with no I/O or workflow
            // decisions. The operation contract retains its 8192-item profile;
            // acknowledged value bounds govern input and complete output bytes.
            if values.len() > 8192 || values.iter().any(|value| !value.is_string()) {
                return Err(input_error());
            }
            finish(
                json!({"digests":values.iter().map(|value| hex::encode(Sha256::digest(value.as_str().expect("validated text").as_bytes()))).collect::<Vec<_>>()}),
                limits,
            )
        }
        "parse" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let text = data.as_str().ok_or_else(input_error)?;
            finish_document(read(text, limits)?, limits)
        }
        "stringify" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let decoded = data.as_str().map(|text| read(text, limits)).transpose()?;
            let value = decoded.as_ref().map(JsonDocument::as_value).unwrap_or(data);
            let mut writer = BoundedWriter {
                bytes: Vec::new(),
                limit: limits.max_bytes,
                exceeded: false,
            };
            if write_json_pretty(&mut writer, value).is_err() {
                return Err(if writer.exceeded {
                    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::OutputTooLarge)
                } else {
                    resource_error()
                });
            }
            let text = String::from_utf8(writer.bytes).map_err(|_| operation_error())?;
            finish(Value::String(text), limits)
        }
        "query" => {
            let data = input.get("data").ok_or_else(input_error)?;
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(input_error)?;
            let decoded = data.as_str().map(|text| read(text, limits)).transpose()?;
            let value = decoded.as_ref().map(JsonDocument::as_value).unwrap_or(data);
            let selected = query_json(value, path)?;
            finish_document(clone_json(selected, limits).map_err(read_error)?, limits)
        }
        "validate" => {
            let valid = match input.get("data").and_then(Value::as_str) {
                Some(text) => match read_json(text, limits, DuplicateKeys::LastWins) {
                    Ok(_) => true,
                    Err(
                        JsonReadError::Syntax
                        | JsonReadError::TrailingData
                        | JsonReadError::DuplicateKey,
                    ) => false,
                    Err(error) => return Err(read_error(error)),
                },
                None => false,
            };
            finish(json!({ "valid": valid }), limits)
        }
        _ => Err(input_error()),
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|bytes| bytes > self.limit)
        {
            self.exceeded = true;
            return Err(io::Error::other("JSON output bound"));
        }
        self.bytes
            .try_reserve(buffer.len())
            .map_err(|_| io::Error::other("JSON output capacity"))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
    fn dispatch(input: &Value) -> Result<Value, FirstPartyCapabilityError> {
        let defaults = MontyExecutionLimits::default();
        dispatch_with_limits(
            input,
            JsonReadLimits {
                max_depth: defaults.max_value_depth as usize,
                max_nodes: defaults.max_value_nodes as usize,
                max_bytes: defaults.max_value_bytes as usize,
            },
        )
        .map(|(value, _)| value)
    }
    #[test]
    fn json_bounds_apply_during_parsing_and_resource_errors_are_not_invalid_documents() {
        let limits = JsonReadLimits {
            max_depth: 8,
            max_nodes: 32,
            max_bytes: 4096,
        };
        for input in [
            json!({"operation":"parse","data":format!("{}true{}", "[".repeat(9), "]".repeat(9))}),
            json!({"operation":"validate","data":serde_json::to_string(&vec![0;32]).unwrap()}),
        ] {
            assert_eq!(
                dispatch_with_limits(&input, limits).unwrap_err().kind(),
                Some(RuntimeDispatchErrorKind::Resource)
            );
        }
        assert_eq!(
            dispatch_with_limits(&json!({"operation":"validate","data":"[1,]"}), limits)
                .unwrap()
                .0,
            json!({"valid":false})
        );
        assert_eq!(
            dispatch_with_limits(
                &json!({"operation":"parse","data":"{\"x\":0,\"x\":1}"}),
                limits
            )
            .unwrap()
            .0,
            json!({"x":1})
        );
        let input = json!({"operation":"query","data":{"null":{"x":"value"}},"path":"null.x"});
        assert_eq!(dispatch_with_limits(&input, limits).unwrap().0, "value");
        let overflowing = json!({"operation":"stringify","data":vec!["x";48]});
        let error = dispatch_with_limits(
            &overflowing,
            JsonReadLimits {
                max_nodes: 128,
                max_bytes: 256,
                ..limits
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), Some(RuntimeDispatchErrorKind::OutputTooLarge));
    }
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
