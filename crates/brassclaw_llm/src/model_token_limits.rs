//! Per-model input ceilings owned by the existing provider definition/revision.
//! Physical context, requested output and observed token counts are separate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTokenLimits {
    pub max_prefix_tokens: u64,
    /// All remaining rendered input, including history/tools/chat framing.
    pub max_prompt_tokens: u64,
}
impl ModelTokenLimits {
    pub fn total_input_tokens(self) -> Option<u64> {
        self.max_prefix_tokens.checked_add(self.max_prompt_tokens)
    }
    pub fn validate(self) -> Result<(), &'static str> {
        // JSON/browser exact-integer representation, not a configurable quota.
        if self.max_prompt_tokens == 0
            || self
                .total_input_tokens()
                .is_none_or(|n| n > 9_007_199_254_740_991)
        {
            return Err("invalid model input ceilings");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ModelTokenLimitOverrides(pub BTreeMap<String, ModelTokenLimits>);
impl<'de> Deserialize<'de> for ModelTokenLimitOverrides {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ModelTokenLimitOverrides;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("distinct model names mapped to complete input ceilings")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut values = BTreeMap::new();
                while let Some((name, limits)) = map.next_entry::<String, ModelTokenLimits>()? {
                    if name.is_empty() || name.trim() != name || name.chars().any(char::is_control)
                    {
                        return Err(serde::de::Error::custom("invalid model name"));
                    }
                    limits.validate().map_err(serde::de::Error::custom)?;
                    if values.insert(name, limits).is_some() {
                        return Err(serde::de::Error::custom("duplicate model token limits"));
                    }
                }
                Ok(ModelTokenLimitOverrides(values))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
impl ModelTokenLimitOverrides {
    pub fn validate(&self, default_model: &str, context: Option<u32>) -> Result<(), &'static str> {
        for (name, limits) in &self.0 {
            if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
                return Err("invalid model name");
            }
            limits.validate()?;
            if name == default_model
                && context.is_some_and(|max| {
                    limits
                        .total_input_tokens()
                        .is_none_or(|n| n > u64::from(max))
                })
            {
                return Err("input ceilings exceed configured model context");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_input_limits_are_exact_complete_and_distinct_from_physical_context() {
        let limits: ModelTokenLimitOverrides = serde_json::from_str(
            r#"{"ornith":{"max_prefix_tokens":100000,"max_prompt_tokens":14688}}"#,
        )
        .unwrap();
        assert_eq!(limits.0["ornith"].total_input_tokens(), Some(114688));
        limits.validate("ornith", Some(131072)).unwrap();
        assert!(limits.validate("ornith", Some(114687)).is_err());
        limits.validate("other-model", Some(16000)).unwrap();
        for invalid in [
            "null",
            r#"{"ornith":{"max_prefix_tokens":100000}}"#,
            r#"{"ornith":{"max_prefix_tokens":100000,"max_prompt_tokens":0}}"#,
            r#"{"ornith":{"max_prefix_tokens":true,"max_prompt_tokens":14688}}"#,
            r#"{"ornith":{"max_prefix_tokens":100000,"max_prompt_tokens":14688,"total_input_tokens":114688}}"#,
            r#"{"ornith":{"max_prefix_tokens":0,"max_prompt_tokens":1},"ornith":{"max_prefix_tokens":1,"max_prompt_tokens":1}}"#,
            r#"{"ornith":{"max_prefix_tokens":9007199254740991,"max_prompt_tokens":1}}"#,
        ] {
            assert!(
                serde_json::from_str::<ModelTokenLimitOverrides>(invalid).is_err(),
                "{invalid}"
            );
        }
        assert!(
            serde_json::from_str::<ModelTokenLimitOverrides>(
                r#"{"ornith":{"max_prefix_tokens":0,"max_prompt_tokens":1}}"#
            )
            .is_ok()
        );
    }
}
