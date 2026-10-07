//! Recursive data contracts from `skills.md`, shared by IBS and host adapters.
//! Only missing consumer inputs receive defaults. Successful outputs are checked
//! without mutation; a rejected output is not a retry instruction.
//!
//! Constructors validate exact schema fields and retain private invariants.
//! The JSON constructors reject duplicate keys before a `Value` can lose them.
//! Limits reject entire documents/values, never truncate data.

use std::{cmp::Ordering, collections::BTreeMap, fmt};

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, Copy)]
pub struct ContractLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_bytes: usize,
}

impl ContractLimits {
    fn validate(self) -> Result<Self, ContractError> {
        if !(1..=64).contains(&self.max_depth) || self.max_nodes == 0 || self.max_bytes == 0 {
            return Err(error("$", "invalid technical bounds"));
        }
        Ok(self)
    }
}

/// Retains the exact structural path privately; dynamic extra-field keys can
/// contain user data. Display/Debug expose classification only. The trusted
/// diagnostic owner must redact paths before showing or logging them.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
#[error("value contract: {reason}")]
pub struct ContractError {
    path: String,
    pub reason: &'static str,
}

impl ContractError {
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Debug for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContractError")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

fn error(path: &str, reason: &'static str) -> ContractError {
    ContractError {
        path: path.into(),
        reason,
    }
}

fn field(path: &str, name: &str) -> String {
    // JSON Pointer escaping makes field names unambiguous, including `/` and `~`.
    format!("{path}/{}", name.replace('~', "~0").replace('/', "~1"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    String,
    Integer,
    Number,
    Boolean,
    Null,
    List,
    Object,
}

impl Kind {
    fn numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Number)
    }
}

#[derive(Clone)]
enum Check {
    MinLength(u64),
    Min(Number),
    Max(Number),
    MaxField(String),
}

#[derive(Clone)]
struct Schema {
    kind: Kind,
    nullable: bool,
    required: Option<bool>,
    default: Option<Value>,
    checks: Vec<Check>,
    items: Option<Box<Schema>>,
    fields: BTreeMap<String, Schema>,
    extra: Option<Box<Schema>>,
}

#[derive(Clone, Copy)]
enum Position {
    Input,
    Field,
    Value,
    Computed,
}

/// Validated declared step-local inputs. The mapping is closed: undeclared
/// inputs fail rather than accidentally becoming code-accessible authority.
#[derive(Clone)]
pub struct InputContract {
    schemas: BTreeMap<String, Schema>,
    limits: ContractLimits,
}

impl fmt::Debug for InputContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputContract")
            .field("input_count", &self.schemas.len())
            .finish()
    }
}

impl InputContract {
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.schemas.keys().map(String::as_str)
    }

    pub fn required_names(&self) -> impl Iterator<Item = &str> {
        self.schemas
            .iter()
            .filter(|(_, schema)| schema.required == Some(true))
            .map(|(name, _)| name.as_str())
    }

    /// Check a supplied constant against one input. Cross-input inequalities
    /// still require `bind` on the complete mapping immediately before use.
    pub fn validate_supplied_input(&self, name: &str, value: &Value) -> Result<(), ContractError> {
        check_size(value, self.limits)?;
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| error("$", "input is undeclared"))?;
        schema.bind_value(
            value,
            &field("$", name),
            &mut SizeBudget {
                limits: self.limits,
                nodes: 0,
                bytes: 0,
            },
            0,
        )?;
        Ok(())
    }

    /// An explicit request to use this consumer's default. It cannot select a
    /// required input or substitute for an invalid/null value supplied at runtime.
    pub fn default_for_input(&self, name: &str) -> Result<Value, ContractError> {
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| error("$", "input is undeclared"))?;
        let value = schema
            .default
            .as_ref()
            .ok_or_else(|| error("$", "input has no default"))?;
        schema.bind_value(
            value,
            &field("$", name),
            &mut SizeBudget {
                limits: self.limits,
                nodes: 0,
                bytes: 0,
            },
            0,
        )
    }

    /// Contract of an input after this complete mapping has successfully bound.
    /// Recursive input defaults become guaranteed fields, never output defaults.
    pub fn bound_value_contract(&self, name: &str) -> Result<ValueContract, ContractError> {
        fn bound(mut schema: Schema) -> Schema {
            schema.fields = schema
                .fields
                .into_iter()
                .map(|(name, mut field)| {
                    if field.default.is_some() {
                        field.required = Some(true);
                    }
                    (name, bound(field))
                })
                .collect();
            schema.items = schema.items.map(|schema| Box::new(bound(*schema)));
            schema.extra = schema.extra.map(|schema| Box::new(bound(*schema)));
            schema.default = None;
            schema
        }
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| error("$", "input is undeclared"))?;
        let mut schema = bound(schema.clone());
        schema.required = None;
        Ok(ValueContract {
            schema,
            limits: self.limits,
        })
    }

    pub fn from_json(source: &str, limits: ContractLimits) -> Result<Self, ContractError> {
        let limits = limits.validate()?;
        let value = strict_json(source, limits)?;
        Self::from_value(&value, limits)
    }

    /// Use only with already decoded, duplicate-checked trusted records. An
    /// ordinary `serde_json::Value` cannot prove duplicate-key rejection.
    pub fn from_value(value: &Value, limits: ContractLimits) -> Result<Self, ContractError> {
        let limits = limits.validate()?;
        check_size(value, limits)?;
        let object = object(value, "$")?;
        let mut schemas = BTreeMap::new();
        for (name, schema) in object {
            if !local_name(name) {
                return Err(error(&field("$", name), "invalid local input name"));
            }
            schemas.insert(
                name.clone(),
                parse_schema(schema, Position::Input, true, &field("$", name), limits)?,
            );
        }
        for (name, schema) in &schemas {
            for check in &schema.checks {
                if let Check::MaxField(target) = check {
                    let Some(other) = schemas.get(target) else {
                        return Err(error(&field("$", name), "cross-input target is undeclared"));
                    };
                    if !other.kind.numeric() || other.nullable || schema.nullable {
                        return Err(error(
                            &field("$", name),
                            "cross-input operands must be numeric and non-null",
                        ));
                    }
                }
            }
        }
        Ok(Self { schemas, limits })
    }

    /// Materialize independent task-owned defaults only after bounded input
    /// validation. Explicit null and invalid supplied values never use defaults.
    pub fn bind(&self, supplied: &Value) -> Result<Map<String, Value>, ContractError> {
        check_size(supplied, self.limits)?;
        let supplied = object(supplied, "$")?;
        if supplied.keys().any(|name| !self.schemas.contains_key(name)) {
            return Err(error("$", "undeclared input"));
        }
        let mut bound = Map::new();
        let mut budget = SizeBudget {
            limits: self.limits,
            nodes: 0,
            bytes: 0,
        };
        budget.enter(0, 0)?;
        for (name, schema) in &self.schemas {
            let path = field("$", name);
            let value = supplied
                .get(name)
                .or(schema.default.as_ref())
                .ok_or_else(|| error(&path, "required value is missing"))?;
            budget.enter(1, name.len())?;
            let value = schema.bind_value(value, &path, &mut budget, 1)?;
            bound.insert(name.clone(), value);
        }
        // Defaults can increase the combined payload. Check their aggregate,
        // rather than allowing every separately small default to evade a cap.
        let result = Value::Object(bound);
        check_size(&result, self.limits)?;
        let bound = result.as_object().expect("constructed object");
        for (name, schema) in &self.schemas {
            for check in &schema.checks {
                if let Check::MaxField(target) = check {
                    let left = bound[name].as_number().expect("validated numeric input");
                    let right = bound[target].as_number().expect("validated numeric target");
                    if number_cmp(left, right) == Ordering::Greater {
                        return Err(error(&field("$", name), "cross-input maximum exceeded"));
                    }
                }
            }
        }
        let Value::Object(bound) = result else {
            unreachable!()
        };
        Ok(bound)
    }
}

/// Result or computed Tool-argument contract. It never fills missing fields.
#[derive(Clone)]
pub struct ValueContract {
    schema: Schema,
    limits: ContractLimits,
}

impl fmt::Debug for ValueContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueContract")
            .field("kind", &self.schema.kind)
            .finish()
    }
}

impl ValueContract {
    pub fn result_from_json(source: &str, limits: ContractLimits) -> Result<Self, ContractError> {
        let limits = limits.validate()?;
        let value = strict_json(source, limits)?;
        Self::result_from_value(&value, limits)
    }

    pub fn result_from_value(value: &Value, limits: ContractLimits) -> Result<Self, ContractError> {
        let limits = limits.validate()?;
        check_size(value, limits)?;
        Ok(Self {
            schema: parse_schema(value, Position::Value, false, "$", limits)?,
            limits,
        })
    }

    pub fn computed_from_json(source: &str, inputs: &InputContract) -> Result<Self, ContractError> {
        let value = strict_json(source, inputs.limits)?;
        Self::computed_from_value(&value, inputs)
    }

    /// The enclosing JSON document must already have passed duplicate-key
    /// checks; a Value cannot recover keys discarded by a permissive parser.
    pub fn computed_from_value(
        value: &Value,
        inputs: &InputContract,
    ) -> Result<Self, ContractError> {
        check_size(value, inputs.limits)?;
        let schema = parse_schema(value, Position::Computed, false, "$", inputs.limits)?;
        let record = object(value, "$")?;
        let dependencies = record["depends_on"]
            .as_array()
            .expect("validated dependencies");
        let mut seen = std::collections::BTreeSet::new();
        for dependency in dependencies {
            let name = dependency.as_str().expect("validated dependency name");
            if !inputs.schemas.contains_key(name) || !seen.insert(name) {
                return Err(error(
                    "$/depends_on",
                    "undeclared or duplicate input dependency",
                ));
            }
        }
        Ok(Self {
            schema,
            limits: inputs.limits,
        })
    }

    pub fn validate(&self, value: &Value) -> Result<(), ContractError> {
        check_size(value, self.limits)?;
        self.schema.validate_value(value, "$")
    }

    /// Prove a complete produced value fits a declared consumer input without
    /// coercion. Optional producer fields cannot satisfy required consumers.
    /// Guarded field selection needs a separate validated Recipe branch; this
    /// method does not infer such a guard from code or prose.
    pub fn require_compatible_input(
        &self,
        consumer: &InputContract,
        name: &str,
    ) -> Result<(), ContractError> {
        let target = consumer
            .schemas
            .get(name)
            .ok_or_else(|| error("$", "consumer input is undeclared"))?;
        compatible(&self.schema, target, &field("$", name))
    }

    /// Select a guaranteed object field path. Optional/null parents need an
    /// explicit verified presence branch; this API never assumes one occurred.
    pub fn required_field_path(&self, parts: &[String]) -> Result<Self, ContractError> {
        if parts.len() > self.limits.max_depth {
            return Err(error("$", "technical path bound exceeded"));
        }
        let mut schema = &self.schema;
        let mut path = "$".to_owned();
        for part in parts {
            if schema.kind != Kind::Object || schema.nullable {
                return Err(error(&path, "field selection needs non-null object"));
            }
            path = field(&path, part);
            schema = schema
                .fields
                .get(part)
                .filter(|schema| schema.required == Some(true))
                .ok_or_else(|| error(&path, "field is not guaranteed present"))?;
        }
        Ok(Self {
            schema: schema.clone(),
            limits: self.limits,
        })
    }

    /// Homogeneous element contract for a verified non-null foreach source.
    /// Empty lists retain the same declared element contract.
    pub fn list_items(&self) -> Result<Self, ContractError> {
        if self.schema.nullable {
            return Err(error("$", "list selection needs non-null list"));
        }
        let schema = self
            .schema
            .items
            .as_deref()
            .ok_or_else(|| error("$", "list selection needs list"))?;
        Ok(Self {
            schema: schema.clone(),
            limits: self.limits,
        })
    }
}

fn compatible(producer: &Schema, consumer: &Schema, path: &str) -> Result<(), ContractError> {
    if producer.kind == Kind::Null {
        return if consumer.nullable || consumer.kind == Kind::Null {
            Ok(())
        } else {
            Err(error(path, "producer null is not accepted"))
        };
    }
    if producer.nullable && !consumer.nullable {
        return Err(error(path, "producer may be null"));
    }
    if producer.kind != consumer.kind {
        return Err(error(path, "producer and consumer types differ"));
    }
    for check in &consumer.checks {
        let guaranteed = match check {
            Check::MinLength(min) => *min == 0 || producer.checks.iter().any(|check| matches!(check, Check::MinLength(bound) if bound >= min)),
            Check::Min(min) => producer.checks.iter().any(|check| matches!(check, Check::Min(bound) if number_cmp(bound, min) != Ordering::Less)),
            Check::Max(max) => producer.checks.iter().any(|check| matches!(check, Check::Max(bound) if number_cmp(bound, max) != Ordering::Greater)),
            // A single edge cannot prove an inequality involving another input.
            Check::MaxField(_) => return Err(error(path, "cross-input compatibility needs explicit validation")),
        };
        if !guaranteed {
            return Err(error(
                path,
                "producer does not guarantee consumer constraint",
            ));
        }
    }
    if let Some(items) = &producer.items {
        compatible(
            items,
            consumer.items.as_deref().expect("compatible list type"),
            &field(path, "items"),
        )?;
    }
    if producer.kind == Kind::Object {
        for (name, schema) in &consumer.fields {
            let offered = producer.fields.get(name);
            if schema.required == Some(true)
                && offered.is_none_or(|value| value.required != Some(true))
            {
                return Err(error(
                    &field(path, name),
                    "producer does not guarantee required field",
                ));
            }
            if let Some(offered) = offered.or(producer.extra.as_deref()) {
                compatible(offered, schema, &field(path, name))?;
            }
        }
        for (name, schema) in &producer.fields {
            if !consumer.fields.contains_key(name) {
                let target = consumer
                    .extra
                    .as_deref()
                    .ok_or_else(|| error(path, "producer may supply undeclared field"))?;
                compatible(schema, target, &field(path, name))?;
            }
        }
        if let Some(extra) = &producer.extra {
            let target = consumer
                .extra
                .as_deref()
                .ok_or_else(|| error(path, "producer permits extra fields"))?;
            compatible(extra, target, &field(path, "extra_fields"))?;
        }
    }
    Ok(())
}

fn local_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value
        .as_object()
        .ok_or_else(|| error(path, "expected object"))
}

fn parse_schema(
    value: &Value,
    position: Position,
    defaults: bool,
    path: &str,
    limits: ContractLimits,
) -> Result<Schema, ContractError> {
    let record = object(value, path)?;
    let kind = match record.get("type").and_then(Value::as_str) {
        Some("string") => Kind::String,
        Some("integer") => Kind::Integer,
        Some("number") => Kind::Number,
        Some("boolean") => Kind::Boolean,
        Some("null") => Kind::Null,
        Some("list") => Kind::List,
        Some("object") => Kind::Object,
        _ => return Err(error(path, "invalid schema type")),
    };
    let present = matches!(position, Position::Input | Position::Field);
    let mut allowed = vec!["type", "checks"];
    if kind != Kind::Null {
        allowed.push("nullable");
    }
    if present {
        allowed.push("required");
        if defaults {
            allowed.push("default");
        }
    }
    if kind == Kind::List {
        allowed.push("items");
    }
    if kind == Kind::Object {
        allowed.extend(["fields", "allow_extra_fields", "extra_fields"]);
    }
    if matches!(position, Position::Computed) {
        allowed.extend(["depends_on", "meaning"]);
    }
    if record.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(error(path, "unknown or misplaced schema field"));
    }
    let nullable = match record.get("nullable") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(error(path, "nullable must be boolean")),
    };
    let required = if present {
        Some(
            record
                .get("required")
                .and_then(Value::as_bool)
                .ok_or_else(|| error(path, "required must be explicit boolean"))?,
        )
    } else {
        None
    };
    let default = record.get("default").cloned();
    if required == Some(true) && default.is_some() {
        return Err(error(path, "required value forbids default"));
    }
    if matches!(position, Position::Input) && required == Some(false) && default.is_none() {
        return Err(error(path, "optional top-level input needs default"));
    }
    let checks = parse_checks(record.get("checks"), kind, position, path)?;
    let items = if kind == Kind::List {
        Some(Box::new(parse_schema(
            record
                .get("items")
                .ok_or_else(|| error(path, "list requires items"))?,
            Position::Value,
            defaults,
            &field(path, "items"),
            limits,
        )?))
    } else {
        None
    };
    let mut fields = BTreeMap::new();
    let extra = if kind == Kind::Object {
        let members = object(
            record
                .get("fields")
                .ok_or_else(|| error(path, "object requires fields"))?,
            path,
        )?;
        for (name, schema) in members {
            if name.is_empty() {
                return Err(error(path, "empty object field name"));
            }
            fields.insert(
                name.clone(),
                parse_schema(
                    schema,
                    Position::Field,
                    defaults,
                    &field(&field(path, "fields"), name),
                    limits,
                )?,
            );
        }
        match record.get("allow_extra_fields").and_then(Value::as_bool) {
            Some(true) => Some(Box::new(parse_schema(
                record
                    .get("extra_fields")
                    .ok_or_else(|| error(path, "extra fields require schema"))?,
                Position::Value,
                defaults,
                &field(path, "extra_fields"),
                limits,
            )?)),
            Some(false) if !record.contains_key("extra_fields") => None,
            _ => return Err(error(path, "invalid extra-field contract")),
        }
    } else {
        None
    };
    if matches!(position, Position::Computed) {
        let dependencies = record
            .get("depends_on")
            .and_then(Value::as_array)
            .ok_or_else(|| error(path, "computed argument requires dependencies"))?;
        if dependencies
            .iter()
            .any(|v| !v.as_str().is_some_and(local_name))
        {
            return Err(error(path, "invalid input dependency"));
        }
        if !record
            .get("meaning")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
        {
            return Err(error(path, "computed argument requires meaning"));
        }
    }
    let schema = Schema {
        kind,
        nullable,
        required,
        default,
        checks,
        items,
        fields,
        extra,
    };
    if let Some(default) = &schema.default {
        // Apply child input defaults to this value only. Missing parents remain
        // absent; output/computed schemas cannot contain defaults at any depth.
        schema.bind_value(
            default,
            &field(path, "default"),
            &mut SizeBudget {
                limits,
                nodes: 0,
                bytes: 0,
            },
            0,
        )?;
    }
    Ok(schema)
}

fn parse_checks(
    value: Option<&Value>,
    kind: Kind,
    position: Position,
    path: &str,
) -> Result<Vec<Check>, ContractError> {
    let values = match value {
        Some(value) => value
            .as_array()
            .ok_or_else(|| error(path, "checks must be list"))?,
        None if matches!(position, Position::Input | Position::Computed) => {
            return Err(error(path, "explicit checks required"));
        }
        None => return Ok(Vec::new()),
    };
    let mut checks = Vec::new();
    for value in values {
        let check = object(value, path)?;
        let name = check
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| error(path, "invalid check"))?;
        let allowed = if name == "max_field" {
            ["kind", "input"]
        } else {
            ["kind", "value"]
        };
        if check.len() != 2 || check.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(error(path, "invalid check fields"));
        }
        let numeric = || -> Result<Number, ContractError> {
            let value = check
                .get("value")
                .and_then(Value::as_number)
                .ok_or_else(|| error(path, "check needs finite number"))?;
            if !kind.numeric() || !finite(value) || (kind == Kind::Integer && !integer(value)) {
                return Err(error(path, "numeric check type mismatch"));
            }
            Ok(value.clone())
        };
        checks.push(match name {
            "min_length" if kind == Kind::String => Check::MinLength(
                check
                    .get("value")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| error(path, "length must be nonnegative integer"))?,
            ),
            "min" => Check::Min(numeric()?),
            "max" => Check::Max(numeric()?),
            "max_field" if kind.numeric() && matches!(position, Position::Input) => {
                let input = check
                    .get("input")
                    .and_then(Value::as_str)
                    .filter(|s| local_name(s))
                    .ok_or_else(|| error(path, "invalid cross-input name"))?;
                Check::MaxField(input.into())
            }
            _ => return Err(error(path, "unsupported check for schema type")),
        });
    }
    let mut lower: Option<&Number> = None;
    let mut upper: Option<&Number> = None;
    for check in &checks {
        match check {
            Check::Min(value)
                if lower
                    .is_none_or(|previous| number_cmp(value, previous) == Ordering::Greater) =>
            {
                lower = Some(value)
            }
            Check::Max(value)
                if upper.is_none_or(|previous| number_cmp(value, previous) == Ordering::Less) =>
            {
                upper = Some(value)
            }
            _ => (),
        }
    }
    if let (Some(lower), Some(upper)) = (lower, upper)
        && number_cmp(lower, upper) == Ordering::Greater
    {
        return Err(error(path, "contradictory numeric bounds"));
    }
    Ok(checks)
}

fn integer(value: &Number) -> bool {
    value.is_i64() || value.is_u64()
}
fn finite(value: &Number) -> bool {
    value.as_f64().is_some_and(f64::is_finite)
}

impl Schema {
    fn validate_scalar(&self, value: &Value, path: &str) -> Result<bool, ContractError> {
        if value.is_null() {
            return if self.kind == Kind::Null || self.nullable {
                Ok(true)
            } else {
                Err(error(path, "null is not allowed"))
            };
        }
        let valid = match self.kind {
            Kind::String => value.is_string(),
            Kind::Boolean => value.is_boolean(),
            Kind::Integer => value.as_number().is_some_and(integer),
            Kind::Number => value.as_number().is_some_and(finite),
            Kind::List => value.is_array(),
            Kind::Object => value.is_object(),
            Kind::Null => false,
        };
        if !valid {
            return Err(error(path, "value type mismatch"));
        }
        for check in &self.checks {
            let valid = match check {
                Check::MinLength(min) => {
                    value.as_str().expect("validated string").chars().count() as u64 >= *min
                }
                Check::Min(min) => {
                    number_cmp(value.as_number().expect("validated number"), min) != Ordering::Less
                }
                Check::Max(max) => {
                    number_cmp(value.as_number().expect("validated number"), max)
                        != Ordering::Greater
                }
                Check::MaxField(_) => true, // Complete input mapping checked after all defaults.
            };
            if !valid {
                return Err(error(path, "value constraint failed"));
            }
        }
        Ok(false)
    }

    fn validate_value(&self, value: &Value, path: &str) -> Result<(), ContractError> {
        if self.validate_scalar(value, path)? {
            return Ok(());
        }
        if let Some(items) = &self.items {
            for (index, item) in value.as_array().expect("validated list").iter().enumerate() {
                items.validate_value(item, &field(path, &index.to_string()))?;
            }
        }
        if self.kind == Kind::Object {
            let object = value.as_object().expect("validated object");
            for (name, schema) in &self.fields {
                match object.get(name) {
                    Some(value) => schema.validate_value(value, &field(path, name))?,
                    None if schema.required == Some(true) => {
                        return Err(error(&field(path, name), "required field is missing"));
                    }
                    None => (),
                }
            }
            for (name, value) in object {
                if !self.fields.contains_key(name) {
                    let extra = self
                        .extra
                        .as_ref()
                        .ok_or_else(|| error(path, "undeclared object field"))?;
                    extra.validate_value(value, &field(path, name))?;
                }
            }
        }
        Ok(())
    }

    fn bind_value(
        &self,
        value: &Value,
        path: &str,
        budget: &mut SizeBudget,
        depth: usize,
    ) -> Result<Value, ContractError> {
        budget.enter(depth, value.as_str().map_or(0, str::len))?;
        if self.validate_scalar(value, path)? {
            return Ok(Value::Null);
        }
        if let Some(items) = &self.items {
            let mut bound = Vec::new();
            for (index, value) in value.as_array().expect("validated list").iter().enumerate() {
                bound.push(items.bind_value(
                    value,
                    &field(path, &index.to_string()),
                    budget,
                    depth + 1,
                )?);
            }
            return Ok(Value::Array(bound));
        }
        if self.kind != Kind::Object {
            return Ok(value.clone());
        }
        let object = value.as_object().expect("validated object");
        let mut bound = Map::new();
        for (name, schema) in &self.fields {
            let supplied = object.get(name).or(schema.default.as_ref());
            if let Some(value) = supplied {
                budget.enter(depth + 1, name.len())?;
                bound.insert(
                    name.clone(),
                    schema.bind_value(value, &field(path, name), budget, depth + 1)?,
                );
            } else if schema.required == Some(true) {
                return Err(error(&field(path, name), "required field is missing"));
            }
        }
        for (name, value) in object {
            if !self.fields.contains_key(name) {
                let extra = self
                    .extra
                    .as_ref()
                    .ok_or_else(|| error(path, "undeclared object field"))?;
                budget.enter(depth + 1, name.len())?;
                bound.insert(
                    name.clone(),
                    extra.bind_value(value, &field(path, name), budget, depth + 1)?,
                );
            }
        }
        Ok(Value::Object(bound))
    }
}

// Compare finite JSON numbers in their decimal representation. Converting u64
// bounds to f64 would wrongly equate adjacent integers above 2^53.
fn number_cmp(left: &Number, right: &Number) -> Ordering {
    fn parts(number: &Number) -> (bool, String, i32) {
        let text = number.to_string();
        let negative = text.starts_with('-');
        let text = text.trim_start_matches('-');
        let (coefficient, exponent) = match text.split_once(['e', 'E']) {
            Some((coefficient, exponent)) => (
                coefficient,
                exponent.parse::<i32>().expect("finite JSON exponent"),
            ),
            None => (text, 0),
        };
        let decimals = coefficient
            .split_once('.')
            .map_or(0, |(_, tail)| tail.len() as i32);
        let mut digits = coefficient
            .replace('.', "")
            .trim_start_matches('0')
            .to_owned();
        let mut scale = decimals - exponent;
        while digits.ends_with('0') {
            digits.pop();
            scale -= 1;
        }
        if digits.is_empty() {
            return (false, "0".into(), 0);
        }
        (negative, digits, scale)
    }
    let (ln, ld, ls) = parts(left);
    let (rn, rd, rs) = parts(right);
    if ln != rn {
        return if ln {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let magnitude = if ld == "0" || rd == "0" {
        match (ld == "0", rd == "0") {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            _ => Ordering::Greater,
        }
    } else {
        (ld.len() as i32 - ls)
            .cmp(&(rd.len() as i32 - rs))
            .then_with(|| {
                let count = ld.len().max(rd.len());
                ld.bytes()
                    .chain(std::iter::repeat(b'0'))
                    .take(count)
                    .cmp(rd.bytes().chain(std::iter::repeat(b'0')).take(count))
            })
    };
    if ln { magnitude.reverse() } else { magnitude }
}

struct SizeBudget {
    limits: ContractLimits,
    nodes: usize,
    bytes: usize,
}

impl SizeBudget {
    fn enter(&mut self, depth: usize, bytes: usize) -> Result<(), ContractError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| error("$", "node bound exceeded"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| error("$", "byte bound exceeded"))?;
        if depth > self.limits.max_depth
            || self.nodes > self.limits.max_nodes
            || self.bytes > self.limits.max_bytes
        {
            return Err(error("$", "technical value bound exceeded"));
        }
        Ok(())
    }
}

fn check_size(value: &Value, limits: ContractLimits) -> Result<(), ContractError> {
    fn walk(value: &Value, budget: &mut SizeBudget, depth: usize) -> Result<(), ContractError> {
        budget.enter(depth, value.as_str().map_or(0, str::len))?;
        match value {
            Value::Array(values) => {
                for value in values {
                    walk(value, budget, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for (name, value) in values {
                    budget.enter(depth + 1, name.len())?;
                    walk(value, budget, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    walk(
        value,
        &mut SizeBudget {
            limits,
            nodes: 0,
            bytes: 0,
        },
        0,
    )
}

/// Check the complete borrowed data graph before copying or serialization.
/// This is a technical bound, not a schema or an artificial token ceiling.
pub fn validate_data_bounds(value: &Value, limits: ContractLimits) -> Result<(), ContractError> {
    check_size(value, limits.validate()?)
}

/// Parse bounded data without losing duplicate keys at any nesting level.
pub fn strict_json(source: &str, limits: ContractLimits) -> Result<Value, ContractError> {
    let limits = limits.validate()?;
    if source.len() > limits.max_bytes {
        return Err(error("$", "JSON document byte bound exceeded"));
    }
    let mut budget = SizeBudget {
        limits,
        nodes: 0,
        bytes: 0,
    };
    let mut decoder = serde_json::Deserializer::from_str(source);
    let value = JsonSeed {
        budget: &mut budget,
        depth: 0,
    }
    .deserialize(&mut decoder)
    .map_err(|_| error("$", "invalid, duplicate-key or oversized JSON"))?;
    decoder
        .end()
        .map_err(|_| error("$", "trailing JSON data"))?;
    Ok(value)
}

struct JsonSeed<'a> {
    budget: &'a mut SizeBudget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for JsonSeed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.budget
            .enter(self.depth, 0)
            .map_err(serde::de::Error::custom)?;
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for JsonSeed<'_> {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON data with unique keys")
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        // Account decoded UTF-8 too: escaped source spelling is not its payload.
        self.budget.bytes = self
            .budget
            .bytes
            .checked_add(value.len())
            .ok_or_else(|| E::custom("byte bound"))?;
        if self.budget.bytes > self.budget.limits.max_bytes {
            return Err(E::custom("byte bound"));
        }
        Ok(Value::String(value.into()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(JsonSeed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget
                .enter(self.depth + 1, key.len())
                .map_err(serde::de::Error::custom)?;
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate key"));
            }
            let value = map.next_value_seed(JsonSeed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
