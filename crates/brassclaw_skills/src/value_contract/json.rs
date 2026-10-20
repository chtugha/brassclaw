//! Component contract profile over the shared bounded reader.
//! Public ContractLimits validation retains its existing depth admission.
use super::{ContractError, ContractLimits, error};
use brassclaw_common::json::{DuplicateKeys, JsonReadError, JsonReadLimits, read_json};
use serde_json::Value;

pub(super) fn parse(source: &str, limits: ContractLimits) -> Result<Value, ContractError> {
    read_json(
        source,
        JsonReadLimits {
            max_depth: limits.max_depth,
            max_nodes: limits.max_nodes,
            max_bytes: limits.max_bytes,
        },
        DuplicateKeys::Reject,
    )
    .map(|document| document.into_value())
    .map_err(|failure| {
        error(
            "$",
            match failure {
                JsonReadError::DocumentBytes => "JSON document byte bound exceeded",
                JsonReadError::TrailingData => "trailing JSON data",
                _ => "invalid, duplicate-key or oversized JSON",
            },
        )
    })
}
