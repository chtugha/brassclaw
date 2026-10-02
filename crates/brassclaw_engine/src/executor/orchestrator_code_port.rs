//! Engine-side port for loading the class-10 Orchestrator component body.
//!
//! Follows the [`ComponentPort`] / [`KohaiPort`] pattern: the trait is defined
//! here in the engine crate and implemented in `brassclaw_reborn_composition`
//! as `PgOrchestratorCodePort`. This is the only correct way to reach
//! `reborn_skills` (class 10) from `brassclaw_engine`.
//!
//! [`ComponentPort`]: crate::executor::composition_port::ComponentPort
//! [`KohaiPort`]: crate::executor::kohai_port::KohaiPort

use async_trait::async_trait;
use thiserror::Error;

/// Errors returned by [`OrchestratorCodePort`].
#[derive(Debug, Error)]
pub enum OrchestratorCodeError {
    #[error("orchestrator component not found in DB; run `brassclaw repair`")]
    NotFound,
    #[error("store error: {reason}")]
    Store { reason: String },
}

/// Port for loading the active Orchestrator code from the component store.
///
/// When `allow_self_modify` is `false`, returns the validated
/// `source='system'` row only. When `true`, prefers a validated
/// operator-customised row if one exists; otherwise falls back to the system
/// row.
///
/// Implemented in `brassclaw_reborn_composition` as `PgOrchestratorCodePort`.
#[async_trait]
pub trait OrchestratorCodePort: Send + Sync {
    async fn load_orchestrator_code(
        &self,
        allow_self_modify: bool,
    ) -> Result<String, OrchestratorCodeError>;
}
