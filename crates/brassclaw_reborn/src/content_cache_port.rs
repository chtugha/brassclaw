//! Content-cache capability port decorator.
//!
//! `ContentCachingCapabilityPort` wraps an inner `LoopCapabilityPort` and
//! intercepts completed tool results that exceed a token threshold. Large
//! results are stored in a `ContentCacheBridge` (an `Arc<Mutex<...>>` shared
//! with the `fetch_cached_content` first-party handler) and replaced with
//! a compact stub in the model-visible output.
//!
//! The decorator is stateless at the factory level: a fresh `ContentCacheBridge`
//! is created per turn inside `create_capability_port()`. The composition root
//! holds a `CurrentCacheBridgeSlot` shared with the `fetch_cached_content`
//! handler; the factory updates that slot on every new turn so the handler
//! always reads from the current turn's bridge.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use brassclaw_agent_loop::content_cache::{
    CachedEntry, ContentCacheBridge, ContentCacheState, estimate_tokens,
};
use brassclaw_turns::run_profile::{
    AgentLoopHostError, CapabilityBatchInvocation, CapabilityBatchOutcome, CapabilityFailureKind,
    CapabilityInvocation, CapabilityOutcome, CapabilityResultMessage, LoopCapabilityPort, LoopRunContext,
    ProviderToolCall, ProviderToolCallCapabilityIds, ProviderToolDefinition, VisibleCapabilityRequest,
    VisibleCapabilitySurface,
};

/// Capability ID for the content-cache retrieval tool.
pub const FETCH_CACHED_CONTENT_CAPABILITY_ID: &str = "brassclaw.fetch_cached_content";

/// Slot holding the bridge for the currently-executing turn.
///
/// Created once at composition time, shared between `ContentCachingPortFactory`
/// and `FetchCachedContentHandler`. The factory writes a new bridge per turn;
/// the handler reads from the current slot.
#[derive(Debug, Clone, Default)]
pub struct CurrentCacheBridgeSlot(pub Arc<Mutex<Option<ContentCacheBridge>>>);

impl CurrentCacheBridgeSlot {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Replace the current bridge with a freshly-created one (called per turn).
    pub fn reset_for_new_turn(&self) -> ContentCacheBridge {
        let bridge = ContentCacheBridge::new();
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(bridge.clone());
        bridge
    }

    /// Read the current cache state (for the handler).
    pub fn with_current<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut ContentCacheState) -> R,
    {
        let slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        slot.as_ref().map(|bridge| bridge.with_lock(f))
    }
}

// ── ContentCachingCapabilityPort ─────────────────────────────────────────────

/// Wraps a `LoopCapabilityPort` to intercept large tool results.
pub struct ContentCachingCapabilityPort {
    inner: Arc<dyn LoopCapabilityPort>,
    bridge: ContentCacheBridge,
    threshold_tokens: usize,
}

impl ContentCachingCapabilityPort {
    pub fn new(
        inner: Arc<dyn LoopCapabilityPort>,
        bridge: ContentCacheBridge,
        threshold_tokens: usize,
    ) -> Self {
        Self {
            inner,
            bridge,
            threshold_tokens,
        }
    }

    /// Check a single `CapabilityOutcome`; if it is `Completed` with a summary
    /// exceeding the threshold (and the capability is not the cache-retrieval
    /// tool itself), replace the summary with a stub and cache the original.
    fn maybe_cache_outcome(
        &self,
        outcome: CapabilityOutcome,
        capability_id_str: &str,
        iteration_hint: usize,
    ) -> CapabilityOutcome {
        // Never cache the fetch tool's own output — that would be recursive.
        if capability_id_str == FETCH_CACHED_CONTENT_CAPABILITY_ID {
            return outcome;
        }

        match outcome {
            CapabilityOutcome::Completed(result) => {
                let token_estimate = estimate_tokens(&result.safe_summary);
                if token_estimate <= self.threshold_tokens {
                    return CapabilityOutcome::Completed(result);
                }
                // Cache the original content and replace with stub.
                let stub = self.bridge.with_lock(|cache| {
                    let key = cache.next_key(capability_id_str, iteration_hint);
                    let entry = CachedEntry::new(
                        key,
                        capability_id_str.to_string(),
                        result.safe_summary.clone(),
                        iteration_hint,
                    );
                    tracing::debug!(
                        key = %entry.key,
                        tool = %capability_id_str,
                        tokens = token_estimate,
                        threshold = self.threshold_tokens,
                        "content cache: caching large tool result"
                    );
                    cache.insert(entry)
                });

                CapabilityOutcome::Completed(CapabilityResultMessage {
                    result_ref: result.result_ref,
                    safe_summary: stub,
                    progress: result.progress,
                    terminate_hint: result.terminate_hint,
                })
            }
            other => other,
        }
    }
}

#[async_trait]
impl LoopCapabilityPort for ContentCachingCapabilityPort {
    fn tool_definitions(&self) -> Result<Vec<ProviderToolDefinition>, AgentLoopHostError> {
        self.inner.tool_definitions()
    }

    fn provider_tool_call_capability_ids(
        &self,
        tool_call: &ProviderToolCall,
    ) -> Result<ProviderToolCallCapabilityIds, AgentLoopHostError> {
        self.inner.provider_tool_call_capability_ids(tool_call)
    }

    fn validate_provider_tool_call(
        &self,
        tool_call: &ProviderToolCall,
    ) -> Result<(), AgentLoopHostError> {
        self.inner.validate_provider_tool_call(tool_call)
    }

    async fn register_provider_tool_call(
        &self,
        tool_call: ProviderToolCall,
    ) -> Result<brassclaw_turns::run_profile::CapabilityCallCandidate, AgentLoopHostError> {
        self.inner.register_provider_tool_call(tool_call).await
    }

    async fn visible_capabilities(
        &self,
        request: VisibleCapabilityRequest,
    ) -> Result<VisibleCapabilitySurface, AgentLoopHostError> {
        self.inner.visible_capabilities(request).await
    }

    async fn invoke_capability(
        &self,
        request: CapabilityInvocation,
    ) -> Result<CapabilityOutcome, AgentLoopHostError> {
        let cap_id = request.capability_id.to_string();
        let outcome = self.inner.invoke_capability(request).await?;
        Ok(self.maybe_cache_outcome(outcome, &cap_id, 0))
    }

    async fn invoke_capability_batch(
        &self,
        request: CapabilityBatchInvocation,
    ) -> Result<CapabilityBatchOutcome, AgentLoopHostError> {
        // Collect capability IDs before moving the invocations into the inner call.
        let cap_ids: Vec<String> = request
            .invocations
            .iter()
            .map(|inv| inv.capability_id.to_string())
            .collect();

        let mut batch = self.inner.invoke_capability_batch(request).await?;

        // Intercept completed results.
        for (outcome, cap_id) in batch.outcomes.iter_mut().zip(cap_ids.iter()) {
            let cached = self.maybe_cache_outcome(
                std::mem::replace(outcome, CapabilityOutcome::Failed(make_placeholder_failure())),
                cap_id,
                0,
            );
            *outcome = cached;
        }
        Ok(batch)
    }
}

fn make_placeholder_failure() -> brassclaw_turns::run_profile::CapabilityFailure {
    brassclaw_turns::run_profile::CapabilityFailure {
        error_kind: CapabilityFailureKind::Internal,
        safe_summary: "content cache: placeholder during interception".to_string(),
        detail: None,
    }
}

// ── ContentCachingLoopCapabilityPortDecorator ─────────────────────────────────

/// `LoopCapabilityPortDecorator` that wraps the inner port with a
/// `ContentCachingCapabilityPort` and resets the `CurrentCacheBridgeSlot`
/// for the new turn.
pub struct ContentCachingPortDecorator {
    slot: CurrentCacheBridgeSlot,
    threshold_tokens: usize,
}

impl ContentCachingPortDecorator {
    pub fn new(slot: CurrentCacheBridgeSlot, threshold_tokens: usize) -> Self {
        Self {
            slot,
            threshold_tokens,
        }
    }
}

impl brassclaw_loop_support::LoopCapabilityPortDecorator for ContentCachingPortDecorator {
    fn decorate(
        &self,
        _run_context: &LoopRunContext,
        inner: Arc<dyn LoopCapabilityPort>,
    ) -> Arc<dyn LoopCapabilityPort> {
        // Create a new bridge for this turn and update the shared slot.
        let bridge = self.slot.reset_for_new_turn();
        Arc::new(ContentCachingCapabilityPort::new(
            inner,
            bridge,
            self.threshold_tokens,
        ))
    }
}
