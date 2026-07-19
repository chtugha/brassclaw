//! Provider role types for the Sempai–Kohai dual-role architecture.

/// The functional role a configured LLM provider is assigned to.
///
/// `Kohai` (後輩 — "junior") is the primary inference provider that executes
/// tool calls, writes code, and produces visible output. `Sempai` (先輩 —
/// "senior") is the teaching and auditing provider that intercepts assembled
/// prompts before Kohai receives them.  `Embedding` is the vector-embedding
/// provider used for memory-chunk indexing and similarity search (§3, §4.30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    /// Primary inference model — executes tool calls, writes code, produces output.
    /// Maps to the `llm.kohai.provider_id` config key.
    Kohai,
    /// Teaching/auditing model — intercepts assembled prompts before Kohai receives them.
    /// Maps to the `llm.sempai.provider_id` config key.
    Sempai,
    /// Vector-embedding provider for memory chunk indexing and similarity search.
    /// Maps to the `embedding.provider_id` config key. A provider MAY hold
    /// `Embedding` alongside `Kohai` or `Sempai`; `Kohai` + `Sempai` still
    /// conflict with each other.
    Embedding,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_role_roundtrips_json() {
        let kohai = serde_json::to_string(&ProviderRole::Kohai).unwrap();
        assert_eq!(kohai, r#""kohai""#);
        let back: ProviderRole = serde_json::from_str(&kohai).unwrap();
        assert_eq!(back, ProviderRole::Kohai);

        let sempai = serde_json::to_string(&ProviderRole::Sempai).unwrap();
        assert_eq!(sempai, r#""sempai""#);
        let back: ProviderRole = serde_json::from_str(&sempai).unwrap();
        assert_eq!(back, ProviderRole::Sempai);

        let embedding = serde_json::to_string(&ProviderRole::Embedding).unwrap();
        assert_eq!(embedding, r#""embedding""#);
        let back: ProviderRole = serde_json::from_str(&embedding).unwrap();
        assert_eq!(back, ProviderRole::Embedding);
    }

    #[test]
    fn provider_role_debug_and_clone() {
        let r = ProviderRole::Sempai;
        assert_eq!(format!("{r:?}"), "Sempai");
        assert_eq!(r.clone(), ProviderRole::Sempai);
    }
}
