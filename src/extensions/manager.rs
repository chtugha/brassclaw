//! Minimal ExtensionManager stub for V2 migration.
//!
//! This is a temporary implementation that maintains API compatibility
//! while the underlying MCP client, WASM runtime, and WASM channel
//! infrastructure is being reimplemented for V2.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::extensions::{
    ActivateResult, AuthResult, ConfigureResult, EnsureReadyIntent, EnsureReadyOutcome,
    ExtensionError, ExtensionKind, ExtensionPhase, ExtensionSource, InstallResult,
    InstalledExtension, InteractiveLoginInfo, InteractiveLoginPollResult,
    InteractiveLoginStartResult, LatentProviderAction, RegistryEntry, ResultSource, SearchResult,
    ToolAuthState, UpgradeOutcome, UpgradeResult,
};
use crate::extensions::discovery::OnlineDiscovery;
use crate::extensions::registry::ExtensionRegistry;
use crate::hooks::HookRegistry;
use crate::pairing::PairingStore;
use crate::secrets::SecretsStore;
use crate::channels::ChannelManager;

/// Minimal ExtensionManager that maintains API compatibility during V2 migration.
pub struct ExtensionManager {
    registry: Arc<ExtensionRegistry>,
    discovery: Arc<OnlineDiscovery>,
    secrets: Arc<dyn SecretsStore>,
    hooks: Arc<HookRegistry>,
    pairing: Arc<PairingStore>,
    channel_manager: Arc<ChannelManager>,
    _pending_auth: Arc<RwLock<HashMap<String, ()>>>,
}

impl ExtensionManager {
    pub fn new(
        registry: Arc<ExtensionRegistry>,
        discovery: Arc<OnlineDiscovery>,
        secrets: Arc<dyn SecretsStore>,
        hooks: Arc<HookRegistry>,
        pairing: Arc<PairingStore>,
        channel_manager: Arc<ChannelManager>,
    ) -> Self {
        Self {
            registry,
            discovery,
            secrets,
            hooks,
            pairing,
            channel_manager,
            _pending_auth: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn search(
        &self,
        query: &str,
        _user_id: &str,
    ) -> Result<Vec<SearchResult>, ExtensionError> {
        // Search local registry
        let local_results = self.registry.search(query).await;
        
        // TODO: Add online discovery when reimplemented
        
        Ok(local_results)
    }

    pub async fn install(
        &self,
        name: &str,
        _url: Option<&str>,
        _kind_hint: Option<ExtensionKind>,
        _user_id: &str,
    ) -> Result<InstallResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension installation not yet available in V2. Extension '{}' cannot be installed. \
             MCP servers, WASM tools, and WASM channels are being reimplemented.",
            name
        )))
    }

    pub async fn ensure_extension_ready(
        &self,
        name: &str,
        _user_id: &str,
        _intent: EnsureReadyIntent,
    ) -> Result<EnsureReadyOutcome, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension activation not yet available in V2. Extension '{}' cannot be activated.",
            name
        )))
    }

    pub async fn list(
        &self,
        _user_id: &str,
        _include_inactive: bool,
    ) -> Result<Vec<InstalledExtension>, ExtensionError> {
        // Return empty list - no extensions installed in V2 yet
        Ok(Vec::new())
    }

    pub async fn remove(&self, name: &str, _user_id: &str) -> Result<String, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension removal not yet available in V2. Extension '{}' cannot be removed.",
            name
        )))
    }

    pub async fn upgrade(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<UpgradeResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension upgrade not yet available in V2. Extension '{}' cannot be upgraded.",
            name
        )))
    }

    pub async fn authenticate(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<AuthResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension authentication not yet available in V2. Extension '{}' cannot be authenticated.",
            name
        )))
    }

    pub async fn configure(
        &self,
        name: &str,
        _config: serde_json::Value,
        _user_id: &str,
    ) -> Result<ConfigureResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension configuration not yet available in V2. Extension '{}' cannot be configured.",
            name
        )))
    }

    pub async fn activate(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<ActivateResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Extension activation not yet available in V2. Extension '{}' cannot be activated.",
            name
        )))
    }

    pub async fn check_tool_auth_status(
        &self,
        _name: &str,
        _user_id: &str,
    ) -> Result<ToolAuthState, ExtensionError> {
        // V1 - Return stub state for all tools during V2 migration
        Ok(ToolAuthState::NoAuth)
    }

    pub async fn start_interactive_login(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<InteractiveLoginStartResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Interactive login not yet available in V2. Extension '{}' login cannot be started.",
            name
        )))
    }

    pub async fn poll_interactive_login(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<InteractiveLoginPollResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Interactive login not yet available in V2. Extension '{}' login cannot be polled.",
            name
        )))
    }

    pub async fn get_interactive_login_info(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<Option<InteractiveLoginInfo>, ExtensionError> {
        Err(ExtensionError::NotImplemented(format!(
            "Interactive login not yet available in V2. Extension '{}' login info unavailable.",
            name
        )))
    }

    pub async fn get_extension_info(
        &self,
        name: &str,
        _user_id: &str,
    ) -> Result<Option<InstalledExtension>, ExtensionError> {
        // Check if extension exists in registry
        if self.registry.get(name).await.is_some() {
            // Return None to indicate not installed
            Ok(None)
        } else {
            Err(ExtensionError::NotFound(format!(
                "Extension '{}' not found in registry",
                name
            )))
        }
    }

    pub async fn get_latent_provider_actions(
        &self,
        _user_id: &str,
    ) -> Result<Vec<LatentProviderAction>, ExtensionError> {
        // Return empty list - no latent actions in V2 yet
        Ok(Vec::new())
    }

    pub fn registry(&self) -> &ExtensionRegistry {
        &self.registry
    }

    pub fn discovery(&self) -> &OnlineDiscovery {
        &self.discovery
    }

    pub fn secrets(&self) -> &dyn SecretsStore {
        &*self.secrets
    }

    pub fn hooks(&self) -> &HookRegistry {
        &self.hooks
    }

    pub fn pairing(&self) -> &PairingStore {
        &self.pairing
    }

    pub fn channel_manager(&self) -> &ChannelManager {
        &self.channel_manager
    }

    /// Get notification target for a channel (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn notification_target_for_channel(&self, _channel_name: &str) -> Option<String> {
        None
    }

    /// Get owner ID (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub fn owner_id(&self) -> &str {
        "stub_owner"
    }

    /// Get active tool names (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub fn active_tool_names(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get pending OAuth flows (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub fn pending_oauth_flows(&self) -> Vec<String> {
        Vec::new()
    }
    /// Get extension info (V1 stub - alias for get_extension_info)
    /// TODO: Remove after V2 migration complete
    pub async fn extension_info(
        &self,
        name: &str,
        user_id: &str,
    ) -> Result<Option<InstalledExtension>, ExtensionError> {
        self.get_extension_info(name, user_id).await
    }

    /// Get latent provider actions for default user (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn latent_provider_actions_default_user(
        &self,
    ) -> Result<Vec<LatentProviderAction>, ExtensionError> {
        Ok(Vec::new())
    }

    /// Get latent provider actions (V1 stub - alias)
    /// TODO: Remove after V2 migration complete
    pub async fn latent_provider_actions(
        &self,
        user_id: &str,
    ) -> Result<Vec<LatentProviderAction>, ExtensionError> {
        self.get_latent_provider_actions(user_id).await
    }

    /// Get a specific latent provider action (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn latent_provider_action(
        &self,
        _name: &str,
        _user_id: &str,
    ) -> Result<Option<LatentProviderAction>, ExtensionError> {
        Ok(None)
    }

    /// Get provider action names (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn provider_action_names(&self, _user_id: &str) -> Result<Vec<String>, ExtensionError> {
        Ok(Vec::new())
    }

    /// Configure token (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn configure_token(
        &self,
        _name: &str,
        _token: String,
        _user_id: &str,
    ) -> Result<ConfigureResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(
            "Token configuration not yet available in V2".to_string()
        ))
    }

    /// Get event publisher (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub fn event_publisher(&self) -> Option<()> {
        None
    }

    /// Start hosted OAuth flow (V1 stub)
    /// TODO: Remove after V2 migration complete
    pub async fn start_hosted_oauth_flow(
        &self,
        _name: &str,
        _user_id: &str,
    ) -> Result<InteractiveLoginStartResult, ExtensionError> {
        Err(ExtensionError::NotImplemented(
            "Hosted OAuth flow not yet available in V2".to_string()
        ))
    }
}
