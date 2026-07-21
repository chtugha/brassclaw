//! Postgres-backed store for [`reborn_extensions_unified`] (Phase 4 Step 4.2).
//!
//! Provides CRUD over the `reborn_extensions_unified` table introduced in
//! [`V032__reborn_extensions_unified.sql`].  Class adapters project each row
//! into the shapes consumed by callers:
//!
//! | Class | Adapter | Consumed by |
//! |-------|---------|-------------|
//! | `mcp_server` / `mcp_client` | [`project_as_manifest_v2`] | Extension lifecycle |
//! | `rusty` | [`project_as_rusty_capability`] | Phase 2 tool surface |
//! | `monty` | [`project_as_recipe_stage`] | `RecipeStage` / `plan_library` |
//! | `llm` | [`project_as_prompt_template`] | Prompt assembler |
//! | `misc` | raw `payload` | Generic consumers |
//!
//! Only rows with `validation_status = 'validated'` AND
//! `'05:validator' != ANY(consumer_tags)` are returned by the fetch paths
//! (SEC-01 delivery filter from spec §3.9).
//!
//! The `postgres` feature gate is required — this module is a no-op when
//! compiled without it.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors raised by unified-extension store operations.
#[derive(Debug, Error)]
pub enum UnifiedStoreError {
    #[error("database pool error: {reason}")]
    Pool { reason: String },
    #[error("database query error: {reason}")]
    Db { reason: String },
    #[error("serialization error: {reason}")]
    Serialize { reason: String },
    #[error("invalid extension class: {class}")]
    InvalidClass { class: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> UnifiedStoreError {
    UnifiedStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> UnifiedStoreError {
    UnifiedStoreError::Db {
        reason: e.to_string(),
    }
}

fn map_json(e: serde_json::Error) -> UnifiedStoreError {
    UnifiedStoreError::Serialize {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Extension class enum
// ---------------------------------------------------------------------------

/// Class of a unified extension row (maps to the `reborn_extension_class`
/// Postgres enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionClass {
    Rusty,
    Monty,
    McpServer,
    McpClient,
    Llm,
    Misc,
}

impl ExtensionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rusty => "rusty",
            Self::Monty => "monty",
            Self::McpServer => "mcp_server",
            Self::McpClient => "mcp_client",
            Self::Llm => "llm",
            Self::Misc => "misc",
        }
    }

    /// Class code per spec §3.7.
    pub fn class_code(self) -> i16 {
        match self {
            Self::Rusty => 4,
            Self::Monty => 5,
            Self::McpServer => 6,
            Self::McpClient => 7,
            Self::Llm => 8,
            Self::Misc => 9,
        }
    }

    /// Default consumer tags per spec §3.9 (without `05:validator` which is
    /// added separately at insert time by application logic).
    pub fn default_consumer_tags(self) -> Vec<String> {
        match self {
            Self::Rusty => vec!["00:rusty".to_string()],
            Self::Monty => vec!["01:monty".to_string(), "02:orchestrator".to_string()],
            Self::McpServer => vec!["01:monty".to_string(), "02:orchestrator".to_string()],
            Self::McpClient => vec!["01:monty".to_string(), "02:orchestrator".to_string()],
            Self::Llm => vec!["03:llm".to_string()],
            Self::Misc => vec!["02:orchestrator".to_string()],
        }
    }

    fn try_from_str(s: &str) -> Result<Self, UnifiedStoreError> {
        match s {
            "rusty" => Ok(Self::Rusty),
            "monty" => Ok(Self::Monty),
            "mcp_server" => Ok(Self::McpServer),
            "mcp_client" => Ok(Self::McpClient),
            "llm" => Ok(Self::Llm),
            "misc" => Ok(Self::Misc),
            other => Err(UnifiedStoreError::InvalidClass {
                class: other.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A fully-decoded unified extension row.
#[derive(Debug, Clone)]
pub struct UnifiedExtension {
    pub id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub class: ExtensionClass,
    /// Extension-specific payload (manifest, steps, prompt template, etc.).
    pub payload: Value,
    /// Optional prior-knowledge text for the Solution Override path (§3.13).
    pub prior_knowledge_content: Option<String>,
    /// Whether this extension uses the Solution Override path.
    pub override_prompt_creation: bool,
    /// Class code (04-09).
    pub class_code: i16,
    /// Monotonic ordering key for prompt assembly.
    pub prompt_uid: i64,
    /// Consumer tags including optional `05:validator`.
    pub consumer_tags: Vec<String>,
    /// Intent examples array (JSON).
    pub intent_examples: Option<Value>,
    /// Current validation status.
    pub validation_status: String,
    /// Queue code for WebUI grouping.
    pub queue_code: Option<String>,
    /// Provenance source label.
    pub source: String,
    pub content_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl UnifiedExtension {
    /// Returns true iff the `05:validator` tag is present (component is in a
    /// validation queue and must not be delivered to consumers — §3.5.1).
    pub fn has_validator_tag(&self) -> bool {
        self.consumer_tags.iter().any(|t| t == "05:validator")
    }

    /// Returns true iff the row is deliverable: validated + no validator tag.
    pub fn is_deliverable(&self) -> bool {
        self.validation_status == "validated" && !self.has_validator_tag()
    }
}

/// Minimal data required to insert a new unified extension row.
#[derive(Debug, Clone)]
pub struct NewUnifiedExtension {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub class: ExtensionClass,
    pub payload: Value,
    pub prior_knowledge_content: Option<String>,
    pub override_prompt_creation: bool,
    /// Consumer tags.  The caller must include `05:validator` if the row is
    /// being inserted through the validation queue.
    pub consumer_tags: Vec<String>,
    pub intent_examples: Option<Value>,
    pub source: String,
}

// ---------------------------------------------------------------------------
// Store trait
// ---------------------------------------------------------------------------

/// Async CRUD contract for the unified extension store.
#[async_trait]
pub trait UnifiedExtensionStore: Send + Sync {
    /// Insert a new extension row.  Returns the assigned row id.
    async fn insert(&self, row: NewUnifiedExtension) -> Result<Uuid, UnifiedStoreError>;

    /// Fetch a single row by id + scope.  Returns `None` if not found or
    /// belongs to a different scope.
    async fn get(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<UnifiedExtension>, UnifiedStoreError>;

    /// List all rows for the scope, without delivery filtering.  Used by the
    /// WebUI admin and validation-queue paths.
    async fn list_all(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<UnifiedExtension>, UnifiedStoreError>;

    /// Fetch rows deliverable to a given consumer tag (spec §3.9):
    ///   - `validation_status = 'validated'`
    ///   - `consumer_tags` contains `consumer_tag`
    ///   - `consumer_tags` does NOT contain `05:validator`
    ///
    /// Ordered by `(class_code ASC, prompt_uid ASC)` for deterministic
    /// prompt assembly.
    async fn fetch_for_consumer(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        consumer_tag: &str,
    ) -> Result<Vec<UnifiedExtension>, UnifiedStoreError>;

    /// Update the validation status of a row.  `review_feedback` is stored as
    /// the latest reviewer note; `review_attempts` is managed by the caller.
    async fn update_validation_status(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        update: ValidationStatusUpdate<'_>,
    ) -> Result<(), UnifiedStoreError>;

    /// Pop the `05:validator` consumer tag from a row (Step-2 manual
    /// validation — §3.5.1).  This is a targeted update so concurrent
    /// consumers see the change atomically.
    async fn pop_validator_tag(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), UnifiedStoreError>;

    /// Update the payload of a row (content edit, gated by the caller's
    /// validation pipeline).
    async fn update_payload(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        update: PayloadUpdate,
    ) -> Result<(), UnifiedStoreError>;

    /// Wipe a row's provenance and creation-process data, then delete the row.
    /// Per §3.5.1 Q4 terminal wipe: never deletes thread/message/event data.
    async fn wipe(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), UnifiedStoreError>;
}

/// Grouped parameters for [`UnifiedExtensionStore::update_validation_status`].
#[derive(Debug)]
pub struct ValidationStatusUpdate<'a> {
    pub validation_status: &'a str,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub queue_code: Option<String>,
}

/// Grouped parameters for [`UnifiedExtensionStore::update_payload`].
#[derive(Debug)]
pub struct PayloadUpdate {
    pub payload: Value,
    pub prior_knowledge_content: Option<String>,
    pub content_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

/// Postgres-backed implementation of [`UnifiedExtensionStore`].
pub struct PgUnifiedExtensionStore {
    pool: Arc<PgPool>,
}

impl PgUnifiedExtensionStore {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

/// Decode a `tokio_postgres::Row` into a [`UnifiedExtension`].
///
/// Column order must match the SELECT list used in every query below.
fn decode_row(row: &tokio_postgres::Row) -> Result<UnifiedExtension, UnifiedStoreError> {
    let id: Uuid = row.get(0);
    let tenant_id: String = row.get(1);
    let user_id: String = row.get(2);
    let agent_id: String = row.get(3);
    let project_id: String = row.get(4);
    let name: String = row.get(5);
    let description: String = row.get(6);
    let class_str: String = row.get(7);
    let payload: Value = row.get(8);
    let prior_knowledge_content: Option<String> = row.get(9);
    let override_prompt_creation: bool = row.get(10);
    let class_code: i16 = row.get(11);
    let prompt_uid: i64 = row.get(12);
    let consumer_tags: Vec<String> = row.get(13);
    let intent_examples: Option<Value> = row.get(14);
    let validation_status: String = row.get(15);
    let queue_code: Option<String> = row.get(16);
    let source: String = row.get(17);
    let content_hash: Option<String> = row.get(18);
    let created_at: chrono::DateTime<chrono::Utc> = row.get(19);
    let updated_at: chrono::DateTime<chrono::Utc> = row.get(20);

    let class = ExtensionClass::try_from_str(&class_str)?;

    Ok(UnifiedExtension {
        id,
        tenant_id,
        user_id,
        agent_id,
        project_id,
        name,
        description,
        class,
        payload,
        prior_knowledge_content,
        override_prompt_creation,
        class_code,
        prompt_uid,
        consumer_tags,
        intent_examples,
        validation_status,
        queue_code,
        source,
        content_hash,
        created_at,
        updated_at,
    })
}

/// The canonical SELECT column list — order must match [`decode_row`].
const SELECT_COLS: &str = "
    id, tenant_id, user_id, agent_id, project_id,
    name, description, class::TEXT, payload,
    prior_knowledge_content, override_prompt_creation,
    class_code, prompt_uid, consumer_tags, intent_examples,
    validation_status, queue_code, source, content_hash,
    created_at, updated_at
";

#[async_trait]
impl UnifiedExtensionStore for PgUnifiedExtensionStore {
    async fn insert(&self, row: NewUnifiedExtension) -> Result<Uuid, UnifiedStoreError> {
        let payload_val = serde_json::to_value(&row.payload).map_err(map_json)?;
        let intent_val = row
            .intent_examples
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(map_json)?;

        let class_code = row.class.class_code();
        let class_str = row.class.as_str();

        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_extensions_unified
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, class, payload,
                     prior_knowledge_content, override_prompt_creation,
                     class_code, consumer_tags, intent_examples, source)
                 VALUES ($1,$2,$3,$4,$5,$6,$7::reborn_extension_class,$8,$9,$10,$11,$12,$13,$14)
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &class_str,
                    &payload_val,
                    &row.prior_knowledge_content,
                    &row.override_prompt_creation,
                    &class_code,
                    &row.consumer_tags,
                    &intent_val,
                    &row.source,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(db_row.get(0))
    }

    async fn get(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<UnifiedExtension>, UnifiedStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {SELECT_COLS} FROM reborn_extensions_unified
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5"
        );
        let row = client
            .query_opt(&q, &[&id, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_row).transpose()
    }

    async fn list_all(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<UnifiedExtension>, UnifiedStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {SELECT_COLS} FROM reborn_extensions_unified
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
             ORDER BY class_code ASC, prompt_uid ASC"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_row).collect()
    }

    async fn fetch_for_consumer(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        consumer_tag: &str,
    ) -> Result<Vec<UnifiedExtension>, UnifiedStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        // SEC-01 delivery filter (§3.9):
        //   • validation_status = 'validated'
        //   • consumer_tags contains consumer_tag
        //   • consumer_tags does NOT contain '05:validator'
        let q = format!(
            "SELECT {SELECT_COLS} FROM reborn_extensions_unified
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
               AND validation_status = 'validated'
               AND $5 = ANY(consumer_tags)
               AND NOT ('05:validator' = ANY(consumer_tags))
             ORDER BY class_code ASC, prompt_uid ASC"
        );
        let rows = client
            .query(
                &q,
                &[&tenant_id, &user_id, &agent_id, &project_id, &consumer_tag],
            )
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_row).collect()
    }

    async fn update_validation_status(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        update: ValidationStatusUpdate<'_>,
    ) -> Result<(), UnifiedStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE reborn_extensions_unified
                 SET validation_status = $1,
                     validation_errors = $2,
                     review_feedback   = COALESCE($3, review_feedback),
                     queue_code        = $4
                 WHERE id = $5
                   AND tenant_id = $6 AND user_id = $7
                   AND agent_id  = $8 AND project_id = $9",
                &[
                    &update.validation_status,
                    &update.validation_errors,
                    &update.review_feedback,
                    &update.queue_code,
                    &id,
                    &tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn pop_validator_tag(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), UnifiedStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        // Remove '05:validator' from consumer_tags atomically (§3.5.1).
        client
            .execute(
                "UPDATE reborn_extensions_unified
                 SET consumer_tags = array_remove(consumer_tags, '05:validator')
                 WHERE id = $1
                   AND tenant_id = $2 AND user_id = $3
                   AND agent_id  = $4 AND project_id = $5",
                &[&id, &tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn update_payload(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        update: PayloadUpdate,
    ) -> Result<(), UnifiedStoreError> {
        let payload_val = serde_json::to_value(&update.payload).map_err(map_json)?;
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE reborn_extensions_unified
                 SET payload                 = $1,
                     prior_knowledge_content = $2,
                     content_hash            = $3
                 WHERE id = $4
                   AND tenant_id = $5 AND user_id = $6
                   AND agent_id  = $7 AND project_id = $8",
                &[
                    &payload_val,
                    &update.prior_knowledge_content,
                    &update.content_hash,
                    &id,
                    &tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn wipe(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), UnifiedStoreError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        // Q4 terminal wipe (§3.5.1): clear provenance columns then delete the
        // row.  Both ops run in a single transaction so a partial failure
        // cannot leave a stripped-but-not-deleted row.
        let tx = client.transaction().await.map_err(map_pg)?;
        tx.execute(
            "UPDATE reborn_extensions_unified
             SET source             = 'wiped',
                 content_hash       = NULL,
                 similarity_parent_id = NULL,
                 parent_mission_id  = NULL,
                 review_feedback    = NULL
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5",
            &[&id, &tenant_id, &user_id, &agent_id, &project_id],
        )
        .await
        .map_err(map_pg)?;
        tx.execute(
            "DELETE FROM reborn_extensions_unified
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5",
            &[&id, &tenant_id, &user_id, &agent_id, &project_id],
        )
        .await
        .map_err(map_pg)?;
        tx.commit().await.map_err(map_pg)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Class adapters
// ---------------------------------------------------------------------------

/// Project a unified extension row into the `payload` JSON suitable for
/// constructing an [`ExtensionManifestV2`] from TOML.
///
/// For `mcp_server` / `mcp_client` rows, `payload` stores the raw TOML
/// manifest text under the `"manifest_toml"` key so the existing
/// [`brassclaw_extensions::v2::ExtensionManifestV2`] parser can handle it
/// without changes.
pub fn project_as_manifest_v2(ext: &UnifiedExtension) -> Option<&str> {
    ext.payload.get("manifest_toml").and_then(|v| v.as_str())
}

/// Extract the capability name and parameter schema from a `rusty`-class row.
///
/// Returns a `(tool_name, param_schema)` pair suitable for feeding the Phase 2
/// tool-capability surface.  Returns `None` if the payload does not carry the
/// expected keys (e.g. the row has not been fully populated yet).
pub fn project_as_rusty_capability(
    ext: &UnifiedExtension,
) -> Option<(String, Option<serde_json::Value>)> {
    if ext.class != ExtensionClass::Rusty {
        return None;
    }
    let tool_name = ext.payload.get("tool_name").and_then(|v| v.as_str())?;
    let schema = ext.payload.get("param_schema").cloned();
    Some((tool_name.to_string(), schema))
}

/// Extract the recipe/plan orchestration body from a `monty`-class row.
///
/// Returns the raw body text that `RecipeStage` / `plan_library` expects.
/// For DocPlan-derived rows this is the thin orchestration recipe body.
pub fn project_as_recipe_stage(ext: &UnifiedExtension) -> Option<&str> {
    if ext.class != ExtensionClass::Monty {
        return None;
    }
    ext.payload
        .get("body")
        .or_else(|| ext.payload.get("recipe_body"))
        .and_then(|v| v.as_str())
}

/// Extract the prompt template from an `llm`-class row.
///
/// Returns the prompt template text for injection into the LLM prompt.
pub fn project_as_prompt_template(ext: &UnifiedExtension) -> Option<&str> {
    if ext.class != ExtensionClass::Llm {
        return None;
    }
    ext.payload
        .get("prompt_template")
        .or_else(|| ext.payload.get("body"))
        .and_then(|v| v.as_str())
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use PgUnifiedExtensionStore as PgUnifiedStore;
