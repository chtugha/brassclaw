//! Native immutable-storage adapter over the same generation owner as WebUI.
//! Prefix rows are instance references, not execution authority. Kernel/attempt
//! fencing and durable effect accounting remain in the existing dispatch path.
use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_host_runtime::{PrefixGenerationBackend, PrefixGenerationBackendError as Error};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{ImportPrefixGenerationRequest, InterceptorConfigServiceError};
use serde_json::Value;
use std::sync::Arc;

pub(crate) struct PgPrefixGenerationBackend {
    pool: Arc<PgPool>,
}
impl PgPrefixGenerationBackend {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl PrefixGenerationBackend for PgPrefixGenerationBackend {
    async fn read(
        &self,
        profile: &str,
        generation: &str,
        _scope: &ResourceScope,
    ) -> Result<Value, Error> {
        let detail = crate::prefix_generations::PgPrefixGenerations::new(self.pool.clone())
            .get(profile, generation)
            .await
            .map_err(map)?;
        serde_json::to_value(detail).map_err(|_| Error::Unavailable)
    }
    async fn retain(
        &self,
        profile: &str,
        expected_profile_revision: u64,
        generation_bytes: &str,
        _scope: &ResourceScope,
    ) -> Result<Value, Error> {
        let detail = crate::prefix_generations::PgPrefixGenerations::new(self.pool.clone())
            .import(
                profile,
                ImportPrefixGenerationRequest {
                    expected_profile_revision,
                    generation_bytes: generation_bytes.into(),
                },
            )
            .await
            .map_err(map)?;
        // Return only the immutable publication receipt. A later read is a
        // separate usage; no quality, design-acceptance or active pointer changes.
        serde_json::to_value(detail.summary).map_err(|_| Error::Unavailable)
    }
}
fn map(error: InterceptorConfigServiceError) -> Error {
    match error {
        InterceptorConfigServiceError::InvalidRequest { .. } => Error::Invalid,
        InterceptorConfigServiceError::Conflict => Error::Conflict,
        InterceptorConfigServiceError::PrefixNotFound { .. }
        | InterceptorConfigServiceError::JobNotFound
        | InterceptorConfigServiceError::GenerationNotFound => Error::NotFound,
        _ => Error::Unavailable,
    }
}
