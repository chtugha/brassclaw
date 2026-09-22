//! Shared Postgres helper for the Settings UI component-list endpoints.
//!
//! Each Settings tab (Skills, Tools, Actions, Extensions, Orchestrators,
//! Scaffolds, Recipes, ToolSkills, PythonCode, ExtensionCatalogues) calls
//! `GET /api/settings/{type}` which delegates to one of the `list_settings_*`
//! methods on [`RebornServicesApi`].  This module provides a single
//! parameterised query that backs all of them so there is no per-table
//! boilerplate.
//!
//! # Table coverage
//!
//! | Tab                  | Table                        | class_codes |
//! |-----------------------|------------------------------|-------------|
//! | Skills                | `reborn_skills`              | 1, 2, 3     |
//! | Tools (v3)             | `reborn_tools`                | 0           |
//! | Actions                | `reborn_actions`              | 16          |
//! | Extensions             | `reborn_extensions_unified`   | 4–8         |
//! | Orchestrators          | `reborn_skills` (filtered)    | 10          |
//! | Scaffolds              | `reborn_skills` (filtered)    | 50          |
//! | Recipes                | `reborn_recipes`              | 21          |
//! | ToolSkills             | `reborn_tool_skills`          | 13          |
//! | PythonCode             | `reborn_python_code`          | 22          |
//! | ExtensionCatalogues    | `reborn_extension_catalogues` | 23          |
//!
//! Orchestrators and Scaffolds are not separate tables: per
//! `class_code_to_table` in `brassclaw_engine::memory::retrieval_source`
//! (the single source of truth for class→table dispatch), classes 10 and 50
//! are stored as rows in `reborn_skills` alongside classes 1–3. Their specs
//! set `class_code_filter` so the shared query adds `AND class_code = $2`.
//!
//! A table that genuinely does not exist in the connected database is a
//! backend configuration bug, not a normal empty state — [`Self::list`]
//! fails loud with [`SettingsListingError::MissingTable`] rather than
//! silently returning an empty list, so such drift cannot be masked (see
//! `AGENTS.md` §Database Rules "never suppress").
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
        /// The configured table does not exist in the connected database.
        /// This is a backend configuration bug (a Settings tab wired to a
        /// table that was never migrated, or a stale table name), never a
        /// normal empty state — callers must surface it as a hard error
        /// rather than silently rendering an empty tab.
        #[error("table {0} does not exist")]
        MissingTable(&'static str),
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
    ///
    /// `class_code_filter`, when set, adds `AND class_code = $2` to the
    /// query. This is how Orchestrators (10) and Scaffolds (50) — which are
    /// rows in the shared `reborn_skills` table, not separate tables — are
    /// distinguished from plain Skills (1–3).
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ComponentTableSpec {
        /// Postgres table name — **must be a static literal, never user input**.
        pub table: &'static str,
        /// Whether the table has a `version` column.
        pub has_version: bool,
        /// Optional `class_code` to filter rows by, for tables shared across
        /// multiple Settings tabs (currently only `reborn_skills`).
        pub class_code_filter: Option<i16>,
    }

    impl ComponentTableSpec {
        pub(crate) const fn new(table: &'static str, has_version: bool) -> Self {
            Self {
                table,
                has_version,
                class_code_filter: None,
            }
        }

        pub(crate) const fn with_class_filter(
            table: &'static str,
            has_version: bool,
            class_code: i16,
        ) -> Self {
            Self {
                table,
                has_version,
                class_code_filter: Some(class_code),
            }
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
    // Orchestrators (class 10) and Scaffolds (class 50) are rows in the
    // shared `reborn_skills` table, per `class_code_to_table` in
    // `brassclaw_engine::memory::retrieval_source` — there are no separate
    // `reborn_orchestrators`/`reborn_scaffolds` tables and never have been.
    pub(crate) const SPEC_ORCHESTRATORS: ComponentTableSpec =
        ComponentTableSpec::with_class_filter("reborn_skills", true, 10);
    pub(crate) const SPEC_SCAFFOLDS: ComponentTableSpec =
        ComponentTableSpec::with_class_filter("reborn_skills", true, 50);
    pub(crate) const SPEC_TOOL_SKILLS: ComponentTableSpec =
        ComponentTableSpec::new("reborn_tool_skills", false);
    pub(crate) const SPEC_PYTHON_CODE: ComponentTableSpec =
        ComponentTableSpec::new("reborn_python_code", false);
    pub(crate) const SPEC_EXTENSION_CATALOGUES: ComponentTableSpec =
        ComponentTableSpec::new("reborn_extension_catalogues", true);

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
        /// Fails loud with [`SettingsListingError::MissingTable`] if the table
        /// does not exist in the connected database. A missing table is a
        /// backend configuration bug (a Settings tab wired to a table name
        /// that was never migrated), never a normal empty state, so it must
        /// not be masked as an empty list — the caller (`RebornServicesApi`)
        /// maps this to a hard error response instead of a quietly-empty tab.
        pub(crate) async fn list(
            &self,
            spec: ComponentTableSpec,
        ) -> Result<SettingsListResponse, SettingsListingError> {
            let client = self.pool.get().await?;

            // Check that the table actually exists before issuing the real
            // query, so a missing table produces a clear diagnostic instead of
            // a raw Postgres "relation does not exist" error string.
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
                tracing::warn!(
                    table = spec.table,
                    "settings listing: configured table does not exist — this is a \
                     backend bug (Settings tab wired to a table that was never \
                     migrated), not a normal empty state"
                );
                return Err(SettingsListingError::MissingTable(spec.table));
            }

            // SECURITY: table name and version expression are static &'static str
            // literals set in ComponentTableSpec — never user-supplied input.
            let version_expr = if spec.has_version {
                "version"
            } else {
                "NULL::text"
            };

            let class_filter_sql = if spec.class_code_filter.is_some() {
                " AND class_code = $2"
            } else {
                ""
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
                   {class_filter_sql} \
                 ORDER BY class_code ASC, prompt_uid ASC \
                 LIMIT 500",
                table = spec.table,
            );

            let rows = if let Some(class_code) = spec.class_code_filter {
                client
                    .query(sql.as_str(), &[&self.tenant_id.as_str(), &class_code])
                    .await
            } else {
                client.query(sql.as_str(), &[&self.tenant_id.as_str()]).await
            }
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
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_RECIPES).await.map_err(map_listing_error)
        }
        async fn list_skills(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_SKILLS).await.map_err(map_listing_error)
        }
        async fn list_tools(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_TOOLS).await.map_err(map_listing_error)
        }
        async fn list_actions(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_ACTIONS).await.map_err(map_listing_error)
        }
        async fn list_extensions(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_EXTENSIONS).await.map_err(map_listing_error)
        }
        async fn list_orchestrators(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_ORCHESTRATORS)
                .await
                .map_err(map_listing_error)
        }
        async fn list_scaffolds(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_SCAFFOLDS).await.map_err(map_listing_error)
        }
        async fn list_tool_skills(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_TOOL_SKILLS)
                .await
                .map_err(map_listing_error)
        }
        async fn list_python_code(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_PYTHON_CODE)
                .await
                .map_err(map_listing_error)
        }
        async fn list_extension_catalogues(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_EXTENSION_CATALOGUES)
                .await
                .map_err(map_listing_error)
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
            SettingsListingError::MissingTable(table) => {
                brassclaw_product_workflow::SettingsListingError::MissingTable(table.to_string())
            }
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgSettingsListingService;
