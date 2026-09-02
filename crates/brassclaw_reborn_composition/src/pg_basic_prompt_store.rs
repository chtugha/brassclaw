//! `PgBasicPromptStore` — Postgres-backed per-scope prefix-cache store.
//!
//! Backs the `reborn_basic_prompt_store` table (V063 migration).
//! One row per `(tenant_id, user_id, agent_id, project_id)` scope.
//!
//! **Stores the full bundle text** in `bundle_json` so that per-turn Kohai and
//! Sempai calls can read it cheaply (single row fetch, no component-table
//! re-assembly).  The bundle is assembled only during [`PgBasicPromptStore::store`],
//! which is called by `do_assemble_bundle` / `regenerate_prefix` on operator demand.
//!
//! Per-turn usage pattern:
//! ```text
//! get_for_scope() → Some(entry) if !is_stale → entry.bundle → prepend to LLM call
//!                 → None or stale → minimal_base_prompt_fallback()
//! ```
//!
//! `fingerprint` = `sha256(bundle_text)` — used to detect whether a re-assembly
//! produced identical output and to skip a redundant DB write (future optimization;
//! Phase K.1 always writes).
//!
//! Feature-gated: `postgres`.

#[cfg(feature = "postgres")]
mod inner {
    use std::sync::Arc;

    use brassclaw_pg::PgPool;
    use tracing::debug;

    // -----------------------------------------------------------------------
    // Error
    // -----------------------------------------------------------------------

    /// Errors returned by [`PgBasicPromptStore`] operations.
    #[derive(Debug, thiserror::Error)]
    pub(crate) enum BasicPromptStoreError {
        #[error("pool error: {0}")]
        Pool(String),
        #[error("database error: {0}")]
        Db(String),
    }

    impl From<deadpool_postgres::PoolError> for BasicPromptStoreError {
        fn from(e: deadpool_postgres::PoolError) -> Self {
            Self::Pool(e.to_string())
        }
    }

    impl From<tokio_postgres::Error> for BasicPromptStoreError {
        fn from(e: tokio_postgres::Error) -> Self {
            Self::Db(e.to_string())
        }
    }

    // -----------------------------------------------------------------------
    // Entry DTO
    // -----------------------------------------------------------------------

    /// Metadata + bundle row returned from `reborn_basic_prompt_store`.
    #[derive(Debug, Clone)]
    pub(crate) struct BasicPromptEntry {
        #[allow(dead_code)]
        pub id: uuid::Uuid,
        /// The assembled bundle text (from `bundle_json` JSONB string value).
        /// Empty when no assembly has run yet.
        pub bundle: String,
        /// `sha256(bundle_text)` for staleness detection.
        pub fingerprint: String,
        /// `true` when a Q2 graduation has occurred since the last assembly.
        pub is_stale: bool,
        pub assembled_at: Option<chrono::DateTime<chrono::Utc>>,
        pub prewarm_last_at: Option<chrono::DateTime<chrono::Utc>>,
        #[allow(dead_code)]
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    // -----------------------------------------------------------------------
    // Store
    // -----------------------------------------------------------------------

    /// Postgres-backed store for the Sempai/Kohai prefix bundle.
    #[derive(Clone)]
    pub(crate) struct PgBasicPromptStore {
        pool: Arc<PgPool>,
        tenant_id: String,
        agent_id: String,
    }

    impl PgBasicPromptStore {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            agent_id: impl Into<String>,
        ) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
                agent_id: agent_id.into(),
            }
        }

        // -------------------------------------------------------------------
        // Queries
        // -------------------------------------------------------------------

        /// Return the stored row for `(user_id, project_id)`, or `None` if no
        /// assembly has been recorded yet for this scope.
        pub(crate) async fn get_for_scope(
            &self,
            user_id: &str,
            project_id: &str,
        ) -> Result<Option<BasicPromptEntry>, BasicPromptStoreError> {
            let client = self.pool.get().await?;
            let row = client
                .query_opt(
                    "SELECT id, bundle_json::text, fingerprint,
                            is_stale, assembled_at, prewarm_last_at, updated_at
                     FROM reborn_basic_prompt_store
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4",
                    &[
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                    ],
                )
                .await?;

            Ok(row.map(|r| {
                // bundle_json is a JSONB string value — extract the inner string.
                let bundle_raw: String = r.get(1);
                let bundle = extract_jsonb_string(&bundle_raw);
                BasicPromptEntry {
                    id: r.get(0),
                    bundle,
                    fingerprint: r.get(2),
                    is_stale: r.get(3),
                    assembled_at: r.get(4),
                    prewarm_last_at: r.get(5),
                    updated_at: r.get(6),
                }
            }))
        }

        /// Upsert the assembled bundle for a scope.
        ///
        /// - Always sets `is_stale = false` and `assembled_at = now()`.
        /// - Sets `prewarm_last_at = now()` when `with_prewarm = true`.
        /// - Computes `fingerprint = sha256(bundle)` before writing.
        pub(crate) async fn store(
            &self,
            user_id: &str,
            project_id: &str,
            bundle: &str,
            with_prewarm: bool,
        ) -> Result<BasicPromptEntry, BasicPromptStoreError> {
            let fp = compute_fingerprint(bundle);
            // Encode bundle as a JSON string value for the JSONB column.
            let bundle_json_str =
                serde_json::to_string(bundle).unwrap_or_else(|_| "\"\"".to_string());

            let client = self.pool.get().await?;
            client
                .execute(
                    "INSERT INTO reborn_basic_prompt_store
                         (tenant_id, user_id, agent_id, project_id,
                          bundle_json, fingerprint, is_stale,
                          assembled_at, prewarm_last_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5::JSONB, $6, false, now(),
                             CASE WHEN $7 THEN now() ELSE NULL END,
                             now())
                     ON CONFLICT ON CONSTRAINT reborn_basic_prompt_store_scope_unique
                     DO UPDATE SET
                         bundle_json     = EXCLUDED.bundle_json,
                         fingerprint     = EXCLUDED.fingerprint,
                         is_stale        = false,
                         assembled_at    = now(),
                         prewarm_last_at = CASE
                             WHEN $7 THEN now()
                             ELSE reborn_basic_prompt_store.prewarm_last_at
                         END,
                         updated_at      = now()",
                    &[
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                        &bundle_json_str,
                        &fp,
                        &with_prewarm,
                    ],
                )
                .await?;

            // Re-read to return DB-coerced values.
            self.get_for_scope(user_id, project_id)
                .await?
                .ok_or_else(|| {
                    BasicPromptStoreError::Db("row missing after upsert in store()".to_string())
                })
        }

        /// Mark the scope's row stale. No-op (`Ok(())`) if no row exists.
        ///
        /// Called after every Q2 graduation so the next per-turn call returns
        /// the minimal fallback until the operator clicks Regenerate.
        pub(crate) async fn mark_stale(
            &self,
            user_id: &str,
            project_id: &str,
        ) -> Result<(), BasicPromptStoreError> {
            let client = self.pool.get().await?;
            client
                .execute(
                    "UPDATE reborn_basic_prompt_store
                     SET is_stale = true, updated_at = now()
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4",
                    &[
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                    ],
                )
                .await?;
            debug!(
                tenant_id = %self.tenant_id,
                user_id,
                project_id,
                "basic_prompt_store: marked stale after Q2 graduation"
            );
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // SystemBundleSource implementation (§K.1.5)
    // -----------------------------------------------------------------------

    #[async_trait::async_trait]
    impl brassclaw_loop_support::SystemBundleSource for PgBasicPromptStore {
        async fn get_system_bundle(&self, user_id: &str, project_id: &str) -> String {
            get_system_bundle(self, user_id, project_id).await
        }
    }

    // -----------------------------------------------------------------------
    // Fingerprint helper
    // -----------------------------------------------------------------------

    /// Compute `sha256(bundle_text)` as a 64-char lowercase hex string.
    ///
    /// Used to detect whether a re-assembly produced identical output, avoiding
    /// a redundant DB write of a large `bundle_json` column.
    pub(crate) fn compute_fingerprint(bundle: &str) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(bundle.as_bytes()))
    }

    /// Extract the inner string from a JSONB string value.
    ///
    /// `bundle_json` is stored as a JSONB string (e.g. `"\"hello world\""`) and
    /// `::text` casting gives back the JSON-encoded form.  Unquoting it gives the
    /// original bundle text.  Falls back to the raw value on parse failure.
    fn extract_jsonb_string(raw: &str) -> String {
        serde_json::from_str::<String>(raw).unwrap_or_else(|_| raw.to_string())
    }

    // -----------------------------------------------------------------------
    // Per-turn bundle retrieval helper
    // -----------------------------------------------------------------------

    /// Return the bundle text to use as the System-message prefix for this turn.
    ///
    /// **Fast path:** a non-stale, non-empty row exists → return `entry.bundle`.
    ///   One cheap single-row DB fetch; no component-table re-assembly.
    ///
    /// **Slow/cold path:** stale, no row, or empty bundle → return
    ///   `minimal_base_prompt_fallback()`.  The operator must click Regenerate
    ///   in the Prefix Tab to restore the full bundle.
    ///
    /// Used by both the Kohai prompt path and the Sempai `run_sempai_review` path.
    pub(crate) async fn get_system_bundle(
        store: &PgBasicPromptStore,
        user_id: &str,
        project_id: &str,
    ) -> String {
        match store.get_for_scope(user_id, project_id).await {
            Ok(Some(entry)) if !entry.is_stale && !entry.bundle.is_empty() => entry.bundle,
            Ok(Some(_)) => minimal_base_prompt_fallback(),
            Ok(None) => minimal_base_prompt_fallback(),
            Err(e) => {
                debug!(
                    user_id,
                    project_id,
                    error = %e,
                    "get_system_bundle: DB error, using fallback"
                );
                minimal_base_prompt_fallback()
            }
        }
    }

    /// Minimal System message when the bundle is unavailable (stale or first boot).
    pub(crate) fn minimal_base_prompt_fallback() -> String {
        "# BrassClaw Reborn — Prefix Knowledge Base\n\
         [Bundle not yet compiled — run Settings → Prefix Cache → Generate]\n"
            .to_string()
    }

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn fingerprint_deterministic_for_same_input() {
            let fp1 = compute_fingerprint("hello world");
            let fp2 = compute_fingerprint("hello world");
            assert_eq!(fp1, fp2, "same input must produce the same fingerprint");
            assert_eq!(fp1.len(), 64, "fingerprint must be 64 hex chars");
        }

        #[test]
        fn fingerprint_changes_on_different_text() {
            let fp_a = compute_fingerprint("bundle A");
            let fp_b = compute_fingerprint("bundle B");
            assert_ne!(
                fp_a, fp_b,
                "different texts must produce different fingerprints"
            );
        }

        #[test]
        fn fingerprint_empty_input_stable() {
            let fp = compute_fingerprint("");
            assert_eq!(fp.len(), 64);
        }

        #[test]
        fn extract_jsonb_string_unquotes_correctly() {
            // serde_json::to_string("hello") → "\"hello\""
            let raw = "\"hello world\"";
            assert_eq!(extract_jsonb_string(raw), "hello world");
        }

        #[test]
        fn extract_jsonb_string_fallback_on_non_string() {
            // If the column somehow contains a non-string JSON value, return raw.
            let raw = "{\"not\":\"a string\"}";
            assert_eq!(extract_jsonb_string(raw), raw);
        }

        #[test]
        fn minimal_fallback_is_non_empty() {
            let fb = minimal_base_prompt_fallback();
            assert!(!fb.is_empty());
            assert!(fb.contains("Bundle not yet compiled"));
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::{
    PgBasicPromptStore, compute_fingerprint, get_system_bundle, minimal_base_prompt_fallback,
};
