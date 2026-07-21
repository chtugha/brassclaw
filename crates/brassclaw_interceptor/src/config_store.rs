//! Interceptor configuration store trait.
//!
//! Keys persisted in `brassclaw_config` table (no new migration):
//! - `interceptor.sempai_base_prompt` — assembled base prompt (Part A)
//! - `interceptor.sempai_base_prompt_assembled_at` — ISO-8601 timestamp
//! - `interceptor.sempai_persona` — Sempai persona text (Part B)
//! - `interceptor.sempai_prewarm_last_at` — ISO-8601 last pre-warm timestamp

use async_trait::async_trait;

/// A key-value pair read from the config store.
#[derive(Debug, Clone)]
pub struct InterceptorConfig {
    /// Assembled base prompt (Part A).  `None` if never assembled.
    pub sempai_base_prompt: Option<String>,
    /// ISO-8601 timestamp when the base prompt was last assembled.  `None`
    /// if never assembled.
    pub sempai_base_prompt_assembled_at: Option<String>,
    /// Sempai persona text (Part B).  Falls back to the compiled-in default
    /// when not set in the config store.
    pub sempai_persona: Option<String>,
    /// ISO-8601 timestamp when `prewarm` last succeeded.  `None` if never.
    pub sempai_prewarm_last_at: Option<String>,
}

/// Persistence port for interceptor configuration keys.
///
/// The default implementation uses the `brassclaw_config` Postgres table
/// (same table the LLM config service uses for role assignments).  The
/// test double can use an in-memory map.
#[async_trait]
pub trait InterceptorConfigStore: Send + Sync {
    /// Load all interceptor config keys.
    async fn load(&self) -> Result<InterceptorConfig, crate::InterceptorError>;

    /// Persist the Sempai persona text.
    async fn save_persona(
        &self,
        persona: &str,
    ) -> Result<(), crate::InterceptorError>;

    /// Persist the assembled base prompt and its assembly timestamp.
    async fn save_base_prompt(
        &self,
        prompt: &str,
        assembled_at: &str,
    ) -> Result<(), crate::InterceptorError>;

    /// Persist the pre-warm timestamp.
    async fn save_prewarm_last_at(
        &self,
        timestamp: &str,
    ) -> Result<(), crate::InterceptorError>;
}
