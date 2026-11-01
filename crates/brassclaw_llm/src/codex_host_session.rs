//! Request-local Codex credentials supplied by the installed host owner.
//! This port stores nothing, chooses no target and performs no model submission.
use crate::{LlmError, ProviderDispatchGuard};
use async_trait::async_trait;
use secrecy::SecretString;
use std::sync::Arc;

/// A token and the host guard bound to that exact token's authenticated source.
/// The host must construct a new immutable guard for each returned snapshot;
/// a guard reading a shared mutable current credential reference is invalid.
pub struct CodexHostSession {
    pub(crate) token: SecretString,
    pub(crate) guard: Arc<dyn ProviderDispatchGuard>,
}
impl CodexHostSession {
    pub fn new(token: SecretString, guard: Arc<dyn ProviderDispatchGuard>) -> Self {
        Self { token, guard }
    }
}
impl std::fmt::Debug for CodexHostSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexHostSession").finish_non_exhaustive()
    }
}
/// The existing host credential/refresh owner authenticates the snapshot,
/// including any durable refresh admission/result/publication. The SDK never
/// refreshes or falls back to a file through this port.
#[async_trait]
pub trait CodexHostSessionSource: std::fmt::Debug + Send + Sync {
    async fn session(&self) -> Result<CodexHostSession, LlmError>;
}
