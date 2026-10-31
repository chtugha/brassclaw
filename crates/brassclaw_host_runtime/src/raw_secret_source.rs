//! Original raw-handle sources on the existing task/invocation owner.
//! No secret material, account selection, workflow or permission is supplied.
use std::fmt;

use async_trait::async_trait;
use brassclaw_host_api::{CapabilityId, CredentialStageError, ResourceScope, SecretHandle};

use crate::RuntimeCredentialAccessSecret;

#[derive(Clone, Copy)]
pub struct RuntimeRawSecretRequest<'a> {
    pub scope: &'a ResourceScope,
    pub capability_id: &'a CapabilityId,
    pub handle: &'a SecretHandle,
}

/// Explicit v3 binding must supply the same original source owner for resolution
/// and every use check. Missing captured material stays missing; implementations
/// must not select a current generation while resolving a later invocation.
#[async_trait]
pub trait RuntimeRawSecretSource: Send + Sync + fmt::Debug {
    async fn resolve_source(
        &self,
        request: RuntimeRawSecretRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError>;

    /// Synchronous before/after-lease/consume check. It grants nothing and must
    /// fence the exact invocation, lifetime and original generation.
    fn check_source_use(
        &self,
        request: &RuntimeRawSecretRequest<'_>,
        source: &RuntimeCredentialAccessSecret,
    ) -> Result<(), CredentialStageError>;
}
