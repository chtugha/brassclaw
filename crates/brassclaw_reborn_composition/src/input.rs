use std::path::PathBuf;
use std::sync::Arc;

use brassclaw_auth::{AuthProductError, CredentialAccountLabel, OAuthClientId, OAuthRedirectUri};
use brassclaw_host_api::runtime_policy::ProcessBackendKind;
use brassclaw_host_api::runtime_policy::{
    EffectiveRuntimePolicy, FilesystemBackendKind, NetworkMode, SecretMode,
};
use brassclaw_host_runtime::{SchedulerTurnRunWakeNotifier, TenantSandboxProcessPort};
use brassclaw_trust::HostTrustPolicy;

use secrecy::SecretString;

use crate::google_oauth::google_provider_spec;
use crate::notion_oauth::notion_provider_spec;
use crate::oauth_dcr::OAuthDcrProviderConfig;
use crate::oauth_provider_client::HostOAuthProviderSpec;
use crate::RebornProductAuthServicePorts;

/// Composition-time OAuth client metadata.
///
/// `RebornBuildInput` owns this seam for product/bootstrap-provided values
/// until a settings-backed source exists.
#[derive(Clone)]
pub struct OAuthClientConfig {
    pub client_id: OAuthClientId,
    pub client_secret: Option<SecretString>,
    pub redirect_uri: OAuthRedirectUri,
    pub hosted_domain_hint: Option<String>,
}

impl OAuthClientConfig {
    pub fn new(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        client_secret: Option<SecretString>,
    ) -> Result<Self, AuthProductError> {
        Ok(Self {
            client_id: OAuthClientId::new(client_id)?,
            client_secret,
            redirect_uri: OAuthRedirectUri::new(redirect_uri)?,
            hosted_domain_hint: None,
        })
    }

    pub fn with_hosted_domain_hint(mut self, hosted_domain_hint: impl Into<String>) -> Self {
        self.hosted_domain_hint = Some(hosted_domain_hint.into());
        self
    }
}

impl std::fmt::Debug for OAuthClientConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthClientConfig")
            .field("client_id", &self.client_id.as_str())
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("redirect_uri", &self.redirect_uri)
            .field(
                "hosted_domain_hint",
                &self.hosted_domain_hint.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OAuthProviderBackendConfig {
    pub(crate) spec: HostOAuthProviderSpec,
    pub(crate) client: OAuthClientConfig,
}

#[derive(Debug, Clone)]
pub(crate) struct OAuthDcrProviderBackendConfig {
    pub(crate) config: OAuthDcrProviderConfig,
}

#[derive(Clone, Debug, Default)]
pub enum RebornRuntimeProcessBinding {
    #[default]
    None,
    TenantSandbox {
        process_port: Arc<TenantSandboxProcessPort>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RebornRuntimeProcessBindingError {
    MissingTenantSandboxProcessPort,
    UnexpectedTenantSandboxProcessPort { process_backend: ProcessBackendKind },
}

impl std::fmt::Display for RebornRuntimeProcessBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTenantSandboxProcessPort => formatter.write_str(
                "production tenant-sandbox process backend requires a tenant sandbox process binding",
            ),
            Self::UnexpectedTenantSandboxProcessPort { process_backend } => write!(
                formatter,
                "production runtime policy uses {process_backend:?} but a tenant sandbox process binding was supplied"
            ),
        }
    }
}

impl RebornRuntimeProcessBinding {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn tenant_sandbox(process_port: Arc<TenantSandboxProcessPort>) -> Self {
        Self::TenantSandbox { process_port }
    }

    pub(crate) fn validate_for_production_policy(
        &self,
        runtime_policy: &EffectiveRuntimePolicy,
    ) -> Result<(), RebornRuntimeProcessBindingError> {
        match (runtime_policy.process_backend, self) {
            (
                ProcessBackendKind::TenantSandbox,
                RebornRuntimeProcessBinding::TenantSandbox { .. },
            ) => Ok(()),
            (ProcessBackendKind::TenantSandbox, RebornRuntimeProcessBinding::None) => {
                Err(RebornRuntimeProcessBindingError::MissingTenantSandboxProcessPort)
            }
            (_, RebornRuntimeProcessBinding::TenantSandbox { .. }) => Err(
                RebornRuntimeProcessBindingError::UnexpectedTenantSandboxProcessPort {
                    process_backend: runtime_policy.process_backend,
                },
            ),
            (_, RebornRuntimeProcessBinding::None) => Ok(()),
        }
    }
}

pub struct RebornBuildInput {
    pub(crate) owner_id: String,
    /// Tenant scope for Postgres-backed durable stores (secret store,
    /// credential broker, product-auth tables).  `None` in pure local-dev
    /// (test) builds where the filesystem backend is used instead.
    pub(crate) tenant_id: Option<String>,
    pub(crate) storage: RebornStorageInput,
    pub(crate) production_trust_policy: Option<Arc<HostTrustPolicy>>,
    pub(crate) runtime_policy: Option<EffectiveRuntimePolicy>,
    pub(crate) turn_run_wake_notifier: Option<Arc<SchedulerTurnRunWakeNotifier>>,
    pub(crate) runtime_process_binding: RebornRuntimeProcessBinding,
    pub(crate) required_runtime_backends: Vec<brassclaw_host_api::RuntimeKind>,
    pub(crate) require_runtime_http_egress: bool,
    pub(crate) product_auth_ports: Option<RebornProductAuthServicePorts>,
    pub(crate) oauth_provider_configs: Vec<OAuthProviderBackendConfig>,
    pub(crate) oauth_dcr_provider_configs: Vec<OAuthDcrProviderBackendConfig>,
}

pub(crate) enum RebornStorageInput {
    Disabled,
    LocalDev {
        root: PathBuf,
        workspace_root: Option<PathBuf>,
        host_home_root: Option<PathBuf>,
    },
    #[cfg(feature = "postgres")]
    Postgres {
        pool: deadpool_postgres::Pool,
        /// Postgres connection URL. Reserved for future use (previously forwarded to
        /// a secondary event-store pool that has since been replaced by the shared pool).
        #[allow(dead_code)]
        url: brassclaw_secrets::SecretMaterial,
        /// Pre-resolved master key (takes priority over per-boot table lookup).
        #[allow(dead_code)]
        secret_master_key: Option<brassclaw_secrets::SecretMaterial>,
        /// `$BRASSCLAW_REBORN_HOME` path, used by per-boot master-key resolution
        /// to locate the raw-key file when `secret_master_key` is `None`, and by
        /// `build_reborn_runtime` to seed the system-prompt storage root on the
        /// pure-postgres path.
        reborn_home: PathBuf,
    },
}

impl RebornBuildInput {
    /// Owner id (string form). Used by the assembled runtime to mint the
    /// `UserId` actor for inbound CLI messages.
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    /// Override the owner id after construction.
    ///
    /// The WebChat v2 serve path uses this to pin the runtime owner to the
    /// authenticated WebUI user *after* the runtime input (and its host-access
    /// disclosure gate) has been built, so the turn-runner loop host reads
    /// thread context from the same `owners/<user>` subtree the v2 facade
    /// wrote to.
    pub fn with_owner_id(mut self, owner_id: impl Into<String>) -> Self {
        self.owner_id = owner_id.into();
        self
    }

    /// Build a disabled (no-op) input, typically used in tests that do not
    /// need a running runtime.
    pub fn disabled(owner_id: impl Into<String>) -> Self {
        Self::new(owner_id, RebornStorageInput::Disabled)
    }

    pub fn local_dev(owner_id: impl Into<String>, root: PathBuf) -> Self {
        Self::new(
            owner_id,
            RebornStorageInput::LocalDev {
                root,
                workspace_root: None,
                host_home_root: None,
            },
        )
    }

    pub fn with_local_dev_workspace_root(mut self, workspace_root: PathBuf) -> Self {
        if let RebornStorageInput::LocalDev {
            workspace_root: root,
            ..
        } = &mut self.storage
        {
            *root = Some(workspace_root);
        }
        self
    }

    pub fn with_local_dev_confirmed_host_home_root(mut self, host_home_root: PathBuf) -> Self {
        if let RebornStorageInput::LocalDev {
            host_home_root: root,
            ..
        } = &mut self.storage
        {
            *root = Some(host_home_root);
        }
        self
    }

    /// Return the Postgres pool from this build input, if one is present.
    ///
    /// Returns `Some` only when the input was created via one of the
    /// `postgres*` constructors. Returns `None` for `local_dev` and `disabled`
    /// inputs. Used by the serve path to extract the pool for access-store
    /// wiring without requiring callers to match on the internal storage enum.
    #[cfg(feature = "postgres")]
    pub fn pg_pool(&self) -> Option<&deadpool_postgres::Pool> {
        match &self.storage {
            RebornStorageInput::Postgres { pool, .. } => Some(pool),
            _ => None,
        }
    }

    /// Return the `reborn_home` path from a Postgres build input, if present.
    ///
    /// Used by `build_reborn_runtime` to derive the system-prompt storage root
    /// on the pure-postgres path (no local-dev substrate).
    #[cfg(feature = "postgres")]
    pub(crate) fn pg_reborn_home(&self) -> Option<&std::path::Path> {
        match &self.storage {
            RebornStorageInput::Postgres { reborn_home, .. } => Some(reborn_home.as_path()),
            _ => None,
        }
    }

    pub fn requires_local_dev_confirmed_host_home_root(&self) -> bool {
        self.runtime_policy.as_ref().is_some_and(|policy| {
            policy.filesystem_backend == FilesystemBackendKind::HostWorkspaceAndHome
        })
    }

    pub fn grants_trusted_laptop_access(&self) -> bool {
        self.runtime_policy.as_ref().is_some_and(|policy| {
            policy.filesystem_backend == FilesystemBackendKind::HostWorkspaceAndHome
                || policy.network_mode == NetworkMode::Direct
                || policy.secret_mode == SecretMode::InheritedEnv
        })
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        owner_id: impl Into<String>,
        pool: deadpool_postgres::Pool,
        url: brassclaw_secrets::SecretMaterial,
        secret_master_key: brassclaw_secrets::SecretMaterial,
        reborn_home: PathBuf,
    ) -> Self {
        Self::new(
            owner_id,
            RebornStorageInput::Postgres {
                pool,
                url,
                secret_master_key: Some(secret_master_key),
                reborn_home,
            },
        )
    }

    /// Build a Postgres input that resolves the master key at boot time from
    /// `brassclaw_secrets_master` using the ceremony selector (§4.4).
    ///
    /// `reborn_home` is the resolved `$BRASSCLAW_REBORN_HOME` path, required
    /// for the raw-key-on-disk ceremony to locate `.secrets-master-key`.
    #[cfg(feature = "postgres")]
    pub fn postgres_with_reborn_home(
        owner_id: impl Into<String>,
        pool: deadpool_postgres::Pool,
        url: brassclaw_secrets::SecretMaterial,
        reborn_home: PathBuf,
    ) -> Self {
        Self::new(
            owner_id,
            RebornStorageInput::Postgres {
                pool,
                url,
                secret_master_key: None,
                reborn_home,
            },
        )
    }

    pub fn with_required_runtime_backends(
        mut self,
        backends: impl IntoIterator<Item = brassclaw_host_api::RuntimeKind>,
    ) -> Self {
        self.required_runtime_backends = backends.into_iter().collect();
        self
    }

    pub fn with_production_trust_policy(mut self, policy: Arc<HostTrustPolicy>) -> Self {
        self.production_trust_policy = Some(policy);
        self
    }

    pub fn with_runtime_policy(mut self, policy: EffectiveRuntimePolicy) -> Self {
        self.runtime_policy = Some(policy);
        self
    }

    pub fn runtime_policy(&self) -> Option<&EffectiveRuntimePolicy> {
        self.runtime_policy.as_ref()
    }

    pub fn with_turn_run_wake_notifier(
        mut self,
        notifier: Arc<SchedulerTurnRunWakeNotifier>,
    ) -> Self {
        self.turn_run_wake_notifier = Some(notifier);
        self
    }

    pub fn with_runtime_process_binding(mut self, binding: RebornRuntimeProcessBinding) -> Self {
        self.runtime_process_binding = binding;
        self
    }

    pub fn require_runtime_http_egress(mut self) -> Self {
        self.require_runtime_http_egress = true;
        self
    }

    /// Inject Reborn-native product-auth service ports.
    ///
    /// Production callers should provide durable implementations here. The
    /// composition root attaches the turn-continuation dispatcher after it has
    /// composed the profile's [`brassclaw_turns::TurnCoordinator`], so OAuth
    /// continuations cannot accidentally bypass the active coordinator.
    pub fn with_product_auth_ports(mut self, ports: RebornProductAuthServicePorts) -> Self {
        self.product_auth_ports = Some(ports);
        self
    }

    /// Record product/bootstrap-provided Google OAuth metadata on the build input.
    ///
    /// `RebornBuildInput` owns this composition seam until a settings-backed
    /// source exists.
    pub fn with_google_oauth_backend(mut self, config: OAuthClientConfig) -> Self {
        self.push_oauth_provider_config(google_provider_spec(), config);
        self
    }

    /// Record product/bootstrap-provided Notion MCP OAuth metadata on the build input.
    ///
    /// This keeps Notion OAuth in the Reborn product-auth provider path; callers
    /// that use dynamic client registration can pass the client metadata they
    /// registered for this host callback URL.
    pub fn with_notion_oauth_backend(mut self, config: OAuthClientConfig) -> Self {
        self.push_oauth_provider_config(notion_provider_spec(), config);
        self
    }

    /// Enable Dynamic Client Registration for the bundled Notion MCP OAuth provider.
    ///
    /// Callers provide the public origin that serves the Reborn product-auth
    /// callback route. Local loopback HTTP origins are accepted; non-loopback
    /// deployments must use HTTPS.
    pub fn with_notion_dcr_oauth_backend(
        mut self,
        callback_origin: impl Into<String>,
        client_name: impl Into<String>,
    ) -> Result<Self, brassclaw_auth::AuthProductError> {
        self.push_oauth_dcr_provider_config(OAuthDcrProviderConfig {
            spec: notion_provider_spec(),
            callback_origin: callback_origin.into(),
            client_name: client_name.into(),
            account_label: CredentialAccountLabel::new("notion")?,
            scopes: Vec::new(),
        });
        Ok(self)
    }

    fn push_oauth_provider_config(
        &mut self,
        spec: HostOAuthProviderSpec,
        client: OAuthClientConfig,
    ) {
        if let Some(existing) = self
            .oauth_provider_configs
            .iter_mut()
            .find(|existing| existing.spec.provider_id == spec.provider_id)
        {
            existing.spec = spec;
            existing.client = client;
            return;
        }
        self.oauth_provider_configs
            .push(OAuthProviderBackendConfig { spec, client });
    }

    fn push_oauth_dcr_provider_config(&mut self, config: OAuthDcrProviderConfig) {
        if let Some(existing) = self
            .oauth_dcr_provider_configs
            .iter_mut()
            .find(|existing| existing.config.spec.provider_id == config.spec.provider_id)
        {
            existing.config = config;
            return;
        }
        self.oauth_dcr_provider_configs
            .push(OAuthDcrProviderBackendConfig { config });
    }

    /// Set the tenant identifier for Postgres-backed durable stores.
    ///
    /// When present on a `local_dev` build input that also carries a PG pool
    /// (the hybrid serve path), `build_local_dev` uses `PgCredentialBroker`
    /// and `PgAuthProductServices` instead of `FilesystemAuthProductServices`.
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    fn new(owner_id: impl Into<String>, storage: RebornStorageInput) -> Self {
        Self {
            owner_id: owner_id.into(),
            tenant_id: None,
            storage,
            production_trust_policy: None,
            runtime_policy: None,
            turn_run_wake_notifier: None,
            runtime_process_binding: RebornRuntimeProcessBinding::default(),
            required_runtime_backends: Vec::new(),
            require_runtime_http_egress: false,
            product_auth_ports: None,
            oauth_provider_configs: Vec::new(),
            oauth_dcr_provider_configs: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use brassclaw_auth::InMemoryAuthProductServices;

    use super::*;

    #[test]
    fn with_product_auth_ports_records_injected_ports() {
        let product_auth = RebornProductAuthServicePorts::from_shared(Arc::new(
            InMemoryAuthProductServices::new(),
        ));

        let input =
            RebornBuildInput::disabled("test-owner").with_product_auth_ports(product_auth.clone());

        assert!(input.product_auth_ports.is_some());
    }
}
