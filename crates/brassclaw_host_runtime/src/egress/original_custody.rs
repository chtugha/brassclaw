//! Trusted host-only original custody. Ordinary RuntimeHttpEgress/Serde requests
//! have no way to select this port or return an unsanitized response.
use async_trait::async_trait;
use brassclaw_host_api::{
    RuntimeHttpEgressError, RuntimeHttpEgressRequest, RuntimeHttpEgressResponse,
};
use brassclaw_secrets::SecretMaterial;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostHttpOriginalCustodyError;
impl std::fmt::Display for HostHttpOriginalCustodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("host original custody unavailable")
    }
}
impl std::error::Error for HostHttpOriginalCustodyError {}

/// One concrete authenticated claim/request custodian, not an observer registry.
/// Hostruntime never interprets product/auth-flow state. The owner must commit
/// actual request custody before accepting transport; retain consumes this sink.
#[async_trait]
pub trait HostHttpOriginalCustody: Send {
    async fn authenticate_request(
        &mut self,
        request: &RuntimeHttpEgressRequest,
    ) -> Result<(), HostHttpOriginalCustodyError>;
    /// Private one-shot encrypted-codec input only. JSON preserves status, ordered
    /// duplicate headers, byte-array body (including non-UTF8), and complete usage.
    /// This material must not be exposed in a DTO, Debug, error or model output.
    async fn retain(
        self: Box<Self>,
        original: SecretMaterial,
    ) -> Result<(), HostHttpOriginalCustodyError>;
}
/// Trusted host composition port; deliberately separate from RuntimeHttpEgress.
/// It returns only the existing normally sanitized public response.
#[async_trait]
pub trait HostHttpOriginalEgress: Send + Sync {
    async fn execute_with_original_custody<'a>(
        &'a self,
        request: RuntimeHttpEgressRequest,
        custody: Box<dyn HostHttpOriginalCustody + 'a>,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError>;
}
/// Sole lossless network-original encoder. Its result goes only to the one-shot
/// custodian, never a generic raw getter/public response or an unsafe log flag.
pub(super) fn original_material(
    response: &brassclaw_network::NetworkHttpResponse,
) -> Result<SecretMaterial, HostHttpOriginalCustodyError> {
    serde_json::to_string(&serde_json::json!({
        "status":response.status,"headers":response.headers,"body":response.body,
        "usage":{"request_bytes":response.usage.request_bytes,"response_bytes":response.usage.response_bytes,
            "resolved_ip":response.usage.resolved_ip}
    })).map(SecretMaterial::from).map_err(|_|HostHttpOriginalCustodyError)
}
