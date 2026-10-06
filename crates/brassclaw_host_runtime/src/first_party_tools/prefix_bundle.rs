//! Host capabilities used to assemble and persist a validated prefix bundle.
//!
//! Scope is carried by an opaque, server-issued ticket. Implementations must
//! resolve that ticket on every operation; callers never provide raw IDs.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode, ResourceScope, RuntimeDispatchErrorKind};
use serde_json::{Value, json};

use crate::FirstPartyCapabilityError;

use super::{first_party_capability_manifest, resource_profile};

pub const SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID: &str = "builtin.sweep_validated_components";
pub const STORE_PREFIX_BUNDLE_CAPABILITY_ID: &str = "builtin.store_prefix_bundle";

#[derive(Debug, Clone)]
pub struct SweepValidatedComponentsResult {
    pub bundle: String,
    pub component_count: usize,
    pub generation_ms: i64,
}

#[derive(Debug, Clone)]
pub struct StorePrefixBundleResult {
    pub fingerprint: String,
    pub generation_ms: i64,
    pub generation_id: uuid::Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum PrefixBundleCapabilityError {
    #[error("prefix bundle backend is not wired")]
    NotWired,
    #[error("prefix bundle operation failed: {0}")]
    Backend(String),
}

#[async_trait]
pub trait SweepValidatedComponentsBackend: Send + Sync {
    async fn sweep_and_format(
        &self,
        scope_ticket: &str,
        runtime_scope: &ResourceScope,
    ) -> Result<SweepValidatedComponentsResult, PrefixBundleCapabilityError>;
}

#[async_trait]
pub trait StorePrefixBundleBackend: Send + Sync {
    async fn store(
        &self,
        scope_ticket: &str,
        bundle: &str,
        generation_ms: i64,
        runtime_scope: &ResourceScope,
    ) -> Result<StorePrefixBundleResult, PrefixBundleCapabilityError>;
}

#[derive(Clone, Default)]
pub struct PrefixBundleCapabilityState {
    sweep: Option<Arc<dyn SweepValidatedComponentsBackend>>,
    store: Option<Arc<dyn StorePrefixBundleBackend>>,
}

impl std::fmt::Debug for PrefixBundleCapabilityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefixBundleCapabilityState")
            .field("sweep_wired", &self.sweep.is_some())
            .field("store_wired", &self.store.is_some())
            .finish()
    }
}

impl PrefixBundleCapabilityState {
    pub fn with_backends(
        sweep: Arc<dyn SweepValidatedComponentsBackend>,
        store: Arc<dyn StorePrefixBundleBackend>,
    ) -> Self {
        Self {
            sweep: Some(sweep),
            store: Some(store),
        }
    }
}

pub(super) fn manifests() -> Result<Vec<CapabilityManifest>, ExtensionError> {
    Ok(vec![
        first_party_capability_manifest(
            SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID,
            "Assemble a prefix bundle from validated components using a server-issued scope ticket.",
            vec![EffectKind::ExternalWrite, EffectKind::DispatchCapability],
            PermissionMode::Allow,
            resource_profile(),
        )?,
        first_party_capability_manifest(
            STORE_PREFIX_BUNDLE_CAPABILITY_ID,
            "Store a prefix bundle using its server-issued scope ticket.",
            vec![EffectKind::ExternalWrite],
            PermissionMode::Allow,
            resource_profile(),
        )?,
    ])
}

pub(super) async fn dispatch(
    state: &PrefixBundleCapabilityState,
    capability_id: &str,
    input: &Value,
    runtime_scope: &ResourceScope,
) -> Result<Value, FirstPartyCapabilityError> {
    match capability_id {
        SWEEP_VALIDATED_COMPONENTS_CAPABILITY_ID => {
            let ticket = required_string(input, "scope_ticket")?;
            let backend = state.sweep.as_ref().ok_or_else(not_wired)?;
            let result = backend
                .sweep_and_format(ticket, runtime_scope)
                .await
                .map_err(backend_error)?;
            Ok(
                json!({"bundle": result.bundle, "component_count": result.component_count, "generation_ms": result.generation_ms}),
            )
        }
        STORE_PREFIX_BUNDLE_CAPABILITY_ID => {
            let ticket = required_string(input, "scope_ticket")?;
            let bundle = required_string(input, "bundle")?;
            let generation_ms = input
                .get("generation_ms")
                .and_then(Value::as_i64)
                .filter(|n| *n >= 0)
                .ok_or_else(|| input_error("generation_ms must be a non-negative integer"))?;
            let backend = state.store.as_ref().ok_or_else(not_wired)?;
            let result = backend
                .store(ticket, bundle, generation_ms, runtime_scope)
                .await
                .map_err(backend_error)?;
            Ok(
                json!({"fingerprint": result.fingerprint, "generation_ms": result.generation_ms, "generation_id": result.generation_id}),
            )
        }
        _ => Err(FirstPartyCapabilityError::new(
            RuntimeDispatchErrorKind::UndeclaredCapability,
        )),
    }
}

fn required_string<'a>(input: &'a Value, key: &str) -> Result<&'a str, FirstPartyCapabilityError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| input_error(&format!("{key} must be a non-empty string")))
}

fn input_error(summary: &str) -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::with_safe_summary(RuntimeDispatchErrorKind::InputEncode, summary)
}

fn backend_error(error: PrefixBundleCapabilityError) -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::with_safe_summary(
        RuntimeDispatchErrorKind::Backend,
        error.to_string(),
    )
}

fn not_wired() -> FirstPartyCapabilityError {
    backend_error(PrefixBundleCapabilityError::NotWired)
}
