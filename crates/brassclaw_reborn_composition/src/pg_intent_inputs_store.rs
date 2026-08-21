//! `PgIntentInputsStore` — Postgres-backed CRUD for `reborn_intent_inputs`.
//!
//! Backs `GET/PUT/DELETE /api/settings/intent-inputs` in the WebUI Settings tab.
//! Thin wrapper around the `list_intent_inputs`, `seed_intent_input`, and
//! `purge_component_inputs` functions in `brassclaw_engine::memory::intent_system`.
//!
//! Feature-gated: `postgres` (pool) + `skills-db` (intent_system fns).

#[cfg(all(feature = "postgres", feature = "skills-db"))]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentScope, IntentSource, purge_component_inputs, seed_intent_input,
    };
    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{IntentInputRow, IntentInputsStore, UpsertIntentInputRequest};
    use uuid::Uuid;

    /// Input text length cap (arbitrary safety ceiling; spec doesn't define one).
    const MAX_TEXT_LEN: usize = 1024;

    #[derive(Debug, thiserror::Error)]
    enum PgIntentInputsError {
        #[error("pool error: {0}")]
        Pool(String),
        #[error("db error: {0}")]
        Db(String),
        #[error("text too long (max {MAX_TEXT_LEN} chars)")]
        TextTooLong,
        #[error("unknown input_class {0}; must be 1, 2, or 3")]
        BadInputClass(i16),
        #[error("invalid component_id UUID: {0}")]
        BadUuid(String),
    }

    /// Postgres-backed intent inputs store.
    #[derive(Clone)]
    pub(crate) struct PgIntentInputsStore {
        pool: Arc<PgPool>,
        tenant_id: String,
        agent_id: String,
    }

    impl PgIntentInputsStore {
        pub(crate) fn new(pool: Arc<PgPool>, tenant_id: String, agent_id: String) -> Self {
            Self {
                pool,
                tenant_id,
                agent_id,
            }
        }
    }

    #[async_trait]
    impl IntentInputsStore for PgIntentInputsStore {
        async fn list(
            &self,
            user_id: &str,
            _agent_id: &str,
            project_id: &str,
            component_id: Option<&str>,
        ) -> Result<Vec<IntentInputRow>, Box<dyn std::error::Error + Send + Sync>> {
            let client = self.pool.get().await.map_err(|e| {
                Box::new(PgIntentInputsError::Pool(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;

            let rows = if let Some(cid) = component_id {
                let cid_uuid = Uuid::parse_str(cid).map_err(|e| {
                    Box::new(PgIntentInputsError::BadUuid(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                client
                    .query(
                        "SELECT id::text, input_text, input_class, component_id::text,
                                component_class_code, score, source, needs_review
                         FROM reborn_intent_inputs
                         WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3
                           AND project_id = $4 AND component_id = $5
                         ORDER BY score DESC, created_at ASC LIMIT 500",
                        &[
                            &self.tenant_id,
                            &user_id,
                            &self.agent_id,
                            &project_id,
                            &cid_uuid,
                        ],
                    )
                    .await
                    .map_err(|e| {
                        Box::new(PgIntentInputsError::Db(e.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?
            } else {
                client
                    .query(
                        "SELECT id::text, input_text, input_class, component_id::text,
                                component_class_code, score, source, needs_review
                         FROM reborn_intent_inputs
                         WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3
                           AND project_id = $4
                         ORDER BY score DESC, created_at ASC LIMIT 500",
                        &[&self.tenant_id, &user_id, &self.agent_id, &project_id],
                    )
                    .await
                    .map_err(|e| {
                        Box::new(PgIntentInputsError::Db(e.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?
            };

            let items = rows
                .iter()
                .map(|row| IntentInputRow {
                    id: row.get::<_, String>(0),
                    input_text: row.get::<_, String>(1),
                    input_class: row.get::<_, i16>(2),
                    component_id: row.get::<_, String>(3),
                    component_class_code: row.get::<_, i16>(4),
                    score: row.get::<_, i32>(5),
                    source: row.get::<_, String>(6),
                    needs_review: row.get::<_, bool>(7),
                })
                .collect();

            Ok(items)
        }

        async fn upsert(
            &self,
            user_id: &str,
            _agent_id: &str,
            _project_id: &str,
            req: &UpsertIntentInputRequest,
        ) -> Result<IntentInputRow, Box<dyn std::error::Error + Send + Sync>> {
            if req.input_text.len() > MAX_TEXT_LEN {
                return Err(Box::new(PgIntentInputsError::TextTooLong));
            }
            let input_class = match req.input_class {
                1 => InputClass::Word,
                2 => InputClass::Partial,
                3 => InputClass::Sentence,
                other => {
                    return Err(Box::new(PgIntentInputsError::BadInputClass(other)));
                }
            };
            let component_id = Uuid::parse_str(&req.component_id).map_err(|e| {
                Box::new(PgIntentInputsError::BadUuid(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;

            let scope = IntentScope {
                tenant_id: self.tenant_id.clone(),
                user_id: user_id.to_string(),
                agent_id: self.agent_id.clone(),
                project_id: req.project_id.clone(),
            };

            seed_intent_input(
                &self.pool,
                &scope,
                &req.input_text,
                input_class,
                component_id,
                req.component_class_code as i32,
                IntentSource::Seeded,
                // step_link: None — the generic WebUI intent-upsert path is not
                // the Recipe-variant seeder. Recipe-variant step_link seeding
                // lands at Phase N Q2-graduation (Q-D2). Non-Recipe seeders pass
                // None per FIND-NEW-03.
                None,
            )
            .await
            .map_err(|e| {
                Box::new(PgIntentInputsError::Db(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;

            // Re-fetch the just-upserted row so we can return canonical state.
            let client = self.pool.get().await.map_err(|e| {
                Box::new(PgIntentInputsError::Pool(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;
            let row = client
                .query_one(
                    "SELECT id::text, input_text, input_class, component_id::text,
                            component_class_code, score, source, needs_review
                     FROM reborn_intent_inputs
                     WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3
                       AND project_id = $4
                       AND input_text = $5 AND input_class = $6 AND component_id = $7",
                    &[
                        &self.tenant_id,
                        &user_id,
                        &self.agent_id,
                        &req.project_id,
                        &req.input_text,
                        &(req.input_class),
                        &component_id,
                    ],
                )
                .await
                .map_err(|e| {
                    Box::new(PgIntentInputsError::Db(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

            Ok(IntentInputRow {
                id: row.get::<_, String>(0),
                input_text: row.get::<_, String>(1),
                input_class: row.get::<_, i16>(2),
                component_id: row.get::<_, String>(3),
                component_class_code: row.get::<_, i16>(4),
                score: row.get::<_, i32>(5),
                source: row.get::<_, String>(6),
                needs_review: row.get::<_, bool>(7),
            })
        }

        async fn purge_for_component(
            &self,
            user_id: &str,
            _agent_id: &str,
            project_id: &str,
            component_id: &str,
        ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
            let component_uuid = Uuid::parse_str(component_id).map_err(|e| {
                Box::new(PgIntentInputsError::BadUuid(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;

            let scope = IntentScope {
                tenant_id: self.tenant_id.clone(),
                user_id: user_id.to_string(),
                agent_id: self.agent_id.clone(),
                project_id: project_id.to_string(),
            };

            let count = purge_component_inputs(&self.pool, &scope, component_uuid)
                .await
                .map_err(|e| {
                    Box::new(PgIntentInputsError::Db(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

            Ok(count)
        }
    }
}

#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub(crate) use inner::PgIntentInputsStore;
