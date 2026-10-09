//! `PgComponentDbBackend` — Postgres implementation of [`ComponentDbBackend`].
//!
//! Implements the `builtin.component_db` tool backend for the composition layer.
//! Operates on `reborn_docus` rows (class 17) using the system-seed scope
//! (`SYSTEM_RESERVED_ID` + `"default"` agent + the runtime tenant).
//!
//! # Upsert invariant
//!
//! Every `upsert` call sets `validation_status='pending'` so the row enters
//! the Q1+Q2 queue — never writes `'validated'` directly (§7, Answer 2).
//!
//! # mark_stale
//!
//! Delegates to [`PgBasicPromptStore::mark_stale`] and acknowledges only a
//! committed invalidation. Database failures propagate to the Recipe caller.
//!
//! Feature-gated: `postgres`.

#![forbid(unsafe_code)]

#[cfg(feature = "postgres")]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_host_runtime::{
        ComponentDbBackend, ComponentDbError, ComponentDbRow, ComponentDbScope, ComponentDbUpsert,
        ComponentDbUpsertResult,
    };
    use brassclaw_pg::PgPool;
    use uuid::Uuid;

    use crate::pg_basic_prompt_store::PgBasicPromptStore;

    /// Postgres backend for `builtin.component_db`.
    ///
    /// Holds a pool + basic-prompt store. Tenant is fixed at construction time
    /// (matches the boot seed tenant).
    pub(crate) struct PgComponentDbBackend {
        pool: Arc<PgPool>,
        tenant_id: String,
        basic_prompt_store: PgBasicPromptStore,
    }

    impl PgComponentDbBackend {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            basic_prompt_store: PgBasicPromptStore,
        ) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
                basic_prompt_store,
            }
        }
    }

    #[async_trait]
    impl ComponentDbBackend for PgComponentDbBackend {
        async fn read_hash(
            &self,
            scope: &ComponentDbScope,
            name: &str,
        ) -> Result<Option<String>, ComponentDbError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            let row = client
                .query_opt(
                    "SELECT content_hash FROM reborn_docus
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4
                       AND name = $5
                     LIMIT 1",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &name,
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
        }

        async fn read_row(
            &self,
            scope: &ComponentDbScope,
            name: &str,
        ) -> Result<Option<ComponentDbRow>, ComponentDbError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            let row = client
                .query_opt(
                    "SELECT id::text, name, content, content_hash, validation_status
                     FROM reborn_docus
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4
                       AND name = $5
                     LIMIT 1",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &name,
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            Ok(row.map(|r| ComponentDbRow {
                id: r.get(0),
                name: r.get(1),
                content: r.get(2),
                content_hash: r.get(3),
                validation_status: r.get(4),
            }))
        }

        async fn upsert(
            &self,
            scope: &ComponentDbScope,
            row: ComponentDbUpsert,
        ) -> Result<ComponentDbUpsertResult, ComponentDbError> {
            let parse_link = |value: &Option<String>| {
                value
                    .as_deref()
                    .map(|value| {
                        Uuid::parse_str(value).ok().filter(|id| !id.is_nil()).ok_or(
                            ComponentDbError::InvalidInput("component links must be nonnil UUIDs"),
                        )
                    })
                    .transpose()
            };
            let similarity = parse_link(&row.similarity_parent_id)?;
            let replaces = parse_link(&row.replaces_id)?;
            let mut client = self
                .pool
                .get()
                .await
                .map_err(|error| ComponentDbError::Db(error.to_string()))?;
            let tx = client
                .transaction()
                .await
                .map_err(|error| ComponentDbError::Db(error.to_string()))?;
            let consumer_tags: Vec<&str> = row.consumer_tags.iter().map(String::as_str).collect();
            let parameters: [&(dyn tokio_postgres::types::ToSql + Sync); 12] = [
                &self.tenant_id,
                &scope.user_id,
                &"default",
                &scope.project_id,
                &row.name,
                &row.description,
                &row.content,
                &row.content_hash,
                &row.source,
                &consumer_tags,
                &similarity,
                &replaces,
            ];
            // The actual INSERT outcome establishes is_new. A separate read
            // before an upsert races with another creator under read-committed.
            let inserted = tx
                .query_opt(
                    "INSERT INTO reborn_docus
                (tenant_id,user_id,agent_id,project_id,name,description,content,content_hash,
                 source,validation_status,consumer_tags,similarity_parent_id,replaces_id)
                VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'pending',$10::text[],$11::uuid,$12::uuid)
                ON CONFLICT(tenant_id,user_id,agent_id,project_id,name) DO NOTHING
                RETURNING id::text,content_hash",
                    &parameters,
                )
                .await
                .map_err(|error| ComponentDbError::Db(error.to_string()))?;
            let is_new = inserted.is_some();
            let result = match inserted {
                Some(result) => result,
                None => tx
                    .query_opt(
                        "UPDATE reborn_docus SET
                    description=$6,content=$7,content_hash=$8,source=$9,
                    validation_status='pending',consumer_tags=$10::text[],
                    similarity_parent_id=$11::uuid,replaces_id=$12::uuid,updated_at=now()
                    WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND name=$5
                    RETURNING id::text,content_hash",
                        &parameters,
                    )
                    .await
                    .map_err(|error| ComponentDbError::Db(error.to_string()))?
                    .ok_or_else(|| {
                        ComponentDbError::Db("component disappeared during upsert".into())
                    })?,
            };
            let receipt = ComponentDbUpsertResult {
                id: result.get(0),
                content_hash: result.get(1),
                is_new,
            };
            tx.commit()
                .await
                .map_err(|error| ComponentDbError::Db(error.to_string()))?;
            Ok(receipt)
        }

        async fn mark_stale(&self, scope: &ComponentDbScope) -> Result<(), ComponentDbError> {
            self.basic_prompt_store
                .mark_stale(&scope.user_id, &scope.project_id)
                .await
                .map_err(|error| ComponentDbError::Db(error.to_string()))
        }
    }

    /// Convenience: build a [`PgComponentDbBackend`] from the system-reserved
    /// user_id + the runtime tenant. Called from `factory.rs` at wiring time.
    pub(crate) fn build_pg_component_db_backend(
        pool: Arc<PgPool>,
        tenant_id: impl Into<String>,
        basic_prompt_store: PgBasicPromptStore,
    ) -> PgComponentDbBackend {
        PgComponentDbBackend::new(pool, tenant_id, basic_prompt_store)
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::build_pg_component_db_backend;

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::build_pg_component_db_backend;
    use std::sync::Arc;

    use brassclaw_host_api::{CapabilityId, ResourceScope, RuntimeDispatchErrorKind};
    use brassclaw_host_runtime::{
        BuiltinFirstPartyTools, COMPONENT_DB_CAPABILITY_ID, ComponentDbBackend, ComponentDbError,
        ComponentDbScope, FirstPartyCapabilityHandler, FirstPartyCapabilityRequest,
    };
    use serde_json::json;

    #[tokio::test]
    async fn native_component_database_handles_missing_hash_and_concurrent_creation_exactly() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client
            .batch_execute(
                "INSERT INTO reborn_docus(tenant_id,user_id,agent_id,project_id,name,content)
            VALUES('component-atomic','owner','default','project','unhashed','legacy content');",
            )
            .await
            .unwrap();
        let store = crate::pg_basic_prompt_store::PgBasicPromptStore::new(
            Arc::clone(&rig.pool),
            "component-atomic",
            "default",
        );
        let backend = Arc::new(build_pg_component_db_backend(
            Arc::clone(&rig.pool),
            "component-atomic",
            store,
        ));
        let scope = ComponentDbScope {
            user_id: "owner".into(),
            project_id: "project".into(),
        };
        assert!(
            backend
                .read_hash(&scope, "unhashed")
                .await
                .unwrap()
                .is_none()
        );
        let existing = backend.read_row(&scope, "unhashed").await.unwrap().unwrap();
        assert_eq!(existing.content, "legacy content");
        assert!(existing.content_hash.is_none());
        let row = |content: &str| brassclaw_host_runtime::ComponentDbUpsert {
            name: "concurrent".into(),
            description: "actual parallel writes".into(),
            content: content.into(),
            content_hash: brassclaw_host_runtime::sha256_hex(content),
            source: "authored".into(),
            similarity_parent_id: None,
            replaces_id: None,
            consumer_tags: vec!["03:llm".into()],
        };
        let (first, second) = tokio::join!(
            backend.upsert(&scope, row("first")),
            backend.upsert(&scope, row("second"))
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first.is_new, second.is_new, "exactly one creator");
        assert_eq!(first.id, second.id);
        let stored = backend
            .read_row(&scope, "concurrent")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.validation_status, "pending");
        let last = if first.is_new { &second } else { &first };
        assert_eq!(
            stored.content_hash.as_deref(),
            Some(last.content_hash.as_str())
        );
        assert_eq!(
            stored.content_hash,
            Some(brassclaw_host_runtime::sha256_hex(&stored.content))
        );
        let mut invalid = row("must not replace the stored value");
        invalid.replaces_id = Some("not a UUID".into());
        assert!(matches!(
            backend.upsert(&scope, invalid).await,
            Err(ComponentDbError::InvalidInput(_))
        ));
        assert_eq!(
            backend
                .read_row(&scope, "concurrent")
                .await
                .unwrap()
                .unwrap()
                .content,
            stored.content
        );
        let tools = BuiltinFirstPartyTools::default().with_component_db(backend);
        for (field, value) in [
            ("source", json!(null)),
            ("consumer_tags", json!(["03:llm", 17])),
            ("similarity_parent_id", json!("invalid")),
            ("replaces_id", json!(null)),
        ] {
            let mut fields = json!({"name":"invalid-fields","description":"unchanged","content":"data","content_hash":"hash"});
            fields[field] = value;
            let error = tools.dispatch(FirstPartyCapabilityRequest::request_for_test(
                CapabilityId::new(COMPONENT_DB_CAPABILITY_ID).unwrap(), ResourceScope::system(),
                json!({"op":"upsert","scope":{"user_id":"owner","project_id":"project"},"fields":fields}), None,
            )).await.unwrap_err();
            assert_eq!(error.kind(), Some(RuntimeDispatchErrorKind::Client));
        }
        let count = client.query_one("SELECT count(*) FROM reborn_docus WHERE tenant_id='component-atomic' AND name='invalid-fields'", &[]).await.unwrap().get::<_, i64>(0);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn native_component_invalidation_reports_failed_commit_through_the_real_tool() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("INSERT INTO reborn_basic_prompt_store(tenant_id,user_id,agent_id,project_id,bundle_json,fingerprint)
            VALUES('component-invalidation','owner','default','project','\"exact retained prefix\"','original');
            CREATE FUNCTION reject_component_invalidation() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                RAISE EXCEPTION 'private row content must never appear in the Tool answer' USING ERRCODE='23514';
            END; $$;
            CREATE TRIGGER reject_component_invalidation BEFORE UPDATE ON reborn_basic_prompt_store
            FOR EACH ROW WHEN (OLD.tenant_id='component-invalidation') EXECUTE FUNCTION reject_component_invalidation();").await.unwrap();
        let store = crate::pg_basic_prompt_store::PgBasicPromptStore::new(
            Arc::clone(&rig.pool),
            "component-invalidation",
            "default",
        );
        let backend = Arc::new(build_pg_component_db_backend(
            Arc::clone(&rig.pool),
            "component-invalidation",
            store,
        ));
        let scope = ComponentDbScope {
            user_id: "owner".into(),
            project_id: "project".into(),
        };
        let failure = backend.mark_stale(&scope).await.unwrap_err();
        assert!(matches!(failure, ComponentDbError::Db(_)));
        let tools = BuiltinFirstPartyTools::default().with_component_db(backend);
        let request = || {
            FirstPartyCapabilityRequest::request_for_test(
                CapabilityId::new(COMPONENT_DB_CAPABILITY_ID).unwrap(),
                ResourceScope::system(),
                json!({"op":"mark_stale","scope":{"user_id":"owner","project_id":"project"}}),
                None,
            )
        };
        let failure = tools.dispatch(request()).await.unwrap_err();
        assert_eq!(failure.kind(), Some(RuntimeDispatchErrorKind::Backend));
        assert_eq!(
            failure.safe_summary(),
            Some("component database operation failed")
        );
        assert!(!format!("{failure:?}").contains("private row content"));
        let row = client.query_one("SELECT is_stale,bundle_json,fingerprint FROM reborn_basic_prompt_store WHERE tenant_id='component-invalidation'", &[]).await.unwrap();
        assert!(!row.get::<_, bool>(0));
        assert_eq!(
            row.get::<_, serde_json::Value>(1),
            json!("exact retained prefix")
        );
        assert_eq!(row.get::<_, String>(2), "original");
        client
            .batch_execute(
                "DROP TRIGGER reject_component_invalidation ON reborn_basic_prompt_store;
            DROP FUNCTION reject_component_invalidation();",
            )
            .await
            .unwrap();
        assert_eq!(
            tools.dispatch(request()).await.unwrap().output,
            json!({"ok":true})
        );
        let row = client.query_one("SELECT is_stale,bundle_json FROM reborn_basic_prompt_store WHERE tenant_id='component-invalidation'", &[]).await.unwrap();
        assert!(row.get::<_, bool>(0));
        assert_eq!(
            row.get::<_, serde_json::Value>(1),
            json!("exact retained prefix")
        );
    }
}
