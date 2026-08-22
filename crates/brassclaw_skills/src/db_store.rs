//! DB-backed skill store for BrassClaw Reborn.
//!
//! Provides CRUD over `reborn_skills` (V027 migration) and wires in the
//! existing validation helpers so every write is fail-closed.
//!
//! # Scope isolation
//! Every read and write filters on the full
//! `(tenant_id, user_id, agent_id, project_id)` 4-tuple. Queries that span
//! multiple scope tuples must do so explicitly via separate calls.
//!
//! # Validation split (§3.6)
//! * **Validation-gated columns** (name, description, body, compatibility,
//!   activation, consumer_tags, intent_examples, …): saved only after
//!   [`DbSkillStore::validate_row`] passes Step-1 checks. A save error is
//!   returned to the caller as [`DbSkillStoreError::Validation`].
//! * **Immediate-write columns** (tier, usage_count, success_count,
//!   failure_count, wilson_lower, confidence, source, audit metadata):
//!   written directly via [`DbSkillStore::update_reward`] with no re-validation.
//!
//! # Consumer-tag rules (§3.9)
//! * Every newly inserted or content-updated row receives `05:validator` in
//!   `consumer_tags` — the validator tag greys out other tags until Step-2
//!   validation removes it.
//! * [`DbSkillStore::fetch_for_consumer`] filters out rows that carry
//!   `05:validator`, regardless of `validation_status`.
//! * The class-default consumer tags are seeded at insert time; see
//!   [`class_default_consumer_tags`].

#[cfg(feature = "db-store")]
mod inner {
    use std::sync::OnceLock;

    use chrono::{DateTime, Utc};
    use serde_json::Value as JsonValue;
    use thiserror::Error;
    use uuid::Uuid;

    use brassclaw_pg::{PgError, PgPool, PgRow};

    use crate::validation::{
        escape_skill_content, normalize_skill_identifier, validate_skill_version,
    };

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    /// Validator consumer tag — added at insert time and removed after Step-2
    /// validation passes. While present it filters the skill from consumer reads.
    const VALIDATOR_CONSUMER_TAG: &str = "05:validator";

    /// Approximate bytes-per-token ratio for English prose (4 bytes ≈ 1 token).
    const BYTES_PER_TOKEN: f64 = 4.0;

    /// Maximum approximate token count allowed for a skill body at save time.
    const SKILL_BODY_MAX_TOKENS: usize = 5000;

    // -----------------------------------------------------------------------
    // Error type
    // -----------------------------------------------------------------------

    #[derive(Debug, Error)]
    pub enum DbSkillStoreError {
        #[error("database error: {0}")]
        Db(#[from] PgError),

        #[error("skill validation failed: {errors:?}")]
        Validation { errors: Vec<String> },

        #[error("skill name cannot be normalized to a valid identifier: {raw:?}")]
        UnnormalizableName { raw: String },

        #[error("unknown class in compatibility field: {value:?}")]
        UnknownClass { value: String },

        #[error("skill not found: {id}")]
        NotFound { id: Uuid },
    }

    // -----------------------------------------------------------------------
    // Scope tuple
    // -----------------------------------------------------------------------

    /// The 4-tuple that scopes every skill row.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct SkillScope {
        pub tenant_id: String,
        pub user_id: String,
        pub agent_id: String,
        pub project_id: String,
    }

    // -----------------------------------------------------------------------
    // Class code helpers
    // -----------------------------------------------------------------------

    /// `class_code` values for `reborn_skills`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(i16)]
    pub enum SkillClassCode {
        /// Rusty — Rust-side tool-execution skill.
        Rusty = 1,
        /// Monty — Python-VM orchestration skill.
        Monty = 2,
        /// Llm — Pure prompt guidance injected into the LLM turn.
        Llm = 3,
    }

    impl SkillClassCode {
        /// Parse from a `compatibility` field containing `brassclaw-class:…`.
        pub fn from_compatibility(compat: &str) -> Option<Self> {
            if compat.contains("brassclaw-class:rusty") {
                Some(Self::Rusty)
            } else if compat.contains("brassclaw-class:monty") {
                Some(Self::Monty)
            } else if compat.contains("brassclaw-class:llm") {
                Some(Self::Llm)
            } else {
                None
            }
        }

        pub fn as_i16(self) -> i16 {
            self as i16
        }
    }

    /// Return the default consumer tags seeded at insert time for a given class.
    /// All rows also receive `05:validator` (added by the caller before insert).
    pub fn class_default_consumer_tags(class: SkillClassCode) -> Vec<String> {
        match class {
            SkillClassCode::Rusty => vec!["00:rusty".into(), "01:monty".into()],
            SkillClassCode::Monty => vec!["01:monty".into(), "02:orchestrator".into()],
            SkillClassCode::Llm => vec!["02:orchestrator".into(), "03:llm".into()],
        }
    }

    // -----------------------------------------------------------------------
    // Row types
    // -----------------------------------------------------------------------

    /// Input for creating or updating a skill row (validation-gated columns).
    #[derive(Debug, Clone)]
    pub struct SkillWriteInput {
        /// Scope for the new row. Must be supplied on every write.
        pub scope: SkillScope,
        /// Skill name. Normalized via [`normalize_skill_identifier`]; rejected
        /// if the result is `None` (returns [`DbSkillStoreError::UnnormalizableName`]).
        pub name: String,
        pub description: String,
        pub body: String,
        /// Must contain `brassclaw-class:rusty|monty|llm`.
        pub compatibility: String,
        pub license: String,
        pub allowed_tools: Vec<String>,
        pub version: String,
        pub keywords: Vec<String>,
        pub exclude_keywords: Vec<String>,
        pub patterns: Vec<String>,
        pub tags: Vec<String>,
        pub max_context_tokens: i32,
        pub setup_marker: Option<String>,
        pub required_binaries: Vec<String>,
        pub required_env: Vec<String>,
        pub required_config: Vec<String>,
        /// Array of `{input: string, class: 1|2|3}` objects.
        pub intent_examples: JsonValue,
        /// Additional consumer tags beyond the class defaults.
        /// The store seeds class defaults automatically; `05:validator` is always
        /// added at write time and must not be supplied here.
        pub extra_consumer_tags: Vec<String>,
        /// SHA-256 of the content; callers compute it externally.
        pub content_hash: String,
        /// Optional ID of the skill this row is replacing (version upgrade).
        pub replaces_id: Option<Uuid>,
        /// Optional similarity parent for dedup lineage.
        pub similarity_parent_id: Option<Uuid>,
        /// Source provenance label.
        pub source: String,
    }

    /// A full skill row as read from `reborn_skills`.
    #[derive(Debug, Clone)]
    pub struct DbSkillRow {
        pub id: Uuid,
        pub scope: SkillScope,
        pub name: String,
        pub description: String,
        pub body: String,
        pub compatibility: String,
        pub license: String,
        pub allowed_tools: Vec<String>,
        pub version: String,
        pub class_code: i16,
        pub prompt_uid: i64,
        pub keywords: Vec<String>,
        pub exclude_keywords: Vec<String>,
        pub patterns: Vec<String>,
        pub tags: Vec<String>,
        pub max_context_tokens: i32,
        pub setup_marker: Option<String>,
        pub required_binaries: Vec<String>,
        pub required_env: Vec<String>,
        pub required_config: Vec<String>,
        pub intent_examples: JsonValue,
        pub consumer_tags: Vec<String>,
        pub tier: String,
        pub usage_count: i64,
        pub success_count: i64,
        pub failure_count: i64,
        pub wilson_lower: f64,
        pub confidence: f64,
        pub source: String,
        pub validation_status: String,
        pub validation_errors: Vec<String>,
        pub review_feedback: Option<String>,
        pub review_attempts: i32,
        pub rejected_at: Option<DateTime<Utc>>,
        pub queue_code: Option<String>,
        pub similarity_parent_id: Option<Uuid>,
        pub replaces_id: Option<Uuid>,
        pub parent_version: Option<String>,
        pub content_hash: String,
        pub last_audit_at: Option<DateTime<Utc>>,
        pub audit_failure_count: i32,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    /// Reward-only update (immediate-write, no re-validation).
    #[derive(Debug, Clone, Default)]
    pub struct RewardUpdate {
        pub tier: Option<String>,
        pub usage_count: Option<i64>,
        pub success_count: Option<i64>,
        pub failure_count: Option<i64>,
        pub wilson_lower: Option<f64>,
        pub confidence: Option<f64>,
    }

    // -----------------------------------------------------------------------
    // Step-1 validation
    // -----------------------------------------------------------------------

    /// Errors returned from Step-1 local validation.
    #[derive(Debug, Clone, Default)]
    pub struct LocalValidationResult {
        pub errors: Vec<String>,
        pub warnings: Vec<String>,
    }

    impl LocalValidationResult {
        pub fn is_ok(&self) -> bool {
            self.errors.is_empty()
        }
    }

    /// Run Step-1 (local, structural) validation on a [`SkillWriteInput`].
    ///
    /// Checks:
    /// 1. Name normalises successfully and passes the strict agentskills.io pattern.
    /// 2. Description length (1–1024) and actionable-verb heuristic.
    /// 3. Token budget ≤ 5000.
    /// 4. Version format via [`validate_skill_version`].
    /// 5. Content escaped form does not cause size explosion (>64 KiB body).
    /// 6. `intent_examples` is a JSON array with `{input, class}` entries where
    ///    `class` is 1, 2, or 3.
    /// 7. Consumer tag format check: each entry must match `^[0-9]{2}(:[a-z0-9-]+)?$`.
    pub fn validate_row(input: &SkillWriteInput) -> LocalValidationResult {
        let mut result = LocalValidationResult::default();

        // 1. Name
        let name = input.name.trim();
        if name.is_empty() {
            result.errors.push("skill name must not be empty".into());
        } else {
            match normalize_skill_identifier(name) {
                None => result.errors.push(format!(
                    "skill name '{name}' cannot be normalized to a valid identifier \
                     (must match ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$)"
                )),
                Some(ref n) if n.len() > 64 => result
                    .errors
                    .push(format!("skill name exceeds 64 chars ({} chars)", n.len())),
                Some(ref n) if n.contains("--") => result.errors.push(format!(
                    "skill name '{n}' contains consecutive hyphens '--'"
                )),
                _ => {}
            }
        }

        // 2. Description
        let desc_len = input.description.trim().chars().count();
        if desc_len == 0 {
            result
                .errors
                .push("skill description must not be empty".into());
        } else if desc_len > 1024 {
            result.errors.push(format!(
                "skill description exceeds 1024 chars ({desc_len} chars)"
            ));
        } else {
            // Actionable-verb heuristic (soft warning).
            static VERB_RE: OnceLock<regex::Regex> = OnceLock::new();
            let re = VERB_RE.get_or_init(|| {
                regex::Regex::new(
                    r"\b(use|run|create|check|extract|process|analyze|configure|list|fetch|send|compute|apply|build|deploy|format|validate|inspect|open|close|delete|update|render|compile|test|sign)\b",
                )
                .unwrap() // safety: hardcoded literal
            });
            if !re.is_match(&input.description.to_lowercase()) {
                result.warnings.push(
                    "skill description does not contain an actionable verb — \
                     consider 'Use when …' phrasing"
                        .into(),
                );
            }
        }

        // 3. Token budget ≤ SKILL_BODY_MAX_TOKENS (rough: BYTES_PER_TOKEN bytes ≈ 1 token)
        let approx_tokens = (input.body.len() as f64 / BYTES_PER_TOKEN) as usize;
        if approx_tokens > SKILL_BODY_MAX_TOKENS {
            result.errors.push(format!(
                "skill body exceeds {SKILL_BODY_MAX_TOKENS} token budget (~{approx_tokens} tokens). \
                 Split into smaller skills or move detail to reference files."
            ));
        }

        // 4. Version format
        if !input.version.is_empty() && !validate_skill_version(&input.version) {
            result.errors.push(format!(
                "skill version '{}' is not a valid version string",
                input.version
            ));
        }

        // 5. Body size
        if input.body.len() > 64 * 1024 {
            result.errors.push(format!(
                "skill body exceeds 64 KiB ({} bytes)",
                input.body.len()
            ));
        }

        // 6. intent_examples structure
        if let Some(arr) = input.intent_examples.as_array() {
            for (i, entry) in arr.iter().enumerate() {
                let has_input = entry.get("input").and_then(|v| v.as_str()).is_some();
                let class_ok = entry
                    .get("class")
                    .and_then(|v| v.as_u64())
                    .map(|c| (1..=3).contains(&c))
                    .unwrap_or(false);
                if !has_input {
                    result
                        .errors
                        .push(format!("intent_examples[{i}] missing 'input' string field"));
                }
                if !class_ok {
                    result
                        .errors
                        .push(format!("intent_examples[{i}] 'class' must be 1, 2, or 3"));
                }
            }
        } else {
            result
                .errors
                .push("intent_examples must be a JSON array".into());
        }

        // 7. Source provenance label
        const VALID_SOURCES: &[&str] = &["authored", "extracted", "migrated", "imported"];
        if !VALID_SOURCES.contains(&input.source.as_str()) {
            result.errors.push(format!(
                "skill source '{}' is not valid; must be one of: authored, extracted, migrated, imported",
                input.source
            ));
        }

        // 8. Consumer tag format
        static TAG_RE: OnceLock<regex::Regex> = OnceLock::new();
        let tag_re = TAG_RE.get_or_init(|| {
            regex::Regex::new(r"^\d{2}(:[a-z0-9-]+)?$").unwrap() // safety: hardcoded literal
        });
        for tag in &input.extra_consumer_tags {
            if !tag_re.is_match(tag) {
                result.errors.push(format!(
                    "consumer tag '{tag}' does not match required format ^[0-9]{{2}}(:[a-z0-9-]+)?$"
                ));
            }
        }

        result
    }

    // -----------------------------------------------------------------------
    // Store
    // -----------------------------------------------------------------------

    /// DB-backed skill store.
    #[derive(Clone)]
    pub struct DbSkillStore {
        pool: PgPool,
    }

    impl DbSkillStore {
        /// Create a new store wrapping the given pool.
        pub fn new(pool: PgPool) -> Self {
            Self { pool }
        }

        /// Insert a new skill row (validation-gated).
        ///
        /// Returns [`DbSkillStoreError::Validation`] if Step-1 validation fails.
        /// Returns the newly created row's UUID on success.
        pub async fn insert(&self, input: &SkillWriteInput) -> Result<Uuid, DbSkillStoreError> {
            // Step-1 local validation.
            let vr = validate_row(input);
            if !vr.is_ok() {
                return Err(DbSkillStoreError::Validation { errors: vr.errors });
            }

            let name = normalize_skill_identifier(input.name.trim()).ok_or_else(|| {
                DbSkillStoreError::UnnormalizableName {
                    raw: input.name.clone(),
                }
            })?;

            let class =
                SkillClassCode::from_compatibility(&input.compatibility).ok_or_else(|| {
                    DbSkillStoreError::UnknownClass {
                        value: input.compatibility.clone(),
                    }
                })?;

            // Seed consumer tags: class defaults + caller extras + 05:validator.
            let mut consumer_tags = class_default_consumer_tags(class);
            for t in &input.extra_consumer_tags {
                if !consumer_tags.contains(t) {
                    consumer_tags.push(t.clone());
                }
            }
            if !consumer_tags.contains(&VALIDATOR_CONSUMER_TAG.to_string()) {
                consumer_tags.push(VALIDATOR_CONSUMER_TAG.into());
            }

            let escaped_body = escape_skill_content(&input.body);

            let client = self.pool.get().await.map_err(PgError::from)?;
            let row = client
                .query_one(
                    "INSERT INTO reborn_skills (
                        tenant_id, user_id, agent_id, project_id,
                        name, description, body, compatibility, license,
                        allowed_tools, version,
                        class_code,
                        keywords, exclude_keywords, patterns, tags,
                        max_context_tokens, setup_marker,
                        required_binaries, required_env, required_config,
                        intent_examples,
                        consumer_tags,
                        source, content_hash,
                        replaces_id, similarity_parent_id,
                        validation_status, queue_code
                    ) VALUES (
                        $1,$2,$3,$4,
                        $5,$6,$7,$8,$9,
                        $10,$11,
                        $12,
                        $13,$14,$15,$16,
                        $17,$18,
                        $19,$20,$21,
                        $22,
                        $23,
                        $24,$25,
                        $26,$27,
                        'pending','q1_auto'
                    )
                    RETURNING id",
                    &[
                        &input.scope.tenant_id,
                        &input.scope.user_id,
                        &input.scope.agent_id,
                        &input.scope.project_id,
                        &name,
                        &input.description,
                        &escaped_body,
                        &input.compatibility,
                        &input.license,
                        &input.allowed_tools,
                        &input.version,
                        &class.as_i16(),
                        &input.keywords,
                        &input.exclude_keywords,
                        &input.patterns,
                        &input.tags,
                        &input.max_context_tokens,
                        &input.setup_marker,
                        &input.required_binaries,
                        &input.required_env,
                        &input.required_config,
                        &input.intent_examples,
                        &consumer_tags,
                        &input.source,
                        &input.content_hash,
                        &input.replaces_id,
                        &input.similarity_parent_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            Ok(row.get::<_, Uuid>(0))
        }

        /// Fetch all validated, non-validator-tagged skills for a consumer.
        ///
        /// `consumer_tag` — e.g. `"03:llm"`, `"02:orchestrator"`.
        /// Returns rows ordered by `(class_code ASC, prompt_uid ASC)` for
        /// deterministic prompt assembly (§3.7).
        pub async fn fetch_for_consumer(
            &self,
            scope: &SkillScope,
            consumer_tag: &str,
        ) -> Result<Vec<DbSkillRow>, DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let rows = client
                .query(
                    &format!(
                        "SELECT
                        id, tenant_id, user_id, agent_id, project_id,
                        name, description, body, compatibility, license,
                        allowed_tools, version, class_code, prompt_uid,
                        keywords, exclude_keywords, patterns, tags,
                        max_context_tokens, setup_marker,
                        required_binaries, required_env, required_config,
                        intent_examples, consumer_tags,
                        tier, usage_count, success_count, failure_count,
                        wilson_lower, confidence, source,
                        validation_status, validation_errors,
                        review_feedback, review_attempts, rejected_at, queue_code,
                        similarity_parent_id, replaces_id, parent_version,
                        content_hash, last_audit_at, audit_failure_count,
                        created_at, updated_at
                     FROM reborn_skills
                    WHERE tenant_id = $1
                      AND user_id   = $2
                      AND agent_id  = $3
                      AND project_id = $4
                      AND validation_status = 'validated'
                      AND $5 = ANY(consumer_tags)
                      AND NOT ('{}' = ANY(consumer_tags))
                    ORDER BY class_code ASC, prompt_uid ASC",
                        VALIDATOR_CONSUMER_TAG
                    ),
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &consumer_tag,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            rows.iter().map(row_from_pg).collect()
        }

        /// Fetch a single skill row by ID (within a scope).
        pub async fn get(
            &self,
            scope: &SkillScope,
            id: Uuid,
        ) -> Result<Option<DbSkillRow>, DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let opt = client
                .query_opt(
                    "SELECT
                        id, tenant_id, user_id, agent_id, project_id,
                        name, description, body, compatibility, license,
                        allowed_tools, version, class_code, prompt_uid,
                        keywords, exclude_keywords, patterns, tags,
                        max_context_tokens, setup_marker,
                        required_binaries, required_env, required_config,
                        intent_examples, consumer_tags,
                        tier, usage_count, success_count, failure_count,
                        wilson_lower, confidence, source,
                        validation_status, validation_errors,
                        review_feedback, review_attempts, rejected_at, queue_code,
                        similarity_parent_id, replaces_id, parent_version,
                        content_hash, last_audit_at, audit_failure_count,
                        created_at, updated_at
                     FROM reborn_skills
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            opt.map(|r| row_from_pg(&r)).transpose()
        }

        /// Fetch a skill row by (scope, name) — returns any validation_status
        /// so the importer can detect existing rows regardless of queue state.
        pub async fn fetch_by_name(
            &self,
            scope: &SkillScope,
            name: &str,
        ) -> Result<Option<DbSkillRow>, DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let opt = client
                .query_opt(
                    "SELECT
                        id, tenant_id, user_id, agent_id, project_id,
                        name, description, body, compatibility, license,
                        allowed_tools, version, class_code, prompt_uid,
                        keywords, exclude_keywords, patterns, tags,
                        max_context_tokens, setup_marker,
                        required_binaries, required_env, required_config,
                        intent_examples, consumer_tags,
                        tier, usage_count, success_count, failure_count,
                        wilson_lower, confidence, source,
                        validation_status, validation_errors,
                        review_feedback, review_attempts, rejected_at, queue_code,
                        similarity_parent_id, replaces_id, parent_version,
                        content_hash, last_audit_at, audit_failure_count,
                        created_at, updated_at
                     FROM reborn_skills
                     WHERE name       = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5
                     LIMIT 1",
                    &[
                        &name,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            opt.map(|r| row_from_pg(&r)).transpose()
        }

        /// Update the content/activation columns of an existing row and reset
        /// validation to `pending` + re-add `05:validator` tag.
        ///
        /// Used by the importer when a SKILL.md body has changed since the last
        /// import (content_hash mismatch).
        pub async fn update_content(
            &self,
            id: Uuid,
            scope: &SkillScope,
            input: &SkillWriteInput,
        ) -> Result<(), DbSkillStoreError> {
            // Step-1 local validation before writing.
            let vr = validate_row(input);
            if !vr.is_ok() {
                return Err(DbSkillStoreError::Validation { errors: vr.errors });
            }

            let escaped_body = escape_skill_content(&input.body);

            let client = self.pool.get().await.map_err(PgError::from)?;
            let affected = client
                .execute(
                    &format!(
                        "UPDATE reborn_skills SET
                        description      = $6,
                        body             = $7,
                        compatibility    = $8,
                        license          = $9,
                        allowed_tools    = $10,
                        version          = $11,
                        keywords         = $12,
                        exclude_keywords = $13,
                        patterns         = $14,
                        tags             = $15,
                        max_context_tokens = $16,
                        setup_marker     = $17,
                        required_binaries = $18,
                        required_env     = $19,
                        required_config  = $20,
                        intent_examples  = $21,
                        content_hash     = $22,
                        -- Reset validation cycle.
                        validation_status = 'pending',
                        queue_code        = 'q1_auto',
                        validation_errors = '{{}}',
                        -- Re-add the validator tag (greys out other consumer tags).
                        consumer_tags = array_append(
                            array_remove(consumer_tags, '{}'),
                            '{0}'
                        )
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5",
                        VALIDATOR_CONSUMER_TAG
                    ),
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &input.description,
                        &escaped_body,
                        &input.compatibility,
                        &input.license,
                        &input.allowed_tools,
                        &input.version,
                        &input.keywords,
                        &input.exclude_keywords,
                        &input.patterns,
                        &input.tags,
                        &input.max_context_tokens,
                        &input.setup_marker,
                        &input.required_binaries,
                        &input.required_env,
                        &input.required_config,
                        &input.intent_examples,
                        &input.content_hash,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }

        /// Immediate-write update for reward / telemetry columns.
        /// Does NOT re-trigger Step-1 validation.
        pub async fn update_reward(
            &self,
            scope: &SkillScope,
            id: Uuid,
            update: &RewardUpdate,
        ) -> Result<(), DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;

            // Only update columns that are explicitly set.
            let affected = client
                .execute(
                    "UPDATE reborn_skills SET
                        tier          = COALESCE($6, tier),
                        usage_count   = COALESCE($7, usage_count),
                        success_count = COALESCE($8, success_count),
                        failure_count = COALESCE($9, failure_count),
                        wilson_lower  = COALESCE($10, wilson_lower),
                        confidence    = COALESCE($11, confidence)
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &update.tier,
                        &update.usage_count,
                        &update.success_count,
                        &update.failure_count,
                        &update.wilson_lower,
                        &update.confidence,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }

        /// Advance validation status (Step-2 manual validate — pops `05:validator`).
        /// Guards that the transition is `auto_passed → validated` (the only
        /// Step-2 path for skills).
        pub async fn mark_validated(
            &self,
            scope: &SkillScope,
            id: Uuid,
        ) -> Result<(), DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;

            let affected = client
                .execute(
                    &format!(
                        "UPDATE reborn_skills SET
                        validation_status = 'validated',
                        queue_code        = NULL,
                        -- Remove {0} tag to activate consumer tags.
                        consumer_tags = array_remove(consumer_tags, '{0}')
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5
                       AND validation_status = 'auto_passed'",
                        VALIDATOR_CONSUMER_TAG
                    ),
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }

        /// Advance a row from `pending` → `auto_passed` after successful Step-1
        /// validation by the queue runner.
        pub async fn mark_auto_passed(
            &self,
            scope: &SkillScope,
            id: Uuid,
        ) -> Result<(), DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let affected = client
                .execute(
                    "UPDATE reborn_skills SET
                        validation_status = 'auto_passed',
                        queue_code        = 'q2_manual',
                        validation_errors = '{}'
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5
                       AND validation_status = 'pending'",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }

        /// Record a Step-1 auto-validation failure.
        pub async fn mark_auto_failed(
            &self,
            scope: &SkillScope,
            id: Uuid,
            errors: &[String],
        ) -> Result<(), DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let err_arr: Vec<&str> = errors.iter().map(|s| s.as_str()).collect();
            let affected = client
                .execute(
                    "UPDATE reborn_skills SET
                        validation_status = 'auto_failed',
                        queue_code        = 'q1_auto',
                        validation_errors = $6
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &err_arr,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }

        /// Delete a skill row (terminal wipe — only for `garbage` status rows).
        pub async fn delete_garbage(
            &self,
            scope: &SkillScope,
            id: Uuid,
        ) -> Result<(), DbSkillStoreError> {
            let client = self.pool.get().await.map_err(PgError::from)?;
            let affected = client
                .execute(
                    "DELETE FROM reborn_skills
                     WHERE id = $1
                       AND tenant_id  = $2
                       AND user_id    = $3
                       AND agent_id   = $4
                       AND project_id = $5
                       AND validation_status = 'garbage'",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(PgError::from)?;

            if affected == 0 {
                return Err(DbSkillStoreError::NotFound { id });
            }
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Row mapping helper
    // -----------------------------------------------------------------------

    fn row_from_pg(row: &PgRow) -> Result<DbSkillRow, DbSkillStoreError> {
        Ok(DbSkillRow {
            id: row.get("id"),
            scope: SkillScope {
                tenant_id: row.get("tenant_id"),
                user_id: row.get("user_id"),
                agent_id: row.get("agent_id"),
                project_id: row.get("project_id"),
            },
            name: row.get("name"),
            description: row.get("description"),
            body: row.get("body"),
            compatibility: row.get("compatibility"),
            license: row.get("license"),
            allowed_tools: row.get("allowed_tools"),
            version: row.get("version"),
            class_code: row.get("class_code"),
            prompt_uid: row.get("prompt_uid"),
            keywords: row.get("keywords"),
            exclude_keywords: row.get("exclude_keywords"),
            patterns: row.get("patterns"),
            tags: row.get("tags"),
            max_context_tokens: row.get("max_context_tokens"),
            setup_marker: row.get("setup_marker"),
            required_binaries: row.get("required_binaries"),
            required_env: row.get("required_env"),
            required_config: row.get("required_config"),
            intent_examples: row.get("intent_examples"),
            consumer_tags: row.get("consumer_tags"),
            tier: row.get("tier"),
            usage_count: row.get("usage_count"),
            success_count: row.get("success_count"),
            failure_count: row.get("failure_count"),
            wilson_lower: row.get("wilson_lower"),
            confidence: row.get("confidence"),
            source: row.get("source"),
            validation_status: row.get("validation_status"),
            validation_errors: row.get("validation_errors"),
            review_feedback: row.get("review_feedback"),
            review_attempts: row.get("review_attempts"),
            rejected_at: row.get("rejected_at"),
            queue_code: row.get("queue_code"),
            similarity_parent_id: row.get("similarity_parent_id"),
            replaces_id: row.get("replaces_id"),
            parent_version: row.get("parent_version"),
            content_hash: row.get("content_hash"),
            last_audit_at: row.get("last_audit_at"),
            audit_failure_count: row.get("audit_failure_count"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use serde_json::json;

        use super::*;

        fn scope() -> SkillScope {
            SkillScope {
                tenant_id: "t1".into(),
                user_id: "u1".into(),
                agent_id: "a1".into(),
                project_id: "p1".into(),
            }
        }

        fn base_input() -> SkillWriteInput {
            SkillWriteInput {
                scope: scope(),
                name: "fetch-issues".into(),
                description: "Fetch open GitHub issues for a repository".into(),
                body: "Use `github.api` to list open issues.".into(),
                compatibility: "brassclaw-class:llm".into(),
                license: "MIT".into(),
                allowed_tools: vec!["github.api".into()],
                version: "1.0.0".into(),
                keywords: vec!["github".into(), "issues".into()],
                exclude_keywords: vec![],
                patterns: vec![],
                tags: vec!["github".into()],
                max_context_tokens: 500,
                setup_marker: None,
                required_binaries: vec![],
                required_env: vec![],
                required_config: vec![],
                intent_examples: json!([
                    {"input": "list open issues", "class": 3},
                    {"input": "github issues", "class": 2}
                ]),
                extra_consumer_tags: vec![],
                content_hash: "abc123".into(),
                replaces_id: None,
                similarity_parent_id: None,
                source: "authored".into(),
            }
        }

        #[test]
        fn valid_input_passes() {
            let vr = validate_row(&base_input());
            assert!(vr.is_ok(), "expected ok, got errors: {:?}", vr.errors);
        }

        #[test]
        fn empty_name_fails() {
            let mut input = base_input();
            input.name = "".into();
            let vr = validate_row(&input);
            assert!(vr.errors.iter().any(|e| e.contains("must not be empty")));
        }

        #[test]
        fn unnormalizable_name_fails() {
            let mut input = base_input();
            input.name = "---".into();
            let vr = validate_row(&input);
            assert!(
                !vr.is_ok(),
                "expected validation failure for unnormalizable name"
            );
        }

        #[test]
        fn description_too_long_fails() {
            let mut input = base_input();
            input.description = "a".repeat(1025);
            let vr = validate_row(&input);
            assert!(vr.errors.iter().any(|e| e.contains("1024 chars")));
        }

        #[test]
        fn body_over_5000_tokens_fails() {
            let mut input = base_input();
            // 5001 * 4 = 20004 bytes → ~5001 tokens
            input.body = "a".repeat(20_004);
            let vr = validate_row(&input);
            assert!(vr.errors.iter().any(|e| e.contains("5000 token budget")));
        }

        #[test]
        fn intent_examples_must_be_array() {
            let mut input = base_input();
            input.intent_examples = json!({"bad": true});
            let vr = validate_row(&input);
            assert!(vr.errors.iter().any(|e| e.contains("must be a JSON array")));
        }

        #[test]
        fn intent_examples_wrong_class_fails() {
            let mut input = base_input();
            input.intent_examples = json!([{"input": "foo", "class": 9}]);
            let vr = validate_row(&input);
            assert!(
                vr.errors
                    .iter()
                    .any(|e| e.contains("'class' must be 1, 2, or 3"))
            );
        }

        #[test]
        fn malformed_consumer_tag_fails() {
            let mut input = base_input();
            input.extra_consumer_tags = vec!["bad-tag".into()];
            let vr = validate_row(&input);
            assert!(
                vr.errors
                    .iter()
                    .any(|e| e.contains("does not match required format"))
            );
        }

        #[test]
        fn invalid_source_fails() {
            let mut input = base_input();
            input.source = "unknown_source".into();
            let vr = validate_row(&input);
            assert!(
                vr.errors
                    .iter()
                    .any(|e| e.contains("not valid") && e.contains("source")),
                "expected source validation error, got: {:?}",
                vr.errors
            );
        }

        #[test]
        fn valid_source_passes() {
            for src in &["authored", "extracted", "migrated", "imported"] {
                let mut input = base_input();
                input.source = (*src).into();
                let vr = validate_row(&input);
                assert!(
                    vr.is_ok(),
                    "expected ok for source={src}, got errors: {:?}",
                    vr.errors
                );
            }
        }

        #[test]
        fn valid_consumer_tag_passes() {
            let mut input = base_input();
            input.extra_consumer_tags = vec!["04:scaffold".into()];
            let vr = validate_row(&input);
            assert!(vr.is_ok(), "{:?}", vr.errors);
        }

        #[test]
        fn class_defaults_seeded_for_llm() {
            let tags = class_default_consumer_tags(SkillClassCode::Llm);
            assert!(tags.contains(&"02:orchestrator".to_string()));
            assert!(tags.contains(&"03:llm".to_string()));
        }

        #[test]
        fn class_defaults_seeded_for_rusty() {
            let tags = class_default_consumer_tags(SkillClassCode::Rusty);
            assert!(tags.contains(&"00:rusty".to_string()));
            assert!(tags.contains(&"01:monty".to_string()));
        }

        #[test]
        fn class_code_from_compatibility() {
            assert_eq!(
                SkillClassCode::from_compatibility("brassclaw-class:rusty"),
                Some(SkillClassCode::Rusty)
            );
            assert_eq!(
                SkillClassCode::from_compatibility("brassclaw-class:monty"),
                Some(SkillClassCode::Monty)
            );
            assert_eq!(
                SkillClassCode::from_compatibility("brassclaw-class:llm"),
                Some(SkillClassCode::Llm)
            );
            assert_eq!(SkillClassCode::from_compatibility("something-else"), None);
        }

        #[test]
        fn immediate_write_reward_update_does_not_need_valid_content() {
            // RewardUpdate has no content fields — this test simply confirms
            // the struct default is all-None (no accidental required fields).
            let u = RewardUpdate::default();
            assert!(u.tier.is_none());
            assert!(u.usage_count.is_none());
        }

        #[test]
        fn row_carrying_validator_tag_not_returned() {
            // This is enforced at the SQL layer (NOT '05:validator' = ANY(consumer_tags)).
            // The test documents the contract; actual enforcement is verified by
            // the integration test in brassclaw_pg.
        }
    }
} // mod inner

#[cfg(feature = "db-store")]
pub use inner::{
    DbSkillRow, DbSkillStore, DbSkillStoreError, LocalValidationResult, RewardUpdate,
    SkillClassCode, SkillScope, SkillWriteInput, class_default_consumer_tags, validate_row,
};
