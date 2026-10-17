//! Immutable generation read/retention primitives. They do not compile, select
//! sources/designs, validate model quality, activate prefixes or choose workflows.
use crate::FirstPartyCapabilityError;
use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode, ResourceScope, RuntimeDispatchErrorKind};
use serde_json::Value;
use std::sync::Arc;

pub const PREFIX_GENERATION_CAPABILITY_ID: &str = "builtin.prefix_generation";
pub(super) const TRANSPORT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PrefixGenerationBackendError {
    #[error("immutable generation backend unavailable")]
    Unavailable,
    #[error("immutable generation request invalid")]
    Invalid,
    #[error("immutable generation predecessor changed")]
    Conflict,
    #[error("immutable generation not found")]
    NotFound,
}
#[async_trait]
pub trait PrefixGenerationBackend: Send + Sync {
    async fn read(
        &self,
        profile: &str,
        generation: &str,
        scope: &ResourceScope,
    ) -> Result<Value, PrefixGenerationBackendError>;
    async fn retain(
        &self,
        profile: &str,
        expected_profile_revision: u64,
        generation_bytes: &str,
        scope: &ResourceScope,
    ) -> Result<Value, PrefixGenerationBackendError>;
}
#[derive(Default, Clone)]
pub(super) struct PrefixGenerationState {
    backend: Option<Arc<dyn PrefixGenerationBackend>>,
}
impl std::fmt::Debug for PrefixGenerationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefixGenerationState")
            .field("wired", &self.backend.is_some())
            .finish()
    }
}
impl PrefixGenerationState {
    pub(super) fn with_backend(backend: Arc<dyn PrefixGenerationBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}
pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    let mut profile = super::resource_profile();
    if let Some(profile) = &mut profile
        && let Some(ceiling) = &mut profile.hard_ceiling
    {
        ceiling.max_output_bytes = Some(TRANSPORT_BYTES);
    }
    super::first_party_capability_manifest(
        PREFIX_GENERATION_CAPABILITY_ID,
        "Read or retain one exact immutable prefix generation; retention is quarantined and never activates a serving selection.",
        vec![EffectKind::ExternalWrite],
        PermissionMode::Allow,
        profile,
    )
}
pub(super) async fn dispatch(
    state: &PrefixGenerationState,
    input: &Value,
    scope: &ResourceScope,
) -> Result<Value, FirstPartyCapabilityError> {
    let object = input.as_object().ok_or_else(invalid)?;
    let profile = string(input, "profile")?;
    if profile.len() > 64
        || !profile
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_lowercase() || (i > 0 && (b.is_ascii_digit() || b == b'-')))
    {
        return Err(invalid());
    }
    let operation = string(input, "operation")?;
    // Decode and reject extra input before looking up a backend or any effects.
    let result = match operation {
        "read" => {
            if object.len() != 3 {
                return Err(invalid());
            }
            let generation = string(input, "generation")?;
            if generation.len() != 64
                || !generation
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid());
            }
            state
                .backend
                .as_ref()
                .ok_or_else(unavailable)?
                .read(profile, generation, scope)
                .await
        }
        "retain" => {
            if object.len() != 4 {
                return Err(invalid());
            }
            let revision = input["expected_profile_revision"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
                .ok_or_else(invalid)?;
            let bytes = string(input, "generation_bytes")?;
            if bytes.len() > TRANSPORT_BYTES as usize {
                return Err(invalid());
            }
            state
                .backend
                .as_ref()
                .ok_or_else(unavailable)?
                .retain(profile, revision, bytes, scope)
                .await
        }
        _ => return Err(invalid()),
    };
    result.map_err(|error| {
        let kind = match error {
            PrefixGenerationBackendError::Invalid => RuntimeDispatchErrorKind::InputEncode,
            PrefixGenerationBackendError::Conflict | PrefixGenerationBackendError::NotFound => {
                RuntimeDispatchErrorKind::OperationFailed
            }
            PrefixGenerationBackendError::Unavailable => RuntimeDispatchErrorKind::Backend,
        };
        FirstPartyCapabilityError::with_safe_summary(kind, error.to_string())
    })
}
fn string<'a>(input: &'a Value, name: &str) -> Result<&'a str, FirstPartyCapabilityError> {
    input[name]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)
}
fn invalid() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::InputEncode)
}
fn unavailable() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::with_safe_summary(
        RuntimeDispatchErrorKind::Backend,
        "immutable generation backend unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Counter(AtomicUsize);
    #[async_trait]
    impl PrefixGenerationBackend for Counter {
        async fn read(
            &self,
            _profile: &str,
            _generation: &str,
            _scope: &ResourceScope,
        ) -> Result<Value, PrefixGenerationBackendError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"fixture":"read"}))
        }
        async fn retain(
            &self,
            _profile: &str,
            _revision: u64,
            _bytes: &str,
            _scope: &ResourceScope,
        ) -> Result<Value, PrefixGenerationBackendError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(PrefixGenerationBackendError::Conflict)
        }
    }
    #[tokio::test]
    async fn prefix_generation_rejects_invalid_or_activation_inputs_before_backend_effects() {
        let backend = Arc::new(Counter(AtomicUsize::new(0)));
        let state = PrefixGenerationState::with_backend(backend.clone());
        let scope = ResourceScope::system();
        for input in [
            json!({"operation":"activate","profile":"base-prompt"}),
            json!({"operation":"retain","profile":"base-prompt","expected_profile_revision":0,"generation_bytes":"typed data"}),
            json!({"operation":"retain","profile":"base-prompt","expected_profile_revision":1,"generation_bytes":"typed data","activate":true}),
            json!({"operation":"read","profile":"../outside","generation":"a".repeat(64)}),
            json!({"operation":"read","profile":"base-prompt","generation":"not-a-hash"}),
        ] {
            assert!(dispatch(&state, &input, &scope).await.is_err());
        }
        assert_eq!(backend.0.load(Ordering::SeqCst), 0);
        let input = json!({"operation":"retain","profile":"base-prompt","expected_profile_revision":1,"generation_bytes":"typed data"});
        assert!(dispatch(&state, &input, &scope).await.is_err());
        assert_eq!(
            backend.0.load(Ordering::SeqCst),
            1,
            "one declared operation, no retry or downstream workflow"
        );
    }
    #[tokio::test]
    async fn prefix_generation_missing_backend_never_falls_back_to_legacy_storage() {
        let input = json!({"operation":"read","profile":"base-prompt","generation":"a".repeat(64)});
        assert!(
            dispatch(
                &PrefixGenerationState::default(),
                &input,
                &ResourceScope::system()
            )
            .await
            .is_err()
        );
    }
}
