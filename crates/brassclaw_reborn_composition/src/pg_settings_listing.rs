//! Shared Postgres helper for the Settings UI component-list endpoints.
//!
//! Each Settings tab (Skills, Tools, Actions, Extensions, Orchestrators,
//! Scaffolds) calls `GET /api/settings/{type}` which delegates to one of the
//! six `list_settings_*` methods on [`RebornServicesApi`].  This module
//! provides a single parameterised query that backs all six methods so there
//! is no per-table boilerplate.
//!
//! # Table coverage
//!
//! | Tab          | Table                      | class_codes |
//! |--------------|----------------------------|-------------|
//! | Skills       | `reborn_skills`            | 1, 2, 3     |
//! | Tools (v3)   | `reborn_tools`             | 0           |
//! | Actions      | `reborn_actions`           | 16          |
//! | Extensions   | `reborn_extensions_unified`| 4–8         |
//! | Orchestrators| `reborn_orchestrators`     | 10 (future) |
//! | Scaffolds    | `reborn_scaffolds`         | 50 (future) |
//!
//! Tables that do not exist yet (orchestrators, scaffolds) are detected via
//! `information_schema.tables` and return an empty list rather than an error.
//!
//! Feature-gated: `postgres`.

#[cfg(feature = "postgres")]
pub(crate) mod inner {
    use std::sync::Arc;

    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{SettingsComponentSummary, SettingsListResponse};

    /// Errors from the settings listing helper.
    #[derive(Debug, thiserror::Error)]
    pub(crate) enum SettingsListingError {
        #[error("db pool: {0}")]
        Pool(String),
        #[error("query error: {0}")]
        Query(String),
    }

    impl From<deadpool_postgres::PoolError> for SettingsListingError {
        fn from(e: deadpool_postgres::PoolError) -> Self {
            Self::Pool(e.to_string())
        }
    }

    impl From<tokio_postgres::Error> for SettingsListingError {
        fn from(e: tokio_postgres::Error) -> Self {
            Self::Query(e.to_string())
        }
    }

    /// Describes a single component table to query.
    ///
    /// `has_version` must be `true` only when the table physically has a
    /// `version TEXT` column (currently only `reborn_skills` and
    /// `reborn_extension_catalogues`).
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ComponentTableSpec {
        /// Postgres table name — **must be a static literal, never user input**.
        pub table: &'static str,
        /// Whether the table has a `version` column.
        pub has_version: bool,
    }

    impl ComponentTableSpec {
        pub(crate) const fn new(table: &'static str, has_version: bool) -> Self {
            Self { table, has_version }
        }
    }

    // ── Known table specs ─────────────────────────────────────────────────────

    pub(crate) const SPEC_SKILLS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_skills", true);
    pub(crate) const SPEC_TOOLS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_tools", false);
    pub(crate) const SPEC_ACTIONS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_actions", false);
    pub(crate) const SPEC_EXTENSIONS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_extensions_unified", false);
    pub(crate) const SPEC_RECIPES: ComponentTableSpec =
        ComponentTableSpec::new("reborn_recipes", false);
    // Future-phase tables — will return empty list if not yet migrated.
    pub(crate) const SPEC_ORCHESTRATORS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_orchestrators", false);
    pub(crate) const SPEC_SCAFFOLDS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_scaffolds", false);

    // ── Listing service ───────────────────────────────────────────────────────

    /// Postgres-backed settings listing service.
    ///
    /// Held as an `Arc` in the WebUI service bundle and passed into the
    /// `list_settings_*` method overrides in `RebornServices`.
    #[derive(Clone)]
    pub(crate) struct PgSettingsListingService {
        pool: Arc<PgPool>,
        tenant_id: String,
    }

    impl PgSettingsListingService {
        pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
            }
        }

        /// List component rows from the given table, scoped to this tenant.
        ///
        /// Returns an empty list if the table does not exist in the DB (graceful
        /// handling of future-phase tables like `reborn_orchestrators`).
        pub(crate) async fn list(
            &self,
            spec: ComponentTableSpec,
        ) -> Result<SettingsListResponse, SettingsListingError> {
            let client = self.pool.get().await?;

            // Check that the table actually exists before issuing the real
            // query — some tables are future-phase and may not be migrated yet.
            let exists_row = client
                .query_opt(
                    "SELECT 1 FROM information_schema.tables \
                     WHERE table_schema = 'public' \
                       AND table_type   = 'BASE TABLE' \
                       AND table_name   = $1",
                    &[&spec.table],
                )
                .await
                .map_err(|e| {
                    tracing::debug!(
                        table = spec.table,
                        error = %e,
                        "settings listing: information_schema check failed"
                    );
                    SettingsListingError::Query(e.to_string())
                })?;

            if exists_row.is_none() {
                tracing::debug!(
                    table = spec.table,
                    "settings listing: table does not exist yet, returning empty list"
                );
                return Ok(SettingsListResponse { items: Vec::new() });
            }

            // SECURITY: table name and version expression are static &'static str
            // literals set in ComponentTableSpec — never user-supplied input.
            let version_expr = if spec.has_version {
                "version"
            } else {
                "NULL::text"
            };

            let sql = format!(
                "SELECT id::text, \
                        name, \
                        class_code, \
                        prompt_uid, \
                        validation_status, \
                        COALESCE(description, '') AS description, \
                        {version_expr} AS version, \
                        COALESCE(consumer_tags, ARRAY[]::text[]) AS consumer_tags \
                 FROM {table} \
                 WHERE tenant_id = $1 \
                   AND validation_status != 'rejected' \
                 ORDER BY class_code ASC, prompt_uid ASC \
                 LIMIT 500",
                table = spec.table,
            );

            let rows = client
                .query(sql.as_str(), &[&self.tenant_id.as_str()])
                .await
                .map_err(|e| {
                    tracing::debug!(
                        table = spec.table,
                        error = %e,
                        "settings listing: query failed"
                    );
                    SettingsListingError::Query(e.to_string())
                })?;

            let items = rows
                .into_iter()
                .filter_map(|row| {
                    let id: String = row.try_get("id").ok()?;
                    let name: String = row.try_get("name").ok()?;
                    let class_code: i16 = row.try_get("class_code").ok()?;
                    let prompt_uid: i64 = row.try_get("prompt_uid").ok()?;
                    let validation_status: String = row.try_get("validation_status").ok()?;
                    let description: String = row.try_get("description").ok()?;
                    let version: Option<String> = row.try_get("version").ok().flatten();
                    let consumer_tags: Vec<String> =
                        row.try_get("consumer_tags").unwrap_or_default();
                    Some(SettingsComponentSummary {
                        id,
                        name,
                        class_code: class_code as u16,
                        prompt_uid: Some(prompt_uid as u64),
                        validation_status,
                        tier: None,
                        consumer_tags,
                        description: if description.is_empty() {
                            None
                        } else {
                            Some(description)
                        },
                        version,
                    })
                })
                .collect();

            Ok(SettingsListResponse { items })
        }
    }

    // ── SettingsListingService impl ───────────────────────────────────────────

    #[async_trait::async_trait]
    impl brassclaw_product_workflow::SettingsListingService for PgSettingsListingService {
        async fn list_recipes(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_RECIPES).await.map_err(map_listing_error)
        }
        async fn list_skills(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_SKILLS).await.map_err(map_listing_error)
        }
        async fn list_tools(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_TOOLS).await.map_err(map_listing_error)
        }
        async fn list_actions(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_ACTIONS).await.map_err(map_listing_error)
        }
        async fn list_extensions(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_EXTENSIONS).await.map_err(map_listing_error)
        }
        async fn list_orchestrators(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_ORCHESTRATORS).await.map_err(map_listing_error)
        }
        async fn list_scaffolds(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError> {
            self.list(SPEC_SCAFFOLDS).await.map_err(map_listing_error)
        }
    }

    fn map_listing_error(
        e: SettingsListingError,
    ) -> brassclaw_product_workflow::SettingsListingError {
        match e {
            SettingsListingError::Pool(msg) => {
                brassclaw_product_workflow::SettingsListingError::Unavailable(msg)
            }
            SettingsListingError::Query(msg) => {
                brassclaw_product_workflow::SettingsListingError::QueryFailed(msg)
            }
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgSettingsListingService;
