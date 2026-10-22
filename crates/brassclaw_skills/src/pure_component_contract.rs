//! Independent pure-PythonCode review declarations. Parsing and exact closure
//! comparison establish selection only, never trusted evidence, semantic review,
//! component activation or execution authority. Real Tool usages retain their
//! separate, unchanged Skill-association formats.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    association_contract::{ComponentRevisionRef, ValidationMode},
    component_revision::RetainedComponentSnapshot,
    value_contract::{ContractError, ContractLimits, strict_json},
};

#[derive(Debug, thiserror::Error)]
pub enum PureComponentContractError {
    #[error("pure component declaration: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Data(#[from] ContractError),
}

fn invalid(reason: &'static str) -> PureComponentContractError {
    PureComponentContractError::Invalid(reason)
}
fn record<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, PureComponentContractError> {
    let value = value
        .as_object()
        .ok_or_else(|| invalid("object required"))?;
    if value.len() != fields.len() || fields.iter().any(|field| !value.contains_key(*field)) {
        return Err(invalid("fields differ from the declared format"));
    }
    Ok(value)
}
fn identifier(value: &Value) -> Result<Uuid, PureComponentContractError> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("canonical non-nil UUID required"))?;
    Uuid::parse_str(text)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == text)
        .ok_or_else(|| invalid("canonical non-nil UUID required"))
}
fn component(value: &Value) -> Result<ComponentRevisionRef, PureComponentContractError> {
    let value = record(value, &["uuid", "class_code", "version", "checksum"])?;
    let uuid = identifier(&value["uuid"])?;
    if value["class_code"].as_i64() != Some(22) {
        return Err(invalid("pure closure contains a non-PythonCode component"));
    }
    let version = value["version"]
        .as_u64()
        .filter(|version| *version > 0 && *version <= i64::MAX as u64)
        .ok_or_else(|| invalid("positive database-representable revision required"))?;
    let text = value["checksum"]
        .as_str()
        .filter(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| invalid("lowercase SHA-256 checksum required"))?;
    let mut checksum = [0; 32];
    for (index, byte) in checksum.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| invalid("invalid checksum"))?;
    }
    Ok(ComponentRevisionRef {
        uuid,
        class_code: 22,
        version,
        checksum,
    })
}

/// Untrusted immutable declaration, not an approval capability. A trusted reader
/// must resolve the actual Q1/behavior records and original review subject, plus
/// authenticated human Q2 for authored changes or noncircular package evidence
/// for shipped seeds. No boolean supplied by a caller establishes any of these.
pub struct PureComponentApprovalDeclaration {
    bytes: String,
    approval_id: Uuid,
    root: ComponentRevisionRef,
    components: BTreeMap<Uuid, ComponentRevisionRef>,
    mode: ValidationMode,
    q1: Uuid,
    q2: Option<Uuid>,
    behavioral: Vec<Uuid>,
}
impl std::fmt::Debug for PureComponentApprovalDeclaration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PureComponentApprovalDeclaration")
            .field("component_count", &self.components.len())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
impl PureComponentApprovalDeclaration {
    pub fn from_json(
        bytes: &str,
        limits: ContractLimits,
    ) -> Result<Self, PureComponentContractError> {
        let value = strict_json(bytes, limits)?;
        let value = record(
            &value,
            &[
                "format",
                "approval_id",
                "root",
                "components",
                "validation_mode",
                "q1_ref",
                "q2_ref",
                "behavioral_refs",
            ],
        )?;
        if value["format"] != "pure-component-approval/1" {
            return Err(invalid("unsupported format"));
        }
        let approval_id = identifier(&value["approval_id"])?;
        let root = component(&value["root"])?;
        let mut components = BTreeMap::new();
        for reference in value["components"]
            .as_array()
            .filter(|references| !references.is_empty())
            .ok_or_else(|| invalid("nonempty component closure required"))?
        {
            let reference = component(reference)?;
            if components.insert(reference.uuid, reference).is_some() {
                return Err(invalid("one revision per component identity required"));
            }
        }
        if components.get(&root.uuid) != Some(&root) {
            return Err(invalid("exact root is absent from the closure"));
        }
        let mode = match value["validation_mode"].as_str() {
            Some("authored") => ValidationMode::Authored,
            Some("system_seed") => ValidationMode::SystemSeed,
            _ => return Err(invalid("unsupported validation mode")),
        };
        let q1 = identifier(&value["q1_ref"])?;
        let q2 = match mode {
            ValidationMode::Authored => Some(identifier(&value["q2_ref"])?),
            ValidationMode::SystemSeed if value["q2_ref"].is_null() => None,
            ValidationMode::SystemSeed => return Err(invalid("shipped seed Q2 must be null")),
        };
        let mut unique = BTreeSet::from([q1]);
        if q2.is_some_and(|q2| !unique.insert(q2)) {
            return Err(invalid("review stages require distinct evidence"));
        }
        let mut behavioral = Vec::new();
        for reference in value["behavioral_refs"]
            .as_array()
            .filter(|references| !references.is_empty())
            .ok_or_else(|| invalid("behavioral evidence required"))?
        {
            let reference = identifier(reference)?;
            if !unique.insert(reference) {
                return Err(invalid("duplicate or reused evidence identity"));
            }
            behavioral.push(reference);
        }
        Ok(Self {
            bytes: bytes.to_owned(),
            approval_id,
            root,
            components,
            mode,
            q1,
            q2,
            behavioral,
        })
    }
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub fn approval_id(&self) -> Uuid {
        self.approval_id
    }
    pub fn root(&self) -> ComponentRevisionRef {
        self.root
    }
    pub fn components(&self) -> &BTreeMap<Uuid, ComponentRevisionRef> {
        &self.components
    }
    pub fn mode(&self) -> ValidationMode {
        self.mode
    }
    pub fn q1_reference(&self) -> Uuid {
        self.q1
    }
    pub fn q2_reference(&self) -> Option<Uuid> {
        self.q2
    }
    pub fn behavioral_references(&self) -> &[Uuid] {
        &self.behavioral
    }
    /// Equality against one complete retained rooted graph is necessary, but
    /// does not establish purity, successful review or trusted seed provenance.
    /// The actual source/behavior and evidence readers supply those contracts.
    pub fn require_selected_snapshot(
        &self,
        snapshot: &RetainedComponentSnapshot,
    ) -> Result<(), PureComponentContractError> {
        if snapshot.roots() != &BTreeSet::from([self.root.uuid])
            || snapshot.revisions().len() != self.components.len()
            || snapshot
                .revisions()
                .iter()
                .any(|(id, revision)| self.components.get(id) != Some(&revision.reference()))
        {
            return Err(invalid(
                "selected source/interface/dependency closure differs",
            ));
        }
        Ok(())
    }
}
