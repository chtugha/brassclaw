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
        /// The requested component id is not a UUID. Every catalog table
        /// keys on `id UUID`, so this is a malformed request rather than a
        /// missing row — rejected in Rust instead of letting the cast fail
        /// inside Postgres.
        #[error("component id is not a valid UUID: {0}")]
        InvalidId(String),
        /// No row with that id in this tenant's scope for the requested spec.
        #[error("component not found: {0}")]
        NotFound(String),
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
    ///
    /// `tier_expr` is the SQL expression yielding the execution tier shown in
    /// the list (`"0"` / `"1"` / `NULL`). Only Recipes have one.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ComponentTableSpec {
        /// Postgres table name — **must be a static literal, never user input**.
        pub table: &'static str,
        /// Whether the table has a `version` column.
        pub has_version: bool,
        /// Optional `class_code` to filter rows by, for tables shared across
        /// multiple Settings tabs (currently only `reborn_skills`).
        pub class_code_filter: Option<i16>,
        /// SQL expression producing the execution tier as text — **must be a
        /// static literal, never user input**. `NULL::text` for every table
        /// where the concept does not apply.
        pub tier_expr: &'static str,
    }

    impl ComponentTableSpec {
        pub(crate) const fn new(table: &'static str, has_version: bool) -> Self {
            Self {
                table,
                has_version,
                class_code_filter: None,
                tier_expr: NO_TIER_EXPR,
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
                tier_expr: NO_TIER_EXPR,
            }
        }

        pub(crate) const fn with_tier_expr(self, tier_expr: &'static str) -> Self {
            Self { tier_expr, ..self }
        }
    }

    /// Tier expression for tables where execution tier is not a concept.
    const NO_TIER_EXPR: &str = "NULL::text";

    /// Execution tier of a Recipe, derived from the canonical `steps` JSONB
    /// shape `{llm_call_required, tier, rust_steps, orchestrator_steps}`
    /// (see `seed_builtin_host.rs`). `llm_call_required == false` is Tier 0,
    /// `true` is Tier 1.
    ///
    /// This is **not** the `reborn_recipes.tier` column: that one is the
    /// reward tier (`seedling` | `growing` | `mature` | `candidate`, V033), a
    /// different concept that must not be shown as an execution tier.
    ///
    /// A recipe whose `steps` is not an object (the legacy array shape, or a
    /// row written before the canonical shape) yields `NULL` rather than a
    /// guessed tier.
    const RECIPE_TIER_EXPR: &str = "CASE \
         WHEN jsonb_typeof(steps) = 'object' AND steps->>'llm_call_required' = 'false' \
             THEN '0' \
         WHEN jsonb_typeof(steps) = 'object' AND steps->>'llm_call_required' = 'true' \
             THEN '1' \
         ELSE NULL \
     END";

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
        ComponentTableSpec::new("reborn_recipes", false).with_tier_expr(RECIPE_TIER_EXPR);
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

        /// Fail loud with [`SettingsListingError::MissingTable`] when the
        /// spec's table is absent from the connected database.
        ///
        /// Run before the real query so a missing table produces a clear
        /// diagnostic instead of a raw Postgres "relation does not exist"
        /// error string.
        async fn ensure_table_exists(
            client: &deadpool_postgres::Client,
            spec: ComponentTableSpec,
        ) -> Result<(), SettingsListingError> {
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

            Ok(())
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
            Self::ensure_table_exists(&client, spec).await?;

            // SECURITY: table name, version expression and tier expression are
            // static &'static str literals set in ComponentTableSpec — never
            // user-supplied input.
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
                        {tier_expr} AS exec_tier, \
                        COALESCE(consumer_tags, ARRAY[]::text[]) AS consumer_tags \
                 FROM {table} \
                 WHERE tenant_id = $1 \
                   AND validation_status != 'rejected' \
                   {class_filter_sql} \
                 ORDER BY class_code ASC, prompt_uid ASC \
                 LIMIT 500",
                table = spec.table,
                tier_expr = spec.tier_expr,
            );

            let rows = if let Some(class_code) = spec.class_code_filter {
                client
                    .query(sql.as_str(), &[&self.tenant_id.as_str(), &class_code])
                    .await
            } else {
                client
                    .query(sql.as_str(), &[&self.tenant_id.as_str()])
                    .await
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
                    let tier: Option<String> = row.try_get("exec_tier").ok().flatten();
                    let consumer_tags: Vec<String> =
                        row.try_get("consumer_tags").unwrap_or_default();
                    Some(SettingsComponentSummary {
                        id,
                        name,
                        class_code: class_code as u16,
                        prompt_uid: Some(prompt_uid as u64),
                        validation_status,
                        tier,
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

        /// Read one component row in full, scoped to this tenant.
        ///
        /// The row is returned as opaque JSON (`to_jsonb(t)`) so the detail
        /// pane gets every class-specific column — a Recipe's
        /// `step_descriptions`, a PythonCode body, a ToolSkill's
        /// `param_schema` — without this crate tracking the engine's columns.
        ///
        /// Unlike [`Self::list`] this does not filter `validation_status`:
        /// the row is addressed by explicit id and the Q2 reviewer must be
        /// able to open rejected and pending components.
        pub(crate) async fn get(
            &self,
            spec: ComponentTableSpec,
            id: &str,
        ) -> Result<(i16, serde_json::Value), SettingsListingError> {
            let uuid = uuid::Uuid::parse_str(id)
                .map_err(|_| SettingsListingError::InvalidId(id.to_string()))?;

            let client = self.pool.get().await?;
            Self::ensure_table_exists(&client, spec).await?;

            // SECURITY: the table name is a static &'static str literal set in
            // ComponentTableSpec — never user-supplied input. The id arrives
            // as a parsed Uuid bind parameter.
            let class_filter_sql = if spec.class_code_filter.is_some() {
                " AND class_code = $3"
            } else {
                ""
            };

            let sql = format!(
                "SELECT class_code, to_jsonb(t) AS component \
                 FROM {table} t \
                 WHERE tenant_id = $1 \
                   AND id = $2 \
                   {class_filter_sql} \
                 LIMIT 1",
                table = spec.table,
            );

            let row = if let Some(class_code) = spec.class_code_filter {
                client
                    .query_opt(
                        sql.as_str(),
                        &[&self.tenant_id.as_str(), &uuid, &class_code],
                    )
                    .await
            } else {
                client
                    .query_opt(sql.as_str(), &[&self.tenant_id.as_str(), &uuid])
                    .await
            }
            .map_err(|e| {
                tracing::debug!(
                    table = spec.table,
                    error = %e,
                    "settings detail: query failed"
                );
                SettingsListingError::Query(e.to_string())
            })?
            .ok_or_else(|| SettingsListingError::NotFound(id.to_string()))?;

            let class_code: i16 = row
                .try_get("class_code")
                .map_err(|e| SettingsListingError::Query(e.to_string()))?;
            let component: serde_json::Value = row
                .try_get("component")
                .map_err(|e| SettingsListingError::Query(e.to_string()))?;

            Ok((class_code, component))
        }

        /// Read every catalog node and every reference between them.
        ///
        /// Three independent edge sources are concatenated:
        /// 1. Recipe `step_descriptions` `include[]` UUIDs — the wiring that
        ///    gives a Recipe its meaning (a `rust` step pre-loading a
        ///    ToolSkill, the `orchestrator` step running the PythonCode that
        ///    dispatches it).
        /// 2. `reborn_tool_skills.tool_name` → `reborn_tools.name`, the
        ///    binding a ToolSkill declares.
        /// 3. `dependency_registry` entries on any catalog table.
        ///
        /// Unlike [`Self::list`] this does not check table existence first:
        /// the graph spans seven tables and a per-table probe would add seven
        /// round trips to a read the UI issues once per Settings visit. A
        /// genuinely missing table still fails loud, as a query error.
        pub(crate) async fn graph(
            &self,
        ) -> Result<
            (
                Vec<brassclaw_product_workflow::SettingsComponentGraphNode>,
                Vec<brassclaw_product_workflow::SettingsComponentGraphEdge>,
            ),
            SettingsListingError,
        > {
            let client = self.pool.get().await?;
            let tenant = self.tenant_id.as_str();

            let nodes = client
                .query(NODES_SQL, &[&tenant])
                .await
                .map_err(|e| graph_query_error("nodes", e))?
                .into_iter()
                .filter_map(|row| {
                    let class_code: i16 = row.try_get("class_code").ok()?;
                    Some(brassclaw_product_workflow::SettingsComponentGraphNode {
                        id: row.try_get("id").ok()?,
                        name: row.try_get("name").ok()?,
                        class_code: class_code as u16,
                        validation_status: row.try_get("validation_status").ok()?,
                    })
                })
                .collect();

            let mut edges = Vec::new();
            for (kind, sql) in [
                (EDGE_KIND_STEP_INCLUDE, STEP_INCLUDE_EDGES_SQL),
                (EDGE_KIND_TOOL_BINDING, TOOL_BINDING_EDGES_SQL),
                (EDGE_KIND_DEPENDENCY, DEPENDENCY_EDGES_SQL),
            ] {
                let rows = client
                    .query(sql, &[&tenant])
                    .await
                    .map_err(|e| graph_query_error(kind, e))?;
                for row in rows {
                    let (Ok(from), Ok(to)) = (row.try_get("from_id"), row.try_get("to_id")) else {
                        continue;
                    };
                    edges.push(brassclaw_product_workflow::SettingsComponentGraphEdge {
                        from,
                        to,
                        kind: kind.to_string(),
                        channel: row.try_get("channel").ok().flatten(),
                        step_ref: row.try_get("step_ref").ok().flatten(),
                        step_label: row.try_get("step_label").ok().flatten(),
                    });
                }
            }

            Ok((nodes, edges))
        }
    }

    fn graph_query_error(part: &str, e: tokio_postgres::Error) -> SettingsListingError {
        tracing::debug!(part, error = %e, "settings component graph: query failed");
        SettingsListingError::Query(e.to_string())
    }

    // ── Component graph SQL ───────────────────────────────────────────────────

    pub(crate) const EDGE_KIND_STEP_INCLUDE: &str = "step_include";
    pub(crate) const EDGE_KIND_TOOL_BINDING: &str = "tool_binding";
    pub(crate) const EDGE_KIND_DEPENDENCY: &str = "dependency";

    /// Every catalog row that can be an endpoint of a reference.
    ///
    /// `reborn_extensions_unified` is deliberately absent: it holds
    /// installable packages (classes 4–8), not class-coded catalog
    /// components, and nothing in `step_descriptions` points at one.
    const NODES_SQL: &str = "\
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_skills              WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_tools               WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_actions             WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_recipes             WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_tool_skills         WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_python_code         WHERE tenant_id = $1 \
        UNION ALL \
        SELECT id::text AS id, name, class_code, validation_status FROM reborn_extension_catalogues WHERE tenant_id = $1 \
        LIMIT 5000";

    /// Recipe → included component, one row per `include[]` entry.
    ///
    /// `step_descriptions` is stored in two shapes: `StepDescriptionEntry[]`
    /// with a nested `steps[]` (what `builtin_bootstrap` writes), and the
    /// canonical flat array of step objects from the v3 docs. The `steps` CTE
    /// normalises both — an entry with a `steps` array contributes its
    /// children, an entry without contributes itself. Channel lives in
    /// `knowledge` in the nested shape and `channel` in the flat one.
    ///
    /// Every `jsonb_array_elements` is guarded by a `CASE` rather than a
    /// `WHERE`: a lateral is evaluated before the `WHERE` clause, so an
    /// unexpected scalar would abort the whole query instead of being skipped.
    const STEP_INCLUDE_EDGES_SQL: &str = "\
        WITH entries AS ( \
            SELECT r.id::text AS recipe_id, e.value AS entry \
              FROM reborn_recipes r \
              CROSS JOIN LATERAL jsonb_array_elements( \
                  CASE WHEN jsonb_typeof(r.step_descriptions) = 'array' \
                       THEN r.step_descriptions ELSE '[]'::jsonb END) AS e(value) \
             WHERE r.tenant_id = $1 \
        ), \
        steps AS ( \
            SELECT recipe_id, s.value AS step \
              FROM entries \
              CROSS JOIN LATERAL jsonb_array_elements( \
                  CASE WHEN jsonb_typeof(entry->'steps') = 'array' \
                       THEN entry->'steps' ELSE '[]'::jsonb END) AS s(value) \
            UNION ALL \
            SELECT recipe_id, entry \
              FROM entries \
             WHERE jsonb_typeof(entry->'steps') IS DISTINCT FROM 'array' \
        ) \
        SELECT recipe_id                                            AS from_id, \
               inc.value #>> '{}'                                   AS to_id, \
               COALESCE(step->>'channel', step->>'knowledge')       AS channel, \
               COALESCE(step->>'step_id', step->>'stepnumber', \
                        step->>'desc_idx')                          AS step_ref, \
               COALESCE(step->>'label', step->>'goal')              AS step_label \
          FROM steps \
          CROSS JOIN LATERAL jsonb_array_elements( \
              CASE WHEN jsonb_typeof(step->'include') = 'array' \
                   THEN step->'include' ELSE '[]'::jsonb END) AS inc(value) \
         WHERE jsonb_typeof(inc.value) = 'string' \
           AND inc.value #>> '{}' <> '' \
         LIMIT 20000";

    /// ToolSkill → the class-0 Tool its `tool_name` names.
    ///
    /// A `tool_name` matching no row yields no edge: the join is the check,
    /// and an unbound ToolSkill simply shows no outbound tool reference.
    const TOOL_BINDING_EDGES_SQL: &str = "\
        SELECT ts.id::text  AS from_id, \
               t.id::text   AS to_id, \
               NULL::text   AS channel, \
               NULL::text   AS step_ref, \
               ts.tool_name AS step_label \
          FROM reborn_tool_skills ts \
          JOIN reborn_tools t \
            ON t.tenant_id = ts.tenant_id \
           AND t.name      = ts.tool_name \
         WHERE ts.tenant_id = $1 \
         LIMIT 5000";

    /// Declared `dependency_registry` entries, whatever table they sit on.
    const DEPENDENCY_EDGES_SQL: &str = "\
        WITH src AS ( \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_skills               WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_tools                WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_actions              WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_recipes              WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_tool_skills          WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_python_code          WHERE tenant_id = $1 \
            UNION ALL \
            SELECT id::text AS id, dependency_registry AS deps FROM reborn_extension_catalogues WHERE tenant_id = $1 \
        ) \
        SELECT src.id                    AS from_id, \
               d.value->>'component_id'  AS to_id, \
               NULL::text                AS channel, \
               d.value->>'idx'           AS step_ref, \
               d.value->>'label'         AS step_label \
          FROM src \
          CROSS JOIN LATERAL jsonb_array_elements( \
              CASE WHEN jsonb_typeof(src.deps) = 'array' \
                   THEN src.deps ELSE '[]'::jsonb END) AS d(value) \
         WHERE d.value->>'component_id' IS NOT NULL \
         LIMIT 20000";

    /// Map a catalog tab onto the table spec backing it.
    fn spec_for(
        component_type: brassclaw_product_workflow::SettingsComponentType,
    ) -> ComponentTableSpec {
        use brassclaw_product_workflow::SettingsComponentType as T;
        match component_type {
            T::Skills => SPEC_SKILLS,
            T::Tools => SPEC_TOOLS,
            T::Actions => SPEC_ACTIONS,
            T::Orchestrators => SPEC_ORCHESTRATORS,
            T::Scaffolds => SPEC_SCAFFOLDS,
            T::Recipes => SPEC_RECIPES,
            T::ToolSkills => SPEC_TOOL_SKILLS,
            T::PythonCode => SPEC_PYTHON_CODE,
            T::ExtensionCatalogues => SPEC_EXTENSION_CATALOGUES,
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
            self.list(SPEC_TOOL_SKILLS).await.map_err(map_listing_error)
        }
        async fn list_python_code(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_PYTHON_CODE).await.map_err(map_listing_error)
        }
        async fn list_extension_catalogues(
            &self,
        ) -> Result<SettingsListResponse, brassclaw_product_workflow::SettingsListingError>
        {
            self.list(SPEC_EXTENSION_CATALOGUES)
                .await
                .map_err(map_listing_error)
        }

        async fn get_component(
            &self,
            component_type: brassclaw_product_workflow::SettingsComponentType,
            id: &str,
        ) -> Result<
            brassclaw_product_workflow::SettingsComponentDetail,
            brassclaw_product_workflow::SettingsListingError,
        > {
            let (class_code, component) = self
                .get(spec_for(component_type), id)
                .await
                .map_err(map_listing_error)?;
            Ok(brassclaw_product_workflow::SettingsComponentDetail {
                id: id.to_string(),
                class_code: class_code as u16,
                component,
            })
        }

        async fn component_graph(
            &self,
        ) -> Result<
            brassclaw_product_workflow::SettingsComponentGraph,
            brassclaw_product_workflow::SettingsListingError,
        > {
            let (nodes, edges) = self.graph().await.map_err(map_listing_error)?;
            Ok(brassclaw_product_workflow::SettingsComponentGraph { nodes, edges })
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
            SettingsListingError::InvalidId(id) => {
                brassclaw_product_workflow::SettingsListingError::InvalidId(id)
            }
            SettingsListingError::NotFound(id) => {
                brassclaw_product_workflow::SettingsListingError::NotFound(id)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Only Recipes carry an execution tier; every other tab must select
        /// `NULL::text` so the list cannot show a tier the class has no
        /// concept of.
        #[test]
        fn only_recipes_have_a_tier_expression() {
            assert_eq!(SPEC_RECIPES.tier_expr, RECIPE_TIER_EXPR);
            for spec in [
                SPEC_SKILLS,
                SPEC_TOOLS,
                SPEC_ACTIONS,
                SPEC_EXTENSIONS,
                SPEC_ORCHESTRATORS,
                SPEC_SCAFFOLDS,
                SPEC_TOOL_SKILLS,
                SPEC_PYTHON_CODE,
                SPEC_EXTENSION_CATALOGUES,
            ] {
                assert_eq!(spec.tier_expr, NO_TIER_EXPR, "table {}", spec.table);
            }
        }

        /// Every catalog tab must resolve to a spec whose table matches the
        /// one its list endpoint queries — a detail read that hit a
        /// different table than the list would return unrelated rows.
        #[test]
        fn spec_for_matches_the_list_endpoint_tables() {
            use brassclaw_product_workflow::SettingsComponentType as T;
            assert_eq!(spec_for(T::Skills).table, SPEC_SKILLS.table);
            assert_eq!(spec_for(T::Tools).table, SPEC_TOOLS.table);
            assert_eq!(spec_for(T::Actions).table, SPEC_ACTIONS.table);
            assert_eq!(spec_for(T::Orchestrators).class_code_filter, Some(10));
            assert_eq!(spec_for(T::Scaffolds).class_code_filter, Some(50));
            assert_eq!(spec_for(T::Recipes).table, SPEC_RECIPES.table);
            assert_eq!(spec_for(T::ToolSkills).table, SPEC_TOOL_SKILLS.table);
            assert_eq!(spec_for(T::PythonCode).table, SPEC_PYTHON_CODE.table);
            assert_eq!(
                spec_for(T::ExtensionCatalogues).table,
                SPEC_EXTENSION_CATALOGUES.table
            );
        }

        // ── Postgres integration tests (skip when docker is unavailable) ──
        //
        // Mirrors the `pg_python_code_store.rs` harness: an isolated
        // Postgres-16 testcontainer with the full migration set, returning
        // early (pass) when docker/testcontainers is unavailable.
        mod pg {
            use super::*;
            use brassclaw_product_workflow::SettingsListingService as _;

            const TENANT: &str = "tenant-settings-detail";

            struct PgRig {
                // Held for the test's lifetime so the container stays up.
                _container: testcontainers_modules::testcontainers::ContainerAsync<
                    testcontainers_modules::postgres::Postgres,
                >,
                service: PgSettingsListingService,
                pool: Arc<PgPool>,
            }

            async fn pg_rig_or_skip() -> Option<PgRig> {
                use deadpool_postgres::{Manager, Pool};
                use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

                let image = testcontainers_modules::postgres::Postgres::default()
                    .with_db_name("brassclaw_test")
                    .with_user("postgres")
                    .with_password("postgres")
                    .with_tag("16-alpine");
                let container = match image.start().await {
                    Ok(c) => c,
                    Err(error) => {
                        eprintln!(
                            "skipping pg_settings_listing pg tests: docker/testcontainers unavailable ({error})"
                        );
                        return None;
                    }
                };
                let host = match container.get_host().await {
                    Ok(h) => h,
                    Err(error) => {
                        eprintln!("skipping pg_settings_listing pg tests: no host ({error})");
                        return None;
                    }
                };
                let port = match container.get_host_port_ipv4(5432).await {
                    Ok(p) => p,
                    Err(error) => {
                        eprintln!("skipping pg_settings_listing pg tests: no port ({error})");
                        return None;
                    }
                };
                let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
                let cfg: tokio_postgres::Config = url.parse().expect("testcontainer url parses");
                let manager = Manager::new(cfg, tokio_postgres::NoTls);
                let pool = Pool::builder(manager)
                    .max_size(4)
                    .build()
                    .expect("Postgres pool must build");
                brassclaw_pg::migrations::run_migrations(&pool)
                    .await
                    .expect("migrations must apply");
                let pool = Arc::new(pool);
                Some(PgRig {
                    _container: container,
                    service: PgSettingsListingService::new(Arc::clone(&pool), TENANT),
                    pool,
                })
            }

            /// Insert a recipe whose canonical `steps` object carries
            /// `llm_call_required`, returning its id.
            async fn insert_recipe(rig: &PgRig, name: &str, llm_call_required: bool) -> String {
                let client = rig.pool.get().await.expect("pool");
                let steps = serde_json::json!({
                    "llm_call_required": llm_call_required,
                    "rust_steps": [],
                    "orchestrator_steps": [],
                });
                let row = client
                    .query_one(
                        "INSERT INTO reborn_recipes \
                             (tenant_id, user_id, agent_id, project_id, name, description, steps) \
                         VALUES ($1, 'u', 'a', 'p', $2, 'detail fixture', $3) \
                         RETURNING id::text",
                        &[&TENANT, &name, &steps],
                    )
                    .await
                    .expect("recipe insert");
                row.get::<_, String>(0)
            }

            async fn insert_python_code(rig: &PgRig, name: &str, content: &str) -> String {
                let client = rig.pool.get().await.expect("pool");
                let row = client
                    .query_one(
                        "INSERT INTO reborn_python_code \
                             (tenant_id, user_id, agent_id, project_id, name, description, content) \
                         VALUES ($1, 'u', 'a', 'p', $2, 'detail fixture', $3) \
                         RETURNING id::text",
                        &[&TENANT, &name, &content],
                    )
                    .await
                    .expect("python_code insert");
                row.get::<_, String>(0)
            }

            #[tokio::test]
            async fn recipe_detail_returns_the_full_row() {
                let Some(rig) = pg_rig_or_skip().await else {
                    return;
                };
                let id = insert_recipe(&rig, "detail-recipe", false).await;

                let detail = rig
                    .service
                    .get_component(
                        brassclaw_product_workflow::SettingsComponentType::Recipes,
                        id.as_str(),
                    )
                    .await
                    .expect("recipe detail");

                assert_eq!(detail.id, id);
                assert_eq!(detail.class_code, 21);
                assert_eq!(detail.component["name"], "detail-recipe");
                // The class-specific column the list row cannot carry.
                assert_eq!(detail.component["steps"]["llm_call_required"], false);
            }

            #[tokio::test]
            async fn python_code_detail_returns_the_body() {
                let Some(rig) = pg_rig_or_skip().await else {
                    return;
                };
                let body = "result = host.read_file(path=\"/tmp/x\")";
                let id = insert_python_code(&rig, "detail-python", body).await;

                let detail = rig
                    .service
                    .get_component(
                        brassclaw_product_workflow::SettingsComponentType::PythonCode,
                        id.as_str(),
                    )
                    .await
                    .expect("python code detail");

                assert_eq!(detail.class_code, 22);
                assert_eq!(detail.component["content"], body);
            }

            /// The recipe list must badge Tier 0 vs Tier 1 from the `steps`
            /// JSONB, never from the unrelated reward-tier column.
            #[tokio::test]
            async fn recipe_list_derives_exec_tier_from_llm_call_required() {
                let Some(rig) = pg_rig_or_skip().await else {
                    return;
                };
                insert_recipe(&rig, "tier-zero-recipe", false).await;
                insert_recipe(&rig, "tier-one-recipe", true).await;

                let listed = rig.service.list_recipes().await.expect("recipe list");
                let tier_of = |name: &str| {
                    listed
                        .items
                        .iter()
                        .find(|item| item.name == name)
                        .and_then(|item| item.tier.clone())
                };
                assert_eq!(tier_of("tier-zero-recipe"), Some("0".to_string()));
                assert_eq!(tier_of("tier-one-recipe"), Some("1".to_string()));
            }

            #[tokio::test]
            async fn detail_rejects_a_non_uuid_id_and_reports_a_missing_row() {
                let Some(rig) = pg_rig_or_skip().await else {
                    return;
                };
                let missing = uuid::Uuid::new_v4().to_string();

                let invalid = rig
                    .service
                    .get_component(
                        brassclaw_product_workflow::SettingsComponentType::Recipes,
                        "not-a-uuid",
                    )
                    .await;
                assert!(matches!(
                    invalid,
                    Err(brassclaw_product_workflow::SettingsListingError::InvalidId(
                        _
                    ))
                ));

                let absent = rig
                    .service
                    .get_component(
                        brassclaw_product_workflow::SettingsComponentType::Recipes,
                        missing.as_str(),
                    )
                    .await;
                assert!(matches!(
                    absent,
                    Err(brassclaw_product_workflow::SettingsListingError::NotFound(
                        _
                    ))
                ));
            }

            /// A detail read must not cross tenants even with a valid id.
            #[tokio::test]
            async fn detail_is_scoped_to_the_tenant() {
                let Some(rig) = pg_rig_or_skip().await else {
                    return;
                };
                let id = insert_recipe(&rig, "other-tenant-recipe", false).await;
                let other =
                    PgSettingsListingService::new(Arc::clone(&rig.pool), "tenant-somebody-else");

                let result = other
                    .get_component(
                        brassclaw_product_workflow::SettingsComponentType::Recipes,
                        id.as_str(),
                    )
                    .await;

                assert!(matches!(
                    result,
                    Err(brassclaw_product_workflow::SettingsListingError::NotFound(
                        _
                    ))
                ));
            }
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgSettingsListingService;
