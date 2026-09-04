//! Step C.5 — engine-side port trait bridging the orchestrator's
//! `host.kohai_complete` host-call to the interceptor ingress (the Kohai LLM
//! handoff: forensic-packet capture → optional Sempai review → provider-prefix
//! swap → provider gateway call → packet close).
//!
//! `brassclaw_engine` cannot depend on `brassclaw_reborn_composition` (the
//! composition layer is downstream of the engine), and the interceptor store
//! (`PgInterceptorStore`), the basic-prompt prefix store (`PgBasicPromptStore`),
//! and the provider `HostManagedModelGateway` that the Kohai flow drives all
//! live there. The composition layer is the sole crate that sees both
//! `brassclaw_engine` and the interceptor stores and provider gateway, so it
//! owns the impl. This mirrors the [`crate::executor::ComponentPort`]
//! engine↔composition port precedent (C.4.5.17).
//!
//! # Contract
//!
//! `host.kohai_complete(prompt={chat_history, user_query, prefix_placeholder})`
//! is a `host.*` MethodCall handled in [`crate::executor::orchestrator`]. The
//! handler thin-calls [`KohaiPort::complete`], which runs the FULL Kohai flow:
//! 1. Build a `CapturedPrompt` from the prompt dict + capture a `ForensicPacket`
//!    `[AwaitingKohai]` → `InterceptorStore::save`.
//! 2. (rerouting) Sempai audit of the prompt → `with_sempai_review` → save
//!    `[SempaiReviewed]`.
//! 3. Resolve the provider-prefix chunk (`get_system_bundle`) → swap the
//!    `prefix_placeholder`.
//! 4. Build the final messages (system prefix + chat_history + user_query) →
//!    `HostManagedModelGateway::stream_model` (the "Kohai" provider call).
//! 5. `with_kohai_response(text, usage)` → save `[Complete]`.
//! 6. Return the answer text + usage.
//!
//! The composition impl owns the provider `HostManagedModelGateway` (the real
//! working LLM) — the engine no longer threads an `LlmBackend` into the port
//! (the `LlmBackend` host path retires with the C.6 Kohai re-architecture).
//! Until the composition impl is wired, the engine passes `None` and the handler
//! degrades gracefully (`{ok:false, error:"kohai_unavailable"}`).

use thiserror::Error;

/// Errors raised by a [`KohaiPort`] implementation.
#[derive(Debug, Clone, Error)]
pub enum KohaiPortError {
    /// No Kohai bridge is wired (`None` port) — the orchestrator falls back.
    #[error("kohai bridge unavailable")]
    Unavailable,
    /// The `prompt` argument was missing or not a dict.
    #[error("invalid prompt: {reason}")]
    InvalidPrompt { reason: String },
    /// The provider-prefix chunk could not be resolved for the scope.
    #[error("provider prefix unavailable: {reason}")]
    PrefixUnavailable { reason: String },
    /// The underlying provider gateway call failed.
    #[error("kohai llm call failed: {reason}")]
    LlmFailed { reason: String },
    /// A forensic-packet store failure (save capture / save response).
    #[error("interceptor store failure: {reason}")]
    StoreFailed { reason: String },
}

/// Turn identity carried into the Kohai flow so the composition impl can scope
/// the forensic packet + resolve the per-scope provider prefix.
#[derive(Debug, Clone)]
pub struct KohaiCallCtx {
    /// Engine run identifier (thread id).
    pub run_id: String,
    /// Orchestrator iteration counter for this turn.
    pub iteration: u32,
    /// Owning user id (scopes the prefix bundle).
    pub user_id: String,
    /// Owning project id (scopes the prefix bundle).
    pub project_id: String,
    /// Tenant id (scopes the interceptor store rows).
    pub tenant_id: String,
}

/// Token usage reported back from the Kohai provider call.
#[derive(Debug, Clone, Copy, Default)]
pub struct KohaiUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

/// The answer returned by [`KohaiPort::complete`] — the provider response text
/// + its usage.
#[derive(Debug, Clone)]
pub struct KohaiAnswer {
    pub content: String,
    pub usage: KohaiUsage,
}

/// Engine-side port over the interceptor ingress (the Kohai LLM handoff). The
/// implementation lives in the composition layer and drives the forensic-packet
/// lifecycle + the provider-prefix swap + the `HostManagedModelGateway` provider
/// call (which the composition impl owns).
///
/// `async` because the backing store + gateway calls drive the DB pool / network
/// — must not be `block_on()`-ed inside a running Tokio runtime (mirrors
/// [`crate::executor::ComponentPort`]). The impl clones its `Arc` stores + the
/// provider gateway `Arc` + the owned `prompt`/`ctx` into the boxed future, so
/// the returned future is `'static` (it borrows neither `&self` nor any
/// `LlmBackend`) — matching the `ComponentPort::compose` precedent.
pub trait KohaiPort: Send + Sync {
    /// Run the FULL Kohai flow for `prompt` (`{chat_history, user_query,
    /// prefix_placeholder}`) under `ctx`. The composition impl drives the
    /// provider call over its own `HostManagedModelGateway`. Returns the
    /// provider answer text + usage.
    fn complete(
        &self,
        prompt: serde_json::Value,
        ctx: KohaiCallCtx,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<KohaiAnswer, KohaiPortError>> + Send + 'static>,
    >;
}
