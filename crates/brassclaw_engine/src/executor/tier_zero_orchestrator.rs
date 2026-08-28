//! Public facade over the Tier-0 deterministic execution channel.
//!
//! [`TierZeroOrchestrator`] bundles the long-lived engine dependencies
//! `execute_tier_zero_channel` needs (`LeaseManager`, `PolicyEngine`,
//! `GateController`, `LlmBackend`, the event broadcast, and the
//! `RetrievalSource`) behind two async entry points. The per-run
//! `EffectExecutor` is intentionally NOT held here — it is built per turn by
//! the composition `TierZeroEffectExecutorBuilder` (Q-H12-2-BUILD = A) and
//! passed into [`TierZeroOrchestrator::run_tier_zero`] so tenant/agent/grants
//! resolve at run time rather than once at wiring time:
//! - [`TierZeroOrchestrator::run_tier_zero`] — deserialize a stashed
//!   `recipe_hint` (`Vec<ComponentItem>`), assemble the orchestrator-channel
//!   prose via [`assemble_pkr_from_items`], and run the deterministic channel.
//! - [`TierZeroOrchestrator::assemble_prior_knowledge`] — wrap
//!   [`assemble_prior_knowledge_with_hint`] for Tier-1 prompt injection.
//!
//! Composition (H.12.4 wiring) constructs this via
//! [`TierZeroOrchestrator::builder`]; H.12.5 (`OrchestratorLookup::run_tier_zero`)
//! calls the methods per turn.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::capability::lease::LeaseManager;
use crate::capability::policy::PolicyEngine;
use crate::gate::{CancellingGateController, GateController};
use crate::memory::{ComponentItem, RetrievalSource};
use crate::traits::effect::EffectExecutor;
use crate::traits::llm::LlmBackend;
use crate::types::error::EngineError;
use crate::types::event::ThreadEvent;
use crate::types::thread::Thread;

use super::orchestrator::{
    PkrAssemblyResult, TierZeroChannelResult, assemble_pkr_from_items,
    assemble_prior_knowledge_with_hint, execute_tier_zero_channel,
};

/// Bundled long-lived engine dependencies for the Tier-0 deterministic
/// execution channel.
///
/// Construct via [`TierZeroOrchestrator::builder`]. Fields are private; the two
/// async methods are the public API consumed by H.12.5 `OrchestratorLookup`.
/// The per-run [`EffectExecutor`] is supplied to
/// [`TierZeroOrchestrator::run_tier_zero`] by the caller (H.12.5) — it is not
/// held on the orchestrator so a single long-lived `Arc<TierZeroOrchestrator>`
/// can serve turns with different tenant/agent/grant scopes.
pub struct TierZeroOrchestrator {
    llm: Arc<dyn LlmBackend>,
    leases: Arc<LeaseManager>,
    policy: Arc<PolicyEngine>,
    gate_controller: Arc<dyn GateController>,
    event_tx: Option<broadcast::Sender<ThreadEvent>>,
    retrieval_source: Option<Arc<dyn RetrievalSource>>,
}

impl TierZeroOrchestrator {
    /// Begin a build with all fields unset; `llm` and `effects` are required.
    pub fn builder() -> TierZeroOrchestratorBuilder {
        TierZeroOrchestratorBuilder::new()
    }

    /// Run the deterministic Tier-0 channel for one turn.
    ///
    /// `recipe_hint` is the stashed `Vec<ComponentItem>` (Tier-1 retrieval
    /// result); it is deserialized and assembled into orchestrator-channel
    /// prose via [`assemble_pkr_from_items`] (no re-fetch). `recipe_rust_context`
    /// is forwarded verbatim as the channel's rust context. `effects` is the
    /// per-run [`EffectExecutor`] built by the composition
    /// `TierZeroEffectExecutorBuilder` (Q-H12-2-BUILD = A); it is passed in
    /// per call rather than held on the orchestrator so tenant/agent/grants
    /// resolve per turn.
    pub async fn run_tier_zero(
        &self,
        thread: &Thread,
        effects: &Arc<dyn EffectExecutor>,
        recipe_hint: &serde_json::Value,
        recipe_rust_context: &serde_json::Value,
    ) -> Result<TierZeroChannelResult, EngineError> {
        let items: Vec<ComponentItem> =
            serde_json::from_value(recipe_hint.clone()).map_err(|e| EngineError::InvalidInput {
                reason: format!(
                    "TierZeroOrchestrator::run_tier_zero: recipe_hint deserialize failed: {e}"
                ),
            })?;
        let pkr = assemble_pkr_from_items(&items);
        execute_tier_zero_channel(
            thread,
            &pkr.orchestrator_content,
            recipe_rust_context,
            effects,
            &self.leases,
            &self.policy,
            &self.gate_controller,
            &self.llm,
            self.event_tx.as_ref(),
        )
        .await
    }

    /// Assemble prior-knowledge prose for Tier-1 prompt injection.
    ///
    /// Thin wrapper over [`assemble_prior_knowledge_with_hint`], supplying this
    /// orchestrator's held `RetrievalSource` (if any).
    pub async fn assemble_prior_knowledge(
        &self,
        thread: &Thread,
        goal: &str,
        token_budget: usize,
        sender_class: &str,
        recipe_hint: Option<serde_json::Value>,
    ) -> Result<PkrAssemblyResult, EngineError> {
        assemble_prior_knowledge_with_hint(
            thread,
            goal,
            token_budget,
            sender_class,
            self.retrieval_source.as_ref(),
            recipe_hint,
        )
        .await
    }
}

/// Builder for [`TierZeroOrchestrator`].
///
/// `llm` is required (its `build`-time absence is an
/// [`EngineError::InvalidInput`]); the per-run `effects` is NOT a builder
/// field (it is supplied to [`TierZeroOrchestrator::run_tier_zero`] per call).
/// The remaining fields default to fresh [`LeaseManager::new`],
/// [`PolicyEngine::new`], and [`CancellingGateController::arc`]; `event_tx` and
/// `retrieval_source` default to `None`.
#[derive(Default)]
pub struct TierZeroOrchestratorBuilder {
    llm: Option<Arc<dyn LlmBackend>>,
    leases: Option<Arc<LeaseManager>>,
    policy: Option<Arc<PolicyEngine>>,
    gate_controller: Option<Arc<dyn GateController>>,
    event_tx: Option<broadcast::Sender<ThreadEvent>>,
    retrieval_source: Option<Arc<dyn RetrievalSource>>,
}

impl TierZeroOrchestratorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn llm(mut self, llm: Arc<dyn LlmBackend>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn leases(mut self, leases: Arc<LeaseManager>) -> Self {
        self.leases = Some(leases);
        self
    }

    pub fn policy(mut self, policy: Arc<PolicyEngine>) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn gate_controller(mut self, gate_controller: Arc<dyn GateController>) -> Self {
        self.gate_controller = Some(gate_controller);
        self
    }

    pub fn event_tx(mut self, event_tx: broadcast::Sender<ThreadEvent>) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    pub fn retrieval_source(mut self, retrieval_source: Arc<dyn RetrievalSource>) -> Self {
        self.retrieval_source = Some(retrieval_source);
        self
    }

    pub fn build(self) -> Result<TierZeroOrchestrator, EngineError> {
        let llm = self.llm.ok_or_else(|| EngineError::InvalidInput {
            reason: "TierZeroOrchestratorBuilder: `llm` is required".into(),
        })?;
        Ok(TierZeroOrchestrator {
            llm,
            leases: self.leases.unwrap_or_else(|| Arc::new(LeaseManager::new())),
            policy: self.policy.unwrap_or_else(|| Arc::new(PolicyEngine::new())),
            gate_controller: self
                .gate_controller
                .unwrap_or_else(CancellingGateController::arc),
            event_tx: self.event_tx,
            retrieval_source: self.retrieval_source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::effect::ThreadExecutionContext;
    use crate::traits::llm::{LlmCallConfig, LlmOutput};
    use crate::types::capability::{ActionDef, CapabilityLease, CapabilitySummary};
    use crate::types::message::ThreadMessage;
    use crate::types::project::ProjectId;
    use crate::types::step::ActionResult;
    use crate::types::thread::{ThreadConfig, ThreadType};

    struct MockLlm;

    #[async_trait::async_trait]
    impl LlmBackend for MockLlm {
        async fn complete(
            &self,
            _messages: &[ThreadMessage],
            _actions: &[ActionDef],
            _config: &LlmCallConfig,
        ) -> Result<LlmOutput, EngineError> {
            Err(EngineError::InvalidInput {
                reason: "mock".into(),
            })
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }

    struct MockEffects;

    #[async_trait::async_trait]
    impl EffectExecutor for MockEffects {
        async fn execute_action(
            &self,
            _action_name: &str,
            _parameters: serde_json::Value,
            _lease: &CapabilityLease,
            _context: &ThreadExecutionContext,
        ) -> Result<ActionResult, EngineError> {
            Err(EngineError::Effect {
                reason: "mock".into(),
            })
        }

        async fn available_actions(
            &self,
            _leases: &[CapabilityLease],
            _context: &ThreadExecutionContext,
        ) -> Result<Vec<ActionDef>, EngineError> {
            Ok(Vec::new())
        }

        async fn available_capabilities(
            &self,
            _leases: &[CapabilityLease],
            _context: &ThreadExecutionContext,
        ) -> Result<Vec<CapabilitySummary>, EngineError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn build_requires_llm() {
        let result = TierZeroOrchestrator::builder().build();
        assert!(matches!(result, Err(EngineError::InvalidInput { .. })));
    }

    #[test]
    fn build_succeeds_with_llm_defaulting_the_rest() {
        let orch = TierZeroOrchestrator::builder()
            .llm(Arc::new(MockLlm))
            .build()
            .expect("llm suffices to build");
        assert_eq!(orch.llm.model_name(), "mock");
    }

    /// `run_tier_zero` accepts the per-run `EffectExecutor` and rejects a
    /// malformed `recipe_hint` (not a `Vec<ComponentItem>`) with
    /// [`EngineError::InvalidInput`] before delegating to
    /// `execute_tier_zero_channel` — so neither the LLM nor the effect
    /// executor is consulted. Proves the deserialize-gate + per-run-executor
    /// param wiring (Q-H12-4-EFFECTS-PARAM = A).
    #[tokio::test]
    async fn run_tier_zero_rejects_malformed_recipe_hint() {
        let orch = TierZeroOrchestrator::builder()
            .llm(Arc::new(MockLlm))
            .build()
            .expect("llm suffices to build");
        let thread = Thread::new(
            "facade-test-goal",
            ThreadType::Foreground,
            ProjectId::new(),
            "facade-test-user",
            ThreadConfig::default(),
        );
        let effects: Arc<dyn EffectExecutor> = Arc::new(MockEffects);
        let result = orch
            .run_tier_zero(
                &thread,
                &effects,
                &serde_json::Value::String("not-a-component-array".into()),
                &serde_json::Value::Null,
            )
            .await;
        assert!(
            matches!(result, Err(EngineError::InvalidInput { .. })),
            "malformed recipe_hint must surface as InvalidInput, got {result:?}"
        );
    }
}
