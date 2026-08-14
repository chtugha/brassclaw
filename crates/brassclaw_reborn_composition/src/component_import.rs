//! One-shot `brassclaw_memory_docs` → class-specific component tables importer.
//!
//! # What this does
//!
//! Reads all [`MemoryDoc`](brassclaw_engine::types::memory::MemoryDoc) rows from
//! the legacy `brassclaw_memory_docs` (V016) table and migrates each one into
//! the appropriate class-specific component table (V036–V043):
//!
//! | DocType  | Table              | class_code |
//! |----------|--------------------|------------|
//! | Spec     | reborn_specs       | 12         |
//! | ToolSkill| reborn_tool_skills | 13         |
//! | Plan     | reborn_plans       | 14         |
//! | Summary  | reborn_summaries   | 15         |
//! | Lesson   | reborn_lessons     | 18         |
//! | Issue    | reborn_issues      | 19         |
//! | Note     | reborn_notes       | 20         |
//!
//! Notes:
//! - `DocType::Skill` is handled by `skill_import.rs` (already migrated).
//! - `DocType::Recipe` is handled by `PgRecipeStoreFacade` / V033.
//! - `DocType::Docu` (class 17, `reborn_docus`) has no legacy `DocType` variant;
//!   `reborn_docus` rows are created fresh and are NOT migrated from V016.
//!
//! # Idempotency
//!
//! Each row is keyed by `(tenant_id, user_id, agent_id, project_id, name)`.
//! The `content_hash` (SHA-256 of title + "\n\n" + content) determines whether
//! an existing row should be updated:
//! - Same hash → skip (no change).
//! - Different hash → update, reset `validation_status = 'pending'` and add
//!   `05:validator` to `consumer_tags`.
//!
//! # Splitting
//!
//! Docs with content longer than `SPLIT_CHUNK_CHARS` (≈5000 tokens at 4 chars/token
//! = 20 000 chars) are split at paragraph boundaries into multiple rows named
//! `{base_name}-part-{N}`.
//!
//! # Scope mapping
//!
//! Legacy `MemoryDoc` rows carry `(tenant_id, user_id, project_id)`.
//! The new tables require `(tenant_id, user_id, agent_id, project_id)`.
//! The caller supplies `agent_id`.

#[cfg(all(feature = "postgres", feature = "skills-db"))]
mod inner {
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    use brassclaw_pg::PgPool;

    // ~5000 tokens at 4 chars/token.
    const SPLIT_CHUNK_CHARS: usize = 20_000;
    /// Maximum number of chunks produced per source document (safety ceiling).
    const MAX_CHUNKS_PER_DOC: usize = 20;

    // -------------------------------------------------------------------------
    // Public types
    // -------------------------------------------------------------------------

    /// Summary returned by [`run_component_import`].
    #[derive(Debug, Default)]
    pub struct ComponentImportSummary {
        /// Rows skipped (unchanged content hash).
        pub skipped: usize,
        /// New rows inserted.
        pub inserted: usize,
        /// Existing rows updated (content changed).
        pub updated: usize,
        /// Source documents that could not be processed.
        pub failed: Vec<(String, String)>,
    }

    /// Error type for the component importer.
    #[derive(Debug, thiserror::Error)]
    pub enum ComponentImportError {
        #[error("database pool error: {0}")]
        Pool(#[from] deadpool_postgres::PoolError),

        #[error("database query error: {0}")]
        Query(#[from] tokio_postgres::Error),
    }

    // -------------------------------------------------------------------------
    // Entry point
    // -------------------------------------------------------------------------

    /// Migrate all eligible legacy `brassclaw_memory_docs` rows into the
    /// class-specific component tables (V036–V043).
    ///
    /// `agent_id` — the agent scope to assign to migrated rows.
    /// `tenant_id` — only rows for this tenant are read and migrated.
    pub async fn run_component_import(
        pool: &Arc<PgPool>,
        agent_id: &str,
        tenant_id: &str,
    ) -> Result<ComponentImportSummary, ComponentImportError> {
        let client = pool.get().await?;
        let mut summary = ComponentImportSummary::default();

        // Read all eligible legacy docs for this tenant.
        // Skill and Recipe types are skipped (handled elsewhere).
        let rows = client
            .query(
                "SELECT id, user_id, project_id, doc_type, title, content
                 FROM brassclaw_memory_docs
                 WHERE tenant_id = $1
                   AND doc_type NOT IN ('Skill', 'Recipe')
                 ORDER BY created_at ASC",
                &[&tenant_id],
            )
            .await?;

        for row in &rows {
            let id: String = row.get(0);
            let user_id: String = row.get(1);
            let project_id: String = row.get(2);
            let doc_type: String = row.get(3);
            let title: String = row.get(4);
            let content: String = row.get(5);

            let Some(table) = doc_type_to_table(&doc_type) else {
                // Unknown or explicitly excluded doc type — skip silently.
                continue;
            };

            let chunks = split_content(&title, &content);
            let base_name = normalize_name(&title);

            for (idx, chunk) in chunks.iter().enumerate() {
                let name = if chunks.len() == 1 {
                    base_name.clone()
                } else {
                    format!("{}-part-{}", base_name, idx + 1)
                };
                // Trim name to 250 chars (table constraint is 256).
                let name: String = name.chars().take(250).collect();
                let hash = sha256_hex(&format!("{}\n\n{}", title, chunk));
                let consumer_tags: Vec<String> = consumer_tags_for_table(table)
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                let intent_examples = extract_intent_examples(&title, chunk);

                match upsert_row(
                    &client,
                    table,
                    tenant_id,
                    &user_id,
                    agent_id,
                    &project_id,
                    &name,
                    &title,
                    chunk,
                    &hash,
                    &consumer_tags,
                    &intent_examples,
                )
                .await
                {
                    Ok(UpsertOutcome::Inserted) => summary.inserted += 1,
                    Ok(UpsertOutcome::Updated) => summary.updated += 1,
                    Ok(UpsertOutcome::Skipped) => summary.skipped += 1,
                    Err(e) => {
                        summary.failed.push((
                            format!("{doc_type}/{id}"),
                            format!("upsert failed for name '{name}': {e}"),
                        ));
                    }
                }
            }
        }

        Ok(summary)
    }

    // -------------------------------------------------------------------------
    // Table routing
    // -------------------------------------------------------------------------

    fn doc_type_to_table(doc_type: &str) -> Option<&'static str> {
        match doc_type {
            "Spec" => Some("reborn_specs"),
            "ToolSkill" => Some("reborn_tool_skills"),
            "Plan" => Some("reborn_plans"),
            "Summary" => Some("reborn_summaries"),
            "Lesson" => Some("reborn_lessons"),
            "Issue" => Some("reborn_issues"),
            "Note" => Some("reborn_notes"),
            // Skill / Recipe handled elsewhere; Docu has no legacy DocType.
            _ => None,
        }
    }

    fn consumer_tags_for_table(table: &str) -> &'static [&'static str] {
        match table {
            "reborn_tool_skills" => &["01:rusty", "02:orchestrator", "05:validator"],
            _ => &["02:orchestrator", "05:validator"],
        }
    }

    // -------------------------------------------------------------------------
    // Name normalisation
    // -------------------------------------------------------------------------

    /// Convert a free-form title to a slug suitable as a component `name`.
    ///
    /// Steps:
    /// 1. Lowercase.
    /// 2. Replace non-alphanumeric runs with a single hyphen.
    /// 3. Strip leading/trailing hyphens.
    /// 4. Truncate to 240 chars (table constraint ≤256, with room for `-part-N` suffix).
    /// 5. Fall back to `"migrated-doc"` if the result is empty.
    fn normalize_name(title: &str) -> String {
        let s = title.to_lowercase();
        // Replace whitespace and punctuation with hyphens.
        let mut out = String::with_capacity(s.len());
        let mut last_hyphen = true; // skip leading hyphens
        for ch in s.chars() {
            if ch.is_alphanumeric() {
                out.push(ch);
                last_hyphen = false;
            } else if !last_hyphen {
                out.push('-');
                last_hyphen = true;
            }
        }
        // Trim trailing hyphen.
        let out = out.trim_end_matches('-').to_string();
        // Ensure not empty.
        let out = if out.is_empty() {
            "migrated-doc".to_string()
        } else {
            out
        };
        // `reborn_tool_skills` restricts names to 64 chars; other tables allow 256.
        // We cap at 240 here to leave room for "-part-N" suffixes.
        drop(s);
        out.chars().take(240).collect()
    }

    // -------------------------------------------------------------------------
    // Content splitting
    // -------------------------------------------------------------------------

    /// Split `content` into ≤[`SPLIT_CHUNK_CHARS`]-char chunks at paragraph
    /// boundaries (blank lines).
    ///
    /// Returns a `Vec` of at most `MAX_CHUNKS_PER_DOC` chunks.  If the content
    /// is short enough it is returned as a single-element vec.
    fn split_content(_title: &str, content: &str) -> Vec<String> {
        if content.len() <= SPLIT_CHUNK_CHARS {
            return vec![content.to_string()];
        }

        let mut chunks: Vec<String> = Vec::new();
        let mut current = String::new();

        for paragraph in content.split("\n\n") {
            let para_with_sep = format!("{paragraph}\n\n");
            if current.len() + para_with_sep.len() > SPLIT_CHUNK_CHARS && !current.is_empty() {
                chunks.push(current.trim_end().to_string());
                current = String::new();
                if chunks.len() >= MAX_CHUNKS_PER_DOC {
                    // Safety ceiling: discard remaining content if too many chunks.
                    break;
                }
            }
            current.push_str(&para_with_sep);
        }

        if !current.trim().is_empty() && chunks.len() < MAX_CHUNKS_PER_DOC {
            chunks.push(current.trim_end().to_string());
        }

        if chunks.is_empty() {
            // Edge case: a single paragraph larger than the chunk limit.
            // Truncate hard.
            chunks.push(content.chars().take(SPLIT_CHUNK_CHARS).collect());
        }

        chunks
    }

    // -------------------------------------------------------------------------
    // Intent-example extraction
    // -------------------------------------------------------------------------

    /// Extract a handful of intent examples from the title and first sentences
    /// of the content.
    ///
    /// Returns a JSON array of `{input: String, class: u8}` objects.
    fn extract_intent_examples(title: &str, content: &str) -> serde_json::Value {
        let mut examples: Vec<serde_json::Value> = Vec::new();

        // Title → class 2 (keyword/phrase)
        let title_trimmed = title.trim();
        if !title_trimmed.is_empty() {
            examples.push(serde_json::json!({
                "input": title_trimmed,
                "class": 2
            }));
        }

        // First 3 sentences from content → class 3 (full sentence)
        let mut sentence_count = 0usize;
        for sentence in content.split_terminator(['.', '!', '?']) {
            let s = sentence.trim();
            if s.len() >= 8 && sentence_count < 3 {
                examples.push(serde_json::json!({
                    "input": s,
                    "class": 3
                }));
                sentence_count += 1;
            }
            if sentence_count >= 3 {
                break;
            }
        }

        serde_json::json!(examples)
    }

    // -------------------------------------------------------------------------
    // Content hash
    // -------------------------------------------------------------------------

    fn sha256_hex(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    // -------------------------------------------------------------------------
    // DB upsert
    // -------------------------------------------------------------------------

    enum UpsertOutcome {
        Inserted,
        Updated,
        Skipped,
    }

    #[allow(clippy::too_many_arguments)]
    async fn upsert_row(
        client: &deadpool_postgres::Client,
        table: &str,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
        title: &str,
        content: &str,
        content_hash: &str,
        consumer_tags: &[String],
        intent_examples: &serde_json::Value,
    ) -> Result<UpsertOutcome, tokio_postgres::Error> {
        // Check for existing row.
        let existing = client
            .query_opt(
                &format!(
                    "SELECT id, content_hash FROM {table}
                     WHERE tenant_id = $1
                       AND user_id   = $2
                       AND agent_id  = $3
                       AND project_id = $4
                       AND name = $5"
                ),
                &[&tenant_id, &user_id, &agent_id, &project_id, &name],
            )
            .await?;

        match existing {
            Some(row) => {
                let existing_hash: Option<String> = row.get(1);
                if existing_hash.as_deref() == Some(content_hash) {
                    return Ok(UpsertOutcome::Skipped);
                }
                // Hash changed — update the row and reset validation state.
                let id: uuid::Uuid = row.get(0);
                client
                    .execute(
                        &format!(
                            "UPDATE {table}
                             SET description     = $2,
                                 content         = $3,
                                 content_hash    = $4,
                                 consumer_tags   = $5,
                                 intent_examples = $6,
                                 validation_status = 'pending',
                                 source          = 'migrated',
                                 updated_at      = now()
                             WHERE id = $1"
                        ),
                        &[
                            &id,
                            &title,
                            &content,
                            &content_hash,
                            &consumer_tags,
                            &intent_examples,
                        ],
                    )
                    .await?;
                Ok(UpsertOutcome::Updated)
            }
            None => {
                // Insert new row.
                client
                    .execute(
                        &format!(
                            "INSERT INTO {table}
                             (tenant_id, user_id, agent_id, project_id,
                              name, description, content, content_hash,
                              consumer_tags, intent_examples, source,
                              validation_status)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'migrated', 'pending')"
                        ),
                        &[
                            &tenant_id,
                            &user_id,
                            &agent_id,
                            &project_id,
                            &name,
                            &title,
                            &content,
                            &content_hash,
                            &consumer_tags,
                            &intent_examples,
                        ],
                    )
                    .await?;
                Ok(UpsertOutcome::Inserted)
            }
        }
    }
}

#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub use inner::{ComponentImportError, ComponentImportSummary, run_component_import};
