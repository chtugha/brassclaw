//! Composition-supplied implementations of the loop retrieval and raw
//! message-text resolver ports (v3 Phase E.0 / plan §H3–§H4).
//!
//! The loop layer (`brassclaw_turns::run_profile`) defines the
//! [`RetrievalLookup`] and [`MessageTextResolver`] traits so the production
//! host (`RebornLoopDriverHost`) can hold `Option<Arc<dyn ...>>` slots without
//! `brassclaw_reborn` depending on the engine or the skill-activation store.
//! This crate — the sole one depending on both — supplies the backing impls:
//!
//! - [`PgRetrievalLookup`] wraps `brassclaw_engine::memory::PostgresSource` and delegates to its intent-driven `fetch_for_turn` (which runs `resolve_intent` + the SEC-01-gated component fetch against Postgres). It is gated on the composition `skills-db` feature, which enables the engine's `skills-db`-gated `PostgresSource` methods. When the feature is off the type is absent and the host's `retrieval_lookup` slot stays `None` so `RecipeStage` falls through to Tier 2 (correct explicit behaviour).
//! - [`SkillActivationMessageTextResolver`] wraps the local-dev `SelectableSkillContextSource` and resolves the **raw** accepted-message body via `peek_message_text` (the non-consuming `messages_by_run` read). It is not engine-gated — raw-text resolution does not need Postgres.
//!
//! E.0 uses conservative routing booleans (`tier0_eligible = false`,
//! `llm_call_required = true`) and leaves `rust_items == orchestrator_items`;
//! Phase E's `SplitResult` populates the real booleans and splits the item
//! payloads. See `docs/agents-v3/subplan_problem_stepE0_of_saved_plan_to_v3.md`.

use std::sync::Arc;

use async_trait::async_trait;

use brassclaw_turns::run_profile::{
    AgentLoopHostError, AgentLoopHostErrorKind, LoopRunContext, MessageTextResolver,
};
#[cfg(feature = "skills-db")]
use brassclaw_turns::run_profile::{RetrievalLookup, RetrievalLookupError, RetrievalTurnResult};

// ---------------------------------------------------------------------------
// PgRetrievalLookup — engine-backed RetrievalLookup (skills-db-gated)
// ---------------------------------------------------------------------------

/// `RetrievalLookup` backed by the engine's `PostgresSource` (v3 Phase E.0 /
/// plan §H4).
///
/// Constructed by the composition runtime when a Postgres pool and the
/// `skills-db` feature are available. `fetch_for_turn` builds a
/// `ComponentScope` from the live turn context and delegates to
/// `PostgresSource::fetch_for_turn`, mapping `FetchForTurnResult` into a
/// serialized `RetrievalTurnResult` for the Phase-H Tier-0/Tier-1 consumer.
#[cfg(feature = "skills-db")]
pub(crate) struct PgRetrievalLookup {
    source: Arc<brassclaw_engine::memory::PostgresSource>,
}

#[cfg(feature = "skills-db")]
impl PgRetrievalLookup {
    pub(crate) fn new(source: Arc<brassclaw_engine::memory::PostgresSource>) -> Self {
        Self { source }
    }
}

#[cfg(feature = "skills-db")]
#[async_trait]
impl RetrievalLookup for PgRetrievalLookup {
    async fn fetch_for_turn(
        &self,
        context: &LoopRunContext,
        query: &str,
        token_budget: usize,
        sender_class_code: &str,
    ) -> Result<Option<RetrievalTurnResult>, RetrievalLookupError> {
        use brassclaw_engine::memory::{FetchForTurnResult, RetrievalSource, RetrievalSourceError};

        let scope = build_component_scope(context);
        let result = self
            .source
            .fetch_for_turn(&scope, query, token_budget, sender_class_code)
            .await
            .map_err(|error| match error {
                RetrievalSourceError::Db(reason) => RetrievalLookupError::Db(reason),
                RetrievalSourceError::Engine(reason) => RetrievalLookupError::Backend(reason),
            })?;
        Ok(match result {
            FetchForTurnResult::Components(items) if items.is_empty() => None,
            FetchForTurnResult::Components(items) => {
                Some(retrieval_turn_result_for_components(items)?)
            }
            FetchForTurnResult::Disambiguation(candidates) if candidates.is_empty() => None,
            FetchForTurnResult::Disambiguation(candidates) => {
                Some(retrieval_turn_result_for_disambiguation(candidates)?)
            }
            FetchForTurnResult::ActionShortCircuit { component_id, name } => Some(
                retrieval_turn_result_for_action_short_circuit(component_id, name),
            ),
            FetchForTurnResult::SplitResult {
                rust_items,
                orchestrator_items,
                routing,
                instruction,
            } => Some(retrieval_turn_result_for_split(
                rust_items,
                orchestrator_items,
                routing,
                instruction,
            )?),
        })
    }
}

/// Build the engine `ComponentScope` for a live turn from the loop run context.
///
/// v3 Phase F (Q-F2 / Q-F4): real tenancy is sourced from the live
/// `LoopRunContext.scope.tenant_id` (a `TurnScope::tenant_id: TenantId`,
/// always present — see `brassclaw_turns::scope`). `agent_id` comes from
/// `scope.agent_id` (falls back to `"default"`) and `project_id` from
/// `scope.project_id`. `user_id` stays the effective turn actor / explicit
/// thread owner / system sentinel (the *user*, distinct from the tenant).
/// This is the LIVE agent-loop retrieval scope; the dormant engine
/// `handle_assemble_prior_knowledge` reads `thread.tenant_id` / `thread.agent_id`
/// (F.1/F.3).
#[cfg(feature = "skills-db")]
fn build_component_scope(context: &LoopRunContext) -> brassclaw_engine::memory::ComponentScope {
    let scope = &context.scope;
    let user_id = context
        .actor
        .as_ref()
        .map(|actor| actor.user_id.as_str().to_string())
        .or_else(|| {
            scope
                .explicit_owner_user_id()
                .map(|uid| uid.as_str().to_string())
        })
        .unwrap_or_else(|| brassclaw_host_api::SYSTEM_RESERVED_ID.to_string());
    let agent_id = scope
        .agent_id
        .as_ref()
        .map(|aid| aid.as_str().to_string())
        .unwrap_or_else(|| "default".to_string());
    let project_id = scope
        .project_id
        .as_ref()
        .map(|pid| pid.as_str().to_string())
        .unwrap_or_default();
    brassclaw_engine::memory::ComponentScope {
        tenant_id: scope.tenant_id.as_str().to_string(),
        user_id,
        agent_id,
        project_id,
    }
}

/// Serialize assembled components into a conservative (un-split) E.0
/// `RetrievalTurnResult`. Phase E replaces this with the `SplitResult`-driven
/// `rust_items` / `orchestrator_items` split and real routing booleans.
#[cfg(feature = "skills-db")]
fn retrieval_turn_result_for_components(
    items: Vec<brassclaw_engine::memory::ComponentItem>,
) -> Result<RetrievalTurnResult, RetrievalLookupError> {
    let count = items.len();
    let rust_items = serde_json::to_value(&items)
        .map_err(|error| RetrievalLookupError::Decode(error.to_string()))?;
    Ok(RetrievalTurnResult {
        tier0_eligible: false,
        llm_call_required: true,
        orchestrator_items: rust_items.clone(),
        rust_items,
        routing_meta: serde_json::json!({ "variant": "components", "count": count }),
        instruction: serde_json::json!(null),
    })
}

/// Serialize disambiguation candidates into a conservative (un-split) E.0
/// `RetrievalTurnResult` so the Phase-H consumer can surface a disambiguation
/// prompt (spec §3.12 Q11).
#[cfg(feature = "skills-db")]
fn retrieval_turn_result_for_disambiguation(
    candidates: Vec<brassclaw_engine::memory::intent_system::IntentCandidate>,
) -> Result<RetrievalTurnResult, RetrievalLookupError> {
    let count = candidates.len();
    let orchestrator_items = serde_json::to_value(&candidates)
        .map_err(|error| RetrievalLookupError::Decode(error.to_string()))?;
    Ok(RetrievalTurnResult {
        tier0_eligible: false,
        llm_call_required: true,
        rust_items: orchestrator_items.clone(),
        orchestrator_items,
        routing_meta: serde_json::json!({ "variant": "disambiguation", "count": count }),
        instruction: serde_json::json!(null),
    })
}

/// Serialize an `ActionShortCircuit` (class-16 intent match) into a
/// `RetrievalTurnResult` with REAL Tier-0 routing booleans (Q5→A): the action
/// executes directly with no LLM, so `tier0_eligible = true` and
/// `llm_call_required = false`. The action identity is carried in
/// `orchestrator_items` (a single `action_short_circuit` descriptor) and
/// `routing_meta.variant == "action_short_circuit"` so the Phase-H consumer can
/// discriminate it (consistent with how `Disambiguation` is encoded). Always
/// `Some` — an Action match is a real short-circuit, never an empty fall-through.
#[cfg(feature = "skills-db")]
fn retrieval_turn_result_for_action_short_circuit(
    component_id: uuid::Uuid,
    name: String,
) -> RetrievalTurnResult {
    let id_str = component_id.to_string();
    RetrievalTurnResult {
        tier0_eligible: true,
        llm_call_required: false,
        rust_items: serde_json::json!([]),
        orchestrator_items: serde_json::json!([
            { "type": "action_short_circuit", "component_id": id_str, "name": name.clone() }
        ]),
        routing_meta: serde_json::json!({
            "variant": "action_short_circuit",
            "component_id": id_str,
            "name": name,
        }),
        instruction: serde_json::json!(null),
    }
}

/// Serialize a `SplitResult` (class-21 recipe intent match with a `step_link`)
/// into a `RetrievalTurnResult` with REAL routing booleans from
/// `TurnRoutingSignals` (replacing E.0's conservative `tier0_eligible = false` /
/// `llm_call_required = true`) and the IBS-split `rust_items` / `orchestrator_
/// items` channels. `routing_meta.variant == "split"` carries the full routing
/// context (variant_label, step_link, wilson_lower, tier0_eligible,
/// llm_call_required, override_prompt_creation, matched_component_ids) so the
/// Phase-H `LoopOrchestratorPort` consumer can dispatch Tier-0/Tier-1. Always
/// `Some` — a recipe match with a `step_link` is a real routing decision even
/// when both channels are empty (the routing booleans still govern the turn).
///
/// The compiled `BuildInstruction` (subplan §7.5 upgrade) is serialized into
/// `instruction` — `null` on the `build_instruction` soft-fail (§7.4), the
/// full per-step structure (with `{{vars.name}}`-substituted `tool_bindings`)
/// on a successful compile. Serialized here at the composition boundary so
/// `brassclaw_turns` stays decoupled from the engine IBS types.
#[cfg(feature = "skills-db")]
fn retrieval_turn_result_for_split(
    rust_items: Vec<brassclaw_engine::memory::ComponentItem>,
    orchestrator_items: Vec<brassclaw_engine::memory::ComponentItem>,
    routing: brassclaw_engine::memory::TurnRoutingSignals,
    instruction: Option<Box<brassclaw_engine::memory::instruction_builder::BuildInstruction>>,
) -> Result<RetrievalTurnResult, RetrievalLookupError> {
    let rust_json = serde_json::to_value(&rust_items)
        .map_err(|error| RetrievalLookupError::Decode(error.to_string()))?;
    let orchestrator_json = serde_json::to_value(&orchestrator_items)
        .map_err(|error| RetrievalLookupError::Decode(error.to_string()))?;
    let instruction_json = serde_json::to_value(&instruction)
        .map_err(|error| RetrievalLookupError::Decode(error.to_string()))?;
    Ok(RetrievalTurnResult {
        tier0_eligible: routing.tier0_eligible,
        llm_call_required: routing.llm_call_required,
        rust_items: rust_json,
        orchestrator_items: orchestrator_json,
        routing_meta: serde_json::json!({
            "variant": "split",
            "variant_label": routing.variant_label,
            "step_link": routing.step_link,
            "wilson_lower": routing.wilson_lower,
            "tier0_eligible": routing.tier0_eligible,
            "llm_call_required": routing.llm_call_required,
            "override_prompt_creation": routing.override_prompt_creation,
            "matched_component_ids": routing.matched_component_ids,
        }),
        instruction: instruction_json,
    })
}

// ---------------------------------------------------------------------------
// SkillActivationMessageTextResolver — messages_by_run-backed resolver
// ---------------------------------------------------------------------------

/// `MessageTextResolver` backed by the skill-activation
/// `SelectableSkillContextSource` (v3 Phase E.0 / plan §H3).
///
/// Generic over the bundle source `S` so the resolver can be unit-tested with
/// a trivial mock `SkillBundleSource` (no filesystem, no Postgres) while the
/// production wiring instantiates it with the local-dev
/// `FilesystemSkillBundleSource<LocalDevRootFilesystem>` substrate — type
/// inference + the `Arc<dyn MessageTextResolver>` upcast keep the call site
/// unchanged. `peek_message_text` only touches the `messages_by_run` store, so
/// the bundle source is never consulted on the resolver path.
///
/// `resolve_message_text` reads the **raw** accepted-message body recorded for
/// the turn's `accepted_message_ref` via the non-consuming `peek_message_text`
/// accessor, so intent matching is not corrupted by `[redacted]` placeholders.
/// Returns `Ok(None)` (soft miss → Tier-2 fall-through) when the turn carries
/// no accepted message. Not engine-gated: raw-text resolution needs no Postgres.
pub(crate) struct SkillActivationMessageTextResolver<S>
where
    S: brassclaw_loop_support::SkillBundleSource + ?Sized,
{
    source: Arc<brassclaw_first_party_extension_ports::SelectableSkillContextSource<S>>,
}

impl<S> SkillActivationMessageTextResolver<S>
where
    S: brassclaw_loop_support::SkillBundleSource + ?Sized,
{
    pub(crate) fn new(
        source: Arc<brassclaw_first_party_extension_ports::SelectableSkillContextSource<S>>,
    ) -> Self {
        Self { source }
    }
}

#[async_trait]
impl<S> MessageTextResolver for SkillActivationMessageTextResolver<S>
where
    S: brassclaw_loop_support::SkillBundleSource + ?Sized,
{
    async fn resolve_message_text(
        &self,
        context: &LoopRunContext,
        _message_ref: &brassclaw_turns::LoopMessageRef,
    ) -> Result<Option<String>, AgentLoopHostError> {
        let Some(accepted_message_ref) = context.accepted_message_ref.as_ref() else {
            // No accepted message recorded for this turn — soft miss so the
            // caller (`InputStage::drain`) leaves `last_user_text = None` and
            // falls through to Tier-2 rather than failing the whole turn.
            return Ok(None);
        };
        self.source
            .peek_message_text(&context.scope, accepted_message_ref)
            .map_err(|error| {
                AgentLoopHostError::new(AgentLoopHostErrorKind::Internal, error.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageTextResolver, SkillActivationMessageTextResolver};
    use async_trait::async_trait;
    use std::sync::Arc;

    use brassclaw_first_party_extension_ports::{
        SelectableSkillContextSource, SkillActivationSelectorConfig,
    };
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
    use brassclaw_loop_support::{
        SkillBundleDescriptor, SkillBundleId, SkillBundleSource, SkillBundleSourceError,
        SkillFilePath,
    };
    use brassclaw_turns::{
        AcceptedMessageRef, LoopMessageRef, TurnActor, TurnId, TurnRunId, TurnScope,
        run_profile::{
            InMemoryRunProfileResolver, LoopRunContext, RunProfileResolutionRequest,
            RunProfileResolver,
        },
    };

    // skills-db-only imports: PgRetrievalLookup + engine intent + mapping helpers.
    #[cfg(feature = "skills-db")]
    use super::{
        PgRetrievalLookup, retrieval_turn_result_for_action_short_circuit,
        retrieval_turn_result_for_components, retrieval_turn_result_for_disambiguation,
        retrieval_turn_result_for_split,
    };
    #[cfg(feature = "skills-db")]
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentCandidate, IntentScope, IntentSource, seed_intent_input,
    };
    #[cfg(feature = "skills-db")]
    use brassclaw_engine::memory::{ComponentItem, PostgresSource, TurnRoutingSignals};
    #[cfg(feature = "skills-db")]
    use brassclaw_turns::run_profile::RetrievalLookup;
    #[cfg(feature = "skills-db")]
    use uuid::Uuid;

    // -------------------------------------------------------------------------
    // Mock SkillBundleSource — the resolver never consults it (peek_message_text
    // only touches messages_by_run), so the trait impls are trivial stubs.
    // -------------------------------------------------------------------------

    struct MockBundleSource;

    #[async_trait]
    impl SkillBundleSource for MockBundleSource {
        async fn list_skill_bundles(
            &self,
            _run_context: &LoopRunContext,
        ) -> Result<Vec<SkillBundleDescriptor>, SkillBundleSourceError> {
            Ok(Vec::new())
        }

        async fn read_skill_bundle_file(
            &self,
            _run_context: &LoopRunContext,
            _bundle_id: &SkillBundleId,
            _path: &SkillFilePath,
        ) -> Result<Vec<u8>, SkillBundleSourceError> {
            Err(SkillBundleSourceError::FileNotFound)
        }
    }

    fn mock_selectable() -> Arc<SelectableSkillContextSource<MockBundleSource>> {
        Arc::new(SelectableSkillContextSource::new(
            Arc::new(MockBundleSource),
            SkillActivationSelectorConfig::default(),
        ))
    }

    /// Build a `LoopRunContext` for the resolver tests. `accepted_message = None`
    /// leaves `accepted_message_ref` unset (the soft-miss path).
    async fn resolver_context(accepted_message: Option<&str>) -> LoopRunContext {
        let resolved = InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .expect("in-memory run profile resolves");
        let scope = TurnScope::new(
            TenantId::new("tenant-a").expect("tenant id"),
            Some(AgentId::new("agent-a").expect("agent id")),
            Some(ProjectId::new("project-a").expect("project id")),
            ThreadId::new("thread-a").expect("thread id"),
        );
        let context = LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), resolved)
            .with_actor(TurnActor::new(UserId::new("user-a").expect("user id")));
        match accepted_message {
            Some(message) => context.with_accepted_message_ref(
                AcceptedMessageRef::new(message).expect("accepted message ref"),
            ),
            None => context,
        }
    }

    // -------------------------------------------------------------------------
    // Resolver tests — no Postgres, run locally under default features.
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn resolver_returns_raw_recorded_message_text() {
        // plan §H3: the resolver surfaces the raw (unsanitized) accepted-message
        // body so intent matching is not corrupted by `[redacted]` placeholders.
        let selectable = mock_selectable();
        let context = resolver_context(Some("accepted-raw")).await;
        let raw = "please run the daily sync with secret hunter2";
        selectable
            .record_user_message(
                context.scope.clone(),
                context
                    .accepted_message_ref
                    .clone()
                    .expect("accepted message ref present"),
                raw,
            )
            .expect("record user message");

        let resolver = SkillActivationMessageTextResolver::new(Arc::clone(&selectable));
        let resolved = resolver
            .resolve_message_text(
                &context,
                &LoopMessageRef::new("msg:ignored").expect("loop message ref"),
            )
            .await
            .expect("resolve_message_text");

        assert_eq!(resolved.as_deref(), Some(raw));
        // Non-consuming: a second resolution still returns the raw text.
        let again = resolver
            .resolve_message_text(
                &context,
                &LoopMessageRef::new("msg:ignored").expect("loop message ref"),
            )
            .await
            .expect("resolve_message_text again");
        assert_eq!(again.as_deref(), Some(raw));
    }

    #[tokio::test]
    async fn resolver_soft_misses_when_no_accepted_message_ref() {
        // No accepted message recorded for the turn → Ok(None) so the caller
        // (`InputStage::drain`) leaves `last_user_text = None` and falls through
        // to Tier-2 instead of failing the whole turn.
        let resolver = SkillActivationMessageTextResolver::new(mock_selectable());
        let context = resolver_context(None).await;
        let resolved = resolver
            .resolve_message_text(
                &context,
                &LoopMessageRef::new("msg:ignored").expect("loop message ref"),
            )
            .await
            .expect("resolve_message_text");
        assert!(
            resolved.is_none(),
            "no accepted_message_ref must soft-miss (Ok(None))"
        );
    }

    #[tokio::test]
    async fn resolver_soft_misses_for_unrecorded_ref() {
        // accepted_message_ref is set but no message was recorded for it
        // (`peek_message_text` miss) → Ok(None) soft miss.
        let resolver = SkillActivationMessageTextResolver::new(mock_selectable());
        let context = resolver_context(Some("accepted-never-written")).await;
        let resolved = resolver
            .resolve_message_text(
                &context,
                &LoopMessageRef::new("msg:ignored").expect("loop message ref"),
            )
            .await
            .expect("resolve_message_text");
        assert!(
            resolved.is_none(),
            "unrecorded accepted_message_ref must soft-miss (Ok(None))"
        );
    }

    // -------------------------------------------------------------------------
    // Mapping-helper unit tests — no Postgres, run locally under `skills-db`.
    // Exercise the ComponentItem / IntentCandidate → RetrievalTurnResult
    // serialization + conservative E.0 routing booleans directly.
    // -------------------------------------------------------------------------

    #[cfg(feature = "skills-db")]
    #[test]
    fn components_mapping_serializes_items_with_conservative_booleans() {
        let items = vec![ComponentItem {
            id: Uuid::nil(),
            class_code: 16,
            prompt_uid: 1,
            name: "daily-sync".to_string(),
            description: "sync description".to_string(),
            effective_content: "sync prior knowledge".to_string(),
            override_prompt_creation: true,
            // Prompt-assembly mapping fixture; executable steps not exercised
            // here (Q-G-STUB1).
            steps: None,
            allowed_tools: None,
        }];
        let result = retrieval_turn_result_for_components(items).expect("map components");
        // E.0 conservative routing booleans (Phase E's SplitResult replaces them).
        assert!(!result.tier0_eligible);
        assert!(result.llm_call_required);
        // E.0: rust_items == orchestrator_items (un-split).
        assert_eq!(result.rust_items, result.orchestrator_items);
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("components")
        );
        assert_eq!(
            result.routing_meta.get("count").and_then(|c| c.as_i64()),
            Some(1)
        );
        assert_eq!(result.orchestrator_items.as_array().map(Vec::len), Some(1));
    }

    #[cfg(feature = "skills-db")]
    #[test]
    fn disambiguation_mapping_serializes_candidates_with_conservative_booleans() {
        let candidates = vec![
            IntentCandidate {
                row_id: Uuid::nil(),
                component_id: Uuid::nil(),
                component_class_code: 16,
                input_class: 3,
                score: 1,
                class_label: "action".to_string(),
            },
            IntentCandidate {
                row_id: Uuid::nil(),
                component_id: Uuid::new_v4(),
                component_class_code: 16,
                input_class: 3,
                score: 1,
                class_label: "action".to_string(),
            },
        ];
        let result =
            retrieval_turn_result_for_disambiguation(candidates).expect("map disambiguation");
        assert!(!result.tier0_eligible);
        assert!(result.llm_call_required);
        assert_eq!(result.rust_items, result.orchestrator_items);
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("disambiguation")
        );
        assert_eq!(
            result.routing_meta.get("count").and_then(|c| c.as_i64()),
            Some(2)
        );
    }

    #[cfg(feature = "skills-db")]
    #[test]
    fn action_short_circuit_mapping_emits_real_tier0_booleans() {
        // Q5→A: an Action (class-16) short-circuit runs Tier-0 with no LLM.
        let id = Uuid::new_v4();
        let result = retrieval_turn_result_for_action_short_circuit(id, "daily-sync".to_string());
        assert!(result.tier0_eligible, "action short-circuit is Tier-0");
        assert!(
            !result.llm_call_required,
            "action short-circuit needs no LLM"
        );
        assert!(
            result
                .rust_items
                .as_array()
                .map(Vec::is_empty)
                .unwrap_or(false),
            "rust channel is empty for an action short-circuit"
        );
        // The action descriptor rides in orchestrator_items.
        let orch = result
            .orchestrator_items
            .as_array()
            .expect("orchestrator_items is an array");
        assert_eq!(orch.len(), 1);
        assert_eq!(
            orch[0].get("type").and_then(|v| v.as_str()),
            Some("action_short_circuit")
        );
        assert_eq!(
            orch[0].get("component_id").and_then(|v| v.as_str()),
            Some(id.to_string()).as_deref()
        );
        assert_eq!(
            orch[0].get("name").and_then(|v| v.as_str()),
            Some("daily-sync")
        );
        // routing_meta discriminates the variant + carries the identity.
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("action_short_circuit")
        );
        assert_eq!(
            result
                .routing_meta
                .get("component_id")
                .and_then(|v| v.as_str()),
            Some(id.to_string()).as_deref()
        );
        assert_eq!(
            result.routing_meta.get("name").and_then(|v| v.as_str()),
            Some("daily-sync")
        );
    }

    #[cfg(feature = "skills-db")]
    #[test]
    fn split_mapping_propagates_routing_booleans_and_split_channels() {
        // SplitResult surfaces REAL routing booleans (replacing E.0's conservative
        // false/true) + the IBS-split channels + a `split` routing_meta.
        let rust_items = vec![ComponentItem {
            id: Uuid::new_v4(),
            class_code: 13, // tool_skill — Rust channel
            prompt_uid: 1,
            name: "shell-run".to_string(),
            description: String::new(),
            effective_content: "rust body".to_string(),
            override_prompt_creation: false,
            // IBS-split mapping fixture; executable steps not exercised here
            // (Q-G-STUB1).
            steps: None,
            allowed_tools: None,
        }];
        let orch_items = vec![
            ComponentItem {
                id: Uuid::new_v4(),
                class_code: 2, // skill — orchestrator channel
                prompt_uid: 2,
                name: "planner".to_string(),
                description: String::new(),
                effective_content: "orch body a".to_string(),
                override_prompt_creation: false,
                steps: None,
                allowed_tools: None,
            },
            ComponentItem {
                id: Uuid::new_v4(),
                class_code: 22, // python_code — orchestrator channel
                prompt_uid: 3,
                name: "formatter".to_string(),
                description: String::new(),
                effective_content: "orch body b".to_string(),
                override_prompt_creation: false,
                steps: None,
                allowed_tools: None,
            },
        ];
        let matched_ids: Vec<String> = orch_items.iter().map(|i| i.id.to_string()).collect();
        let routing = TurnRoutingSignals {
            override_prompt_creation: true,
            matched_component_ids: matched_ids.clone(),
            variant_label: "v1".to_string(),
            step_link: "recipe.daily-sync#v1".to_string(),
            llm_call_required: false,
            wilson_lower: 0.82,
            tier0_eligible: true,
            recipe_id: None,
        };
        let instruction = Some(Box::new(
            brassclaw_engine::memory::instruction_builder::BuildInstruction {
                llm_call_required: false,
                variable_patterns: Vec::new(),
                basic_prompt_section_refs: Vec::new(),
                rust_steps: Vec::new(),
                orchestrator_steps: Vec::new(),
            },
        ));
        let result = retrieval_turn_result_for_split(rust_items, orch_items, routing, instruction)
            .expect("map split");

        // Real booleans propagated from routing.
        assert!(result.tier0_eligible);
        assert!(!result.llm_call_required);
        // Channels are split (not duplicated as in E.0).
        assert_eq!(result.rust_items.as_array().map(Vec::len), Some(1));
        assert_eq!(result.orchestrator_items.as_array().map(Vec::len), Some(2));
        assert_ne!(result.rust_items, result.orchestrator_items);
        // The compiled BuildInstruction is serialized through (§7.5 upgrade).
        assert!(result.instruction.is_object());
        assert!(result.instruction.get("rust_steps").is_some());
        // routing_meta carries the full routing context.
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("split")
        );
        assert_eq!(
            result
                .routing_meta
                .get("variant_label")
                .and_then(|v| v.as_str()),
            Some("v1")
        );
        assert_eq!(
            result
                .routing_meta
                .get("step_link")
                .and_then(|v| v.as_str()),
            Some("recipe.daily-sync#v1")
        );
        assert_eq!(
            result
                .routing_meta
                .get("wilson_lower")
                .and_then(|v| v.as_f64()),
            Some(0.82)
        );
        assert_eq!(
            result
                .routing_meta
                .get("override_prompt_creation")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            result.routing_meta.get("matched_component_ids"),
            Some(&serde_json::to_value(&matched_ids).expect("serialize ids"))
        );
    }

    // -------------------------------------------------------------------------
    // PgRetrievalLookup end-to-end tests — real Postgres-16 testcontainer.
    // Skip cleanly when docker/testcontainers is unavailable; run in CI.
    // -------------------------------------------------------------------------

    /// A unique sentence-class query (≥5 whitespace tokens, no terminal
    /// punctuation) so `classify_query` → `Sentence` and `match_order = [3,2,1]`;
    /// the seeded `input_class = Sentence` is therefore in the `ANY($6)` set.
    #[cfg(feature = "skills-db")]
    fn unique_sentence_query() -> String {
        format!("trigger the recipe intent match {}", Uuid::new_v4())
    }

    /// Insert a minimal, `validation_status = 'validated'` class-16
    /// `reborn_actions` row so `fetch_component_by_id` (which gates on
    /// `validation_status = 'validated'` and `'05:validator' != ALL(consumer_tags)`)
    /// returns it. `consumer_tags` defaults to `'{}'` (vacuously passes the
    /// validator-tag gate). UUID-derived name keeps parallel runs off the
    /// `UNIQUE(scope, name)` constraint.
    #[cfg(feature = "skills-db")]
    async fn insert_validated_action_row(
        pool: &brassclaw_pg::PgPool,
        scope: &IntentScope,
        id: Uuid,
        name: &str,
    ) {
        let client = pool.get().await.expect("pool client");
        client
            .execute(
                "INSERT INTO reborn_actions
                     (id, tenant_id, user_id, agent_id, project_id,
                      name, description, validation_status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,'validated')",
                &[
                    &id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &name,
                    &"validated test action description",
                ],
            )
            .await
            .expect("insert validated reborn_actions row");
    }

    /// Build a `LoopRunContext` whose `build_component_scope` projection matches
    /// the seeded `IntentScope`. `build_component_scope` sets `tenant_id` =
    /// `user_id` = the actor's user id, `agent_id` = `scope.agent_id`,
    /// `project_id` = `scope.project_id`; `scope.tenant_id` is unused (Phase-F
    /// stub).
    #[cfg(feature = "skills-db")]
    async fn lookup_context(user: &str, agent: &str, project: &str) -> LoopRunContext {
        let resolved = InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .expect("in-memory run profile resolves");
        let scope = TurnScope::new(
            TenantId::new("tenant-lookup").expect("tenant id"),
            Some(AgentId::new(agent).expect("agent id")),
            Some(ProjectId::new(project).expect("project id")),
            ThreadId::new("thread-lookup").expect("thread id"),
        );
        LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), resolved)
            .with_actor(TurnActor::new(UserId::new(user).expect("user id")))
    }

    #[cfg(feature = "skills-db")]
    fn fresh_intent_scope(user: &str, agent: &str, project: &str) -> IntentScope {
        IntentScope {
            tenant_id: user.to_string(),
            user_id: user.to_string(),
            agent_id: agent.to_string(),
            project_id: project.to_string(),
        }
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test]
    async fn pg_retrieval_lookup_returns_components_for_validated_action() {
        let rig = match crate::runtime::test_pg::pg_rig().await {
            Some(r) => r,
            None => return,
        };
        let _guard = rig.lock_db().await;
        let pool = Arc::clone(&rig.pool);

        let user = Uuid::new_v4().to_string();
        let agent = Uuid::new_v4().to_string();
        let project = Uuid::new_v4().to_string();
        let intent_scope = fresh_intent_scope(&user, &agent, &project);
        let query = unique_sentence_query();
        let action_id = Uuid::new_v4();
        let action_name = format!("daily-sync-{}", Uuid::new_v4().simple());

        insert_validated_action_row(pool.as_ref(), &intent_scope, action_id, &action_name).await;
        seed_intent_input(
            pool.as_ref(),
            &intent_scope,
            &query,
            InputClass::Sentence,
            action_id,
            16,
            IntentSource::Seeded,
            None,
        )
        .await
        .expect("seed intent input");

        let context = lookup_context(&user, &agent, &project).await;
        let lookup = PgRetrievalLookup::new(Arc::new(PostgresSource::new(Arc::clone(&pool))));
        let result = lookup
            .fetch_for_turn(&context, &query, 4096, "02")
            .await
            .expect("fetch_for_turn")
            .expect("expected Components, got soft-miss None");

        assert!(
            !result.tier0_eligible,
            "E.0 conservative: tier0_eligible false"
        );
        assert!(
            result.llm_call_required,
            "E.0 conservative: llm_call_required true"
        );
        assert_eq!(
            result.rust_items, result.orchestrator_items,
            "E.0: rust_items == orchestrator_items (un-split)"
        );
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("components")
        );
        assert_eq!(
            result.routing_meta.get("count").and_then(|c| c.as_i64()),
            Some(1)
        );
        assert_eq!(result.orchestrator_items.as_array().map(Vec::len), Some(1));
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test]
    async fn pg_retrieval_lookup_returns_disambiguation_for_two_near_equal_inputs() {
        let rig = match crate::runtime::test_pg::pg_rig().await {
            Some(r) => r,
            None => return,
        };
        let _guard = rig.lock_db().await;
        let pool = Arc::clone(&rig.pool);

        let user = Uuid::new_v4().to_string();
        let agent = Uuid::new_v4().to_string();
        let project = Uuid::new_v4().to_string();
        let intent_scope = fresh_intent_scope(&user, &agent, &project);
        let query = unique_sentence_query();
        // Two same-text intent inputs, different component_ids, same class+score
        // → resolve_intent returns Disambiguation (DISAMBIGUATION_SPREAD = 2).
        seed_intent_input(
            pool.as_ref(),
            &intent_scope,
            &query,
            InputClass::Sentence,
            Uuid::new_v4(),
            16,
            IntentSource::Seeded,
            None,
        )
        .await
        .expect("seed intent input a");
        seed_intent_input(
            pool.as_ref(),
            &intent_scope,
            &query,
            InputClass::Sentence,
            Uuid::new_v4(),
            16,
            IntentSource::Seeded,
            None,
        )
        .await
        .expect("seed intent input b");

        let context = lookup_context(&user, &agent, &project).await;
        let lookup = PgRetrievalLookup::new(Arc::new(PostgresSource::new(Arc::clone(&pool))));
        let result = lookup
            .fetch_for_turn(&context, &query, 4096, "02")
            .await
            .expect("fetch_for_turn")
            .expect("expected Disambiguation, got soft-miss None");

        assert!(!result.tier0_eligible);
        assert!(result.llm_call_required);
        assert_eq!(result.rust_items, result.orchestrator_items);
        assert_eq!(
            result.routing_meta.get("variant").and_then(|v| v.as_str()),
            Some("disambiguation")
        );
        assert_eq!(
            result.routing_meta.get("count").and_then(|c| c.as_i64()),
            Some(2)
        );
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test]
    async fn pg_retrieval_lookup_soft_misses_when_nothing_matches() {
        let rig = match crate::runtime::test_pg::pg_rig().await {
            Some(r) => r,
            None => return,
        };
        let _guard = rig.lock_db().await;
        let pool = Arc::clone(&rig.pool);

        // Fresh unique scope with no seeded components → resolve_intent NoMatch →
        // fallback fetch_for_consumer (full UNION ALL) finds no validated rows in
        // this scope → Components([]) → soft-miss None.
        let user = Uuid::new_v4().to_string();
        let agent = Uuid::new_v4().to_string();
        let project = Uuid::new_v4().to_string();
        let query = unique_sentence_query();

        let context = lookup_context(&user, &agent, &project).await;
        let lookup = PgRetrievalLookup::new(Arc::new(PostgresSource::new(Arc::clone(&pool))));
        let result = lookup
            .fetch_for_turn(&context, &query, 4096, "02")
            .await
            .expect("fetch_for_turn");
        assert!(
            result.is_none(),
            "fresh scope with no components must soft-miss (Ok(None))"
        );
    }
}
