//! Durable operator rulesets, with validated one-time import of legacy V016 data.
//! The retired engine reduction loader is not a consumer of this settings port.

use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::orchestrator::invalidate_reduction_rules_cache, types::project::ProjectId,
};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    ReductionRuleConfigView, ReductionRuleStore, ReductionRuleStoreError, sort_for_storage,
};
use tokio_postgres::{GenericClient, Transaction};

pub(crate) struct PgReductionRuleStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgReductionRuleStore {
    pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    fn validate_key(&self, user_id: &str, project_id: &str) -> Result<(), ReductionRuleStoreError> {
        if self.tenant_id.is_empty() || user_id.is_empty() {
            return Err(ReductionRuleStoreError::Invalid(
                "empty ruleset owner".into(),
            ));
        }
        if project_id.is_empty()
            || project_id.len() > 64
            || !project_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(ReductionRuleStoreError::Invalid(
                "invalid project_id".into(),
            ));
        }
        Ok(())
    }

    async fn read<C: GenericClient + Sync>(
        &self,
        client: &C,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<Vec<ReductionRuleConfigView>>, ReductionRuleStoreError> {
        let row = client
            .query_opt(
                "SELECT rules FROM brassclaw_reduction_rulesets
                 WHERE tenant_id=$1 AND user_id=$2 AND project_id=$3",
                &[&self.tenant_id, &user_id, &project_id],
            )
            .await
            .map_err(database_error)?;
        row.map(|row| {
            let rules = serde_json::from_value(row.get(0))
                .map_err(|_| corrupt("stored ruleset JSON is invalid"))?;
            validate_rules(rules).map_err(|_| corrupt("stored ruleset is invalid"))
        })
        .transpose()
    }

    async fn lock(
        &self,
        transaction: &Transaction<'_>,
        user_id: &str,
        project_id: &str,
    ) -> Result<(), ReductionRuleStoreError> {
        // An encoded tuple avoids ambiguous concatenation. Hash collisions only
        // serialize unrelated transactions; row selection always uses full keys.
        let key = serde_json::to_string(&(&self.tenant_id, user_id, project_id))
            .map_err(|_| corrupt("ruleset key encoding failed"))?;
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&key],
            )
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn legacy(
        &self,
        transaction: &Transaction<'_>,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        // Historical writers stored this derived UUID and the original slug in
        // the title. Read those columns directly, without MemoryDoc's rehash.
        let legacy_project =
            ProjectId::from_slug("brassclaw-reduction-rules", project_id).to_string();
        let title = format!("reduction_rules:{user_id}:{project_id}");
        let rows = transaction
            .query(
                "SELECT content FROM brassclaw_memory_docs
                 WHERE tenant_id=$1 AND user_id=$2 AND project_id=$3 AND title=$4
                 AND tags @> ARRAY['reduction_rule']::text[] ORDER BY id",
                &[&self.tenant_id, &user_id, &legacy_project, &title],
            )
            .await
            .map_err(database_error)?;
        let mut rules = Vec::new();
        for row in rows {
            let selected: Vec<ReductionRuleConfigView> = serde_json::from_str(row.get(0))
                .map_err(|_| corrupt("legacy ruleset JSON is invalid"))?;
            rules.extend(selected);
        }
        validate_rules(rules).map_err(|_| corrupt("legacy ruleset is invalid"))
    }

    async fn write(
        &self,
        transaction: &Transaction<'_>,
        user_id: &str,
        project_id: &str,
        rules: &[ReductionRuleConfigView],
    ) -> Result<(), ReductionRuleStoreError> {
        let payload =
            serde_json::to_value(rules).map_err(|_| corrupt("ruleset JSON encoding failed"))?;
        transaction
            .execute(
                "INSERT INTO brassclaw_reduction_rulesets (tenant_id,user_id,project_id,rules)
                 VALUES ($1,$2,$3,$4) ON CONFLICT (tenant_id,user_id,project_id)
                 DO UPDATE SET rules=EXCLUDED.rules",
                &[&self.tenant_id, &user_id, &project_id, &payload],
            )
            .await
            .map_err(database_error)?;
        Ok(())
    }
}

fn database_error(error: tokio_postgres::Error) -> ReductionRuleStoreError {
    // Diagnostics stay in the store's error channel; WebUI maps this to an
    // internal error rather than presenting corrupt data as an empty ruleset.
    ReductionRuleStoreError::Internal(format!("reduction rules database failed: {error}"))
}
fn corrupt(reason: &str) -> ReductionRuleStoreError {
    ReductionRuleStoreError::Internal(reason.to_owned())
}
fn validate_rules(
    mut rules: Vec<ReductionRuleConfigView>,
) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
    let mut ids = BTreeSet::new();
    for rule in &rules {
        rule.validate()
            .map_err(|error| ReductionRuleStoreError::Invalid(error.to_string()))?;
        if !ids.insert(&rule.id) {
            return Err(ReductionRuleStoreError::Invalid("duplicate rule id".into()));
        }
    }
    sort_for_storage(&mut rules);
    Ok(rules)
}

#[async_trait]
impl ReductionRuleStore for PgReductionRuleStore {
    async fn list(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        self.validate_key(user_id, project_id)?;
        let mut client = self.pool.get().await.map_err(|error| {
            ReductionRuleStoreError::Unavailable(format!("reduction rules pool failed: {error}"))
        })?;
        if let Some(rules) = self.read(&**client, user_id, project_id).await? {
            return Ok(rules);
        }
        let transaction = client.transaction().await.map_err(database_error)?;
        self.lock(&transaction, user_id, project_id).await?;
        let rules = if let Some(rules) = self.read(&*transaction, user_id, project_id).await? {
            rules
        } else {
            let rules = self.legacy(&transaction, user_id, project_id).await?;
            self.write(&transaction, user_id, project_id, &rules)
                .await?;
            rules
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(rules)
    }

    async fn replace(
        &self,
        user_id: &str,
        project_id: &str,
        rules: Vec<ReductionRuleConfigView>,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        self.validate_key(user_id, project_id)?;
        let rules = validate_rules(rules)?;
        let mut client = self.pool.get().await.map_err(|error| {
            ReductionRuleStoreError::Unavailable(format!("reduction rules pool failed: {error}"))
        })?;
        let transaction = client.transaction().await.map_err(database_error)?;
        self.lock(&transaction, user_id, project_id).await?;
        // A validated explicit full replacement is also the operator's repair
        // path for a corrupt ruleset. It does not fabricate a successful read or
        // erase the preserved legacy records; the new complete list is authority.
        self.write(&transaction, user_id, project_id, &rules)
            .await?;
        transaction.commit().await.map_err(database_error)?;
        invalidate_reduction_rules_cache();
        Ok(rules)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use brassclaw_product_workflow::{ReductionRulesRequest, RuleType};

    fn sample_rules() -> Vec<ReductionRuleConfigView> {
        vec![
            ReductionRuleConfigView {
                id: "later".into(),
                rule_type: RuleType::Drop,
                params: serde_json::json!({"field":"context"}),
                priority: 100,
            },
            ReductionRuleConfigView {
                id: "first".into(),
                rule_type: RuleType::Truncate,
                params: serde_json::json!({"field":"content","max_chars":256}),
                priority: 10,
            },
        ]
    }

    async fn seed_legacy(pool: &PgPool, project: &str, content: &str) {
        let project_uuid = ProjectId::from_slug("brassclaw-reduction-rules", project).to_string();
        let title = format!("reduction_rules:operator:{project}");
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, title.as_bytes()).to_string();
        pool.get()
            .await
            .unwrap()
            .execute(
                "INSERT INTO brassclaw_memory_docs
             (id,tenant_id,user_id,project_id,doc_type,title,content,tags)
             VALUES ($1,'rules-tenant','operator',$2,'note',$3,$4,ARRAY['reduction_rule'])",
                &[&id, &project_uuid, &title, &content],
            )
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_rulesets_import_preserve_and_replace_without_resurrection() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let legacy = serde_json::to_string(&sample_rules()).unwrap();
        seed_legacy(&rig.pool, "bootstrap", &legacy).await;
        let store = PgReductionRuleStore::new(rig.pool.clone(), "rules-tenant");
        // Concurrent cold reads must converge on one imported row rather than
        // racing an INSERT or publishing divergent legacy selections.
        let (first, second) = tokio::join!(
            store.list("operator", "bootstrap"),
            store.list("operator", "bootstrap"),
        );
        let imported = first.unwrap();
        assert_eq!(imported, second.unwrap());
        assert_eq!(imported[0].id, "first");
        assert_eq!(imported[1].id, "later");
        // A fresh adapter has no process cache and reads the durable successor.
        drop(store);
        let store = PgReductionRuleStore::new(rig.pool.clone(), "rules-tenant");
        let only = vec![sample_rules().remove(0)];
        store
            .replace("operator", "bootstrap", only.clone())
            .await
            .unwrap();
        let reopened = PgReductionRuleStore::new(rig.pool.clone(), "rules-tenant");
        assert_eq!(
            serde_json::to_value(reopened.list("operator", "bootstrap").await.unwrap()).unwrap(),
            serde_json::to_value(&only).unwrap()
        );
        assert!(
            reopened
                .list("other", "bootstrap")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            reopened
                .list("operator", "different")
                .await
                .unwrap()
                .is_empty()
        );
        let other_tenant = PgReductionRuleStore::new(rig.pool.clone(), "another-tenant");
        assert!(
            other_tenant
                .list("operator", "bootstrap")
                .await
                .unwrap()
                .is_empty()
        );
        store
            .replace("operator", "bootstrap", Vec::new())
            .await
            .unwrap();
        drop(store);
        assert!(
            PgReductionRuleStore::new(rig.pool.clone(), "rules-tenant")
                .list("operator", "bootstrap")
                .await
                .unwrap()
                .is_empty()
        );
        let saved: String=rig.pool.get().await.unwrap().query_one(
            "SELECT content FROM brassclaw_memory_docs WHERE tenant_id='rules-tenant' AND user_id='operator' AND title='reduction_rules:operator:bootstrap'",&[],
        ).await.unwrap().get(0);
        assert_eq!(
            saved, legacy,
            "import/replacement preserves original legacy evidence"
        );
        let mut invalid = sample_rules();
        invalid[0].id.clear();
        assert!(matches!(
            reopened.replace("operator", "bootstrap", invalid).await,
            Err(ReductionRuleStoreError::Invalid(_))
        ));
        assert!(
            reopened
                .list("operator", "bootstrap")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            reopened.list("operator", "has space").await,
            Err(ReductionRuleStoreError::Invalid(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_rulesets_report_corruption_and_allow_explicit_complete_repair() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let mut invalid = sample_rules();
        invalid[0].id.clear();
        let duplicate = vec![sample_rules().remove(0); 2];
        let cases = [
            ("malformed", "{bad".to_string()),
            ("invalid", serde_json::to_string(&invalid).unwrap()),
            ("duplicate", serde_json::to_string(&duplicate).unwrap()),
        ];
        let store = PgReductionRuleStore::new(rig.pool.clone(), "rules-tenant");
        for (project, content) in cases {
            seed_legacy(&rig.pool, project, &content).await;
            assert!(matches!(
                store.list("operator", project).await,
                Err(ReductionRuleStoreError::Internal(_))
            ));
            let count:i64=rig.pool.get().await.unwrap().query_one(
                "SELECT count(*) FROM brassclaw_reduction_rulesets WHERE tenant_id='rules-tenant' AND user_id='operator' AND project_id=$1",&[&project],
            ).await.unwrap().get(0);
            assert_eq!(
                count, 0,
                "corrupt legacy read cannot publish an empty imported row"
            );
            store
                .replace("operator", project, sample_rules())
                .await
                .unwrap();
            assert_eq!(store.list("operator", project).await.unwrap().len(), 2);
        }
        store
            .replace("operator", "modern", sample_rules())
            .await
            .unwrap();
        let client = rig.pool.get().await.unwrap();
        client.execute("UPDATE brassclaw_reduction_rulesets SET rules='[{\"id\":\"bad\",\"rule_type\":\"truncate\",\"params\":{\"field\":\"content\",\"max_chars\":0}}]'::jsonb WHERE project_id='modern'",&[]).await.unwrap();
        assert!(matches!(
            store.list("operator", "modern").await,
            Err(ReductionRuleStoreError::Internal(_))
        ));
        store
            .replace("operator", "modern", sample_rules())
            .await
            .unwrap();
        assert_eq!(store.list("operator", "modern").await.unwrap().len(), 2);
        let malformed=client.execute("UPDATE brassclaw_reduction_rulesets SET rules='{}'::jsonb WHERE project_id='modern'",&[]).await.unwrap_err();
        assert_eq!(
            malformed.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        drop(client);
        rig.pool.close();
        assert!(matches!(
            store.list("operator", "modern").await,
            Err(ReductionRuleStoreError::Unavailable(_))
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_webui_rulesets_are_wired_and_survive_runtime_rebuild() {
        let rig = crate::runtime::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let home = tempfile::tempdir().unwrap();
        let gateway = Arc::new(crate::test_support::BudgetTestGateway::new());
        for pass in 0..2 {
            let input = crate::RebornRuntimeInput::from_services(
                rig.build_input("rules-operator", home.path())
                    .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
            )
            .with_model_gateway_override(gateway.clone());
            let runtime = Arc::new(crate::build_reborn_runtime(input).await.unwrap());
            let bundle = crate::webui::build_webui_services(runtime.clone(), None)
                .await
                .unwrap();
            let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
                brassclaw_host_api::TenantId::new(runtime.webui_tenant_id()).unwrap(),
                brassclaw_host_api::UserId::new("rules-operator").unwrap(),
                None,
                None,
            );
            if pass == 0 {
                let replaced = bundle
                    .api
                    .replace_reduction_rules(
                        caller.clone(),
                        "bootstrap",
                        ReductionRulesRequest {
                            rules: sample_rules(),
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(replaced.rules[0].id, "first");
            }
            let listed = bundle
                .api
                .list_reduction_rules(caller.clone(), "bootstrap")
                .await
                .unwrap();
            assert_eq!(listed.rules.len(), 2);
            assert_eq!(listed.rules[0].id, "first");
            drop(bundle);
            Arc::try_unwrap(runtime)
                .unwrap_or_else(|_| panic!("runtime facade unexpectedly retained"))
                .shutdown()
                .await
                .unwrap();
        }
        assert_eq!(
            gateway.call_count(),
            0,
            "settings CRUD does not invoke a model"
        );
    }
}
