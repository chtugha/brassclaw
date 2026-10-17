//! Operator-configured embedding contracts, retained with a provider revision.
//! These are expected response shapes, not model availability/quality evidence.
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingModelContract {
    /// Increase when the embedding function changes under the same model name.
    #[serde(
        default = "initial_vector_revision",
        skip_serializing_if = "is_initial_vector_revision"
    )]
    pub vector_revision: NonZeroU32,
    pub dimension: NonZeroU32,
    /// Zero disables retention. Capacity is a ceiling, not reserved memory.
    pub cache_max_entries: u32,
}

fn initial_vector_revision() -> NonZeroU32 {
    NonZeroU32::MIN
}

fn is_initial_vector_revision(value: &NonZeroU32) -> bool {
    *value == NonZeroU32::MIN
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EmbeddingModelContracts(pub BTreeMap<String, EmbeddingModelContract>);

impl<'de> Deserialize<'de> for EmbeddingModelContracts {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ContractsVisitor;
        impl<'de> serde::de::Visitor<'de> for ContractsVisitor {
            type Value = EmbeddingModelContracts;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object mapping distinct model names to embedding contracts")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut models = BTreeMap::new();
                while let Some((name, contract)) =
                    map.next_entry::<String, EmbeddingModelContract>()?
                {
                    if name.is_empty()
                        || name.trim() != name.as_str()
                        || name.chars().any(char::is_control)
                    {
                        return Err(serde::de::Error::custom("embedding model name is invalid"));
                    }
                    if models.insert(name, contract).is_some() {
                        return Err(serde::de::Error::custom(
                            "duplicate embedding model contract",
                        ));
                    }
                }
                Ok(EmbeddingModelContracts(models))
            }
        }
        deserializer.deserialize_map(ContractsVisitor)
    }
}

impl EmbeddingModelContracts {
    pub fn validate_for_protocol(
        &self,
        protocol: crate::ProviderProtocol,
    ) -> Result<(), &'static str> {
        if !self.0.is_empty()
            && !matches!(
                protocol,
                crate::ProviderProtocol::OpenAiCompletions
                    | crate::ProviderProtocol::OpenRouter
                    | crate::ProviderProtocol::NearAi
                    | crate::ProviderProtocol::Ollama
                    | crate::ProviderProtocol::Bedrock
            )
        {
            return Err("this protocol has no supported embedding adapter");
        }
        if self.0.keys().any(|name| {
            name.is_empty() || name.trim() != name.as_str() || name.chars().any(char::is_control)
        }) {
            return Err("embedding model name is invalid");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_contracts_reject_ambiguous_invalid_or_incomplete_values() {
        for invalid in [
            "null",
            "[]",
            r#"{"m":{"dimension":0,"cache_max_entries":0}}"#,
            r#"{"m":{"dimension":1}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":-1}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":0,"extra":1}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":0},"m":{"dimension":2,"cache_max_entries":1}}"#,
            r#"{" m":{"dimension":1,"cache_max_entries":0}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":4294967296}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":0,"vector_revision":0}}"#,
            r#"{"m":{"dimension":1,"cache_max_entries":0,"vector_revision":null}}"#,
        ] {
            assert!(
                serde_json::from_str::<EmbeddingModelContracts>(invalid).is_err(),
                "{invalid}"
            );
        }
        let models: EmbeddingModelContracts = serde_json::from_str(
            r#"{"custom-vector-model":{"dimension":3,"cache_max_entries":0}}"#,
        )
        .unwrap();
        assert_eq!(models.0["custom-vector-model"].dimension.get(), 3);
        assert_eq!(models.0["custom-vector-model"].cache_max_entries, 0);
        assert_eq!(models.0["custom-vector-model"].vector_revision.get(), 1);
        let revised: EmbeddingModelContracts = serde_json::from_str(
            r#"{"custom-vector-model":{"dimension":3,"cache_max_entries":0,"vector_revision":2}}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(revised).unwrap()["custom-vector-model"]["vector_revision"],
            2
        );
        assert!(
            models
                .validate_for_protocol(crate::ProviderProtocol::OpenAiCompletions)
                .is_ok()
        );
        assert!(
            models
                .validate_for_protocol(crate::ProviderProtocol::Anthropic)
                .is_err()
        );
    }
}
