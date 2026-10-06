//! PostgreSQL implementations of the prefix-bundle host capabilities.

use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_host_runtime::{
    PrefixBundleCapabilityError, StorePrefixBundleBackend, StorePrefixBundleResult,
    SweepValidatedComponentsBackend, SweepValidatedComponentsResult,
};
use brassclaw_pg::PgPool;

use crate::{
    builtin_bootstrap::{AGENTS_MD_SEED, CLAUDE_MD_SEED},
    interceptor_config_service::class_label,
    pg_basic_prompt_store::compute_fingerprint,
    pg_prefix_scope_ticket::{PgPrefixScopeTicketStore, ticket_digest},
};

/// Static component source list for prefix assembly. Table names, expressions,
/// and class codes are reviewed SQL literals; never derive them from request data.
const PREFIX_COMPONENT_TABLES: &[(&str, u16, &str, bool)] = &[
    (
        "reborn_skills",
        1,
        "COALESCE(NULLIF(prior_knowledge_content,''), body)",
        true,
    ),
    (
        "reborn_tools",
        0,
        "COALESCE(prior_knowledge_content, description)",
        false,
    ),
    (
        "reborn_actions",
        16,
        "COALESCE(prior_knowledge_content, description)",
        false,
    ),
    (
        "reborn_specs",
        12,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_summaries",
        15,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_lessons",
        18,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_issues",
        19,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_notes",
        20,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_tool_skills",
        13,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_plans",
        14,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_docus",
        17,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_python_code",
        22,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        false,
    ),
    (
        "reborn_recipes",
        21,
        "COALESCE(NULLIF(prior_knowledge_content,''), '')",
        false,
    ),
    (
        "reborn_extensions_unified",
        9,
        "COALESCE(prior_knowledge_content, description)",
        true,
    ),
    (
        "reborn_extension_catalogues",
        23,
        "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)",
        false,
    ),
];

#[derive(Clone)]
pub(crate) struct PgSweepValidatedComponentsBackend {
    pool: Arc<PgPool>,
    tickets: PgPrefixScopeTicketStore,
}

impl PgSweepValidatedComponentsBackend {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self {
            tickets: PgPrefixScopeTicketStore::new(Arc::clone(&pool)),
            pool,
        }
    }
}

#[async_trait]
impl SweepValidatedComponentsBackend for PgSweepValidatedComponentsBackend {
    async fn sweep_and_format(
        &self,
        scope_ticket: &str,
        runtime_scope: &ResourceScope,
    ) -> Result<SweepValidatedComponentsResult, PrefixBundleCapabilityError> {
        let started = Instant::now();
        let scope = self
            .tickets
            .resolve(scope_ticket)
            .await
            .map_err(backend_error)?;
        validate_runtime_scope(&scope, runtime_scope)?;
        let client = self.pool.get().await.map_err(backend_error)?;
        let mut parts: Vec<(u16, i64, String, String)> = Vec::new();

        for &(table, class_code, content_expr, has_dynamic_class) in PREFIX_COMPONENT_TABLES {
            let sql = if has_dynamic_class {
                format!("SELECT class_code::int, prompt_uid, name, ({content_expr}) AS content
                    FROM {table} WHERE validation_status='validated'
                      AND tenant_id=$1 AND ((user_id=$2 AND agent_id=$3 AND project_id=$4) OR source='system')
                    ORDER BY class_code, prompt_uid LIMIT 1000")
            } else {
                format!("SELECT prompt_uid, name, ({content_expr}) AS content
                    FROM {table} WHERE validation_status='validated'
                      AND tenant_id=$1 AND ((user_id=$2 AND agent_id=$3 AND project_id=$4) OR source='system')
                    ORDER BY prompt_uid LIMIT 1000")
            };
            let rows = client
                .query(
                    &sql,
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(backend_error)?;
            for row in rows {
                let (code, uid, name, content) = if has_dynamic_class {
                    (
                        row.get::<_, i32>(0) as u16,
                        row.get::<_, i64>(1),
                        row.get::<_, String>(2),
                        row.get::<_, String>(3),
                    )
                } else {
                    (
                        class_code,
                        row.get::<_, i64>(0),
                        row.get::<_, String>(1),
                        row.get::<_, String>(2),
                    )
                };
                if uid >= 0 {
                    parts.push((code, uid, name, content));
                }
            }
        }
        parts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut bundle = String::new();
        for (code, uid, name, content) in &parts {
            bundle.push_str(&format!(
                "\n\n## {code}:{uid}  {}  \"{name}\"\n\n{content}",
                class_label(*code)
            ));
        }
        append_markdown_section(
            &mut bundle,
            "CLAUDE.md — Architecture & Design Reference",
            CLAUDE_MD_SEED,
            "\n## Architecture\n",
        );
        append_markdown_section(
            &mut bundle,
            "AGENTS.md — Routing & Design Reference",
            AGENTS_MD_SEED,
            "\n## Architecture Mental Model\n",
        );
        bundle.push_str(concat!(
            "\n\n## Sempai Response Schema\n\n",
            "```json\n",
            "{\n",
            "  \"adjusted_volatile_messages\": [[\"role\", \"content\"], ...],\n",
            "  \"bridge_messages\": [[\"role\", \"content\"], ...],\n",
            "  \"composition_summary\": \"string\",\n",
            "  \"proposed_recipe_updates\": [],\n",
            "  \"proposed_intent_examples\": [],\n",
            "  \"settings_adjustments\": []\n",
            "}\n",
            "```\n"
        ));
        Ok(SweepValidatedComponentsResult {
            bundle,
            component_count: parts.len(),
            generation_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        })
    }
}

#[derive(Clone)]
pub(crate) struct PgStorePrefixBundleBackend {
    pool: Arc<PgPool>,
}

impl PgStorePrefixBundleBackend {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StorePrefixBundleBackend for PgStorePrefixBundleBackend {
    async fn store(
        &self,
        scope_ticket: &str,
        bundle: &str,
        generation_ms: i64,
        runtime_scope: &ResourceScope,
    ) -> Result<StorePrefixBundleResult, PrefixBundleCapabilityError> {
        if bundle.len() > 16 * 1024 * 1024 {
            return Err(PrefixBundleCapabilityError::Backend(
                "bundle exceeds the 16 MiB storage limit".into(),
            ));
        }
        let digest = ticket_digest(scope_ticket);
        let mut client = self.pool.get().await.map_err(backend_error)?;
        let tx = client.transaction().await.map_err(backend_error)?;
        let row = tx
            .query_opt(
                "UPDATE reborn_prefix_scope_tickets SET consumed_at=now()
             WHERE ticket_hash=$1
               AND consumed_at IS NULL AND expires_at > now()
             RETURNING tenant_id, user_id, agent_id, project_id,
                       conversation_id, request_id",
                &[&digest],
            )
            .await
            .map_err(backend_error)?
            .ok_or_else(|| {
                PrefixBundleCapabilityError::Backend(
                    "scope ticket is invalid, expired, or consumed".into(),
                )
            })?;
        let tenant_id: String = row.get(0);
        let user_id: String = row.get(1);
        let agent_id: String = row.get(2);
        let project_id: String = row.get(3);
        let conversation_id: String = row.get(4);
        let generation_id: uuid::Uuid = row.get(5);
        let ticket_scope = crate::pg_prefix_scope_ticket::PrefixScopeTicket {
            tenant_id,
            user_id: user_id.clone(),
            agent_id,
            project_id: project_id.clone(),
            conversation_id,
            request_id: generation_id,
        };
        validate_runtime_scope(&ticket_scope, runtime_scope)?;
        let fingerprint = compute_fingerprint(bundle);
        let bundle_json = serde_json::to_string(bundle).map_err(backend_error)?;
        tx.execute(
            "INSERT INTO reborn_basic_prompt_store
                (tenant_id,user_id,agent_id,project_id,bundle_json,fingerprint,is_stale,
                 assembled_at,prewarm_last_at,updated_at,generation_ms,generation_id)
             VALUES ($1,$2,$3,$4,$5::text::jsonb,$6,false,now(),NULL,now(),$7,$8)
             ON CONFLICT ON CONSTRAINT reborn_basic_prompt_store_scope_unique
             DO UPDATE SET bundle_json=EXCLUDED.bundle_json,fingerprint=EXCLUDED.fingerprint,
                 is_stale=false,assembled_at=now(),updated_at=now(),
                 generation_ms=EXCLUDED.generation_ms,generation_id=EXCLUDED.generation_id",
            &[
                &ticket_scope.tenant_id,
                &user_id,
                &ticket_scope.agent_id,
                &project_id,
                &bundle_json,
                &fingerprint,
                &generation_ms,
                &generation_id,
            ],
        )
        .await
        .map_err(backend_error)?;
        tx.commit().await.map_err(backend_error)?;
        Ok(StorePrefixBundleResult {
            fingerprint,
            generation_ms,
            generation_id,
        })
    }
}

fn validate_runtime_scope(
    ticket: &crate::pg_prefix_scope_ticket::PrefixScopeTicket,
    scope: &ResourceScope,
) -> Result<(), PrefixBundleCapabilityError> {
    let ticket_project = (!ticket.project_id.is_empty()).then_some(ticket.project_id.as_str());
    let runtime_project = scope.project_id.as_ref().map(|id| id.as_str());
    let runtime_thread = scope.thread_id.as_ref().map(|id| id.as_str());
    if ticket.tenant_id != scope.tenant_id.as_str()
        || ticket.user_id != scope.user_id.as_str()
        || ticket.agent_id
            != scope
                .agent_id
                .as_ref()
                .map(|id| id.as_str())
                .unwrap_or_default()
        || ticket_project != runtime_project
        || runtime_thread != Some(ticket.conversation_id.as_str())
    {
        return Err(PrefixBundleCapabilityError::Backend(
            "scope ticket does not match the trusted turn scope".into(),
        ));
    }
    Ok(())
}

fn append_markdown_section(output: &mut String, title: &str, source: &str, marker: &str) {
    let start = source.find(marker).map(|at| at + 1).unwrap_or(source.len());
    output.push_str(&format!("\n\n---\n\n## {title}\n\n"));
    output.push_str(&source[start..]);
}

fn backend_error(error: impl std::fmt::Display) -> PrefixBundleCapabilityError {
    PrefixBundleCapabilityError::Backend(error.to_string())
}
