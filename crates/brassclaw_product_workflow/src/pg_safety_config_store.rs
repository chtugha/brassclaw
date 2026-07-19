//! Postgres-backed [`SafetyConfigStore`] and [`CapabilityPermissionStore`] implementation.
//!
//! A single struct implements both traits — matching `SqliteSafetyConfigStore` which
//! already implements both in production (see `safety_config_store.rs`).
//!
//! `brassclaw_safety_config` rows are scoped by `(tenant_id, user_id)`.
//! `brassclaw_capability_permissions` rows are scoped by `(tenant_id, capability_id)`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::PermissionMode;
use brassclaw_pg::PgPool;

use crate::CapabilityPermissionStore;
use crate::safety_config::{SafetyConfigResponse, SafetyEntry};
use crate::safety_config_store::{SafetyCategory, SafetyConfigStore};

fn map_pool(e: deadpool_postgres::PoolError) -> Box<dyn std::error::Error + Send + Sync> {
    e.to_string().into()
}

fn map_pg(e: tokio_postgres::Error) -> Box<dyn std::error::Error + Send + Sync> {
    e.to_string().into()
}

fn permission_mode_str(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Allow => "allow",
        PermissionMode::Ask => "ask",
        PermissionMode::Deny => "deny",
    }
}

fn parse_permission_mode(s: &str) -> Option<PermissionMode> {
    match s {
        "allow" => Some(PermissionMode::Allow),
        "ask" => Some(PermissionMode::Ask),
        "deny" => Some(PermissionMode::Deny),
        _ => None,
    }
}

/// Default safety patterns seeded by `initialize_defaults`.
fn default_patterns(category: SafetyCategory) -> Vec<(&'static str, bool)> {
    match category {
        SafetyCategory::SensitivePaths => vec![
            ("*.pem", true),
            ("*.key", true),
            ("*.env", true),
            ("*/.ssh/*", true),
            ("*/.aws/*", true),
            ("*/credentials", true),
            ("*/id_rsa*", true),
        ],
        SafetyCategory::WorkspaceRules => vec![
            ("MEMORY.md", true),
            ("IDENTITY.md", true),
            ("CONTEXT.md", true),
            ("*.brassclaw/*", true),
        ],
        SafetyCategory::BlockedPaths => vec![
            ("/dev/zero", true),
            ("/dev/random", true),
            ("/proc/kcore", true),
            ("/sys/firmware/*", true),
        ],
    }
}

/// Postgres-backed implementation of both [`SafetyConfigStore`] and
/// [`CapabilityPermissionStore`], matching the dual-trait shape of
/// `SqliteSafetyConfigStore`.
pub struct PgSafetyConfigStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgSafetyConfigStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl SafetyConfigStore for PgSafetyConfigStore {
    async fn get_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let cat = category.as_str();

        // Seed defaults if no entries exist for this (tenant, user, category).
        let count_row = client
            .query_one(
                "SELECT COUNT(*) FROM brassclaw_safety_config \
                 WHERE tenant_id = $1 AND user_id = $2 AND category = $3",
                &[&self.tenant_id, &user_id, &cat],
            )
            .await
            .map_err(map_pg)?;
        let count: i64 = count_row.get(0);
        if count == 0 {
            for (pattern, enabled) in default_patterns(category) {
                client
                    .execute(
                        "INSERT INTO brassclaw_safety_config \
                         (id, tenant_id, user_id, category, pattern, is_enabled, is_default) \
                         VALUES ($1, $2, $3, $4, $5, $6, true) \
                         ON CONFLICT (tenant_id, user_id, category, pattern) DO NOTHING",
                        &[
                            &uuid::Uuid::new_v4().to_string(),
                            &self.tenant_id,
                            &user_id,
                            &cat,
                            &pattern,
                            &enabled,
                        ],
                    )
                    .await
                    .map_err(map_pg)?;
            }
        }

        let rows = client
            .query(
                "SELECT pattern, is_enabled, is_default \
                 FROM brassclaw_safety_config \
                 WHERE tenant_id = $1 AND user_id = $2 AND category = $3 \
                 ORDER BY is_default DESC, pattern ASC",
                &[&self.tenant_id, &user_id, &cat],
            )
            .await
            .map_err(map_pg)?;
        let entries = rows
            .into_iter()
            .map(|r| SafetyEntry {
                pattern: r.get(0),
                enabled: r.get(1),
                is_default: r.get(2),
            })
            .collect();
        Ok(SafetyConfigResponse { entries })
    }

    async fn update_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
        entries: Vec<SafetyEntry>,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let cat = category.as_str();

        // Delete non-default entries, then upsert new ones — mirrors libSQL logic.
        client
            .execute(
                "DELETE FROM brassclaw_safety_config \
                 WHERE tenant_id = $1 AND user_id = $2 AND category = $3 AND is_default = false",
                &[&self.tenant_id, &user_id, &cat],
            )
            .await
            .map_err(map_pg)?;

        for entry in &entries {
            if entry.is_default {
                client
                    .execute(
                        "UPDATE brassclaw_safety_config \
                         SET is_enabled = $1 \
                         WHERE tenant_id = $2 AND user_id = $3 AND category = $4 \
                           AND pattern = $5 AND is_default = true",
                        &[&entry.enabled, &self.tenant_id, &user_id, &cat, &entry.pattern],
                    )
                    .await
                    .map_err(map_pg)?;
            } else {
                client
                    .execute(
                        "INSERT INTO brassclaw_safety_config \
                         (id, tenant_id, user_id, category, pattern, is_enabled, is_default) \
                         VALUES ($1, $2, $3, $4, $5, $6, false) \
                         ON CONFLICT (tenant_id, user_id, category, pattern) \
                         DO UPDATE SET is_enabled = excluded.is_enabled",
                        &[
                            &uuid::Uuid::new_v4().to_string(),
                            &self.tenant_id,
                            &user_id,
                            &cat,
                            &entry.pattern,
                            &entry.enabled,
                        ],
                    )
                    .await
                    .map_err(map_pg)?;
            }
        }

        self.get_config(user_id, category).await
    }

    async fn initialize_defaults(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let categories = [
            SafetyCategory::SensitivePaths,
            SafetyCategory::WorkspaceRules,
            SafetyCategory::BlockedPaths,
        ];
        for category in &categories {
            // get_config seeds defaults on first call.
            self.get_config(user_id, *category).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl CapabilityPermissionStore for PgSafetyConfigStore {
    async fn get_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT permission_mode FROM brassclaw_capability_permissions \
                 WHERE tenant_id = $1 AND capability_id = $2",
                &[&tenant_id, &capability_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(row.and_then(|r| parse_permission_mode(r.get::<_, &str>(0))))
    }

    async fn set_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_capability_permissions \
                 (tenant_id, capability_id, permission_mode) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (tenant_id, capability_id) \
                 DO UPDATE SET permission_mode = excluded.permission_mode",
                &[&tenant_id, &capability_id, &permission_mode_str(mode)],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn delete_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let n = client
            .execute(
                "DELETE FROM brassclaw_capability_permissions \
                 WHERE tenant_id = $1 AND capability_id = $2",
                &[&tenant_id, &capability_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(n > 0)
    }

    async fn list_capability_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT capability_id, permission_mode \
                 FROM brassclaw_capability_permissions \
                 WHERE tenant_id = $1",
                &[&tenant_id],
            )
            .await
            .map_err(map_pg)?;
        let mut map = HashMap::new();
        for row in rows {
            let capability_id: String = row.get(0);
            let mode_str: String = row.get(1);
            if let Some(mode) = parse_permission_mode(&mode_str) {
                map.insert(capability_id, mode);
            }
        }
        Ok(map)
    }
}
