//! `PgSecuritySettingsStore` — Postgres-backed operator-level security config.
//!
//! Reads and writes the `reborn_security_settings` table (V068 migration). One
//! row per `tenant_id` (the operator-level posture; NOT per-user / per-agent).
//! A missing row yields [`SecurityModeConfig::default()`] (all `Auto`) — no
//! DB-less mode, no seed row; the WebUI PUT upserts on first save.
//!
//! Implements [`SecurityConfigSource`] so the C.6 cross-turn driver can load the
//! per-turn posture through [`LoopSecurityPort`] without a direct composition
//! dependency from the driver site. The store captures `tenant_id` at
//! construction (the trait's `load_config` takes no tenant arg); the WebUI route
//! (slice 3) constructs one per authenticated operator session and calls
//! [`PgSecuritySettingsStore::save`].

#[cfg(feature = "postgres")]
mod inner {
    use std::str::FromStr;
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_pg::PgPool;
    use brassclaw_turns::run_profile::{
        SecurityConfigError, SecurityConfigSource, SecurityLayerOverride, SecurityModeConfig,
    };
    use tracing::debug;

    /// Postgres-backed operator-level security-config store.
    #[derive(Clone)]
    pub(crate) struct PgSecuritySettingsStore {
        pool: Arc<PgPool>,
        tenant_id: String,
    }

    impl PgSecuritySettingsStore {
        pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
            }
        }

        /// Load the [`SecurityModeConfig`] for this store's tenant. A missing row
        /// returns [`SecurityModeConfig::default()`] (all `Auto`).
        pub(crate) async fn load(&self) -> Result<SecurityModeConfig, SecurityConfigError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| SecurityConfigError::Load(e.to_string()))?;
            let row = client
                .query_opt(
                    "SELECT policy_override, leases_override, gate_override,
                            event_emission_override, sensitive_tool_scoping_override,
                            namespace_filtering_override
                     FROM reborn_security_settings
                     WHERE tenant_id = $1",
                    &[&self.tenant_id.as_str()],
                )
                .await
                .map_err(|e| SecurityConfigError::Load(e.to_string()))?;

            let Some(r) = row else {
                debug!(
                    tenant_id = %self.tenant_id,
                    "security_settings: no row, returning default (all Auto)"
                );
                return Ok(SecurityModeConfig::default());
            };

            Ok(SecurityModeConfig {
                policy_override: SecurityLayerOverride::from_str(r.get::<_, &str>(0))?,
                leases_override: SecurityLayerOverride::from_str(r.get::<_, &str>(1))?,
                gate_override: SecurityLayerOverride::from_str(r.get::<_, &str>(2))?,
                event_emission_override: SecurityLayerOverride::from_str(r.get::<_, &str>(3))?,
                sensitive_tool_scoping_override: SecurityLayerOverride::from_str(r.get::<_, &str>(4))?,
                namespace_filtering_override: SecurityLayerOverride::from_str(r.get::<_, &str>(5))?,
            })
        }

        /// Upsert the [`SecurityModeConfig`] for this store's tenant. Returns the
        /// row as stored (re-read so the caller sees DB-coerced values).
        pub(crate) async fn save(
            &self,
            cfg: &SecurityModeConfig,
        ) -> Result<SecurityModeConfig, SecurityConfigError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| SecurityConfigError::Load(e.to_string()))?;
            client
                .execute(
                    "INSERT INTO reborn_security_settings
                         (tenant_id,
                          policy_override, leases_override, gate_override,
                          event_emission_override, sensitive_tool_scoping_override,
                          namespace_filtering_override)
                     VALUES ($1,$2,$3,$4,$5,$6,$7)
                     ON CONFLICT ON CONSTRAINT reborn_security_settings_tenant_unique
                     DO UPDATE SET
                         policy_override                 = EXCLUDED.policy_override,
                         leases_override                 = EXCLUDED.leases_override,
                         gate_override                   = EXCLUDED.gate_override,
                         event_emission_override         = EXCLUDED.event_emission_override,
                         sensitive_tool_scoping_override = EXCLUDED.sensitive_tool_scoping_override,
                         namespace_filtering_override    = EXCLUDED.namespace_filtering_override,
                         updated_at                      = now()",
                    &[
                        &self.tenant_id.as_str(),
                        &cfg.policy_override.as_str(),
                        &cfg.leases_override.as_str(),
                        &cfg.gate_override.as_str(),
                        &cfg.event_emission_override.as_str(),
                        &cfg.sensitive_tool_scoping_override.as_str(),
                        &cfg.namespace_filtering_override.as_str(),
                    ],
                )
                .await
                .map_err(|e| SecurityConfigError::Load(e.to_string()))?;
            debug!(tenant_id = %self.tenant_id, "security_settings: upserted");
            self.load().await
        }
    }

    #[async_trait]
    impl SecurityConfigSource for PgSecuritySettingsStore {
        async fn load_config(&self) -> Result<SecurityModeConfig, SecurityConfigError> {
            self.load().await
        }
    }

    #[async_trait]
    impl brassclaw_product_workflow::SecuritySettingsStore for PgSecuritySettingsStore {
        async fn get(
            &self,
        ) -> Result<SecurityModeConfig, brassclaw_product_workflow::SecuritySettingsError> {
            self.load().await.map_err(map_security_settings_error)
        }

        async fn upsert(
            &self,
            config: &SecurityModeConfig,
        ) -> Result<SecurityModeConfig, brassclaw_product_workflow::SecuritySettingsError> {
            self.save(config)
                .await
                .map_err(map_security_settings_error)
        }
    }

    /// Map the turns-layer [`SecurityConfigError`] (pool/pg + deserialize) into
    /// the product-workflow [`SecuritySettingsError`] surface the WebUI route
    /// consumes. `Load` (pool/query) → `Internal`; `Deserialize` (bad override
    /// text) → `Invalid`.
    fn map_security_settings_error(
        error: SecurityConfigError,
    ) -> brassclaw_product_workflow::SecuritySettingsError {
        match error {
            SecurityConfigError::Load(reason) => {
                brassclaw_product_workflow::SecuritySettingsError::Internal(reason)
            }
            SecurityConfigError::Deserialize(reason) => {
                brassclaw_product_workflow::SecuritySettingsError::Invalid(reason)
            }
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgSecuritySettingsStore;
