//! Default [`McpListenerSpawner`] implementation for the host binary.
//!
//! This file lives in `brassclaw_reborn_webui_ingress` (the host-owned ingress
//! crate) rather than `brassclaw_reborn_composition` because it calls
//! `TcpListener::bind` and `axum::serve`, which are forbidden in product/API
//! crates under the `reborn_product_api_crates_do_not_bind_http_ingress`
//! architecture contract.
//!
//! Wire this into the composition layer at startup by replacing the default
//! `NoopMcpListenerSpawner` with a `DefaultMcpListenerSpawner` when configuring
//! `McpServerServiceImpl` (or via a future `RebornWebuiBundle::with_mcp_spawner`
//! wiring helper).

use std::net::SocketAddr;

use brassclaw_reborn_composition::mcp_server_service::{McpListenerSpawner, McpServerServiceError};
use tokio::task::JoinHandle;

/// Concrete [`McpListenerSpawner`] that binds a `TcpListener` and drives
/// `axum::serve`.
///
/// Lives in the host-owned ingress crate so product/API crates remain
/// free of socket-binding calls (enforced by
/// `reborn_product_api_crates_do_not_bind_http_ingress`).
pub struct DefaultMcpListenerSpawner;

#[async_trait::async_trait]
impl McpListenerSpawner for DefaultMcpListenerSpawner {
    async fn bind_and_serve(
        &self,
        port: u16,
        router: axum::Router,
    ) -> Result<(u16, JoinHandle<()>), McpServerServiceError> {
        let addr: SocketAddr = format!("0.0.0.0:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                McpServerServiceError::Invalid(e.to_string())
            })?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| McpServerServiceError::Internal(e.to_string()))?;

        let bound_port = listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(port);

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::debug!("MCP server stopped: {e}");
            }
        });

        Ok((bound_port, handle))
    }
}
