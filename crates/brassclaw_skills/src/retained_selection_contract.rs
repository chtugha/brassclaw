//! Strict passive `/1` and `/3` retained-selection wire contract.
//! Captured limits apply before decoding. The exact original bytes/checksum are
//! retained; parsing authenticates no approval and activates no workflow.
//! Database graph/evidence authentication stays with the existing store owners.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::RetainedComponentSnapshot,
    value_contract::{ContractError, ContractLimits, InputContract},
};

// ---------------------------------------------------------------------------
// Public format-version constants
// ---------------------------------------------------------------------------

pub const FORMAT_V1: &str = "monty-retained-recipe-selection/1";
pub const FORMAT_V3: &str = "monty-retained-recipe-selection/3";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SelectionContractError {
    #[error("retained selection: invalid structure")]
    Invalid,
    #[error("retained selection: document exceeds capacity limit")]
    Capacity,
}

impl From<ContractError> for SelectionContractError {
    fn from(_: ContractError) -> Self {
        SelectionContractError::Invalid
    }
}

// ---------------------------------------------------------------------------
// Approval-profile enum
// ---------------------------------------------------------------------------

/// Passive profile tag carried in a `/3` occurrence pin. The existing trusted
/// review owner must authenticate its provenance; parsing grants no approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApprovalProfile {
    PureOwn,
    AuthoredFunction,
}

impl ApprovalProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            ApprovalProfile::PureOwn => "pure_own",
            ApprovalProfile::AuthoredFunction => "authored_function",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pure_own" => Some(Self::PureOwn),
            "authored_function" => Some(Self::AuthoredFunction),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Validated record types (private fields, public accessors)
// ---------------------------------------------------------------------------

/// Validated per-root approval reference inside a `FunctionalOccurrencePin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnApprovalReference {
    root: ComponentRevisionRef,
    approval_id: Uuid,
    approval_checksum: [u8; 32],
    subject_checksum: [u8; 32],
}

impl OwnApprovalReference {
    pub fn root(&self) -> ComponentRevisionRef {
        self.root
    }
    pub fn approval_id(&self) -> Uuid {
        self.approval_id
    }
    pub fn approval_checksum(&self) -> [u8; 32] {
        self.approval_checksum
    }
    pub fn subject_checksum(&self) -> [u8; 32] {
        self.subject_checksum
    }

    /// Serialize to canonical JSON for embedding in a `/3` document.
    pub fn to_value(&self) -> Value {
        serde_json::json!({
            "root": native_ref_value(self.root),
            "approval_id": self.approval_id,
            "approval_checksum": hex_encode(&self.approval_checksum),
            "subject_checksum": hex_encode(&self.subject_checksum),
        })
    }

    fn from_object(
        obj: &serde_json::Map<String, Value>,
        key_uuid: Uuid,
    ) -> Result<Self, SelectionContractError> {
        closed_object(
            obj,
            &[
                "root",
                "approval_id",
                "approval_checksum",
                "subject_checksum",
            ],
        )?;
        let root = validated_component_ref(&obj["root"])?;
        // Map key must match root.uuid.
        if key_uuid != root.uuid {
            return Err(SelectionContractError::Invalid);
        }
        let approval_id = parse_uuid_str(&obj["approval_id"])?;
        if obj["approval_id"] != Value::String(approval_id.to_string()) {
            return Err(SelectionContractError::Invalid);
        }
        let approval_checksum = parse_sha256_hex_str(&obj["approval_checksum"])?;
        let subject_checksum = parse_sha256_hex_str(&obj["subject_checksum"])?;
        Ok(Self {
            root,
            approval_id,
            approval_checksum,
            subject_checksum,
        })
    }
}

/// Validated per-step functional occurrence pin inside a `/3` selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionalOccurrencePin {
    profile: ApprovalProfile,
    implementation: ComponentRevisionRef,
    export: String,
    interface_checksum: [u8; 32],
    own_approvals: BTreeMap<String, OwnApprovalReference>,
}

impl FunctionalOccurrencePin {
    pub fn profile(&self) -> ApprovalProfile {
        self.profile
    }
    pub fn implementation(&self) -> ComponentRevisionRef {
        self.implementation
    }
    pub fn export(&self) -> &str {
        &self.export
    }
    pub fn interface_checksum(&self) -> [u8; 32] {
        self.interface_checksum
    }
    pub fn own_approvals(&self) -> &BTreeMap<String, OwnApprovalReference> {
        &self.own_approvals
    }

    /// Serialize to canonical JSON for embedding in a `/3` document.
    pub fn to_value(&self) -> Value {
        let approvals: serde_json::Map<String, Value> = self
            .own_approvals
            .iter()
            .map(|(k, v)| (k.clone(), v.to_value()))
            .collect();
        serde_json::json!({
            "profile": self.profile.as_str(),
            "implementation": native_ref_value(self.implementation),
            "export": self.export,
            "interface_checksum": hex_encode(&self.interface_checksum),
            "own_approvals": Value::Object(approvals),
        })
    }

    fn from_object(obj: &serde_json::Map<String, Value>) -> Result<Self, SelectionContractError> {
        closed_object(
            obj,
            &[
                "profile",
                "implementation",
                "export",
                "interface_checksum",
                "own_approvals",
            ],
        )?;
        let profile = ApprovalProfile::parse(
            obj["profile"]
                .as_str()
                .ok_or(SelectionContractError::Invalid)?,
        )
        .ok_or(SelectionContractError::Invalid)?;
        let implementation = validated_component_ref(&obj["implementation"])?;
        if implementation.class_code != 22 {
            return Err(SelectionContractError::Invalid);
        }
        let export = obj["export"]
            .as_str()
            .filter(|s| crate::preload_identifier::public_export(s))
            .ok_or(SelectionContractError::Invalid)?
            .to_owned();
        let interface_checksum = parse_sha256_hex_str(&obj["interface_checksum"])?;
        let approvals_map = obj["own_approvals"]
            .as_object()
            .filter(|o| !o.is_empty())
            .ok_or(SelectionContractError::Invalid)?;
        let mut own_approvals = BTreeMap::new();
        for (key, entry) in approvals_map {
            let key_uuid = Uuid::parse_str(key)
                .ok()
                .filter(|id| !id.is_nil() && id.to_string() == *key)
                .ok_or(SelectionContractError::Invalid)?;
            let entry_obj = entry.as_object().ok_or(SelectionContractError::Invalid)?;
            let approval = OwnApprovalReference::from_object(entry_obj, key_uuid)?;
            if own_approvals.insert(key.clone(), approval).is_some() {
                return Err(SelectionContractError::Invalid);
            }
        }
        Ok(Self {
            profile,
            implementation,
            export,
            interface_checksum,
            own_approvals,
        })
    }
}

// ---------------------------------------------------------------------------
// Parsed /3 record
// ---------------------------------------------------------------------------

/// Structurally validated base, shared by both versions. No normalization of
/// optional fields or byte representation occurs during historical reads.
pub struct ParsedSelection {
    bytes: String,
    checksum: [u8; 32],
    value: Value,
    recipe: ComponentRevisionRef,
    components: Vec<ComponentRevisionRef>,
}
impl ParsedSelection {
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn recipe(&self) -> ComponentRevisionRef {
        self.recipe
    }
    pub fn components(&self) -> &[ComponentRevisionRef] {
        &self.components
    }
    pub fn into_value(self) -> Value {
        self.value
    }
}

pub struct ParsedSelectionV3 {
    selection: ParsedSelection,
    association_approvals: BTreeMap<String, Value>,
    functional_approvals: BTreeMap<String, FunctionalOccurrencePin>,
}
impl ParsedSelectionV3 {
    pub fn exact_bytes(&self) -> &str {
        self.selection.exact_bytes()
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.selection.checksum()
    }
    pub fn base(&self) -> BaseFields<'_> {
        BaseFields {
            raw: &self.selection.value,
        }
    }
    pub fn association_approvals(&self) -> &BTreeMap<String, Value> {
        &self.association_approvals
    }
    pub fn functional_approvals(&self) -> &BTreeMap<String, FunctionalOccurrencePin> {
        &self.functional_approvals
    }
    pub fn model_input_binding(&self) -> Option<&Value> {
        self.selection.value.get("model_input_binding")
    }
}

pub enum SelectionVersion {
    V1(ParsedSelection),
    V3(ParsedSelectionV3),
}
impl SelectionVersion {
    pub fn selection(&self) -> &ParsedSelection {
        match self {
            Self::V1(v) => v,
            Self::V3(v) => &v.selection,
        }
    }
    pub fn into_value(self) -> Value {
        match self {
            Self::V1(v) => v.into_value(),
            Self::V3(v) => v.selection.into_value(),
        }
    }
}

/// The existing base fields, excluding the version tag.
pub const BASE_FIELDS: &[&str] = &[
    "recipe",
    "variant",
    "workflow_class",
    "input_layout",
    "invocation_layout",
    "step_order",
    "components",
];

/// Borrowed projection of the seven base fields. Parsed selections lend their
/// existing Value; they do not keep a second owned input-layout/constant graph.
#[derive(Clone, Copy)]
pub struct BaseFields<'a> {
    raw: &'a Value,
}
impl<'a> BaseFields<'a> {
    /// Fresh builder input must contain exactly the base fields. This cannot
    /// strip an unknown format/capture/layout extension from an arbitrary record.
    pub fn from_value(raw: &'a Value) -> Result<Self, SelectionContractError> {
        closed_object(object(raw)?, BASE_FIELDS)?;
        Ok(Self { raw })
    }
    pub fn get(&self, name: &str) -> Option<&'a Value> {
        if BASE_FIELDS.contains(&name) {
            self.raw.get(name)
        } else {
            None
        }
    }
}

/// Decode once under the actual acknowledged/original policy. The common JSON
/// reader supplies duplicate-key rejection and classified resource failures;
/// ContractLimits retains the existing supported schema-depth ceiling.
pub fn parse_selection_version(
    bytes: &str,
    limits: ContractLimits,
) -> Result<SelectionVersion, SelectionContractError> {
    use brassclaw_common::json::{DuplicateKeys, JsonReadError, JsonReadLimits, read_json};
    limits.validate_binding_limits()?;
    let raw = read_json(
        bytes,
        JsonReadLimits {
            max_depth: limits.max_depth,
            max_nodes: limits.max_nodes,
            max_bytes: limits.max_bytes,
        },
        DuplicateKeys::Reject,
    )
    .map_err(|error| match error {
        JsonReadError::DocumentBytes | JsonReadError::Resource => SelectionContractError::Capacity,
        _ => SelectionContractError::Invalid,
    })?
    .into_value();
    parse_value(bytes, raw, limits)
}

/// Borrowed checked flat base. Top-level format/profile/evidence/capture fields
/// outside the historical base stay with their versioned codec owner. This is
/// the adoption boundary for Q's reserved `/2`; it never interprets that tag.
pub struct ValidatedBaseWire<'a> {
    raw: &'a Value,
    recipe: ComponentRevisionRef,
    components: Vec<ComponentRevisionRef>,
    components_by_uuid: BTreeMap<Uuid, ComponentRevisionRef>,
    association_approvals: BTreeMap<String, Value>,
}
impl ValidatedBaseWire<'_> {
    pub fn recipe(&self) -> ComponentRevisionRef {
        self.recipe
    }
    pub fn components(&self) -> &[ComponentRevisionRef] {
        &self.components
    }
    pub fn require_snapshot(
        &self,
        snapshot: &RetainedComponentSnapshot,
    ) -> Result<(), SelectionContractError> {
        require_base_snapshot(self.raw, self.recipe, &self.components, snapshot, None)
    }
}
/// Check the existing flat base under caller-supplied bounds without dispatching
/// a format or accepting its extensions. Structured layout/flow adoption must
/// extend this same owner deliberately; this profile never discards those fields.
pub fn validate_base_wire(
    raw: &Value,
    limits: ContractLimits,
) -> Result<ValidatedBaseWire<'_>, SelectionContractError> {
    limits.validate_binding_limits()?;
    crate::value_contract::validate_data_bounds(raw, limits)
        .map_err(|_| SelectionContractError::Capacity)?;
    validate_base_fields(raw, limits)
}
fn validate_base_fields(
    raw: &Value,
    limits: ContractLimits,
) -> Result<ValidatedBaseWire<'_>, SelectionContractError> {
    let obj = object(raw)?;
    nonempty(obj.get("format").ok_or(SelectionContractError::Invalid)?)?;
    if BASE_FIELDS.iter().any(|key| !obj.contains_key(*key))
        || !matches!(
            obj["workflow_class"].as_str(),
            Some("deterministic" | "requires_model")
        )
    {
        return Err(SelectionContractError::Invalid);
    }
    let recipe = base_component_ref(&obj["recipe"])?;
    if recipe.class_code != 21 {
        return Err(SelectionContractError::Invalid);
    }
    let mut components = Vec::new();
    let mut selected = BTreeMap::new();
    for value in array(&obj["components"])? {
        let reference = base_component_ref(value)?;
        if selected.insert(reference.uuid, reference).is_some() {
            return Err(SelectionContractError::Invalid);
        }
        components.push(reference);
    }
    if selected.get(&recipe.uuid) != Some(&recipe) {
        return Err(SelectionContractError::Invalid);
    }
    let steps = array(&obj["step_order"])?;
    if steps.is_empty() {
        return Err(SelectionContractError::Invalid);
    }
    let mut order = BTreeMap::new();
    for (index, step) in steps.iter().enumerate() {
        let id = step.as_str().ok_or(SelectionContractError::Invalid)?;
        step_coordinates(id)?;
        if order.insert(id, index).is_some() {
            return Err(SelectionContractError::Invalid);
        }
    }
    validate_variant(&obj["variant"])?;
    let variant = obj["variant"]["variant_key"]
        .as_str()
        .ok_or(SelectionContractError::Invalid)?;
    validate_input_layout(&obj["input_layout"], &order, &obj["variant"], limits)?;
    validate_invocations(
        &obj["invocation_layout"],
        variant,
        &order,
        &obj["input_layout"],
    )?;
    if let Some(binding) = obj.get("model_input_binding") {
        closed_object(object(binding)?, &["format", "binding"])?;
        if binding["format"] != "recipe-model-input-selection/1" {
            return Err(SelectionContractError::Invalid);
        }
        let model: crate::recipe_model_input::RecipeModelInput =
            serde_json::from_value(binding["binding"].clone())
                .map_err(|_| SelectionContractError::Invalid)?;
        let doc = serde_json::json!({"model_input_bindings":{"format":"recipe-model-input-bindings/1","variants":{variant:binding["binding"]}}});
        crate::recipe_model_input::RecipeModelInput::for_variant(
            &doc,
            &BTreeSet::from([variant.to_owned()]),
            variant,
        )
        .map_err(|_| SelectionContractError::Invalid)?;
        if model.retained_binding() != *binding {
            return Err(SelectionContractError::Invalid);
        }
    }
    let mut association_approvals = BTreeMap::new();
    if let Some(pins) = obj.get("association_approvals") {
        for (step, value) in object(pins)? {
            if !object(&obj["input_layout"]["steps"])?.contains_key(step) {
                return Err(SelectionContractError::Invalid);
            }
            closed_object(object(value)?, &["approval_id", "checksum"])?;
            parse_uuid_str(&value["approval_id"])?;
            parse_sha256_hex_str(&value["checksum"])?;
            association_approvals.insert(step.clone(), value.clone());
        }
    }
    Ok(ValidatedBaseWire {
        raw,
        recipe,
        components,
        components_by_uuid: selected,
        association_approvals,
    })
}

fn parse_value(
    bytes: &str,
    raw: Value,
    limits: ContractLimits,
) -> Result<SelectionVersion, SelectionContractError> {
    let obj = object(&raw)?;
    let v3 = match obj.get("format").and_then(Value::as_str) {
        Some(FORMAT_V1) => false,
        Some(FORMAT_V3) => true,
        _ => return Err(SelectionContractError::Invalid),
    };
    for key in obj.keys() {
        if !BASE_FIELDS.contains(&key.as_str())
            && !matches!(
                key.as_str(),
                "format" | "association_approvals" | "model_input_binding"
            )
            && !(v3 && key == "functional_approvals")
        {
            return Err(SelectionContractError::Invalid);
        }
    }
    let ValidatedBaseWire {
        recipe,
        components,
        components_by_uuid: selected,
        association_approvals,
        ..
    } = validate_base_fields(&raw, limits)?;
    let variant = obj["variant"]["variant_key"]
        .as_str()
        .ok_or(SelectionContractError::Invalid)?;
    let mut functional_approvals = BTreeMap::new();
    if v3 {
        let pins = object(
            obj.get("functional_approvals")
                .ok_or(SelectionContractError::Invalid)?,
        )?;
        if pins.is_empty() {
            return Err(SelectionContractError::Invalid);
        }
        for (step, value) in pins {
            if !object(&obj["input_layout"]["steps"])?.contains_key(step)
                || association_approvals.contains_key(step)
            {
                return Err(SelectionContractError::Invalid);
            }
            let pin = FunctionalOccurrencePin::from_object(object(value)?)?;
            if selected.get(&pin.implementation.uuid) != Some(&pin.implementation)
                || pin
                    .own_approvals
                    .values()
                    .any(|approval| selected.get(&approval.root.uuid) != Some(&approval.root))
            {
                return Err(SelectionContractError::Invalid);
            }
            require_functional_roots(&pin)?;
            let export = obj["invocation_layout"]
                .get(variant)
                .and_then(|v| v.get(step))
                .and_then(Value::as_str);
            if export.is_some_and(|export| export != pin.export) {
                return Err(SelectionContractError::Invalid);
            }
            functional_approvals.insert(step.clone(), pin);
        }
    }
    let selection = ParsedSelection {
        bytes: bytes.to_owned(),
        checksum: sha256_of(bytes.as_bytes()),
        value: raw,
        recipe,
        components,
    };
    if v3 {
        Ok(SelectionVersion::V3(ParsedSelectionV3 {
            selection,
            association_approvals,
            functional_approvals,
        }))
    } else {
        Ok(SelectionVersion::V1(selection))
    }
}

// An authored function may change only its Skill. Its unchanged code can
// retain S prerequisite evidence; no invented A own-code approval is required.
fn require_functional_roots(pin: &FunctionalOccurrencePin) -> Result<(), SelectionContractError> {
    if (pin.profile == ApprovalProfile::PureOwn
        && !pin
            .own_approvals
            .values()
            .any(|approval| approval.root == pin.implementation))
        || pin
            .own_approvals
            .values()
            .any(|approval| match approval.root.class_code {
                22 => approval.root != pin.implementation,
                1..=3 => pin.profile != ApprovalProfile::AuthoredFunction,
                _ => true,
            })
    {
        return Err(SelectionContractError::Invalid);
    }
    Ok(())
}

/// Serialize through the established canonical Value owner, then parse once
/// under the caller's captured limits. No new global quota or byte-domain change.
pub fn build_v3_bytes(
    base_fields: BaseFields<'_>,
    association_approvals: &BTreeMap<String, Value>,
    functional_approvals: &BTreeMap<String, FunctionalOccurrencePin>,
    model_input_binding: Option<&Value>,
    limits: ContractLimits,
) -> Result<(String, [u8; 32]), SelectionContractError> {
    let mut obj = serde_json::Map::new();
    for key in BASE_FIELDS {
        obj.insert(
            (*key).to_owned(),
            base_fields
                .get(key)
                .ok_or(SelectionContractError::Invalid)?
                .clone(),
        );
    }
    obj.insert("format".into(), Value::String(FORMAT_V3.into()));
    if !association_approvals.is_empty() {
        obj.insert(
            "association_approvals".into(),
            serde_json::json!(association_approvals),
        );
    }
    obj.insert(
        "functional_approvals".into(),
        Value::Object(
            functional_approvals
                .iter()
                .map(|(k, v)| (k.clone(), v.to_value()))
                .collect(),
        ),
    );
    if let Some(binding) = model_input_binding {
        obj.insert("model_input_binding".into(), binding.clone());
    }
    let value = Value::Object(obj);
    crate::value_contract::validate_data_bounds(&value, limits)
        .map_err(|_| SelectionContractError::Capacity)?;
    let bytes = value.to_string();
    drop(value); // release the new document tree before its validation decode
    let parsed = parse_selection_version(&bytes, limits)?;
    Ok((bytes, parsed.selection().checksum()))
}

fn object(value: &Value) -> Result<&serde_json::Map<String, Value>, SelectionContractError> {
    value.as_object().ok_or(SelectionContractError::Invalid)
}
fn array(value: &Value) -> Result<&Vec<Value>, SelectionContractError> {
    value.as_array().ok_or(SelectionContractError::Invalid)
}
fn nonempty(value: &Value) -> Result<&str, SelectionContractError> {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or(SelectionContractError::Invalid)
}
fn input_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn step_coordinates(id: &str) -> Result<(usize, u32), SelectionContractError> {
    let (desc, step) = id.split_once(':').ok_or(SelectionContractError::Invalid)?;
    let desc: usize = desc.parse().map_err(|_| SelectionContractError::Invalid)?;
    let step: u32 = step.parse().map_err(|_| SelectionContractError::Invalid)?;
    if step == 0 || format!("{desc}:{step}") != id {
        return Err(SelectionContractError::Invalid);
    }
    Ok((desc, step))
}
fn validate_variant(raw: &Value) -> Result<(), SelectionContractError> {
    let obj = object(raw)?;
    let allowed = [
        "variant_key",
        "description",
        "step_link",
        "intent_examples",
        "variable_patterns",
    ];
    if obj.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(SelectionContractError::Invalid);
    }
    nonempty(&raw["variant_key"])?;
    if obj
        .get("description")
        .is_some_and(|v| !v.is_null() && !v.is_string())
    {
        return Err(SelectionContractError::Invalid);
    }
    let link = nonempty(&raw["step_link"])?;
    crate::retained_step_link::parse_step_link(link)
        .map_err(|_| SelectionContractError::Invalid)?;
    if let Some(examples) = obj.get("intent_examples") {
        for example in array(examples)? {
            example.as_str().ok_or(SelectionContractError::Invalid)?;
        }
    }
    if let Some(patterns) = obj.get("variable_patterns") {
        let mut names = BTreeSet::new();
        for pattern in array(patterns)? {
            let fields = object(pattern)?;
            if fields
                .keys()
                .any(|k| !["name", "pattern", "description"].contains(&k.as_str()))
            {
                return Err(SelectionContractError::Invalid);
            }
            let name = nonempty(&pattern["name"])?;
            if !input_name(name) || !names.insert(name) {
                return Err(SelectionContractError::Invalid);
            }
            for key in ["pattern", "description"] {
                if fields
                    .get(key)
                    .is_some_and(|v| !v.is_null() && !v.is_string())
                {
                    return Err(SelectionContractError::Invalid);
                }
            }
        }
    }
    Ok(())
}
fn validate_input_layout(
    raw: &Value,
    order: &BTreeMap<&str, usize>,
    variant: &Value,
    limits: ContractLimits,
) -> Result<(), SelectionContractError> {
    closed_object(object(raw)?, &["format", "task_inputs", "steps"])?;
    if raw["format"] != "recipe-input-layout/1" {
        return Err(SelectionContractError::Invalid);
    }
    let inputs = InputContract::from_value(&raw["task_inputs"], limits)?;
    let names: BTreeSet<_> = inputs.names().collect();
    if let Some(patterns) = variant.get("variable_patterns") {
        for pattern in array(patterns)? {
            if !names.contains(nonempty(&pattern["name"])?) {
                return Err(SelectionContractError::Invalid);
            }
        }
    }
    let steps = object(&raw["steps"])?;
    for (step, locals) in steps {
        let position = order
            .get(step.as_str())
            .ok_or(SelectionContractError::Invalid)?;
        for (local, source) in object(locals)? {
            if !input_name(local) {
                return Err(SelectionContractError::Invalid);
            }
            let fields = object(source)?;
            match source["kind"].as_str() {
                Some("task_input") => {
                    closed_object(fields, &["kind", "reference"])?;
                    let name = source["reference"]
                        .as_str()
                        .and_then(|s| s.strip_prefix("{{vars."))
                        .and_then(|s| s.strip_suffix("}}"))
                        .filter(|s| input_name(s))
                        .ok_or(SelectionContractError::Invalid)?;
                    if !names.contains(name) {
                        return Err(SelectionContractError::Invalid);
                    }
                }
                Some("constant") => closed_object(fields, &["kind", "value"])?,
                Some("consumer_default") => closed_object(fields, &["kind"])?,
                Some("result") => {
                    closed_object(fields, &["kind", "step_id", "path"])?;
                    let producer = nonempty(&source["step_id"])?;
                    if !steps.contains_key(producer)
                        || order.get(producer).is_none_or(|p| p >= position)
                    {
                        return Err(SelectionContractError::Invalid);
                    }
                    for field in array(&source["path"])? {
                        nonempty(field)?;
                    }
                }
                _ => return Err(SelectionContractError::Invalid),
            }
        }
    }
    Ok(())
}
fn validate_invocations(
    raw: &Value,
    selected: &str,
    order: &BTreeMap<&str, usize>,
    layout: &Value,
) -> Result<(), SelectionContractError> {
    if raw.is_null() {
        return Ok(());
    } // historical missing Recipe.invocations serializes as null
    for (variant, steps) in object(raw)? {
        if variant.trim().is_empty() {
            return Err(SelectionContractError::Invalid);
        }
        for (step, export) in object(steps)? {
            step_coordinates(step)?;
            if !crate::preload_identifier::public_export(nonempty(export)?) {
                return Err(SelectionContractError::Invalid);
            }
            if variant == selected
                && (!order.contains_key(step.as_str())
                    || !object(&layout["steps"])?.contains_key(step))
            {
                return Err(SelectionContractError::Invalid);
            }
        }
    }
    Ok(())
}
fn base_component_ref(v: &Value) -> Result<ComponentRevisionRef, SelectionContractError> {
    closed_object(object(v)?, &["uuid", "class_code", "version", "checksum"])?;
    let uuid = parse_uuid_str(&v["uuid"])?;
    let class_code = v["class_code"]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .filter(|c| matches!(c, 0..=10 | 12..=23 | 50))
        .ok_or(SelectionContractError::Invalid)?;
    let version = v["version"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= i64::MAX as u64)
        .ok_or(SelectionContractError::Invalid)?;
    Ok(ComponentRevisionRef {
        uuid,
        class_code,
        version,
        checksum: parse_sha256_hex_str(&v["checksum"])?,
    })
}
fn native_ref_value(reference: ComponentRevisionRef) -> Value {
    serde_json::json!({"uuid":reference.uuid,"class_code":reference.class_code,"version":reference.version,"checksum":reference.checksum})
}

// ---------------------------------------------------------------------------
// Shared internal helpers
// ---------------------------------------------------------------------------

/// Deserialize a `ComponentRevisionRef` via its strict serde Visitor, then
/// apply the additional semantic constraints the Visitor does not enforce:
/// - `uuid` must not be nil
/// - `version` must be > 0
fn validated_component_ref(v: &Value) -> Result<ComponentRevisionRef, SelectionContractError> {
    let r = serde_json::from_value::<ComponentRevisionRef>(v.clone())
        .map_err(|_| SelectionContractError::Invalid)?;
    if r.uuid.is_nil() {
        return Err(SelectionContractError::Invalid);
    }
    if r.version == 0
        || r.version > i64::MAX as u64
        || !matches!(r.class_code, 0..=10 | 12..=23 | 50)
    {
        return Err(SelectionContractError::Invalid);
    }
    Ok(r)
}

/// Require `obj` to contain exactly the declared fields (all present, no extras).
fn closed_object(
    obj: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> Result<(), SelectionContractError> {
    if obj.len() != fields.len() || fields.iter().any(|f| !obj.contains_key(*f)) {
        return Err(SelectionContractError::Invalid);
    }
    Ok(())
}

/// Parse a non-nil UUID from a JSON string value.
fn parse_uuid_str(v: &Value) -> Result<Uuid, SelectionContractError> {
    v.as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .filter(|id| !id.is_nil())
        .ok_or(SelectionContractError::Invalid)
}

/// Parse a 64-char lowercase hex SHA-256 string into a `[u8; 32]`.
fn parse_sha256_hex_str(v: &Value) -> Result<[u8; 32], SelectionContractError> {
    let s = v.as_str().ok_or(SelectionContractError::Invalid)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(SelectionContractError::Invalid);
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|_| SelectionContractError::Invalid)?;
    }
    Ok(out)
}

/// Encode 32 bytes as 64-char lowercase hex.
fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn sha256_of(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).into()
}

/// Existing general/private symbol rule, retained for downstream callers.
pub use crate::preload_identifier::identifier as is_valid_python_identifier;

/// Check static relationships against an already authenticated exact graph.
impl SelectionVersion {
    pub fn require_snapshot(
        &self,
        snapshot: &RetainedComponentSnapshot,
    ) -> Result<(), SelectionContractError> {
        let selection = self.selection();
        let pins = match self {
            Self::V1(_) => None,
            Self::V3(v3) => Some(&v3.functional_approvals),
        };
        require_base_snapshot(
            &selection.value,
            selection.recipe,
            &selection.components,
            snapshot,
            pins,
        )
    }
}
fn require_base_snapshot(
    raw: &Value,
    recipe: ComponentRevisionRef,
    components: &[ComponentRevisionRef],
    snapshot: &RetainedComponentSnapshot,
    functional: Option<&BTreeMap<String, FunctionalOccurrencePin>>,
) -> Result<(), SelectionContractError> {
    let revision = snapshot
        .revisions()
        .get(&recipe.uuid)
        .filter(|r| r.reference() == recipe)
        .ok_or(SelectionContractError::Invalid)?;
    if snapshot.revisions().len() != components.len()
        || components.iter().any(|r| {
            snapshot
                .revisions()
                .get(&r.uuid)
                .is_none_or(|actual| actual.reference() != *r)
        })
    {
        return Err(SelectionContractError::Invalid);
    }
    let document = revision.draft().document();
    let key = nonempty(&raw["variant"]["variant_key"])?;
    let variants = array(&document["variants"])?;
    let mut declared = BTreeSet::new();
    let mut selected = None;
    for variant in variants {
        let name = nonempty(&variant["variant_key"])?;
        if !declared.insert(name.to_owned()) {
            return Err(SelectionContractError::Invalid);
        }
        if name == key {
            validate_variant(variant)?;
            selected = Some(variant);
        }
    }
    let variant = selected.ok_or(SelectionContractError::Invalid)?;
    // RecipeVariant's supported serializer emits these defaults explicitly.
    let normalize = |v: &Value| {
        let patterns: Vec<Value> = v.get("variable_patterns").and_then(Value::as_array)
                .into_iter().flatten().map(|pattern| serde_json::json!({
                    "name":pattern["name"], "pattern":pattern.get("pattern"), "description":pattern.get("description"),
                })).collect();
        serde_json::json!({
            "variant_key":v["variant_key"], "description":v.get("description"), "step_link":v["step_link"],
            "intent_examples":v.get("intent_examples").cloned().unwrap_or_else(|| serde_json::json!([])),
            "variable_patterns":patterns,
        })
    };
    if normalize(variant) != normalize(&raw["variant"])
        || raw["input_layout"] != document["input_layouts"][key]
        || &raw["invocation_layout"] != document.get("invocations").unwrap_or(&Value::Null)
    {
        return Err(SelectionContractError::Invalid);
    }
    if let Some(layouts) = raw["invocation_layout"].as_object()
        && layouts.keys().any(|name| !declared.contains(name))
    {
        return Err(SelectionContractError::Invalid);
    }
    let model = crate::recipe_model_input::RecipeModelInput::for_variant(document, &declared, key)
        .map_err(|_| SelectionContractError::Invalid)?;
    let expected_model = document
        .get("model_input_bindings")
        .map(|_| model.retained_binding());
    if raw.get("model_input_binding") != expected_model.as_ref() {
        return Err(SelectionContractError::Invalid);
    }
    let mut descriptions = BTreeMap::new();
    for entry in array(&document["step_descriptions"])? {
        let desc = entry["desc_idx"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(SelectionContractError::Invalid)?;
        let mut steps = Vec::new();
        let mut previous = 0;
        if let Some(values) = entry.get("steps") {
            for step in array(values)? {
                let number = step["stepnumber"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or(SelectionContractError::Invalid)?;
                if number <= previous {
                    return Err(SelectionContractError::Invalid);
                }
                previous = number;
                steps.push((number, step));
            }
        }
        if descriptions.insert(desc, steps).is_some() {
            return Err(SelectionContractError::Invalid);
        }
    }
    let link = nonempty(&variant["step_link"])?;
    let ranges = crate::retained_step_link::parse_step_link(link)
        .map_err(|_| SelectionContractError::Invalid)?;
    let mut actual_order = Vec::new();
    let mut executable = BTreeSet::new();
    for range in ranges {
        let steps = descriptions
            .get(&range.desc_idx)
            .ok_or(SelectionContractError::Invalid)?;
        if steps.is_empty() {
            continue;
        } // existing IBS empty-description behavior
        let start = if range.start == 0 {
            0
        } else {
            steps
                .iter()
                .position(|(n, _)| *n == range.start)
                .ok_or(SelectionContractError::Invalid)?
        };
        let end = match range.end {
            crate::retained_step_link::StepBound::End => steps.len() - 1,
            crate::retained_step_link::StepBound::Num(n) => steps
                .iter()
                .position(|(number, _)| *number == n)
                .ok_or(SelectionContractError::Invalid)?,
        };
        if start > end {
            return Err(SelectionContractError::Invalid);
        }
        for (number, step) in &steps[start..=end] {
            match step["type"].as_str() {
                Some("text") => continue,
                Some("component") => {}
                _ => return Err(SelectionContractError::Invalid),
            }
            let id = format!("{}:{number}", range.desc_idx);
            let includes = array(&step["include"])?;
            if includes.len() != 1 {
                return Err(SelectionContractError::Invalid);
            }
            let uuid = parse_uuid_str(&includes[0])?;
            if !revision.draft().dependencies().contains(&uuid) {
                return Err(SelectionContractError::Invalid);
            }
            let reference = snapshot
                .revisions()
                .get(&uuid)
                .ok_or(SelectionContractError::Invalid)?
                .reference();
            let expected_class = match step["knowledge"].as_str() {
                Some("rust") => 13,
                Some("orchestrator") => 22,
                _ => return Err(SelectionContractError::Invalid),
            };
            if reference.class_code != expected_class {
                return Err(SelectionContractError::Invalid);
            }
            if expected_class == 22 {
                executable.insert(id.clone());
                let code = snapshot.revisions()[&uuid].draft().document();
                let explicit = raw["invocation_layout"]
                    .get(key)
                    .and_then(|layout| layout.get(&id))
                    .and_then(Value::as_str);
                let export = if let Some(preload) = code.get("preload") {
                    let export = explicit
                        .or_else(|| preload["default_export"].as_str())
                        .ok_or(SelectionContractError::Invalid)?;
                    if preload["format"] != "python-preload/2"
                        || !crate::preload_identifier::public_export(export)
                        || !object(&preload["exports"])?.contains_key(export)
                    {
                        return Err(SelectionContractError::Invalid);
                    }
                    Some(export)
                } else {
                    // Historical step bodies have no preload export. An
                    // invocation-layout entry cannot invent one for them.
                    if explicit.is_some() {
                        return Err(SelectionContractError::Invalid);
                    }
                    None
                };
                if let Some(pin) = functional.and_then(|pins| pins.get(&id))
                    && (pin.implementation != reference || export != Some(pin.export.as_str()))
                {
                    return Err(SelectionContractError::Invalid);
                }
            }
            actual_order.push(Value::String(id));
        }
    }
    if raw["step_order"] != Value::Array(actual_order)
        || object(&raw["input_layout"]["steps"])?
            .keys()
            .collect::<BTreeSet<_>>()
            != executable.iter().collect::<BTreeSet<_>>()
    {
        return Err(SelectionContractError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
#[path = "retained_selection_contract_tests.rs"]
mod selection_contract_tests;
