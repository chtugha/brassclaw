//! Reduction rule management for WebUI v2.
//!
//! The orchestrator Python (`default.py`) progressively shrinks the assembled
//! message list with a chain of these rules once the prompt exceeds the
//! per-turn token budget. Rules are passed to Python as a list of plain JSON
//! objects via the engine's `__get_reduction_rules__()` host function; the
//! host function reads them once when the builder resolves, caches the list
//! in a process-wide map keyed by `(project_id, user_id)`, and exposes the
//! cache to a small REST surface here so operators (and the WebUI's
//! Reduction-Rules tab) can list, replace, and author rules without
//! restarting the runtime.
//!
//! Persistence is delegated through [`ReductionRuleStore`]. The WebUI's
//! default implementation stores the full list per
//! `(user_id, project_id)` in the libSQL settings table — the same table the
//! per-provider token limits use — so there's exactly one operator-facing
//! settings subsystem to back up. The engine-side `__get_reduction_rules__`
//! consumes the same list through an in-crate `ReductionRulePersistence`
//! port; the orchestration layer wires the libSQL-backed implementation so a
//! rule saved by the WebUI shows up on the very next over-budget turn with
//! no cache flush required (the invalidator drops stale slots on every PUT).
//!
//! ## Rule types
//!
//! Five `rule_type` values are recognised, in the order the orchestrator runs
//! them. `field` is the JSON message key the rule targets; numeric params
//! live in `params`.
//!
//! - `truncate` — trims `params['max_chars']` characters off `field` of the
//!   last user message.
//! - `summarize` — flags `field` of the last user message for the host to
//!   summarize on the next turn; the Python pipeline marks the field, the
//!   actual summarization is a deferred host concern.
//! - `drop` — removes `field` of the last user message completely.
//! - `priority` — drops fields listed in `params['fields']` tail-first (the
//!   LAST `fields` entry is the lowest priority, dropped first) until the
//!   prompt fits the budget or the head of the list is reached.
//! - `history_compact` — keeps only the most recent
//!   `params['keep_recent_n']` non-System messages; system messages are
//!   preserved verbatim so the cache-stable prefix doesn't shift.
//!
//! All five types are mirrored verbatim in
//! `crates/brassclaw_engine/orchestrator/default.py` and the CPython
//! reference at `crates/brassclaw_engine/orchestrator/segment_reduction.py`.
//! Adding a new rule type here without updating both Python files will
//! silently no-op for the new variant.

#![forbid(unsafe_code)]

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ProductWorkflowError;

/// Canonical rule type tags. The string representations match the `type`
/// keys consumed by the orchestrator Python and the CPython reference
/// implementation; they intentionally duplicate `default.py`'s
/// `REDUCE_TYPE_*` constants.
pub mod rule_type {
    pub const TRUNCATE: &str = "truncate";
    pub const SUMMARIZE: &str = "summarize";
    pub const DROP: &str = "drop";
    pub const PRIORITY: &str = "priority";
    pub const HISTORY_COMPACT: &str = "history_compact";
}

/// Maximum number of rules stored per `(user_id, project_id)`. The
/// orchestrator runs all rules once per over-budget turn, so the upper
/// bound exists to keep the worst-case O(n) reduction work bounded.
pub const REDUCTION_RULES_MAX_PER_USER: usize = 64;

/// Hard caps for individual rule fields. These are deliberately generous —
/// the orchestrator's actual reduction work is bounded by the prompt budget
/// — and exist only to prevent a misconfigured rule from blowing the array
/// it lives in. Limits are validated at every code path that ingests rules
/// from outside the trusted host boundary.
pub const REDUCTION_RULE_FIELDS_MAX: usize = 16;
pub const REDUCTION_RULE_FIELD_NAME_MAX_BYTES: usize = 64;
pub const REDUCTION_RULE_KEEP_RECENT_N_MAX: u32 = 4096;
pub const REDUCTION_RULE_MAX_CHARS_MAX: u32 = 1_048_576; // 1 MiB
/// `priority` (rule ordering): 0 runs first. `u32` so wiring can preserve
/// caller order even when the storage layer doesn't pin ordering.
pub const REDUCTION_RULE_PRIORITY_MAX: u32 = 65_535;

/// The five reduction rule variants. Marked `#[non_exhaustive]` so adding a
/// new variant in a future crate release doesn't accidentally deserialize
/// as a no-op in older binaries — callers must opt-in via the `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RuleType {
    /// Used only when a request body omits `rule_type` (e.g. during
    /// serde defaults). Authoring rules without a type is an error
    /// upstream; Default just keeps the struct constructible.
    #[default]
    Truncate,
    Summarize,
    Drop,
    Priority,
    HistoryCompact,
}

impl RuleType {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Truncate => rule_type::TRUNCATE,
            Self::Summarize => rule_type::SUMMARIZE,
            Self::Drop => rule_type::DROP,
            Self::Priority => rule_type::PRIORITY,
            Self::HistoryCompact => rule_type::HISTORY_COMPACT,
        }
    }

    pub fn from_wire_str(s: &str) -> Option<Self> {
        match s {
            rule_type::TRUNCATE => Some(Self::Truncate),
            rule_type::SUMMARIZE => Some(Self::Summarize),
            rule_type::DROP => Some(Self::Drop),
            rule_type::PRIORITY => Some(Self::Priority),
            rule_type::HISTORY_COMPACT => Some(Self::HistoryCompact),
            _ => None,
        }
    }
}

impl fmt::Display for RuleType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_wire_str())
    }
}

/// Strongly-typed parameters for each rule variant. The orchestrator Python
/// reads these as a free-form `params` JSON object; Rust validation pins
/// the shape so we don't depend on the runtime to catch malformed input.
///
/// Unknown variants in input JSON are rejected at deserialization; the
/// `non_exhaustive` guard makes future additions a conscious upgrade step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule_type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ReductionRuleConfigTyped {
    Truncate {
        id: String,
        field: String,
        #[serde(default = "default_priority")]
        priority: u32,
        max_chars: u32,
    },
    Summarize {
        id: String,
        field: String,
        #[serde(default = "default_priority")]
        priority: u32,
    },
    Drop {
        id: String,
        field: String,
        #[serde(default = "default_priority")]
        priority: u32,
    },
    Priority {
        id: String,
        /// Highest-priority field FIRST. The reduction pipeline walks this
        /// list in reverse so the last entry is dropped first. Stored
        /// verbatim (after validation); do NOT reverse-server-side.
        fields: Vec<String>,
        #[serde(default = "default_priority")]
        priority: u32,
    },
    HistoryCompact {
        id: String,
        keep_recent_n: u32,
        #[serde(default = "default_priority")]
        priority: u32,
    },
}

impl ReductionRuleConfigTyped {
    pub fn id(&self) -> &str {
        match self {
            Self::Truncate { id, .. }
            | Self::Summarize { id, .. }
            | Self::Drop { id, .. }
            | Self::Priority { id, .. }
            | Self::HistoryCompact { id, .. } => id,
        }
    }

    pub fn priority(&self) -> u32 {
        match self {
            Self::Truncate { priority, .. }
            | Self::Summarize { priority, .. }
            | Self::Drop { priority, .. }
            | Self::Priority { priority, .. }
            | Self::HistoryCompact { priority, .. } => *priority,
        }
    }

    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Truncate { field, .. }
            | Self::Summarize { field, .. }
            | Self::Drop { field, .. } => Some(field),
            Self::Priority { .. } | Self::HistoryCompact { .. } => None,
        }
    }

    /// Strict validation. Bounded by [`REDUCTION_RULE_*`] caps above.
    pub fn validate(&self) -> Result<(), ReductionRuleValidationError> {
        validate_id(self.id())?;
        match self {
            Self::Truncate {
                field, max_chars, ..
            } => {
                validate_field_name(field)?;
                if *max_chars == 0 {
                    return Err(ReductionRuleValidationError::ZeroMaxChars);
                }
                if *max_chars > REDUCTION_RULE_MAX_CHARS_MAX {
                    return Err(ReductionRuleValidationError::MaxCharsTooLarge {
                        max_chars: *max_chars,
                        cap: REDUCTION_RULE_MAX_CHARS_MAX,
                    });
                }
            }
            Self::Summarize { field, .. } | Self::Drop { field, .. } => {
                validate_field_name(field)?;
            }
            Self::Priority { fields, .. } => {
                if fields.is_empty() {
                    return Err(ReductionRuleValidationError::EmptyPriorityFields);
                }
                if fields.len() > REDUCTION_RULE_FIELDS_MAX {
                    return Err(ReductionRuleValidationError::TooManyPriorityFields {
                        count: fields.len(),
                        cap: REDUCTION_RULE_FIELDS_MAX,
                    });
                }
                for name in fields {
                    validate_field_name(name)?;
                }
            }
            Self::HistoryCompact { keep_recent_n, .. } => {
                if *keep_recent_n == 0 {
                    return Err(ReductionRuleValidationError::ZeroKeepRecentN);
                }
                if *keep_recent_n > REDUCTION_RULE_KEEP_RECENT_N_MAX {
                    return Err(ReductionRuleValidationError::KeepRecentNTooLarge {
                        keep_recent_n: *keep_recent_n,
                        cap: REDUCTION_RULE_KEEP_RECENT_N_MAX,
                    });
                }
            }
        }
        Ok(())
    }

    /// Serialize into the engine's `serde_json::Value` shape that the
    /// orchestrator Python reads from `__get_reduction_rules__`. The wire
    /// shape is the same whether validation accepted a `Truncate` /
    /// `Summarize` / `Drop` / `Priority` / `HistoryCompact` variant; the
    /// Python side is untyped so we cannot use a Rust-enum here.
    pub fn to_wire_json(&self) -> serde_json::Value {
        match self {
            Self::Truncate {
                id,
                field,
                priority,
                max_chars,
            } => serde_json::json!({
                "type": rule_type::TRUNCATE,
                "id": id,
                "field": field,
                "max_chars": max_chars,
                "priority": priority,
            }),
            Self::Summarize {
                id,
                field,
                priority,
            } => serde_json::json!({
                "type": rule_type::SUMMARIZE,
                "id": id,
                "field": field,
                "priority": priority,
            }),
            Self::Drop {
                id,
                field,
                priority,
            } => serde_json::json!({
                "type": rule_type::DROP,
                "id": id,
                "field": field,
                "priority": priority,
            }),
            Self::Priority {
                id,
                fields,
                priority,
            } => serde_json::json!({
                "type": rule_type::PRIORITY,
                "id": id,
                "fields": fields,
                "priority": priority,
            }),
            Self::HistoryCompact {
                id,
                keep_recent_n,
                priority,
            } => serde_json::json!({
                "type": rule_type::HISTORY_COMPACT,
                "id": id,
                "keep_recent_n": keep_recent_n,
                "priority": priority,
            }),
        }
    }

    /// Parse a wire-format `serde_json::Value` from the engine or from a
    /// forward-compatible storage row. Unknown fields are an error so the
    /// ingest path never silently drops a configuration.
    pub fn from_wire_json(value: &serde_json::Value) -> Result<Self, ReductionRuleValidationError> {
        match value {
            serde_json::Value::Object(_) => {}
            _ => return Err(ReductionRuleValidationError::WireShapeMismatch),
        }
        serde_json::from_value(value.clone()).map_err(|source| {
            ReductionRuleValidationError::WireShapeMismatchDetail {
                detail: source.to_string(),
            }
        })
    }
}

/// Owned, validated clone of a [`ReductionRuleConfigTyped`] used by the
/// WebUI REST layer as the wire DTO. Internal layers (orchestrator, store)
/// use this typed shape; everything that crosses the WebUI boundary
/// (request body, response payload) is `serde_json::Value` so we don't
/// break when the WebUI uses a slightly different field ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionRuleConfigView {
    pub id: String,
    pub rule_type: RuleType,
    /// Free-form parameters keyed by `params[<name>]`. The Python side
    /// reads these directly; Rust validation enforces them via the typed
    /// shape on the storage path, but the wire form stays open-ended so the
    /// frontend and an older backend stay interoperable.
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "default_priority")]
    pub priority: u32,
}

impl ReductionRuleConfigView {
    pub fn validate(&self) -> Result<(), ReductionRuleValidationError> {
        validate_id(&self.id)?;
        if self.priority > REDUCTION_RULE_PRIORITY_MAX {
            return Err(ReductionRuleValidationError::PriorityTooLarge {
                priority: self.priority,
                cap: REDUCTION_RULE_PRIORITY_MAX,
            });
        }
        let typed = self.clone().into_typed()?;
        typed.validate()
    }

    /// Pure conversion into the validated typed form for storage. Returns
    /// the typed rule if shape and field-level validation both pass.
    pub fn into_typed(self) -> Result<ReductionRuleConfigTyped, ReductionRuleValidationError> {
        let ReductionRuleConfigView {
            id,
            rule_type,
            params,
            priority,
        } = self;
        let typed = match rule_type {
            RuleType::Truncate => {
                let max_chars = params_u32(&params, "max_chars")?;
                ReductionRuleConfigTyped::Truncate {
                    id,
                    field: params_str(&params, "field")?,
                    priority,
                    max_chars,
                }
            }
            RuleType::Summarize => ReductionRuleConfigTyped::Summarize {
                id,
                field: params_str(&params, "field")?,
                priority,
            },
            RuleType::Drop => ReductionRuleConfigTyped::Drop {
                id,
                field: params_str(&params, "field")?,
                priority,
            },
            RuleType::Priority => {
                let fields = params_string_array(&params, "fields")?;
                ReductionRuleConfigTyped::Priority {
                    id,
                    fields,
                    priority,
                }
            }
            RuleType::HistoryCompact => {
                let keep_recent_n = params_u32(&params, "keep_recent_n")?;
                ReductionRuleConfigTyped::HistoryCompact {
                    id,
                    keep_recent_n,
                    priority,
                }
            }
        };
        Ok(typed)
    }
}

impl From<ReductionRuleConfigTyped> for ReductionRuleConfigView {
    fn from(typed: ReductionRuleConfigTyped) -> Self {
        match typed {
            ReductionRuleConfigTyped::Truncate {
                id,
                field,
                max_chars,
                priority,
            } => Self {
                id,
                rule_type: RuleType::Truncate,
                params: serde_json::json!({"field": field, "max_chars": max_chars}),
                priority,
            },
            ReductionRuleConfigTyped::Summarize {
                id,
                field,
                priority,
            } => Self {
                id,
                rule_type: RuleType::Summarize,
                params: serde_json::json!({"field": field}),
                priority,
            },
            ReductionRuleConfigTyped::Drop {
                id,
                field,
                priority,
            } => Self {
                id,
                rule_type: RuleType::Drop,
                params: serde_json::json!({"field": field}),
                priority,
            },
            ReductionRuleConfigTyped::Priority {
                id,
                fields,
                priority,
            } => Self {
                id,
                rule_type: RuleType::Priority,
                params: serde_json::json!({"fields": fields}),
                priority,
            },
            ReductionRuleConfigTyped::HistoryCompact {
                id,
                keep_recent_n,
                priority,
            } => Self {
                id,
                rule_type: RuleType::HistoryCompact,
                params: serde_json::json!({"keep_recent_n": keep_recent_n}),
                priority,
            },
        }
    }
}

/// Wire-shape response and request bodies that the WebUI v2 handlers
/// exchange with the browser. The `rules` field is the full ordered list —
/// the orchestrator Python runs them in the order returned, then re-orders
/// by ascending `priority` value before running them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReductionRulesResponse {
    pub project_id: String,
    pub rules: Vec<ReductionRuleConfigView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReductionRulesRequest {
    pub rules: Vec<ReductionRuleConfigView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthorReductionRuleRequest {
    pub rule_type: RuleType,
    /// Required shape mirrors the body's `params` slot for the matching
    /// rule type. For `truncate` requires `{field, max_chars}`; for
    /// `summarize`/`drop` requires `{field}`; for `priority` requires
    /// `{fields: [...]}`; for `history_compact` requires `{keep_recent_n}`.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Optional free-form description stored alongside the rule's
    /// metadata. Not used by the reduction pipeline; carried for
    /// operators reviewing the saved rule library.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorReductionRuleResponse {
    pub rule: ReductionRuleConfigView,
    /// Echoed back verbatim (when provided) so the WebUI can render the
    /// intent alongside the validated rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Port the WebUI v2 facade depends on. Implementations are free to use
/// any backing store; the composition crate ships a libSQL-backed
/// adapter that reuses the same settings table as the per-provider token
/// limits. Reads are user+project scoped so per-project isolation
/// matches the engine orchestrator's `(project_id, user_id)` cache key.
#[async_trait]
pub trait ReductionRuleStore: Send + Sync {
    /// List the rules for `(user_id, project_id)`, ordered with ascending
    /// `priority` so a lower `priority` field runs first in the
    /// orchestrator's reduction pipeline.
    async fn list(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError>;

    /// Replace the rule set atomically. Returns the canonical ordered
    /// list as it now exists in the store. Implementations must drop a
    /// stale entry from the in-process cache the engine keeps; the
    /// composition-backed adapter wraps the engine's
    /// `invalidate_reduction_rules_cache()` so subsequent over-budget
    /// turns pick up the change without waiting for a restart.
    async fn replace(
        &self,
        user_id: &str,
        project_id: &str,
        rules: Vec<ReductionRuleConfigView>,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError>;
}

/// Storage-side error taxonomy. Implementations map their inner error
/// type to one of these; the WebUI surface maps them to HTTP status
/// codes (400 invalid, 500 internal, 503 unavailable).
#[derive(Debug, thiserror::Error)]
pub enum ReductionRuleStoreError {
    #[error("invalid reduction rule payload: {0}")]
    Invalid(String),
    #[error("reductions store unavailable: {0}")]
    Unavailable(String),
    #[error("reductions store internal error: {0}")]
    Internal(String),
}

impl From<ReductionRuleStoreError> for ProductWorkflowError {
    fn from(error: ReductionRuleStoreError) -> Self {
        match error {
            ReductionRuleStoreError::Invalid(reason) => {
                ProductWorkflowError::InvalidBindingRequest { reason }
            }
            ReductionRuleStoreError::Unavailable(reason) => {
                ProductWorkflowError::Transient { reason }
            }
            ReductionRuleStoreError::Internal(reason) => {
                ProductWorkflowError::BeforeInboundPolicyFailed {
                    reason,
                    permanent: false,
                }
            }
        }
    }
}

/// Per-rule validation failure. `WireShapeMismatchDetail` carries
/// serde's failure string verbatim — the messages stay tool-friendly but
/// are not user-friendly; the WebUI surface renders them inside a
/// 400-validation banner.
#[derive(Debug, thiserror::Error)]
pub enum ReductionRuleValidationError {
    #[error("rule id must be 1..={max_bytes} bytes and match [a-z0-9_-]")]
    InvalidIdFormat { max_bytes: usize },
    #[error("rule id is empty")]
    EmptyId,
    #[error("rule id is too long (max {max_bytes} bytes)")]
    IdTooLong { max_bytes: usize },
    #[error("priority exceeds cap {cap}")]
    PriorityTooLarge { priority: u32, cap: u32 },
    #[error("field name must be 1..={max_bytes} bytes and match [a-z0-9_-]")]
    InvalidFieldFormat { max_bytes: usize },
    #[error("field name is empty")]
    EmptyField,
    #[error("field name is too long (max {max_bytes} bytes)")]
    FieldTooLong { max_bytes: usize },
    #[error("`max_chars` must be > 0")]
    ZeroMaxChars,
    #[error("`max_chars` exceeds cap {cap}")]
    MaxCharsTooLarge { max_chars: u32, cap: u32 },
    #[error("`fields` array must be non-empty")]
    EmptyPriorityFields,
    #[error("`fields` array has {count} entries, exceeding cap {cap}")]
    TooManyPriorityFields { count: usize, cap: usize },
    #[error("`keep_recent_n` must be > 0")]
    ZeroKeepRecentN,
    #[error("`keep_recent_n` exceeds cap {cap}")]
    KeepRecentNTooLarge { keep_recent_n: u32, cap: u32 },
    #[error("reduction rule wire shape mismatch: expected an object")]
    WireShapeMismatch,
    #[error("reduction rule wire shape mismatch: {detail}")]
    WireShapeMismatchDetail { detail: String },
    #[error("missing or invalid `params.{key}` value")]
    MissingParam { key: &'static str },
}

const fn default_priority() -> u32 {
    100
}

fn validate_id(id: &str) -> Result<(), ReductionRuleValidationError> {
    if id.is_empty() {
        return Err(ReductionRuleValidationError::EmptyId);
    }
    if id.len() > REDUCTION_RULE_FIELD_NAME_MAX_BYTES {
        return Err(ReductionRuleValidationError::IdTooLong {
            max_bytes: REDUCTION_RULE_FIELD_NAME_MAX_BYTES,
        });
    }
    if !id.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    }) {
        return Err(ReductionRuleValidationError::InvalidIdFormat {
            max_bytes: REDUCTION_RULE_FIELD_NAME_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_field_name(name: &str) -> Result<(), ReductionRuleValidationError> {
    if name.is_empty() {
        return Err(ReductionRuleValidationError::EmptyField);
    }
    if name.len() > REDUCTION_RULE_FIELD_NAME_MAX_BYTES {
        return Err(ReductionRuleValidationError::FieldTooLong {
            max_bytes: REDUCTION_RULE_FIELD_NAME_MAX_BYTES,
        });
    }
    if !name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
    }) {
        return Err(ReductionRuleValidationError::InvalidFieldFormat {
            max_bytes: REDUCTION_RULE_FIELD_NAME_MAX_BYTES,
        });
    }
    Ok(())
}

fn params_str(
    params: &serde_json::Value,
    key: &'static str,
) -> Result<String, ReductionRuleValidationError> {
    match params.get(key) {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(ReductionRuleValidationError::MissingParam { key }),
        None => Err(ReductionRuleValidationError::MissingParam { key }),
    }
}

fn params_u32(
    params: &serde_json::Value,
    key: &'static str,
) -> Result<u32, ReductionRuleValidationError> {
    match params.get(key) {
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(ReductionRuleValidationError::MissingParam { key }),
        Some(_) => Err(ReductionRuleValidationError::MissingParam { key }),
        None => Err(ReductionRuleValidationError::MissingParam { key }),
    }
}

fn params_string_array(
    params: &serde_json::Value,
    key: &'static str,
) -> Result<Vec<String>, ReductionRuleValidationError> {
    match params.get(key) {
        Some(serde_json::Value::Array(arr)) => {
            let mut out = Vec::with_capacity(arr.len());
            for entry in arr {
                match entry {
                    serde_json::Value::String(s) => out.push(s.clone()),
                    _ => {
                        return Err(ReductionRuleValidationError::MissingParam { key });
                    }
                }
            }
            Ok(out)
        }
        Some(_) => Err(ReductionRuleValidationError::MissingParam { key }),
        None => Err(ReductionRuleValidationError::MissingParam { key }),
    }
}

/// Stable ordering for storage and cache reads. Ascending `priority`
/// (lower runs first); tie-broken by `id` so two rules with the same
/// priority run in deterministic order across processes.
pub fn sort_for_storage(rules: &mut [ReductionRuleConfigView]) {
    rules.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
}

/// Default priority assigned to `author_reduction_rule`-generated rules.
/// Set equal to the rule id's natural-insertion spot in the existing
/// sort order `sort_for_storage`, so authored rules mix into the
/// operator's library without re-ordering pre-existing entries.
pub const fn sort_for_storage_default_priority() -> u32 {
    100
}

const _: u32 = sort_for_storage_default_priority();

#[cfg(test)]
mod tests {
    use super::*;

    fn typed_truncate(id: &str, field: &str, max_chars: u32) -> ReductionRuleConfigTyped {
        ReductionRuleConfigTyped::Truncate {
            id: id.to_string(),
            field: field.to_string(),
            priority: default_priority(),
            max_chars,
        }
    }

    #[test]
    fn rule_type_strings_match_python_constants() {
        assert_eq!(RuleType::Truncate.as_wire_str(), rule_type::TRUNCATE);
        assert_eq!(RuleType::Summarize.as_wire_str(), rule_type::SUMMARIZE);
        assert_eq!(RuleType::Drop.as_wire_str(), rule_type::DROP);
        assert_eq!(RuleType::Priority.as_wire_str(), rule_type::PRIORITY);
        assert_eq!(
            RuleType::HistoryCompact.as_wire_str(),
            rule_type::HISTORY_COMPACT
        );
    }

    #[test]
    fn rule_type_round_trip_through_strings() {
        for rule_type in [
            RuleType::Truncate,
            RuleType::Summarize,
            RuleType::Drop,
            RuleType::Priority,
            RuleType::HistoryCompact,
        ] {
            assert_eq!(
                RuleType::from_wire_str(rule_type.as_wire_str()),
                Some(rule_type)
            );
        }
        assert_eq!(RuleType::from_wire_str("unknown"), None);
    }

    #[test]
    fn wire_json_has_python_consumable_keys() {
        let cases = [
            (
                typed_truncate("foo", "content", 12).to_wire_json(),
                "truncate",
            ),
            (
                ReductionRuleConfigTyped::Summarize {
                    id: "s".into(),
                    field: "f".into(),
                    priority: 1,
                }
                .to_wire_json(),
                "summarize",
            ),
            (
                ReductionRuleConfigTyped::Drop {
                    id: "d".into(),
                    field: "f".into(),
                    priority: 1,
                }
                .to_wire_json(),
                "drop",
            ),
            (
                ReductionRuleConfigTyped::Priority {
                    id: "p".into(),
                    fields: vec!["a".into(), "b".into()],
                    priority: 1,
                }
                .to_wire_json(),
                "priority",
            ),
            (
                ReductionRuleConfigTyped::HistoryCompact {
                    id: "h".into(),
                    keep_recent_n: 5,
                    priority: 1,
                }
                .to_wire_json(),
                "history_compact",
            ),
        ];
        for (json, expected_type) in cases {
            assert_eq!(json["type"], expected_type);
        }
    }

    #[test]
    fn validate_rejects_empty_id() {
        let rule = ReductionRuleConfigView {
            id: "".to_string(),
            rule_type: RuleType::Summarize,
            params: serde_json::json!({"field": "content"}),
            priority: 1,
        };
        assert!(matches!(
            rule.validate(),
            Err(ReductionRuleValidationError::EmptyId)
        ));
    }

    #[test]
    fn validate_rejects_unknown_id_characters() {
        let rule = ReductionRuleConfigView {
            id: "Bad Id!".to_string(),
            rule_type: RuleType::Summarize,
            params: serde_json::json!({"field": "content"}),
            priority: 1,
        };
        assert!(matches!(
            rule.validate(),
            Err(ReductionRuleValidationError::InvalidIdFormat { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_max_chars() {
        let rule = ReductionRuleConfigView {
            id: "r".into(),
            rule_type: RuleType::Truncate,
            params: serde_json::json!({"field": "content", "max_chars": 0}),
            priority: 1,
        };
        assert!(matches!(
            rule.validate(),
            Err(ReductionRuleValidationError::ZeroMaxChars)
        ));
    }

    #[test]
    fn validate_rejects_empty_priority_fields() {
        let rule = ReductionRuleConfigView {
            id: "r".into(),
            rule_type: RuleType::Priority,
            params: serde_json::json!({"fields": []}),
            priority: 1,
        };
        assert!(matches!(
            rule.validate(),
            Err(ReductionRuleValidationError::EmptyPriorityFields)
        ));
    }

    #[test]
    fn validate_rejects_zero_keep_recent_n() {
        let rule = ReductionRuleConfigView {
            id: "r".into(),
            rule_type: RuleType::HistoryCompact,
            params: serde_json::json!({"keep_recent_n": 0}),
            priority: 1,
        };
        assert!(matches!(
            rule.validate(),
            Err(ReductionRuleValidationError::ZeroKeepRecentN)
        ));
    }

    #[test]
    fn sort_orders_lower_priority_first() {
        let mut rules = vec![
            ReductionRuleConfigView {
                id: "b".into(),
                rule_type: RuleType::Summarize,
                params: serde_json::json!({"field": "f"}),
                priority: 50,
            },
            ReductionRuleConfigView {
                id: "a".into(),
                rule_type: RuleType::Truncate,
                params: serde_json::json!({"field": "f", "max_chars": 1}),
                priority: 10,
            },
        ];
        sort_for_storage(&mut rules);
        assert_eq!(rules[0].id, "a");
        assert_eq!(rules[1].id, "b");
    }

    #[test]
    fn sort_tiebreaks_on_id() {
        let mut rules = vec![
            ReductionRuleConfigView {
                id: "zebra".into(),
                rule_type: RuleType::Summarize,
                params: serde_json::json!({"field": "f"}),
                priority: 5,
            },
            ReductionRuleConfigView {
                id: "apple".into(),
                rule_type: RuleType::Summarize,
                params: serde_json::json!({"field": "f"}),
                priority: 5,
            },
        ];
        sort_for_storage(&mut rules);
        assert_eq!(rules[0].id, "apple");
        assert_eq!(rules[1].id, "zebra");
    }
}
