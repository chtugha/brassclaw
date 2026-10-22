//! Cost-table types consumed by [`crate::GovernorBackedAccountant`].
//!
//! A [`ModelCostTable`] resolves a [`ModelProfileId`] to a [`ModelCost`]
//! (per-token USD prices + model max-output tokens). Implementations
//! bridge the `LlmProvider::cost_per_token()` family from
//! `brassclaw_llm` into the loop layer without re-exporting LLM crate
//! types. This crate ships two: [`ZeroCostTable`] for free/local
//! providers and tests, and [`StaticModelCostTable`] for composition-
//! driven lookups.

use std::collections::HashMap;

use brassclaw_turns::run_profile::ModelProfileId;
use rust_decimal::Decimal;

/// Static cost-per-token + max-output-tokens table for a single model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelCost {
    /// Input USD per token. `Decimal::ZERO` for free/local models.
    pub input_per_token: Decimal,
    /// Output USD per token. `Decimal::ZERO` for free/local models.
    pub output_per_token: Decimal,
    /// Model's max output tokens — used for worst-case pre-call estimate.
    /// `0` is treated as "unknown" and uses the acknowledged
    /// [`ModelQuoteSettings::output_tokens`] reservation estimate.
    pub max_output_tokens: u64,
    /// Cache-creation multiplier ×1000 applied to `input_per_token` for
    /// tokens written to the provider's prompt cache on this call
    /// (Anthropic: 1250 → 1.25×). Stored as an integer so the struct
    /// stays `Copy` without dragging in extra dependencies.
    ///
    /// Providers without caching return `0` for `cache_creation_input_tokens`,
    /// so the multiplier math naturally produces zero cost — no explicit
    /// provider guard is needed.
    pub cache_write_multiplier_milli: u32,
    /// Cache-read multiplier ×1000 applied to `input_per_token` for tokens
    /// served from the provider's prompt cache (Anthropic: 100 → 0.10×).
    /// Same zero-cost invariant as `cache_write_multiplier_milli`.
    pub cache_read_multiplier_milli: u32,
}

impl ModelCost {
    /// Construct a cost row with Anthropic-style cache pricing
    /// (cache write 1.25×, cache read 0.10×).
    pub fn with_cache_pricing(
        input_per_token: Decimal,
        output_per_token: Decimal,
        max_output_tokens: u64,
    ) -> Self {
        Self {
            input_per_token,
            output_per_token,
            max_output_tokens,
            cache_write_multiplier_milli: 1250,
            cache_read_multiplier_milli: 100,
        }
    }

    /// Cost of writing tokens to the prompt cache. Invalid or unrepresentable
    /// configured arithmetic returns None, never a panic or a zero-price fallback.
    pub fn cache_write_cost(&self, tokens: u64) -> Option<Decimal> {
        self.cache_cost(tokens, self.cache_write_multiplier_milli)
    }

    /// Checked cost of reading tokens from the prompt cache.
    pub fn cache_read_cost(&self, tokens: u64) -> Option<Decimal> {
        self.cache_cost(tokens, self.cache_read_multiplier_milli)
    }

    fn cache_cost(&self, tokens: u64, multiplier_milli: u32) -> Option<Decimal> {
        if self.input_per_token < Decimal::ZERO {
            return None;
        }
        if tokens == 0 || multiplier_milli == 0 {
            return Some(Decimal::ZERO);
        }
        let multiplier = Decimal::new(i64::from(multiplier_milli), 3);
        self.fresh_input_cost(tokens)?
            .checked_mul(multiplier)
            .filter(|amount| self.input_per_token.is_zero() || !amount.is_zero())
    }

    /// Checked cost of fresh (non-cached) input tokens.
    pub fn fresh_input_cost(&self, tokens: u64) -> Option<Decimal> {
        if self.input_per_token < Decimal::ZERO {
            return None;
        }
        self.input_per_token.checked_mul(Decimal::from(tokens))
    }

    /// Checked cost of output tokens.
    pub fn output_cost(&self, tokens: u64) -> Option<Decimal> {
        if self.output_per_token < Decimal::ZERO {
            return None;
        }
        self.output_per_token.checked_mul(Decimal::from(tokens))
    }
}

/// Resolves [`ModelProfileId`] → [`ModelCost`]. Implementations bridge
/// the `LlmProvider::cost_per_token()` family from `brassclaw_llm` into
/// the loop layer without re-exporting LLM crate types.
pub trait ModelCostTable: Send + Sync + std::fmt::Debug {
    fn cost_for(&self, model: &ModelProfileId) -> Option<ModelCost>;
}

/// Host-owned reservation policy. It does not change provider output limits,
/// token-budget enablement or the prices used to settle observed work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelQuoteSettings {
    pub revision: u64,
    pub output_tokens: u32,
}
impl ModelQuoteSettings {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.revision == 0 || self.output_tokens == 0 {
            return Err("invalid model reservation estimate settings");
        }
        Ok(self)
    }
}
/// Synchronous host source of the last acknowledged reservation policy.
pub trait ModelQuoteSettingsSource: Send + Sync {
    fn snapshot(&self) -> Result<ModelQuoteSettings, &'static str>;
}

/// Constant cost table used in tests and as a safe baseline for
/// free/local providers. Every model returns `(0, 0, 0)` so reservation
/// succeeds with a zero-USD estimate.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroCostTable;

impl ModelCostTable for ZeroCostTable {
    fn cost_for(&self, _model: &ModelProfileId) -> Option<ModelCost> {
        Some(ModelCost {
            input_per_token: Decimal::ZERO,
            output_per_token: Decimal::ZERO,
            max_output_tokens: 0,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        })
    }
}

/// Static `(ModelProfileId → ModelCost)` lookup. Composition layers
/// populate this from their model-route registry (provider model name →
/// known per-token price via `brassclaw_llm::costs::model_cost`) so the
/// accountant can compute actual USD spend on every reconcile.
///
/// Profiles missing from the table return `None`. The production accountant
/// then applies its configured conservative fallback price. Known free/local
/// models must have an explicit zero-cost entry or use `ZeroCostTable`.
#[derive(Debug, Default, Clone)]
pub struct StaticModelCostTable {
    costs: HashMap<ModelProfileId, ModelCost>,
}

impl StaticModelCostTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entry(mut self, profile: ModelProfileId, cost: ModelCost) -> Self {
        self.costs.insert(profile, cost);
        self
    }

    pub fn insert(&mut self, profile: ModelProfileId, cost: ModelCost) {
        self.costs.insert(profile, cost);
    }

    pub fn len(&self) -> usize {
        self.costs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.costs.is_empty()
    }
}

impl ModelCostTable for StaticModelCostTable {
    fn cost_for(&self, model: &ModelProfileId) -> Option<ModelCost> {
        self.costs.get(model).copied()
    }
}
