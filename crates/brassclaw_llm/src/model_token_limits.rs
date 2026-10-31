//! Per-model input ceilings owned by the existing provider definition/revision.
//! Physical context, requested output and observed token counts are separate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Counts from one complete request encoding, not independently tokenized
/// prefix/prompt strings. Their source/request identity must be checked by the
/// consuming guard; these values alone establish no provenance or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRequestTokenCounts {
    pub prefix_tokens: u64,
    pub prompt_tokens: u64,
    pub input_tokens: u64,
}

/// Sanitized technical failures; no request text, credentials or source bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModelTokenCheckError {
    #[error("model token policy belongs to another target")]
    TargetMismatch,
    #[error("invalid model token policy")]
    InvalidPolicy,
    #[error("model input ceilings are unavailable")]
    MissingInputLimits,
    #[error("model context window is unavailable or invalid")]
    UnknownContext,
    #[error("positive output reservation is unavailable")]
    UnknownOutputReservation,
    #[error("token arithmetic overflow")]
    ArithmeticOverflow,
    #[error("prefix and prompt counts differ from the full input count")]
    InconsistentMeasurement,
    #[error("prefix token ceiling exceeded")]
    PrefixExceeded,
    #[error("prompt token ceiling exceeded")]
    PromptExceeded,
    #[error("input and reserved output exceed the context window")]
    ContextExceeded,
}

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
    /// Exact technical arithmetic over supplied whole-request measurements.
    /// This neither measures/verifies a request nor reads live policy. It has
    /// no discretionary-budget switch and never truncates or reserves unused
    /// portions of the configured ceilings.
    pub fn check_measured_request(
        self,
        counts: ModelRequestTokenCounts,
        context_window_tokens: u64,
        reserved_output_tokens: u64,
    ) -> Result<(), ModelTokenCheckError> {
        self.validate()
            .map_err(|_| ModelTokenCheckError::InvalidPolicy)?;
        if context_window_tokens == 0 {
            return Err(ModelTokenCheckError::UnknownContext);
        }
        if reserved_output_tokens == 0 {
            return Err(ModelTokenCheckError::UnknownOutputReservation);
        }
        let input = counts
            .prefix_tokens
            .checked_add(counts.prompt_tokens)
            .ok_or(ModelTokenCheckError::ArithmeticOverflow)?;
        if input != counts.input_tokens {
            return Err(ModelTokenCheckError::InconsistentMeasurement);
        }
        let required = input
            .checked_add(reserved_output_tokens)
            .ok_or(ModelTokenCheckError::ArithmeticOverflow)?;
        if counts.prefix_tokens > self.max_prefix_tokens {
            return Err(ModelTokenCheckError::PrefixExceeded);
        }
        if counts.prompt_tokens > self.max_prompt_tokens {
            return Err(ModelTokenCheckError::PromptExceeded);
        }
        if required > context_window_tokens {
            return Err(ModelTokenCheckError::ContextExceeded);
        }
        Ok(())
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

#[cfg(test)]
mod measured_tests {
    use super::*;
    fn counts(prefix_tokens: u64, prompt_tokens: u64) -> ModelRequestTokenCounts {
        ModelRequestTokenCounts {
            prefix_tokens,
            prompt_tokens,
            input_tokens: prefix_tokens + prompt_tokens,
        }
    }
    fn limits() -> ModelTokenLimits {
        ModelTokenLimits {
            max_prefix_tokens: 100000,
            max_prompt_tokens: 14688,
        }
    }
    #[test]
    fn exact_caps_and_context_pass_and_each_single_token_overflow_is_distinct() {
        assert_eq!(
            limits().check_measured_request(counts(100000, 14688), 131072, 16384),
            Ok(())
        );
        assert_eq!(
            limits().check_measured_request(counts(100001, 14687), 131072, 16384),
            Err(ModelTokenCheckError::PrefixExceeded)
        );
        assert_eq!(
            limits().check_measured_request(counts(99999, 14689), 131072, 16384),
            Err(ModelTokenCheckError::PromptExceeded)
        );
        assert_eq!(
            limits().check_measured_request(counts(100000, 14688), 131072, 16385),
            Err(ModelTokenCheckError::ContextExceeded)
        );
        assert_eq!(
            limits().check_measured_request(counts(100000, 14688), 131071, 16384),
            Err(ModelTokenCheckError::ContextExceeded)
        );
    }
    #[test]
    fn actual_input_and_output_are_charged_without_reserving_unused_ceilings() {
        assert_eq!(
            limits().check_measured_request(counts(10, 20), 31, 1),
            Ok(())
        );
        let disabled = ModelTokenLimits {
            max_prefix_tokens: 0,
            max_prompt_tokens: 20,
        };
        assert_eq!(
            disabled.check_measured_request(counts(0, 20), 21, 1),
            Ok(())
        );
        assert_eq!(
            disabled.check_measured_request(counts(1, 19), 21, 1),
            Err(ModelTokenCheckError::PrefixExceeded)
        );
    }
    #[test]
    fn no_rounding_overflow_bad_totals_unknown_reservations_or_invalid_policy_pass() {
        let wrong = ModelRequestTokenCounts {
            prefix_tokens: 10,
            prompt_tokens: 20,
            input_tokens: 29,
        };
        assert_eq!(
            limits().check_measured_request(wrong, 100, 1),
            Err(ModelTokenCheckError::InconsistentMeasurement)
        );
        let overflow = ModelRequestTokenCounts {
            prefix_tokens: u64::MAX,
            prompt_tokens: 1,
            input_tokens: 0,
        };
        assert_eq!(
            limits().check_measured_request(overflow, u64::MAX, 1),
            Err(ModelTokenCheckError::ArithmeticOverflow)
        );
        assert_eq!(
            limits().check_measured_request(counts(1, 0), u64::MAX, u64::MAX),
            Err(ModelTokenCheckError::ArithmeticOverflow)
        );
        assert_eq!(
            limits().check_measured_request(counts(1, 0), 0, 1),
            Err(ModelTokenCheckError::UnknownContext)
        );
        assert_eq!(
            limits().check_measured_request(counts(1, 0), 100, 0),
            Err(ModelTokenCheckError::UnknownOutputReservation)
        );
        assert_eq!(
            ModelTokenLimits {
                max_prefix_tokens: 1,
                max_prompt_tokens: 0
            }
            .check_measured_request(counts(1, 0), 100, 1),
            Err(ModelTokenCheckError::InvalidPolicy)
        );
    }
}
