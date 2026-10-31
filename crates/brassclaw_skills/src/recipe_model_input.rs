//! Reviewed Recipe/variant model-input declarations. Parsing grants nothing;
//! consumers still bind the original pair and exact accepted generation, or
//! independently qualify a prefix-free workflow. Runtime prompt data cannot
//! change this immutable declaration.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeModelInput {
    #[default]
    Default,
    Selected {
        profile: String,
    },
    PrefixFreeWorkflow {
        purpose: PrefixFreePurpose,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixFreePurpose {
    BasicProviderConfigurationValues,
    ModelDesign,
    PrefixGeneration,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid retained Recipe model-input declaration")]
pub struct ModelInputError;

impl RecipeModelInput {
    /// Exact model-input envelope in an immutable Recipe selection. Callers
    /// include it only when the original document declares a binding map;
    /// historical absence remains distinct from an explicit Default. This is
    /// selection data, never Prefix qualification or model dispatch authority.
    pub fn retained_binding(&self) -> Value {
        serde_json::json!({"format":"recipe-model-input-selection/1", "binding":self})
    }

    /// Missing declaration preserves the historical Default contract. Null,
    /// malformed/unknown declarations and undeclared variants never default.
    /// Validate the complete declared map, not only its currently selected entry.
    pub fn for_variant(
        document: &Value,
        declared_variants: &BTreeSet<String>,
        selected_variant: &str,
    ) -> Result<Self, ModelInputError> {
        if !declared_variants.contains(selected_variant) {
            return Err(ModelInputError);
        }
        let document = document.as_object().ok_or(ModelInputError)?;
        let Some(raw) = document.get("model_input_bindings") else {
            return Ok(Self::Default);
        };
        let object = raw.as_object().ok_or(ModelInputError)?;
        if object.len() != 2
            || object.get("format").and_then(Value::as_str) != Some("recipe-model-input-bindings/1")
        {
            return Err(ModelInputError);
        }
        let variants = object
            .get("variants")
            .and_then(Value::as_object)
            .ok_or(ModelInputError)?;
        let mut selected = Self::Default;
        for (name, value) in variants {
            if !declared_variants.contains(name) {
                return Err(ModelInputError);
            }
            let fields = value.as_object().ok_or(ModelInputError)?;
            let expected: &[&str] = match fields.get("selection").and_then(Value::as_str) {
                Some("default") => &["selection"],
                Some("selected") => &["selection", "profile"],
                Some("prefix_free_workflow") => &["selection", "purpose"],
                _ => return Err(ModelInputError),
            };
            if fields.len() != expected.len()
                || expected.iter().any(|field| !fields.contains_key(*field))
            {
                return Err(ModelInputError);
            }
            let policy: Self =
                serde_json::from_value(value.clone()).map_err(|_| ModelInputError)?;
            if let Self::Selected { profile } = &policy
                && (profile.is_empty()
                    || profile.trim() != profile
                    || profile.chars().any(char::is_control))
            {
                return Err(ModelInputError);
            }
            if name == selected_variant {
                selected = policy;
            }
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn selected(value: Value) -> Result<RecipeModelInput, ModelInputError> {
        RecipeModelInput::for_variant(
            &json!({"model_input_bindings":value}),
            &BTreeSet::from(["first".into(), "second".into()]),
            "first",
        )
    }
    #[test]
    fn absent_and_explicit_default_preserve_default_but_null_does_not() {
        let variants = BTreeSet::from(["first".into()]);
        assert_eq!(
            RecipeModelInput::for_variant(&json!({}), &variants, "first").unwrap(),
            RecipeModelInput::Default
        );
        assert_eq!(selected(json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"default"}}})).unwrap(), RecipeModelInput::Default);
        assert!(selected(Value::Null).is_err());
        assert!(RecipeModelInput::for_variant(&json!({}), &variants, "missing").is_err());
    }
    #[test]
    fn exact_variant_policy_is_isolated_and_never_interprets_prompt_data() {
        let document = json!({"model_input_bindings":{"format":"recipe-model-input-bindings/1",
            "variants":{"first":{"selection":"selected","profile":"defensive"},
                "second":{"selection":"prefix_free_workflow","purpose":"prefix_generation"}}},
            "inputs":{"selection":"prefix_free_workflow"}});
        let variants = BTreeSet::from(["first".into(), "second".into(), "ordinary".into()]);
        assert_eq!(
            RecipeModelInput::for_variant(&document, &variants, "first").unwrap(),
            RecipeModelInput::Selected {
                profile: "defensive".into()
            }
        );
        assert_eq!(
            RecipeModelInput::for_variant(&document, &variants, "second").unwrap(),
            RecipeModelInput::PrefixFreeWorkflow {
                purpose: PrefixFreePurpose::PrefixGeneration
            }
        );
        assert_eq!(
            RecipeModelInput::for_variant(&document, &variants, "ordinary").unwrap(),
            RecipeModelInput::Default
        );
    }
    #[test]
    fn malformed_sibling_or_unknown_shape_is_not_silently_ignored() {
        for value in [
            json!({"format":"recipe-model-input-bindings/2","variants":{}}),
            json!({"format":"recipe-model-input-bindings/1","variants":null}),
            json!({"format":"recipe-model-input-bindings/1","variants":{},"extra":true}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"unknown":{"selection":"default"}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"second":{"selection":"unknown"}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"default","profile":"base-prompt"}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"selected","profile":null}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"selected","profile":" "}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"selected","profile":"base-prompt\n"}}}),
            json!({"format":"recipe-model-input-bindings/1","variants":{"first":{"selection":"prefix_free_workflow","purpose":"chat"}}}),
        ] {
            assert!(selected(value).is_err());
        }
    }
}
