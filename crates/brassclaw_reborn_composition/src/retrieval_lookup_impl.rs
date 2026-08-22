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
        })
    }
}

/// Build the engine `ComponentScope` for a live turn from the loop run context.
///
/// Matches the Phase-1 stub convention from `orchestrator.rs:2586–2591`: real
/// tenancy / agent scoping arrives in Phase F (when `Thread` carries
/// `tenant_id` / `agent_id`). Until then `tenant_id` mirrors the effective
/// user id (the turn actor, else the explicit thread owner, else the system
/// sentinel) and `agent_id` falls back to `"default"`.
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
        tenant_id: user_id.clone(),
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
        PgRetrievalLookup, retrieval_turn_result_for_components,
        retrieval_turn_result_for_disambiguation,
    };
    #[cfg(feature = "skills-db")]
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentCandidate, IntentScope, IntentSource, seed_intent_input,
    };
    #[cfg(feature = "skills-db")]
    use brassclaw_engine::memory::{ComponentItem, PostgresSource};
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
