//! Default [`McpListenerSpawner`] implementation for the host binary.
//!
//! This file lives in `brassclaw_reborn_webui_ingress` (the host-owned ingress
//! crate) rather than `brassclaw_reborn_composition` because it calls
//! `TcpListener::bind` and `axum::serve`, which are forbidden in product/API
//! crates under the `reborn_product_api_crates_do_not_bind_http_ingress`
//! architecture contract.
//!
//! This legacy host adapter is not currently wired by production startup.
//! Qualify the Recipe catalogue and ordinary-chat service, then install an
//! authenticated ingress before starting it. Local binding is loopback-only;
//! provider reachability must use an explicit authenticated transport.

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
        let addr: SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| McpServerServiceError::Invalid(e.to_string()))?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| McpServerServiceError::Internal(e.to_string()))?;

        let bound_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::debug!("MCP server stopped: {e}");
            }
        });

        Ok((bound_port, handle))
    }
}

#[cfg(feature = "skills-db")]
mod instance_listener {
    use brassclaw_reborn_composition::{
        mcp_chat_bridge::McpChatBridge,
        mcp_server_service::{
            McpListenerStatus, McpServerServiceError, McpServerState, McpServerStatusResponse,
        },
        orchestrator_mcp_server::{OrchestratorMcpServerConfig, orchestrator_mcp_router_with_chat},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    struct Observation {
        port: u16,
        alive: AtomicBool,
    }
    impl McpListenerStatus for Observation {
        fn status(&self) -> McpServerStatusResponse {
            let alive = self.alive.load(Ordering::SeqCst);
            McpServerStatusResponse {
                state: if alive {
                    McpServerState::Running
                } else {
                    McpServerState::Stopped
                },
                port: alive.then_some(self.port),
                endpoint_url: alive.then(|| format!("http://127.0.0.1:{}/mcp", self.port)),
                error: None,
            }
        }
    }
    struct LiveGuard(Arc<Observation>);
    impl Drop for LiveGuard {
        fn drop(&mut self) {
            self.0.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Host-owned instance listener. Provider exchange cleanup has no access to
    /// its shutdown signal. Dropping an incomplete startup stops admission.
    pub struct InboundMcpListener {
        bridge: Arc<McpChatBridge>,
        observation: Arc<Observation>,
        shutdown: Option<tokio::sync::oneshot::Sender<()>>,
        task: Option<tokio::task::JoinHandle<Result<(), std::io::Error>>>,
    }
    impl InboundMcpListener {
        pub async fn start(
            port: u16,
            bridge: Arc<McpChatBridge>,
        ) -> Result<Self, McpServerServiceError> {
            bridge
                .prepare_startup()
                .await
                .map_err(|error| McpServerServiceError::Unavailable(error.to_string()))?;
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .map_err(|error| McpServerServiceError::Internal(error.to_string()))?;
            let port = listener
                .local_addr()
                .map_err(|error| McpServerServiceError::Internal(error.to_string()))?
                .port();
            let observation = Arc::new(Observation {
                port,
                alive: AtomicBool::new(true),
            });
            let live_guard = LiveGuard(observation.clone());
            let router = orchestrator_mcp_router_with_chat(
                OrchestratorMcpServerConfig::default(),
                bridge.clone(),
            );
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                let _live_guard = live_guard;
                axum::serve(listener, router)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await
            });
            Ok(Self {
                bridge,
                observation,
                shutdown: Some(shutdown_tx),
                task: Some(task),
            })
        }
        pub fn status(&self) -> Arc<dyn McpListenerStatus> {
            self.observation.clone()
        }
        pub async fn shutdown(mut self) -> Result<(), McpServerServiceError> {
            let disconnect = self.bridge.disconnect_all().await;
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(mut task) = self.task.take() {
                match tokio::time::timeout(std::time::Duration::from_secs(30), &mut task).await {
                    Ok(Ok(Ok(()))) => {}
                    Ok(Ok(Err(error))) => {
                        return Err(McpServerServiceError::Internal(error.to_string()));
                    }
                    Ok(Err(error)) => {
                        return Err(McpServerServiceError::Internal(error.to_string()));
                    }
                    Err(_) => {
                        task.abort();
                        let _ = task.await;
                        return Err(McpServerServiceError::Unavailable(
                            "MCP listener drain timed out; original call evidence is retained"
                                .into(),
                        ));
                    }
                }
            }
            disconnect.map_err(|error| McpServerServiceError::Internal(error.to_string()))
        }
    }
    impl Drop for InboundMcpListener {
        fn drop(&mut self) {
            self.bridge.revoke_all();
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(task) = self.task.take() {
                task.abort();
            }
            self.observation.alive.store(false, Ordering::SeqCst);
        }
    }
}
#[cfg(feature = "skills-db")]
pub use instance_listener::InboundMcpListener;
