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
//! - **`proposed_recipe_updates`** — each blob is expected to carry
//!   `"name"` and `"description"` strings and the supported Recipe constructor
//!   fields, including IBS descriptions, variants and dependencies. Unknown
//!   fields and malformed types are rejected, never silently projected away.
//!
//! - **`proposed_intent_examples`** — each blob carries at minimum an
//!   `"input"` string (the example text).  The example is stored as a
//!   class-21 recipe row with `source = "sempai_intent_proposal"` and
//!   the raw blob serialised into the `intent_examples` JSONB column.
//!   Once an operator validates the row the WebUI handler can seed the
//!   content into `reborn_intent_inputs`.
//!
//! - **`proposed_components`** (§0.23.6) — each blob carries a `class_code`
//!   and a raw JSON payload.  The sink dispatches each entry to the correct
//!   class table. Unsupported class codes are reported without inserting a row.
//!
//! This is the legacy pending-row/queue adapter, not immutable v3 review
//! admission or approval. Nested authoring contracts still need Q1 and human Q2.

#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_interceptor::{
        ComponentProposal, InterceptorError, ProposalSubmitResult, SempaiProposalSink,
    };
    use brassclaw_pg::PgPool;
    use serde::Deserialize;
    use serde_json::Value;
    use tracing::{debug, warn};
    use uuid::Uuid;

    use crate::pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore};
    use crate::pg_recipe_store::{NewPgRecipe, PgRecipeStore};
    use crate::validation_queue::ValidationQueueStore;

    // Only authorable constructor fields cross this boundary. Scope, identity,
    // source, status, approval, tier and integrity checksums belong to the host.
    // Structured field contents remain candidate data for Q1, not executable
    // source or evidence that their schema/behavior has passed review.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RecipeProposal {
        name: String,
        description: String,
        #[serde(default, deserialize_with = "present_json")]
        trigger: Option<Value>,
        #[serde(default)]
        steps: Vec<Value>,
        prior_knowledge_content: Option<String>,
        #[serde(default)]
        override_prompt_creation: bool,
        #[serde(default)]
        consumer_tags: Vec<String>,
        #[serde(default, deserialize_with = "present_json")]
        intent_examples: Option<Value>,
        #[serde(default, deserialize_with = "present_json")]
        step_descriptions: Option<Value>,
        #[serde(default, deserialize_with = "present_json")]
        variants: Option<Value>,
        #[serde(default, deserialize_with = "present_json")]
        dependency_registry: Option<Value>,
        validates_class_code: Option<i16>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PythonCodeProposal {
        name: String,
        description: String,
        content: String,
        prior_knowledge_content: Option<String>,
        #[serde(default)]
        override_prompt_creation: bool,
        #[serde(default)]
        consumer_tags: Vec<String>,
        #[serde(default, deserialize_with = "present_json")]
        intent_examples: Option<Value>,
        #[serde(default, deserialize_with = "present_json")]
        dependency_registry: Option<Value>,
        #[serde(default)]
        includes: Vec<Uuid>,
    }

    // SQL NULL (absent field) and a present JSON null remain distinguishable.
    fn present_json<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Value>, D::Error> {
        Value::deserialize(deserializer).map(Some)
    }

    fn pending_tags(mut tags: Vec<String>) -> Vec<String> {
        if !tags.iter().any(|tag| tag == "05:validator") {
            tags.push("05:validator".into());
        }
        tags
    }

    /// Postgres-backed [`SempaiProposalSink`].
    ///
    /// Constructed from a shared [`PgPool`] and the fixed scope identifiers
    /// (`tenant_id`, `agent_id`) that are baked into the runtime at startup.
    /// Per-call `user_id` / `project_id` are provided by the caller.
    ///
    /// All proposals are non-builtin — they must go through the Q1→Q2
    /// validation queue before becoming usable. `create_and_submit` on the
    /// recipe/python_code stores commits the draft and its state-1 queue entry
    /// together. No passing review or activation is recorded here.
    #[derive(Clone)]
    pub(crate) struct PgSempaiProposalSink {
        recipe_store: PgRecipeStore,
        python_code_store: PgPythonCodeStore,
        queue_store: ValidationQueueStore,
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
            let tenant_id = tenant_id.into();
            let agent_id = agent_id.into();
            Self {
                recipe_store: PgRecipeStore::new(Arc::clone(&pool)),
                python_code_store: PgPythonCodeStore::new(Arc::clone(&pool)),
                queue_store: ValidationQueueStore::new(Arc::clone(&pool)),
                tenant_id,
                agent_id,
            }
        }

        /// Insert a recipe-class (21) proposal row and enqueue it for Q1.
        ///
        /// Uses `create_and_submit` so the row is immediately tracked by
        /// `reborn_validation_queue` (state 1). The row carries
        /// `05:validator` in `consumer_tags`; pending validation status prevents
        /// ordinary delivery. Tags alone are never approval evidence.
        async fn insert_recipe_proposal(
            &self,
            blob: &serde_json::Value,
            user_id: &str,
            project_id: &str,
            source: &str,
        ) -> bool {
            let Ok(proposal) = RecipeProposal::deserialize(blob) else {
                // Parser diagnostics can contain candidate values: don't log
                // private model/source data just to report a rejected shape.
                warn!("sempai_proposal: unsupported recipe fields or malformed field types");
                return false;
            };

            let row = NewPgRecipe {
                tenant_id: self.tenant_id.clone(),
                user_id: user_id.to_string(),
                agent_id: self.agent_id.clone(),
                project_id: project_id.to_string(),
                name: proposal.name,
                description: proposal.description,
                trigger: proposal.trigger,
                steps: Value::Array(proposal.steps),
                prior_knowledge_content: proposal.prior_knowledge_content,
                override_prompt_creation: proposal.override_prompt_creation,
                consumer_tags: pending_tags(proposal.consumer_tags),
                intent_examples: proposal.intent_examples,
                source: source.to_string(),
                step_descriptions: proposal.step_descriptions,
                variants: proposal.variants,
                dependency_registry: proposal.dependency_registry,
                validates_class_code: proposal.validates_class_code,
            };

            match self
                .recipe_store
                .create_and_submit(row, &self.queue_store)
                .await
            {
                Ok(id) => {
                    debug!(%id, "sempai_proposal: recipe row inserted and submitted to Q1");
                    true
                }
                Err(err) => {
                    warn!(error = %err, "sempai_proposal: recipe draft transaction failed");
                    false
                }
            }
        }

        /// Insert a python_code-class (22) proposal row and enqueue it for Q1.
        ///
        /// Uses `create_and_submit` so the row is immediately tracked by
        /// `reborn_validation_queue` (state 1).
        async fn insert_python_code_proposal(
            &self,
            blob: &serde_json::Value,
            user_id: &str,
            project_id: &str,
        ) -> bool {
            let Ok(proposal) = PythonCodeProposal::deserialize(blob) else {
                warn!("sempai_proposal: unsupported PythonCode fields or malformed field types");
                return false;
            };

            let row = NewPgPythonCode {
                tenant_id: self.tenant_id.clone(),
                user_id: user_id.to_string(),
                agent_id: self.agent_id.clone(),
                project_id: project_id.to_string(),
                name: proposal.name,
                description: proposal.description,
                content: proposal.content,
                prior_knowledge_content: proposal.prior_knowledge_content,
                override_prompt_creation: proposal.override_prompt_creation,
                source: "sempai_proposal".to_string(),
                consumer_tags: pending_tags(proposal.consumer_tags),
                intent_examples: proposal.intent_examples,
                dependency_registry: proposal.dependency_registry,
                includes: proposal.includes,
                content_checksum: None,
            };

            match self
                .python_code_store
                .create_and_submit(row, &self.queue_store)
                .await
            {
                Ok(id) => {
                    debug!(%id, "sempai_proposal: python_code row inserted and submitted to Q1");
                    true
                }
                Err(err) => {
                    warn!(error = %err, "sempai_proposal: PythonCode draft transaction failed");
                    false
                }
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
            proposed_components: &[ComponentProposal],
        ) -> Result<ProposalSubmitResult, InterceptorError> {
            let mut recipe_updates_queued: u32 = 0;
            let mut intent_examples_queued: u32 = 0;
            let mut components_queued: u32 = 0;

            // ── Recipe/skill update proposals ────────────────────────────
            for blob in proposed_recipe_updates {
                if self
                    .insert_recipe_proposal(blob, user_id, project_id, "sempai_proposal")
                    .await
                {
                    recipe_updates_queued += 1;
                }
            }

            // ── Intent-example proposals ─────────────────────────────────
            for (idx, blob) in proposed_intent_examples.iter().enumerate() {
                if blob
                    .get("input")
                    .and_then(|v| v.as_str())
                    .is_none_or(str::is_empty)
                {
                    warn!(
                        idx,
                        "sempai_proposal: proposed_intent_example missing input — skipped"
                    );
                    continue;
                }

                // Store as a class-21 recipe row with the example blob in
                // intent_examples JSONB.  The operator validates the row and
                // the WebUI handler can then seed it into reborn_intent_inputs.
                // Recipe names are ASCII slugs of at most 64 characters. The
                // example is data, not an identifier or shortened description.
                let synthetic_blob = serde_json::json!({
                    "name": format!("intent-proposal-{}", uuid::Uuid::new_v4()),
                    "description": "Sempai-proposed intent example",
                    "intent_examples": [blob],
                });
                if self
                    .insert_recipe_proposal(
                        &synthetic_blob,
                        user_id,
                        project_id,
                        "sempai_intent_proposal",
                    )
                    .await
                {
                    intent_examples_queued += 1;
                }
            }

            // ── Generalised multi-class proposals (§0.23.6) ─────────────
            for (idx, proposal) in proposed_components.iter().enumerate() {
                let queued = match proposal.class_code {
                    // class 21: Recipe
                    21 => {
                        self.insert_recipe_proposal(
                            &proposal.payload,
                            user_id,
                            project_id,
                            "sempai_proposal",
                        )
                        .await
                    }
                    // class 22: PythonCode
                    22 => {
                        self.insert_python_code_proposal(&proposal.payload, user_id, project_id)
                            .await
                    }
                    other => {
                        warn!(
                            idx,
                            class_code = other,
                            "sempai_proposal: unsupported class_code — not submitted"
                        );
                        false
                    }
                };
                if queued {
                    components_queued += 1;
                }
            }

            Ok(ProposalSubmitResult {
                recipe_updates_queued,
                intent_examples_queued,
                components_queued,
            })
        }
    }
}

#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
pub(crate) use inner::PgSempaiProposalSink;

#[cfg(all(test, feature = "postgres", feature = "root-llm-provider"))]
#[path = "sempai_proposal_sink_tests.rs"]
mod tests;
