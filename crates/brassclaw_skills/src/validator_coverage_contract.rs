//! Passive validator metadata. Parsing authenticates no checks, execution,
//! qualification or component approval. Engine verifies the actual retained IBS.
use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

/// A route's reviewed subject shape, independent of its primary component class.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidatorSubjectKind {
    Component,
    Function,
}

/// Mechanical source checks, not complete function Q1 or behavior coverage.
pub const FUNCTION_SOURCE_RULE_IDS: [&str; 5] = [
    "python_definitions",
    "python_contracts",
    "python_graph",
    "skill_interface",
    "skill_call_targets",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorRule {
    rule_id: String,
    step_id: String,
}
impl ValidatorRule {
    pub fn into_parts(self) -> (String, String) {
        (self.rule_id, self.step_id)
    }
}

#[derive(Deserialize)]
enum FunctionKind {
    #[serde(rename = "function")]
    Function,
}

#[derive(Deserialize)]
#[serde(tag = "format", deny_unknown_fields)]
enum Wire {
    #[serde(rename = "validator-coverage/1")]
    Component {
        rules: Vec<ValidatorRule>,
        aggregate_step: String,
        receipt_step: String,
    },
    #[serde(rename = "validator-coverage/2")]
    Function {
        subject_kind: FunctionKind,
        rules: Vec<ValidatorRule>,
        aggregate_step: String,
        receipt_step: String,
    },
}

/// Closed metadata from an already parsed immutable revision document.
/// Original JSON duplicate-key checks remain with the revision's strict reader.
pub struct ValidatorCoverageDeclaration {
    subject_kind: ValidatorSubjectKind,
    rules: Vec<ValidatorRule>,
    aggregate_step: String,
    receipt_step: String,
}
impl ValidatorCoverageDeclaration {
    pub fn from_value(value: &Value, primary_class: i32) -> Result<Self, &'static str> {
        let class = i16::try_from(primary_class).map_err(|_| "unsupported validator class")?;
        if !super::VALIDATOR_CLASS_CODES.contains(&class) {
            return Err("unsupported validator class");
        }
        let (subject_kind, rules, aggregate_step, receipt_step) =
            match serde_json::from_value::<Wire>(value.clone())
                .map_err(|_| "invalid validator coverage declaration")?
            {
                Wire::Component {
                    rules,
                    aggregate_step,
                    receipt_step,
                } => (
                    ValidatorSubjectKind::Component,
                    rules,
                    aggregate_step,
                    receipt_step,
                ),
                Wire::Function {
                    subject_kind: FunctionKind::Function,
                    rules,
                    aggregate_step,
                    receipt_step,
                } => {
                    if !matches!(class, 1..=3 | 22) {
                        return Err("function validator requires a Skill or PythonCode primary");
                    }
                    (
                        ValidatorSubjectKind::Function,
                        rules,
                        aggregate_step,
                        receipt_step,
                    )
                }
            };
        if rules.is_empty() {
            return Err("nonempty validator coverage required");
        }
        let mut names = BTreeSet::new();
        let mut steps = BTreeSet::new();
        for rule in &rules {
            let mut bytes = rule.rule_id.bytes();
            if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
                || !bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
                || !names.insert(rule.rule_id.as_str())
                || !steps.insert(rule.step_id.as_str())
            {
                return Err("distinct validator rule and step identities required");
            }
        }
        if subject_kind == ValidatorSubjectKind::Function
            && FUNCTION_SOURCE_RULE_IDS
                .iter()
                .any(|rule| !names.contains(rule))
        {
            return Err("complete function source rules required");
        }
        drop(names);
        drop(steps);
        Ok(Self {
            subject_kind,
            rules,
            aggregate_step,
            receipt_step,
        })
    }

    pub fn subject_kind(&self) -> ValidatorSubjectKind {
        self.subject_kind
    }

    pub fn into_parts(self) -> (ValidatorSubjectKind, Vec<ValidatorRule>, String, String) {
        (
            self.subject_kind,
            self.rules,
            self.aggregate_step,
            self.receipt_step,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn function() -> Value {
        json!({"format":"validator-coverage/2","subject_kind":"function",
            "rules":FUNCTION_SOURCE_RULE_IDS.iter().enumerate().map(|(index, rule)|
                json!({"rule_id":rule,"step_id":format!("0:{}",index+4)})).collect::<Vec<_>>(),
            "aggregate_step":"0:10","receipt_step":"0:11"})
    }

    #[test]
    fn legacy_component_coverage_keeps_its_original_scope() {
        let value = json!({"format":"validator-coverage/1",
            "rules":[{"rule_id":"structural_presence","step_id":"0:1"}],
            "aggregate_step":"0:3","receipt_step":"0:4"});
        for class in super::super::VALIDATOR_CLASS_CODES {
            let declared =
                ValidatorCoverageDeclaration::from_value(&value, i32::from(*class)).unwrap();
            assert!(declared.subject_kind() == ValidatorSubjectKind::Component);
            let (_, rules, aggregate, receipt) = declared.into_parts();
            assert_eq!(
                rules.into_iter().next().unwrap().into_parts(),
                ("structural_presence".into(), "0:1".into())
            );
            assert_eq!(aggregate, "0:3");
            assert_eq!(receipt, "0:4");
        }
        let mut widened = value;
        widened["subject_kind"] = json!("function");
        assert!(ValidatorCoverageDeclaration::from_value(&widened, 22).is_err());
    }

    #[test]
    fn function_coverage_requires_real_supported_primary_and_all_source_rules() {
        for class in [1, 2, 3, 22] {
            let declared = ValidatorCoverageDeclaration::from_value(&function(), class).unwrap();
            assert!(declared.subject_kind() == ValidatorSubjectKind::Function);
        }
        for class in [0, 10, 13, 21, 23, 50, -1, i32::MAX] {
            assert!(ValidatorCoverageDeclaration::from_value(&function(), class).is_err());
        }
        for missing in 0..FUNCTION_SOURCE_RULE_IDS.len() {
            let mut incomplete = function();
            incomplete["rules"].as_array_mut().unwrap().remove(missing);
            assert!(ValidatorCoverageDeclaration::from_value(&incomplete, 22).is_err());
        }
    }

    #[test]
    fn contradictory_partial_or_unknown_function_profiles_are_rejected() {
        for value in [Value::Null, json!([]), json!("function")] {
            assert!(ValidatorCoverageDeclaration::from_value(&value, 22).is_err());
        }
        for value in [Value::Null, json!("component"), json!(true)] {
            let mut invalid = function();
            invalid["subject_kind"] = value;
            assert!(ValidatorCoverageDeclaration::from_value(&invalid, 22).is_err());
        }
        for field in ["subject_kind", "aggregate_step", "receipt_step", "rules"] {
            let mut missing = function();
            missing.as_object_mut().unwrap().remove(field);
            assert!(ValidatorCoverageDeclaration::from_value(&missing, 22).is_err());
        }
        let mut unknown = function();
        unknown["approved"] = json!(true);
        assert!(ValidatorCoverageDeclaration::from_value(&unknown, 22).is_err());
        unknown = function();
        unknown["rules"][0]["approved"] = json!(true);
        assert!(ValidatorCoverageDeclaration::from_value(&unknown, 22).is_err());
        unknown = function();
        unknown["format"] = json!("validator-coverage/3");
        assert!(ValidatorCoverageDeclaration::from_value(&unknown, 22).is_err());
    }

    #[test]
    fn duplicate_rule_or_step_cannot_fill_a_missing_check() {
        for field in ["rule_id", "step_id"] {
            let mut invalid = function();
            invalid["rules"][1][field] = invalid["rules"][0][field].clone();
            assert!(ValidatorCoverageDeclaration::from_value(&invalid, 22).is_err());
        }
        for name in ["", "Python_graph", "python-graph", "1graph"] {
            let mut invalid = function();
            invalid["rules"][0]["rule_id"] = json!(name);
            assert!(ValidatorCoverageDeclaration::from_value(&invalid, 22).is_err());
        }
    }
}
