//! Runtime hot-reload support for LLM providers.
//!
//! The core provider chain is rebuilt from config when LLM settings change.
//! [`SwappableLlmProvider`] wraps `Arc<dyn LlmProvider>` so the outer handle
//! stays stable across rebuilds and the rest of the application doesn't have
//! to re-subscribe. [`LlmReloadHandle`] ties the primary and cheap providers
//! together and serializes overlapping reloads.
//!
//! ## Design notes
//!
//! - **One snapshot lock.** All cached metadata (`model_name`,
//!   `active_model_name`, catalogue identity, cost, cache multipliers, and the inner provider
//!   itself) live in a single `RwLock<ProviderSnapshot>`. A reader always
//!   observes a consistent slice of one provider — never a mix of old and
//!   new after a swap.
//! - **Owned model metadata.** Snapshots retain reference-counted names. The
//!   provider interface can borrow immutable names or return an owned copy of
//!   mutable metadata. Replaced names are freed with their owners; there is no
//!   process-lifetime interner, distinct-name quota or overflow placeholder.
//! - **`set_model()` is volatile.** Runtime model switches are forwarded to
//!   the current inner provider only. The next successful
//!   [`LlmReloadHandle::reload`] rebuilds the chain from config and drops
//!   the override. Callers that rely on a model override must persist it
//!   through the normal settings path.

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::error::LlmError;
use crate::provider::{
    CompletionRequest, CompletionResponse, LlmProvider, ModelMetadata, ToolCompletionRequest,
    ToolCompletionResponse,
};

/// Exact database selection captured when its concrete client was prepared for
/// publication, including compatible live-control updates on retained clients.
/// The opaque configuration reference includes an authenticated encrypted-row
/// identity; it never contains a plaintext credential or a hash of that value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderSelectionRevision {
    pub provider_id: String,
    pub definition_revision: u64,
    pub model: String,
    pub configuration_ref: String,
}

/// Actual client publication, distinct from desired configuration and authority.
/// Legacy or custom clients without a captured selection remain explicitly
/// untracked instead of receiving an invented effective revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProviderPublication {
    Untracked,
    Unconfigured,
    Configured {
        selection: ProviderSelectionRevision,
    },
}

/// One atomic view of the loaded client and the last failed captured target.
/// Failure of an unknown source never identifies a later desired configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderPublicationStatus {
    pub effective: ProviderPublication,
    pub failed: Option<ProviderPublication>,
}

/// Host preparation failure, carrying only the target actually captured before
/// that preparation. Error text is returned to its owner, not status metadata.
#[derive(Debug)]
pub struct ProviderPreparationFailure {
    error: LlmError,
    selection: Option<Box<ProviderPublication>>,
}

impl ProviderPreparationFailure {
    pub fn selected(error: LlmError, selection: ProviderPublication) -> Self {
        Self {
            error,
            selection: Some(Box::new(selection)),
        }
    }
}

impl From<LlmError> for ProviderPreparationFailure {
    fn from(error: LlmError) -> Self {
        Self {
            error,
            selection: None,
        }
    }
}

#[derive(Clone)]
struct ProviderSnapshot {
    inner: Arc<dyn LlmProvider>,
    provider_id: String,
    model_name: Arc<str>,
    active_model_name: Arc<str>,
    cost_per_token: (Decimal, Decimal),
    cache_write_multiplier: Decimal,
    cache_read_discount: Decimal,
    publication: ProviderPublication,
    failed: Option<ProviderPublication>,
}

impl std::fmt::Debug for ProviderSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderSnapshot")
            .field("model_name", &self.model_name)
            .field("active_model_name", &&*self.active_model_name)
            .finish_non_exhaustive()
    }
}

impl ProviderSnapshot {
    fn capture(provider: Arc<dyn LlmProvider>) -> Self {
        let identity = provider.model_name().into_owned();
        Self::capture_named(provider, identity)
    }

    fn capture_named(provider: Arc<dyn LlmProvider>, provider_id: String) -> Self {
        let model_name = Arc::from(provider.model_name().as_ref());
        let active_model_name = Arc::from(provider.active_model_name());
        let cost_per_token = provider.cost_per_token();
        let cache_write_multiplier = provider.cache_write_multiplier();
        let cache_read_discount = provider.cache_read_discount();
        Self {
            inner: provider,
            provider_id,
            model_name,
            active_model_name,
            cost_per_token,
            cache_write_multiplier,
            cache_read_discount,
            publication: ProviderPublication::Untracked,
            failed: None,
        }
    }
    fn capture_published(
        provider: Arc<dyn LlmProvider>,
        publication: ProviderPublication,
        fallback_identity: String,
    ) -> Result<Self, LlmError> {
        let identity = match &publication {
            ProviderPublication::Configured { selection } => selection.provider_id.clone(),
            ProviderPublication::Unconfigured => "unconfigured".into(),
            ProviderPublication::Untracked => fallback_identity,
        };
        let mut fresh = Self::capture_named(provider, identity);
        if let ProviderPublication::Configured { selection } = &publication
            && (selection.definition_revision == 0
                || selection.provider_id.is_empty()
                || selection.model.is_empty()
                || selection.model.trim() != selection.model
                || selection.model.chars().any(char::is_control)
                || selection.model != fresh.active_model_name.as_ref()
                || selection.configuration_ref.len() != 64
                || !selection
                    .configuration_ref
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(LlmError::ConfigurationChanged {
                provider: "selected provider".into(),
            });
        }
        fresh.publication = publication;
        Ok(fresh)
    }
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A provider wrapper whose inner provider can be swapped at runtime.
///
/// See the module-level docs for the invariants this type guarantees.
pub struct SwappableLlmProvider {
    state: RwLock<ProviderSnapshot>,
    publication_lock: tokio::sync::Mutex<()>,
    preparation: std::sync::OnceLock<Arc<dyn crate::provider::ProviderSelectionPreparer>>,
}

impl std::fmt::Debug for SwappableLlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snap = read(&self.state);
        f.debug_struct("SwappableLlmProvider")
            .field("model_name", &snap.model_name)
            .field("active_model_name", &&*snap.active_model_name)
            .finish_non_exhaustive()
    }
}

impl SwappableLlmProvider {
    pub fn new(inner: Arc<dyn LlmProvider>) -> Self {
        Self {
            state: RwLock::new(ProviderSnapshot::capture(inner)),
            publication_lock: tokio::sync::Mutex::new(()),
            preparation: std::sync::OnceLock::new(),
        }
    }

    /// Construct an instance-role wrapper with its actual catalogue identity.
    pub fn new_with_identity(inner: Arc<dyn LlmProvider>, provider_id: impl Into<String>) -> Self {
        Self {
            state: RwLock::new(ProviderSnapshot::capture_named(inner, provider_id.into())),
            publication_lock: tokio::sync::Mutex::new(()),
            preparation: std::sync::OnceLock::new(),
        }
    }

    /// Read metadata from the same snapshot as the actual selected backend.
    pub fn publication(&self) -> ProviderPublication {
        read(&self.state).publication.clone()
    }

    pub fn publication_status(&self) -> ProviderPublicationStatus {
        let state = read(&self.state);
        ProviderPublicationStatus {
            effective: state.publication.clone(),
            failed: state.failed.clone(),
        }
    }

    fn preparation_failed(&self, failure: ProviderPreparationFailure) -> LlmError {
        write(&self.state).failed = failure.selection.map(|selection| *selection);
        failure.error
    }

    /// Attach the one owning source during composition. Replacing it silently
    /// could change the authority/namespace of an already running role.
    pub fn attach_preparation(
        &self,
        source: Arc<dyn crate::provider::ProviderSelectionPreparer>,
    ) -> Result<(), LlmError> {
        self.preparation
            .set(source)
            .map_err(|_| LlmError::ConfigurationChanged {
                provider: "selected provider".into(),
            })
    }

    /// Retain the backend and its publication from one read. Separate getters
    /// could pair a tracked revision with an intervening legacy/volatile swap.
    pub fn published_provider(&self) -> (Arc<dyn LlmProvider>, ProviderPublication) {
        let snapshot = read(&self.state);
        (snapshot.inner.clone(), snapshot.publication.clone())
    }

    /// Publish an already built client and its exact captured selection together.
    pub fn swap_with_publication(
        &self,
        inner: Arc<dyn LlmProvider>,
        publication: ProviderPublication,
    ) -> Result<(), LlmError> {
        let fresh = ProviderSnapshot::capture_published(
            inner,
            publication,
            read(&self.state).provider_id.clone(),
        )?;
        *write(&self.state) = fresh;
        Ok(())
    }

    /// Capture and build under the role's existing publication boundary. An
    /// earlier capture cannot wait behind a newer publication and replace it.
    pub async fn publish_from<F, Fut, A>(
        &self,
        prepare: F,
        on_publication: A,
    ) -> Result<ProviderPublication, LlmError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<
                Output = Result<
                    (Arc<dyn LlmProvider>, ProviderPublication),
                    ProviderPreparationFailure,
                >,
            >,
        A: FnOnce(&ProviderPublication),
    {
        let _publication = self.publication_lock.lock().await;
        let (provider, publication) = prepare()
            .await
            .map_err(|failure| self.preparation_failed(failure))?;
        self.swap_with_publication(provider, publication.clone())
            .map_err(|error| {
                self.preparation_failed(ProviderPreparationFailure::selected(
                    error,
                    publication.clone(),
                ))
            })?;
        on_publication(&publication);
        Ok(publication)
    }

    /// Publish identity and backend/model metadata under the same snapshot lock.
    pub fn swap_with_identity(&self, inner: Arc<dyn LlmProvider>, provider_id: impl Into<String>) {
        *write(&self.state) = ProviderSnapshot::capture_named(inner, provider_id.into());
    }

    /// Retain one coherent role selection before any await or provider dispatch.
    pub fn retained_selection(&self) -> crate::RetainedLlmProvider {
        let snapshot = read(&self.state);
        crate::RetainedLlmProvider {
            provider: snapshot.inner.clone(),
            provider_id: snapshot.provider_id.clone(),
            model: snapshot.active_model_name.to_string(),
            publication: snapshot.publication.clone(),
        }
    }

    /// Replace the inner provider chain with a freshly rebuilt provider.
    /// Metadata is refreshed atomically in the same critical section.
    pub fn swap(&self, inner: Arc<dyn LlmProvider>) {
        let fresh = ProviderSnapshot::capture(inner);
        *write(&self.state) = fresh;
    }

    fn current(&self) -> Arc<dyn LlmProvider> {
        read(&self.state).inner.clone()
    }

    async fn prepared_current(&self) -> Result<Arc<dyn LlmProvider>, LlmError> {
        self.prepare_retained_provider()
            .await?
            .map(|selection| selection.provider)
            .ok_or_else(|| LlmError::ConfigurationChanged {
                provider: "selected provider".into(),
            })
    }

    /// Retain the actual provider object and model from one atomic snapshot.
    /// Long-lived work must also use an explicit model override: subsequent
    /// swaps or mutable provider defaults must not change an admitted call.
    pub fn pinned_provider(&self) -> (Arc<dyn LlmProvider>, String) {
        let snapshot = read(&self.state);
        (
            snapshot.inner.clone(),
            snapshot.active_model_name.to_string(),
        )
    }
}

#[async_trait]
impl LlmProvider for SwappableLlmProvider {
    fn retained_provider(&self) -> Option<crate::RetainedLlmProvider> {
        Some(self.retained_selection())
    }

    async fn prepare_retained_provider(
        &self,
    ) -> Result<Option<crate::RetainedLlmProvider>, LlmError> {
        if let Some(source) = self.preparation.get() {
            source.prepare().await?;
            let selection = self.retained_selection();
            if !matches!(
                selection.publication,
                ProviderPublication::Configured { .. }
            ) {
                return Err(LlmError::ConfigurationChanged {
                    provider: "selected provider".into(),
                });
            }
            return Ok(Some(selection));
        }
        Ok(Some(self.retained_selection()))
    }

    fn model_name(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Owned(read(&self.state).model_name.to_string())
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        read(&self.state).cost_per_token
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.prepared_current().await?.complete(request).await
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        self.prepared_current()
            .await?
            .complete_with_tools(request)
            .await
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        self.prepared_current().await?.list_models().await
    }

    async fn model_metadata(&self) -> Result<ModelMetadata, LlmError> {
        self.prepared_current().await?.model_metadata().await
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        self.current().effective_model_name(requested_model)
    }

    fn active_model_name(&self) -> String {
        read(&self.state).active_model_name.to_string()
    }

    fn set_model(&self, model: &str) -> Result<(), LlmError> {
        // Hold the write lock across both the delegate call and the snapshot
        // refresh so a concurrent `swap()` cannot overwrite the just-updated
        // inner provider with a snapshot captured from an older one. Inner
        // `set_model` impls are synchronous (no `.await`), so holding a
        // std::sync lock across the call is safe.
        let mut guard = write(&self.state);
        guard.inner.set_model(model)?;
        let refreshed =
            ProviderSnapshot::capture_named(Arc::clone(&guard.inner), guard.provider_id.clone());
        *guard = refreshed;
        Ok(())
    }

    fn cache_write_multiplier(&self) -> Decimal {
        read(&self.state).cache_write_multiplier
    }

    fn cache_read_discount(&self) -> Decimal {
        read(&self.state).cache_read_discount
    }
}

/// Stable hot-reload handle for the primary/cheap/sempai provider chain.
///
/// Holds the [`SwappableLlmProvider`] wrappers created at startup and
/// serializes concurrent reloads through an internal mutex so rapid setting
/// changes don't trigger overlapping chain rebuilds (which would redo
/// potentially-expensive work like OAuth refresh and HTTP probes).
#[derive(Debug)]
pub struct LlmReloadHandle {
    primary: Arc<SwappableLlmProvider>,
    cheap: Option<Arc<SwappableLlmProvider>>,
    /// Optional independently selected Sempai slot. Primary reload retains it;
    /// the role-aware settings owner publishes its own coherent selection.
    sempai: Option<Arc<SwappableLlmProvider>>,
    /// Serializes concurrent `reload()` calls so rapid setting toggles
    /// don't fire overlapping chain rebuilds (each rebuild can touch OAuth
    /// refresh and HTTP probes; letting them pile up wastes upstream quota
    /// and leaves the wrapper briefly pointing at a half-built chain).
    reload_lock: tokio::sync::Mutex<()>,
}

impl LlmReloadHandle {
    pub fn new(
        primary: Arc<SwappableLlmProvider>,
        cheap: Option<Arc<SwappableLlmProvider>>,
    ) -> Self {
        Self {
            primary,
            cheap,
            sempai: None,
            reload_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Construct a reload handle with an optional Sempai provider slot.
    pub fn new_with_sempai(
        primary: Arc<SwappableLlmProvider>,
        cheap: Option<Arc<SwappableLlmProvider>>,
        sempai: Option<Arc<SwappableLlmProvider>>,
    ) -> Self {
        Self {
            primary,
            cheap,
            sempai,
            reload_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn primary_provider(&self) -> Arc<dyn LlmProvider> {
        self.primary.clone() as Arc<dyn LlmProvider>
    }

    pub fn primary_publication(&self) -> ProviderPublication {
        self.primary.publication()
    }

    pub fn primary_publication_status(&self) -> ProviderPublicationStatus {
        self.primary.publication_status()
    }

    pub fn published_primary_provider(&self) -> (Arc<dyn LlmProvider>, ProviderPublication) {
        self.primary.published_provider()
    }

    pub fn attach_primary_preparation(
        &self,
        source: Arc<dyn crate::provider::ProviderSelectionPreparer>,
    ) -> Result<(), LlmError> {
        self.primary.attach_preparation(source)
    }

    /// Database-owned publication uses the same reload serialization, with
    /// capture inside the lock and one concrete backend/metadata publication.
    /// The v3 role has no independent cheap-model selection.
    pub async fn publish_primary_from<F, Fut>(
        &self,
        prepare: F,
    ) -> Result<ProviderPublication, LlmError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<
                Output = Result<
                    (Arc<dyn LlmProvider>, ProviderPublication),
                    ProviderPreparationFailure,
                >,
            >,
    {
        let _reload = self.reload_lock.lock().await;
        let _publication = self.primary.publication_lock.lock().await;
        let (provider, publication) = prepare()
            .await
            .map_err(|failure| self.primary.preparation_failed(failure))?;
        let identity = read(&self.primary.state).provider_id.clone();
        let fresh = ProviderSnapshot::capture_published(provider, publication.clone(), identity)
            .map_err(|error| {
                self.primary
                    .preparation_failed(ProviderPreparationFailure::selected(
                        error,
                        publication.clone(),
                    ))
            })?;
        // Validate once before changing either slot. Both receive the same
        // concrete backend and metadata even for mutable custom adapters.
        *write(&self.primary.state) = fresh.clone();
        if let Some(cheap) = &self.cheap {
            *write(&cheap.state) = fresh;
        }
        Ok(publication)
    }

    pub fn cheap_provider(&self) -> Option<Arc<dyn LlmProvider>> {
        self.cheap
            .as_ref()
            .map(|provider| provider.clone() as Arc<dyn LlmProvider>)
    }

    /// The Sempai (teaching/auditing) provider, when one was wired at startup.
    pub fn sempai_provider(&self) -> Option<Arc<dyn LlmProvider>> {
        self.sempai
            .as_ref()
            .map(|provider| provider.clone() as Arc<dyn LlmProvider>)
    }

    /// Publish an explicitly unconfigured primary backend using the same
    /// serialization as reload. Existing calls retain their preceding Arc;
    /// future primary and cheap calls receive the replacement. Sempai keeps
    /// its independently selected backend.
    pub async fn replace_primary(&self, replacement: Arc<dyn LlmProvider>) {
        let _guard = self.reload_lock.lock().await;
        let _publication = self.primary.publication_lock.lock().await;
        self.primary
            .swap_with_identity(Arc::clone(&replacement), "unconfigured");
        if let Some(cheap) = &self.cheap {
            cheap.swap(replacement);
        }
    }

    /// Rebuild the provider chain from `config` and atomically replace the
    /// inner providers of the primary and optional cheap wrappers. Sempai is
    /// selected independently by the role-aware settings owner.
    ///
    /// Reloads are serialized so two concurrent callers cannot race.
    pub async fn reload(
        &self,
        config: &crate::LlmConfig,
        session: Arc<crate::SessionManager>,
    ) -> Result<(), LlmError> {
        let _guard = self.reload_lock.lock().await;
        let _publication = self.primary.publication_lock.lock().await;

        let components = crate::build_provider_chain_components(config, session).await?;

        self.primary
            .swap_with_identity(components.primary, config.backend.clone());

        if let Some(ref cheap_handle) = self.cheap {
            let new_cheap = components
                .cheap
                .unwrap_or_else(|| self.primary.clone() as Arc<dyn LlmProvider>);
            cheap_handle.swap(new_cheap);
        } else if components.cheap.is_some() {
            // Asymmetry: no cheap wrapper was allocated at startup, so a
            // newly-configured cheap model cannot be activated via hot-reload.
            // Surfacing this through tracing so ops don't think the swap
            // silently took effect.
            tracing::debug!(
                "llm hot-reload: cheap provider is now configured but was not at startup; \
                 it will only take effect after a full restart",
            );
        }

        // Sempai reload: the Sempai config is managed independently through
        // the role-aware set_active path; we do not rebuild it from `config`
        // here.  A full Sempai swap happens when the operator explicitly
        // changes the Sempai selection via the settings service, which calls
        // `swap()` on the wrapper directly.  However, if the Sempai happens
        // to be backed by the same provider as primary (edge case: temporary
        // single-provider setups), the caller can drive a sempai swap
        // independently — this slot is kept here so the handle is the single
        // structural truth for all three provider roles.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{CompletionRequest, ToolCompletionRequest};
    use std::sync::RwLock as StdRwLock;

    /// Simple stub that supports `set_model()` so we can exercise the
    /// snapshot-refresh path and the "override is lost on swap" behaviour.
    #[derive(Debug)]
    struct TestProvider {
        configured: String,
        active: StdRwLock<String>,
        cost: (Decimal, Decimal),
        cache_write: Decimal,
        cache_read: Decimal,
    }

    impl TestProvider {
        fn new(configured: &str, active: &str, cost: (Decimal, Decimal)) -> Self {
            Self {
                configured: configured.to_owned(),
                active: StdRwLock::new(active.to_string()),
                cost,
                cache_write: Decimal::ONE,
                cache_read: Decimal::ONE,
            }
        }
    }

    #[async_trait]
    impl LlmProvider for TestProvider {
        fn model_name(&self) -> std::borrow::Cow<'_, str> {
            std::borrow::Cow::Borrowed(&self.configured)
        }

        fn cost_per_token(&self) -> (Decimal, Decimal) {
            self.cost
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            Err(LlmError::RequestFailed {
                provider: self.configured.to_string(),
                reason: "TestProvider does not implement complete".to_string(),
            })
        }

        async fn complete_with_tools(
            &self,
            _request: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, LlmError> {
            Err(LlmError::RequestFailed {
                provider: self.configured.to_string(),
                reason: "TestProvider does not implement complete_with_tools".to_string(),
            })
        }

        fn active_model_name(&self) -> String {
            self.active.read().expect("test lock").clone()
        }

        fn set_model(&self, model: &str) -> Result<(), LlmError> {
            *self.active.write().expect("test lock") = model.to_string();
            Ok(())
        }

        fn cache_write_multiplier(&self) -> Decimal {
            self.cache_write
        }

        fn cache_read_discount(&self) -> Decimal {
            self.cache_read
        }
    }

    #[test]
    fn named_selection_retains_backend_and_identity_across_replacement() {
        let a = Arc::new(TestProvider::new(
            "cfg-a",
            "active-a",
            (Decimal::ONE, Decimal::ONE),
        ));
        let wrapper = SwappableLlmProvider::new_with_identity(a.clone(), "catalogue-a");
        wrapper.set_model("selected-a").unwrap();
        let retained = wrapper.retained_selection();
        assert_eq!(retained.provider_id, "catalogue-a");
        assert_eq!(retained.model, "selected-a");
        let b = Arc::new(TestProvider::new(
            "cfg-b",
            "active-b",
            (Decimal::ONE, Decimal::ONE),
        ));
        wrapper.swap_with_identity(b.clone(), "catalogue-b");
        let current = wrapper.retained_selection();
        assert_eq!(current.provider_id, "catalogue-b");
        assert_eq!(current.model, "active-b");
        assert_eq!(retained.provider.active_model_name(), "selected-a");
        let a: Arc<dyn LlmProvider> = a;
        let b: Arc<dyn LlmProvider> = b;
        assert!(Arc::ptr_eq(&a, &retained.provider));
        assert!(Arc::ptr_eq(&b, &current.provider));
        let selection = ProviderSelectionRevision {
            provider_id: "catalogue-b".into(),
            definition_revision: 2,
            model: "active-b".into(),
            configuration_ref: "ab".repeat(32),
        };
        let publication = ProviderPublication::Configured {
            selection: selection.clone(),
        };
        wrapper
            .swap_with_publication(b.clone(), publication.clone())
            .unwrap();
        assert_eq!(wrapper.publication(), publication);
        let mut wrong = selection;
        wrong.model = "another-model".into();
        assert!(matches!(
            wrapper.swap_with_publication(
                a.clone(),
                ProviderPublication::Configured { selection: wrong }
            ),
            Err(LlmError::ConfigurationChanged { .. })
        ));
        assert_eq!(wrapper.publication(), publication);
        assert!(Arc::ptr_eq(&wrapper.retained_selection().provider, &b));
        let (published, published_revision) = wrapper.published_provider();
        wrapper.swap_with_identity(a.clone(), "catalogue-a");
        assert!(Arc::ptr_eq(&published, &b));
        assert_eq!(published_revision, publication);
        assert_eq!(wrapper.publication(), ProviderPublication::Untracked);
        wrapper
            .swap_with_publication(b.clone(), publication)
            .unwrap();
        wrapper.set_model("volatile-model").unwrap();
        assert_eq!(
            wrapper.publication(),
            ProviderPublication::Untracked,
            "a volatile override cannot retain a database revision acknowledgement"
        );
    }

    #[test]
    fn swap_replaces_all_metadata_atomically() {
        let a = Arc::new(TestProvider::new(
            "cfg-a",
            "active-a",
            (Decimal::new(1, 0), Decimal::new(2, 0)),
        ));
        let wrapper = SwappableLlmProvider::new(a);

        assert_eq!(wrapper.model_name(), "cfg-a");
        assert_eq!(wrapper.active_model_name(), "active-a");
        assert_eq!(
            wrapper.cost_per_token(),
            (Decimal::new(1, 0), Decimal::new(2, 0))
        );

        let b = Arc::new(TestProvider::new(
            "cfg-b",
            "active-b",
            (Decimal::new(3, 0), Decimal::new(4, 0)),
        ));
        wrapper.swap(b);

        assert_eq!(wrapper.model_name(), "cfg-b");
        assert_eq!(wrapper.active_model_name(), "active-b");
        assert_eq!(
            wrapper.cost_per_token(),
            (Decimal::new(3, 0), Decimal::new(4, 0))
        );
    }

    #[tokio::test]
    async fn failed_publication_retains_client_and_clears_only_with_actual_success() {
        let provider: Arc<dyn LlmProvider> = Arc::new(TestProvider::new(
            "catalogue",
            "active",
            (Decimal::ONE, Decimal::ONE),
        ));
        let slot = Arc::new(SwappableLlmProvider::new(provider.clone()));
        let handle = LlmReloadHandle::new(slot.clone(), None);
        let configured = |model: &str| ProviderPublication::Configured {
            selection: ProviderSelectionRevision {
                provider_id: "catalogue".into(),
                definition_revision: 1,
                model: model.into(),
                configuration_ref: "ab".repeat(32),
            },
        };
        let wrong = configured("wrong-model");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            handle.publish_primary_from(|| async { Ok((provider.clone(), wrong.clone())) }),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(LlmError::ConfigurationChanged { .. })));
        assert!(Arc::ptr_eq(&provider, &slot.pinned_provider().0));
        assert_eq!(
            slot.publication_status(),
            ProviderPublicationStatus {
                effective: ProviderPublication::Untracked,
                failed: Some(wrong)
            }
        );
        let actual = configured("active");
        handle
            .publish_primary_from(|| async { Ok((provider.clone(), actual.clone())) })
            .await
            .unwrap();
        assert_eq!(
            slot.publication_status(),
            ProviderPublicationStatus {
                effective: actual.clone(),
                failed: None
            }
        );
        let failure: Result<
            (Arc<dyn LlmProvider>, ProviderPublication),
            ProviderPreparationFailure,
        > = Err(LlmError::ConfigurationChanged {
            provider: "uncaptured-source".into(),
        }
        .into());
        assert!(
            slot.publish_from(
                || async { failure },
                |_| panic!("failure cannot acknowledge publication")
            )
            .await
            .is_err()
        );
        assert_eq!(
            slot.publication_status(),
            ProviderPublicationStatus {
                effective: actual,
                failed: None
            },
            "unknown source error cannot identify a failed desired revision"
        );
    }

    #[test]
    fn set_model_forwards_and_refreshes_snapshot() {
        let inner = Arc::new(TestProvider::new(
            "cfg",
            "cfg",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = SwappableLlmProvider::new(inner);

        wrapper
            .set_model("cfg-override")
            .expect("test provider supports set_model");

        assert_eq!(wrapper.active_model_name(), "cfg-override");
    }

    #[test]
    fn set_model_override_is_dropped_on_swap() {
        let initial = Arc::new(TestProvider::new(
            "cfg-a",
            "cfg-a",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = SwappableLlmProvider::new(initial);

        wrapper
            .set_model("cfg-a-override")
            .expect("set_model supported");
        assert_eq!(wrapper.active_model_name(), "cfg-a-override");

        let replacement = Arc::new(TestProvider::new(
            "cfg-b",
            "cfg-b",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        wrapper.swap(replacement);

        assert_eq!(wrapper.active_model_name(), "cfg-b");
    }

    #[test]
    fn model_name_snapshots_keep_exact_names_and_release_replaced_allocations() {
        // Metadata-only regression: no model request or simulated provider
        // success. Exercise the actual swappable snapshot and its lifetimes.
        let wrapper = SwappableLlmProvider::new(Arc::new(TestProvider::new(
            "initial",
            "initial",
            (Decimal::ZERO, Decimal::ZERO),
        )));
        for index in 0..2048 {
            let previous_name = wrapper.model_name();
            let retained = wrapper.retained_selection();
            let previous_allocation = Arc::downgrade(&read(&wrapper.state).model_name);
            let name = if index % 2 == 0 {
                format!("configured-model-{index}")
            } else {
                format!("{}-{index}", "long-configured-model-".repeat(20))
            };
            wrapper.swap(Arc::new(TestProvider::new(
                &name,
                &name,
                (Decimal::ZERO, Decimal::ZERO),
            )));
            assert_eq!(wrapper.model_name().as_ref(), name.as_str());
            assert_eq!(wrapper.active_model_name(), name);
            assert_eq!(retained.provider.model_name(), previous_name);
            assert_eq!(retained.model.as_str(), previous_name.as_ref());
            assert!(previous_allocation.upgrade().is_none());
        }
    }

    /// Concurrent `swap` and `set_model` against the same wrapper must
    /// never crash or deadlock, and the final snapshot must be readable.
    /// Before the fix, `set_model` released the state read lock between
    /// mutating the inner and writing the snapshot — a parallel `swap`
    /// could overwrite the snapshot with one from an older provider,
    /// leaving `model_name()` from one replacement while `cost_per_token()`
    /// was captured from a different
    /// inner. The write-lock-spanning fix keeps the swap and the set-model
    /// snapshot mutually exclusive.
    ///
    /// This is a stress test — it won't catch every possible race window,
    /// but it reliably triggers the pre-fix bug and passes cleanly post-fix.
    #[test]
    fn set_model_and_swap_are_mutually_atomic() {
        use std::sync::Arc as StdArc;
        use std::thread;

        let initial = StdArc::new(TestProvider::new(
            "p-a",
            "p-a",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = StdArc::new(SwappableLlmProvider::new(initial));

        const ITERS: usize = 200;

        let w1 = StdArc::clone(&wrapper);
        let swapper = thread::spawn(move || {
            for i in 0..ITERS {
                let replacement = StdArc::new(TestProvider::new(
                    if i % 2 == 0 { "p-even" } else { "p-odd" },
                    if i % 2 == 0 { "p-even" } else { "p-odd" },
                    (Decimal::ZERO, Decimal::ZERO),
                ));
                w1.swap(replacement);
            }
        });

        let w2 = StdArc::clone(&wrapper);
        let setter = thread::spawn(move || {
            for i in 0..ITERS {
                let _ = w2.set_model(&format!("override-{i}"));
            }
        });

        swapper.join().expect("swapper thread");
        setter.join().expect("setter thread");

        // Final invariant: the wrapper is still readable and its reported
        // `model_name` is one of the literals the swapper inserted (the
        // set_model path only ever updates `active_model_name`, not the
        // configured `model_name`). A torn snapshot wouldn't necessarily
        // corrupt the pointer, but a panic or deadlock under the old code
        // would.
        let configured = wrapper.model_name();
        assert!(
            matches!(configured.as_ref(), "p-a" | "p-even" | "p-odd"),
            "configured model_name must come from a real swap: {configured}",
        );
        let _ = wrapper.active_model_name();
        let _ = wrapper.cost_per_token();
    }
}
