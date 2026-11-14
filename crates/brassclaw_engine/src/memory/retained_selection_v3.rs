//! Engine-side adapter for `monty-retained-recipe-selection/3`.
//!
//! All structural validation lives in `brassclaw_skills::retained_selection_contract`,
//! which is consumed by both this module and `review_selection_store` without a
//! crate cycle. This module re-exports the shared types and wraps the validated
//! bytes into the Engine's `RetainedRecipeSelection` newtype.
//!
//! ## What this module does
//!
//! - Re-exports the shared types and parse entry point from `retained_selection_contract`.
//! - Provides `build_v3_selection`: calls `build_v3_bytes` from the contract and
//!   wraps the result in a `RetainedRecipeSelection`.
//!
//! ## What this module does NOT do
//!
//! - It does not own validation logic (that lives in `retained_selection_contract`).
//! - It does not approve components, dispatch Tools, or grant kernel permissions.

use std::collections::BTreeMap;

use brassclaw_skills::retained_selection_contract::{
    self as contract, FunctionalOccurrencePin, SelectionContractError,
};
use serde_json::Value;

// Re-export the shared types so callers in the Engine crate can import from here.
pub use brassclaw_skills::retained_selection_contract::{
    ApprovalProfile, BASE_FIELDS, BaseFields, FORMAT_V1, FORMAT_V3, OwnApprovalReference,
    ParsedSelectionV3, SelectionVersion, is_valid_python_identifier, parse_selection_version,
};

use crate::memory::retained_instruction::RetainedSelectionError;

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

impl From<SelectionContractError> for RetainedSelectionError {
    fn from(e: SelectionContractError) -> Self {
        match e {
            SelectionContractError::Invalid => RetainedSelectionError::Invalid,
            SelectionContractError::Capacity => RetainedSelectionError::Capacity,
        }
    }
}

// ---------------------------------------------------------------------------
// Builder — wraps contract bytes into the Engine's RetainedRecipeSelection
// ---------------------------------------------------------------------------

/// Build a `/3` selection and wrap it in `RetainedRecipeSelection`.
///
/// Delegates all validation to `retained_selection_contract::build_v3_bytes`,
/// which performs a full strict-JSON round-trip before returning. The resulting
/// `RetainedRecipeSelection` holds the validated bytes and their SHA-256.
pub fn build_v3_selection(
    base_fields: BaseFields<'_>,
    association_approvals: &BTreeMap<String, Value>,
    functional_approvals: &BTreeMap<String, FunctionalOccurrencePin>,
    model_input_binding: Option<&Value>,
    limits: brassclaw_skills::value_contract::ContractLimits,
) -> Result<super::retained_instruction::RetainedRecipeSelection, RetainedSelectionError> {
    let (bytes, checksum) = contract::build_v3_bytes(
        base_fields,
        association_approvals,
        functional_approvals,
        model_input_binding,
        limits,
    )?;
    Ok(super::retained_instruction::RetainedRecipeSelection::from_parts(bytes, checksum, limits))
}
