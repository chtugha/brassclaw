//! `PgSempaiProposalSink` — Postgres-backed implementation of
//! [`SempaiProposalSink`].
//!
//! Routes Sempai-proposed component updates and intent examples into Q1
//! (`validation_status = 'pending'`, `queue_code = 'q1_auto'`,
//! `consumer_tags = ['05:validator']`) so operators can review and validate
//! them before they are applied.
//!
//! # Proposal shapes
//!
//! - **`proposed_recipe_updates`** — each blob is expected to carry at
//!   minimum a `"name"` string and either a `"steps"` array (recipe) or
//!   a `"description"` field.  Missing or malformed blobs are skipped
//!   (logged at debug).
//!
//! - **`proposed_intent_examples`** — each blob carries at minimum an
//!   `"input"` string (the example text).  The example is stored as a
//!   class-21 recipe row with `source = "sempai_intent_proposal"` and
//!   the raw blob serialised into the `intent_examples` JSONB column.
//!   Once an operator validates the row the WebUI handler can seed the
//!   content into `reborn_intent_inputs`.

#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_interceptor::{InterceptorError, ProposalSubmitResult, SempaiProposalSink};
    use brassclaw_pg::PgPool;
    use tracing::debug;

    use crate::pg_recipe_store::{NewPgRecipe, PgRecipeStore};

    /// Postgres-backed [`SempaiProposalSink`].
    ///
    /// Constructed from a shared [`PgPool`] and the fixed scope identifiers
    /// (`tenant_id`, `agent_id`) that are baked into the runtime at startup.
    /// Per-call `user_id` / `project_id` are provided by the caller.
    #[derive(Clone)]
    pub(crate) struct PgSempaiProposalSink {
        store: PgRecipeStore,
        tenant_id: String,
        agent_id: String,
    }

    impl PgSempaiProposalSink {
        /// Create a new sink backed by `pool`.
        ///
        /// `tenant_id` and `agent_id` are the installation-level identifiers
        /// used for all rows inserted by this sink (matching the values used
        /// in `PgRecipeStoreFacade`).
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            agent_id: impl Into<String>,
        ) -> Self {
            Self {
                store: PgRecipeStore::new(pool),
                tenant_id: tenant_id.into(),
                agent_id: agent_id.into(),
            }
        }
    }

    #[async_trait]
    impl SempaiProposalSink for PgSempaiProposalSink {
        async fn submit_proposals(
            &self,
            user_id: &str,
            project_id: &str,
            proposed_recipe_updates: &[serde_json::Value],
            proposed_intent_examples: &[serde_json::Value],
        ) -> Result<ProposalSubmitResult, InterceptorError> {
            let mut recipe_updates_queued: u32 = 0;
            let mut intent_examples_queued: u32 = 0;

            // ── Recipe/skill update proposals ────────────────────────────
            for (idx, blob) in proposed_recipe_updates.iter().enumerate() {
                let name = match blob.get("name").and_then(|v| v.as_str()) {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        debug!(
                            idx,
                            "sempai_proposal: proposed_recipe_update missing name — skipped"
                        );
                        continue;
                    }
                };
                let description = blob
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let trigger = blob.get("trigger").cloned();
                let steps = blob.get("steps").cloned().unwrap_or(serde_json::json!([]));
                let prior_knowledge_content = blob
                    .get("prior_knowledge_content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let intent_examples = blob.get("intent_examples").cloned();

                let row = NewPgRecipe {
                    tenant_id: self.tenant_id.clone(),
                    user_id: user_id.to_string(),
                    agent_id: self.agent_id.clone(),
                    project_id: project_id.to_string(),
                    name,
                    description,
                    trigger,
                    steps,
                    prior_knowledge_content,
                    override_prompt_creation: false,
                    // New rows start with 05:validator so they are invisible to
                    // consumers until an operator validates them (spec §3.9).
                    consumer_tags: vec!["05:validator".to_string()],
                    intent_examples,
                    source: "sempai_proposal".to_string(),
                };

                match self.store.insert(row).await {
                    Ok(id) => {
                        debug!(%id, "sempai_proposal: recipe update queued in Q1");
                        recipe_updates_queued += 1;
                    }
                    Err(err) => {
                        debug!(error = %err, "sempai_proposal: failed to queue recipe update — skipped");
                    }
                }
            }

            // ── Intent-example proposals ─────────────────────────────────
            for (idx, blob) in proposed_intent_examples.iter().enumerate() {
                let input_text = match blob.get("input").and_then(|v| v.as_str()) {
                    Some(t) if !t.is_empty() => t.to_string(),
                    _ => {
                        debug!(
                            idx,
                            "sempai_proposal: proposed_intent_example missing input — skipped"
                        );
                        continue;
                    }
                };

                // Store as a class-21 recipe row with the example blob in
                // intent_examples JSONB.  The operator validates the row and
                // the WebUI handler can then seed it into reborn_intent_inputs.
                let row = NewPgRecipe {
                    tenant_id: self.tenant_id.clone(),
                    user_id: user_id.to_string(),
                    agent_id: self.agent_id.clone(),
                    project_id: project_id.to_string(),
                    name: format!(
                        "intent_proposal:{}",
                        &input_text[..input_text.len().min(60)]
                    ),
                    description: format!("Sempai-proposed intent example: {input_text}"),
                    trigger: None,
                    steps: serde_json::json!([]),
                    prior_knowledge_content: None,
                    override_prompt_creation: false,
                    consumer_tags: vec!["05:validator".to_string()],
                    intent_examples: Some(serde_json::json!([blob])),
                    source: "sempai_intent_proposal".to_string(),
                };

                match self.store.insert(row).await {
                    Ok(id) => {
                        debug!(%id, "sempai_proposal: intent example queued in Q1");
                        intent_examples_queued += 1;
                    }
                    Err(err) => {
                        debug!(error = %err, "sempai_proposal: failed to queue intent example — skipped");
                    }
                }
            }

            Ok(ProposalSubmitResult {
                recipe_updates_queued,
                intent_examples_queued,
            })
        }
    }
}

#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
pub(crate) use inner::PgSempaiProposalSink;
