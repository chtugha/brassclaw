//! Exact usage/combination declarations from `skills.md`.
//!
//! These are validated data, not trusted approvals. The catalogue owner must
//! resolve referenced classes, implementation artifacts, dependencies and actual
//! successful Q1/Q2/behavioral records before activation. No parsing, checksum
//! comparison or manifest can replace that evidence or grant Tool permission.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::value_contract::{
    ContractError, ContractLimits, InputContract, ValueContract, strict_json, validate_data_bounds,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssociationError {
    #[error("association contract: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Value(#[from] ContractError),
}

fn invalid(reason: &'static str) -> AssociationError {
    AssociationError::Invalid(reason)
}
fn record<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, AssociationError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("expected an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid("record fields do not match the declared format"));
    }
    Ok(object)
}
fn nonempty(value: &Value) -> Result<&str, AssociationError> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid("nonempty text required"))
}
fn uuid(value: &Value) -> Result<Uuid, AssociationError> {
    let id = Uuid::parse_str(nonempty(value)?).map_err(|_| invalid("invalid UUID"))?;
    if id.is_nil() {
        return Err(invalid("nil UUID is forbidden"));
    }
    Ok(id)
}
fn checksum(value: &Value) -> Result<[u8; 32], AssociationError> {
    let text = value.as_str().ok_or_else(|| invalid("invalid checksum"))?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("checksum must be lowercase SHA-256 hexadecimal"));
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| invalid("invalid checksum"))?;
    }
    Ok(digest)
}
fn parameter(name: &str) -> bool {
    let mut chars = name.bytes();
    chars
        .next()
        .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
        && chars.all(|b| b == b'_' || b.is_ascii_alphanumeric())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureAction {
    Stop,
    Retry,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idempotency {
    NotAssumed,
    ReadOnly,
    Deduplicated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RetryableOutcome {
    ConfirmedNoEffectTransient,
    UnknownCompletion,
}

/// A declared policy. Resolving evidence, durable invocation counts/keys,
/// cancellation, current policy and effect completion remains runner work.
#[derive(Clone)]
pub struct FailureContract {
    action: FailureAction,
    max_attempts: u64,
    idempotency: Idempotency,
    evidence: Option<String>,
    outcomes: BTreeSet<RetryableOutcome>,
}
impl fmt::Debug for FailureContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FailureContract")
            .field("action", &self.action)
            .field("max_attempts", &self.max_attempts)
            .field("idempotency", &self.idempotency)
            .field("outcomes", &self.outcomes)
            .finish_non_exhaustive()
    }
}
impl FailureContract {
    fn parse(value: &Value) -> Result<Self, AssociationError> {
        let v = record(
            value,
            &[
                "action",
                "max_attempts",
                "idempotency",
                "idempotency_evidence_ref",
                "retryable_outcomes",
            ],
        )?;
        let action = match v["action"].as_str() {
            Some("stop") => FailureAction::Stop,
            Some("retry") => FailureAction::Retry,
            _ => return Err(invalid("invalid failure action")),
        };
        let max_attempts = v["max_attempts"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("positive integer attempt count required"))?;
        let idempotency = match v["idempotency"].as_str() {
            Some("not_assumed") => Idempotency::NotAssumed,
            Some("read_only") => Idempotency::ReadOnly,
            Some("deduplicated") => Idempotency::Deduplicated,
            _ => return Err(invalid("invalid idempotency declaration")),
        };
        let evidence = if v["idempotency_evidence_ref"].is_null() {
            None
        } else {
            Some(nonempty(&v["idempotency_evidence_ref"])?.to_owned())
        };
        let mut outcomes = BTreeSet::new();
        for outcome in v["retryable_outcomes"]
            .as_array()
            .ok_or_else(|| invalid("outcome list required"))?
        {
            let outcome = match outcome.as_str() {
                Some("confirmed_no_effect_transient") => {
                    RetryableOutcome::ConfirmedNoEffectTransient
                }
                Some("unknown_completion") => RetryableOutcome::UnknownCompletion,
                _ => return Err(invalid("invalid retryable outcome")),
            };
            if !outcomes.insert(outcome) {
                return Err(invalid("duplicate retryable outcome"));
            }
        }
        match action {
            FailureAction::Stop if max_attempts != 1 || !outcomes.is_empty() => {
                return Err(invalid(
                    "stop permits exactly one dispatch and no retry outcomes",
                ));
            }
            FailureAction::Retry
                if max_attempts < 2
                    || outcomes.is_empty()
                    || idempotency == Idempotency::NotAssumed =>
            {
                return Err(invalid(
                    "retry requires attempts, eligible outcomes and declared evidence",
                ));
            }
            _ => {}
        }
        match idempotency {
            Idempotency::NotAssumed if evidence.is_some() => {
                return Err(invalid(
                    "unverified idempotency cannot carry retry evidence",
                ));
            }
            Idempotency::ReadOnly | Idempotency::Deduplicated if evidence.is_none() => {
                return Err(invalid("idempotency evidence reference required"));
            }
            _ => {}
        }
        Ok(Self {
            action,
            max_attempts,
            idempotency,
            evidence,
            outcomes,
        })
    }
    pub fn action(&self) -> FailureAction {
        self.action
    }
    pub fn max_attempts(&self) -> u64 {
        self.max_attempts
    }
    pub fn idempotency(&self) -> Idempotency {
        self.idempotency
    }
    pub fn evidence_reference(&self) -> Option<&str> {
        self.evidence.as_deref()
    }
    pub fn declared_outcomes(&self) -> &BTreeSet<RetryableOutcome> {
        &self.outcomes
    }
}

/// Immutable association bytes are retained exactly: reformatting JSON changes
/// the checksum reviewed by the separate approval declaration.
#[derive(Clone)]
pub struct SkillAssociation {
    bytes: String,
    checksum: [u8; 32],
    skill: Uuid,
    python: Uuid,
    tool_skill: Uuid,
    tool: Uuid,
    callable: String,
    inputs: InputContract,
    arguments: BTreeMap<String, String>,
    computed: BTreeMap<String, ValueContract>,
    result: ValueContract,
    failure: FailureContract,
    limits: ContractLimits,
}
impl fmt::Debug for SkillAssociation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillAssociation")
            .field("checksum", &self.checksum)
            .field(
                "argument_count",
                &(self.arguments.len() + self.computed.len()),
            )
            .finish_non_exhaustive()
    }
}
impl SkillAssociation {
    pub fn from_json(source: &str, limits: ContractLimits) -> Result<Self, AssociationError> {
        let value = strict_json(source, limits)?;
        let v = record(
            &value,
            &[
                "format",
                "skill_uuid",
                "python_code_uuid",
                "tool_skill_uuid",
                "tool_uuid",
                "callable",
                "inputs",
                "arguments",
                "code_arguments",
                "result",
                "failure",
            ],
        )?;
        if v["format"] != "skill-association/1" {
            return Err(invalid("unsupported association format"));
        }
        let (skill, python, tool_skill, tool) = (
            uuid(&v["skill_uuid"])?,
            uuid(&v["python_code_uuid"])?,
            uuid(&v["tool_skill_uuid"])?,
            uuid(&v["tool_uuid"])?,
        );
        if BTreeSet::from([skill, python, tool_skill, tool]).len() != 4 {
            return Err(invalid("usage roles require distinct component UUIDs"));
        }
        let callable = nonempty(&v["callable"])?;
        if !callable.strip_prefix("host.").is_some_and(parameter) {
            return Err(invalid("one host callable identifier required"));
        }
        let inputs = InputContract::from_value(&v["inputs"], limits)?;
        let mut arguments = BTreeMap::new();
        for (name, input) in v["arguments"]
            .as_object()
            .ok_or_else(|| invalid("argument mapping required"))?
        {
            let input = nonempty(input)?;
            if !parameter(name) || !inputs.names().any(|name| name == input) {
                return Err(invalid(
                    "argument references an invalid parameter or undeclared input",
                ));
            }
            arguments.insert(name.clone(), input.to_owned());
        }
        let mut computed = BTreeMap::new();
        for (name, contract) in v["code_arguments"]
            .as_object()
            .ok_or_else(|| invalid("computed argument mapping required"))?
        {
            if !parameter(name) || arguments.contains_key(name) {
                return Err(invalid("invalid or overlapping Tool argument"));
            }
            computed.insert(
                name.clone(),
                ValueContract::computed_from_value(contract, &inputs)?,
            );
        }
        let result = ValueContract::result_from_value(&v["result"], limits)?;
        let failure = FailureContract::parse(&v["failure"])?;
        Ok(Self {
            bytes: source.to_owned(),
            checksum: Sha256::digest(source.as_bytes()).into(),
            skill,
            python,
            tool_skill,
            tool,
            callable: callable.to_owned(),
            inputs,
            arguments,
            computed,
            result,
            failure,
            limits,
        })
    }
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub fn skill_uuid(&self) -> Uuid {
        self.skill
    }
    pub fn python_code_uuid(&self) -> Uuid {
        self.python
    }
    pub fn tool_skill_uuid(&self) -> Uuid {
        self.tool_skill
    }
    pub fn tool_uuid(&self) -> Uuid {
        self.tool
    }
    pub fn callable(&self) -> &str {
        &self.callable
    }
    pub fn inputs(&self) -> &InputContract {
        &self.inputs
    }
    pub fn result(&self) -> &ValueContract {
        &self.result
    }
    pub fn failure(&self) -> &FailureContract {
        &self.failure
    }

    /// Prove declared argument schemas fit the registered Tool parameter
    /// contracts and cover all required parameters. This does not prove the
    /// code's calculation or replace actual argument validation at dispatch.
    pub fn require_tool_input_contract(
        &self,
        tool_inputs: &InputContract,
    ) -> Result<(), AssociationError> {
        for name in tool_inputs.required_names() {
            if !self.arguments.contains_key(name) && !self.computed.contains_key(name) {
                return Err(invalid("usage omits a required Tool parameter"));
            }
        }
        for (name, input) in &self.arguments {
            self.inputs
                .bound_value_contract(input)?
                .require_compatible_input(tool_inputs, name)?;
        }
        for (name, contract) in &self.computed {
            contract.require_compatible_input(tool_inputs, name)?;
        }
        Ok(())
    }

    /// Check actual Tool arguments immediately before dispatch. No calculation
    /// is interpreted from `meaning`; approved Python supplied these values.
    /// The registered Tool's own schema/representation and policy still apply.
    pub fn validate_arguments(
        &self,
        supplied_inputs: &Value,
        actual: &Value,
    ) -> Result<(), AssociationError> {
        validate_data_bounds(actual, self.limits)?;
        let bound = self.inputs.bind(supplied_inputs)?;
        let args = actual
            .as_object()
            .ok_or_else(|| invalid("Tool arguments must be an object"))?;
        if args.len() != self.arguments.len() + self.computed.len() {
            return Err(invalid("Tool argument mapping is incomplete or has extras"));
        }
        for (name, input) in &self.arguments {
            if args.get(name) != bound.get(input) {
                return Err(invalid("direct Tool argument differs from its bound input"));
            }
        }
        for (name, contract) in &self.computed {
            contract.validate(
                args.get(name)
                    .ok_or_else(|| invalid("computed Tool argument missing"))?,
            )?;
        }
        Ok(())
    }
}

/// Content/metadata/implementation checksum must be supplied by the immutable
/// catalogue; this declaration cannot establish that an artifact exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentRevisionRef {
    pub uuid: Uuid,
    pub class_code: i32,
    pub version: u64,
    pub checksum: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Authored,
    SystemSeed,
}

/// Untrusted evidence declaration, even after structural validation. Trusted
/// stores must independently resolve its references and verify success for the
/// exact combination. This type intentionally has no `approved` boolean.
#[derive(Clone)]
pub struct AssociationApprovalDeclaration {
    bytes: String,
    approval_id: Uuid,
    association_checksum: [u8; 32],
    components: BTreeMap<Uuid, ComponentRevisionRef>,
    mode: ValidationMode,
    q1: String,
    q2: Option<String>,
    behavioral: Vec<String>,
}
impl fmt::Debug for AssociationApprovalDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssociationApprovalDeclaration")
            .field("component_count", &self.components.len())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
impl AssociationApprovalDeclaration {
    pub fn from_json(source: &str, limits: ContractLimits) -> Result<Self, AssociationError> {
        let value = strict_json(source, limits)?;
        let v = record(
            &value,
            &[
                "format",
                "approval_id",
                "association_checksum",
                "components",
                "validation_mode",
                "q1_ref",
                "q2_ref",
                "behavioral_refs",
            ],
        )?;
        if v["format"] != "skill-association-approval/1" {
            return Err(invalid("unsupported approval format"));
        }
        let approval_id = uuid(&v["approval_id"])?;
        let association_checksum = checksum(&v["association_checksum"])?;
        let mut components = BTreeMap::new();
        for component in v["components"]
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| invalid("nonempty component list required"))?
        {
            let c = record(component, &["uuid", "class_code", "version", "checksum"])?;
            let id = uuid(&c["uuid"])?;
            let class_code = c["class_code"]
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| invalid("integer component class required"))?;
            let version = c["version"]
                .as_u64()
                .filter(|v| *v > 0)
                .ok_or_else(|| invalid("positive integer component version required"))?;
            let component = ComponentRevisionRef {
                uuid: id,
                class_code,
                version,
                checksum: checksum(&c["checksum"])?,
            };
            if components.insert(id, component).is_some() {
                return Err(invalid("one selected revision per component UUID required"));
            }
        }
        let mode = match v["validation_mode"].as_str() {
            Some("authored") => ValidationMode::Authored,
            Some("system_seed") => ValidationMode::SystemSeed,
            _ => return Err(invalid("unsupported validation mode")),
        };
        let q1 = nonempty(&v["q1_ref"])?.to_owned();
        let q2 = match mode {
            ValidationMode::Authored => Some(nonempty(&v["q2_ref"])?.to_owned()),
            ValidationMode::SystemSeed if v["q2_ref"].is_null() => None,
            _ => return Err(invalid("system seed Q2 reference must be null")),
        };
        let mut behavioral = Vec::new();
        for reference in v["behavioral_refs"]
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| invalid("behavioral evidence references required"))?
        {
            let reference = nonempty(reference)?;
            behavioral.push(reference.to_owned());
        }
        Ok(Self {
            bytes: source.into(),
            approval_id,
            association_checksum,
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
    pub fn components(&self) -> &BTreeMap<Uuid, ComponentRevisionRef> {
        &self.components
    }
    pub fn mode(&self) -> ValidationMode {
        self.mode
    }
    pub fn q1_reference(&self) -> &str {
        &self.q1
    }
    pub fn q2_reference(&self) -> Option<&str> {
        self.q2.as_deref()
    }
    pub fn behavioral_references(&self) -> &[String] {
        &self.behavioral
    }

    /// Compare against the caller's complete selected transitive graph, not
    /// merely the four root components. This proves equality only; evidence
    /// resolution, graph completeness and approved activation remain separate.
    pub fn require_selected_combination(
        &self,
        association: &SkillAssociation,
        selected: &[ComponentRevisionRef],
    ) -> Result<(), AssociationError> {
        if self.association_checksum != association.checksum {
            return Err(invalid(
                "association bytes differ from reviewed combination",
            ));
        }
        let mut actual = BTreeMap::new();
        for component in selected {
            if component.uuid.is_nil()
                || component.version == 0
                || actual.insert(component.uuid, *component).is_some()
            {
                return Err(invalid("invalid or duplicate selected component"));
            }
        }
        if actual != self.components {
            return Err(invalid(
                "selected revisions differ from reviewed dependency combination",
            ));
        }
        for (id, class) in [
            (association.python, 22),
            (association.tool_skill, 13),
            (association.tool, 0),
        ] {
            if actual.get(&id).is_none_or(|r| r.class_code != class) {
                return Err(invalid("component role has the wrong class"));
            }
        }
        if !actual
            .get(&association.skill)
            .is_some_and(|r| (1..=3).contains(&r.class_code))
        {
            return Err(invalid("association owner is not a usage Skill"));
        }
        Ok(())
    }
}
