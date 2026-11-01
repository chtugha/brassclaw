//! Closed first-party qualification of the exact packaged B1/B2 answer Recipe.
//! Structural scaffold only — behavioral execution requires the complete
//! contained fixture/oracle and ResultOnly admission owner (BUILD-004/BUILD-002).
//! This owner constructs QualifiedPrefixFreeWorkflow; general authored support,
//! seed labels and JSON evidence cannot construct the opaque successful type.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use std::{path::Path, sync::Arc};
use tokio_postgres::Transaction;

/// Opaque qualification proof. Constructible only after exact Q1/behavior/oracle
/// evidence is durably retained and read back coherently by this module.
/// No Deserialize; no label/JSON path constructs this type.
pub(super) struct QualifiedPrefixFreeWorkflow {
    #[allow(dead_code)]
    program: Arc<InspectedRetainedProgram>,
}

impl QualifiedPrefixFreeWorkflow {
    /// The exact inspected program, independently of any mutable source path.
    #[allow(dead_code)]
    pub(super) fn program(&self) -> Arc<InspectedRetainedProgram> {
        self.program.clone()
    }

    /// Verify that exact stored Q1/behavior evidence still holds at call time.
    /// Called during verify_retained_selection; never performs model I/O.
    pub(super) async fn verify_in_transaction(
        &self,
        _tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        // Behavioral evidence storage pending; this stub is unreachable
        // because qualify() always returns Ok(None) until full implementation.
        Err(invalid("free workflow qualification evidence missing"))
    }
}

/// Attempt to qualify the B1/B2 answer workflow from existing pool owners.
/// Returns Ok(None) when prerequisite evidence is absent or the behavioral
/// fixture execution has not yet been completed.
///
/// The three unavailable() guards in task_model_provider.rs and the
/// PrefixFreeWorkflow=>false gate in global_recipe_ports.rs remain in effect
/// until this function can return Ok(Some(_)).
pub(super) async fn qualify(
    _pool: Arc<PgPool>,
    _worker_path: &Path,
    _limits: UtilityLimits,
) -> Result<Option<QualifiedPrefixFreeWorkflow>, RebornBuildError> {
    // Behavioral qualification requires:
    //  1. Retained exact packaged B1/B2 answer Recipe candidate (BUILD-032 ✓)
    //  2. bootstrap_free_workflow_fixture.rs with contained oracle cases
    //  3. ResultOnly contained-review admission (BUILD-004 ✓)
    //  4. bootstrap_free_workflow_validation.rs execution against fixture
    //  5. Durably retained Q1/behavior evidence (bootstrap_workflow_evidence.rs)
    // Until all five gates are present, return None to leave the existing
    // PrefixFreeWorkflow guards intact.
    Ok(None)
}
