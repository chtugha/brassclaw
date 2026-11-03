//! Postgres-backed [`PureApprovalStore`] wiring.
//!
//! Delegates directly to
//! [`pg_pure_component_approval::resolve_existing_pure_component_approval`];
//! this module adds only the trait adapter needed by
//! [`brassclaw_product_workflow::RebornServices`].

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::PureApprovalStore;

/// Postgres-backed [`PureApprovalStore`] that delegates to
/// `pg_pure_component_approval::resolve_existing_pure_component_approval`.
/// The legacy declaration endpoint resolves an exact existing decision only;
/// it cannot create an approval from caller-selected evidence or provenance.
#[derive(Clone)]
pub(crate) struct PgPureApprovalStore {
    pool: Arc<PgPool>,
}

impl PgPureApprovalStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PureApprovalStore for PgPureApprovalStore {
    async fn submit_pure_component_approval(
        &self,
        json_body: &str,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let root_id = crate::pg_pure_component_approval::resolve_existing_pure_component_approval(
            &self.pool, json_body,
        )
        .await?;
        Ok(root_id)
    }
}
