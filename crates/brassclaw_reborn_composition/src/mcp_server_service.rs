//! Phase V — `McpServerServiceImpl`.
//!
//! Implements [`brassclaw_product_workflow::McpServerService`]: manages the
//! lifecycle of the Orchestrator MCP Server (start / stop / status / settings).
//!
//! The server runs as a tokio background task holding an Axum `TcpListener`.
//! Settings are kept in a `Mutex<McpServerSettings>` (in-process only; no DB
//! row needed — the port and auto_start are operator-level runtime toggles).
//!
//! # Feature gate
//!
//! Compiled only when the `skills-db` feature is enabled (the MCP server
//! projection layer it delegates to is `skills-db`-gated).

#![allow(dead_code)]
#![forbid(unsafe_code)]

#[cfg(feature = "skills-db")]
mod inner {
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use brassclaw_engine::executor::ComponentPort;
    use brassclaw_engine::memory::ComponentScope;
    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{
        McpServerActionResponse, McpServerService, McpServerServiceError, McpServerSettings,
        McpServerSettingsResponse, McpServerStartRequest, McpServerState, McpServerStatusResponse,
        UpdateMcpServerSettingsRequest,
    };
    use tokio::task::JoinHandle;

    use crate::orchestrator_mcp_server::{OrchestratorMcpServerConfig, orchestrator_mcp_router};

    // ── Internal state ────────────────────────────────────────────────────────

    #[derive(Default)]
    struct RunningServer {
        port: u16,
        handle: Option<JoinHandle<()>>,
    }

    impl RunningServer {
        fn is_running(&self) -> bool {
            self.handle
                .as_ref()
                .map(|h| !h.is_finished())
                .unwrap_or(false)
        }
    }

    // ── Public service struct ─────────────────────────────────────────────────

    /// Composition-side impl of [`McpServerService`].
    ///
    /// Holds settings + a handle to the running server task (when started).
    pub(crate) struct McpServerServiceImpl {
        pool: Arc<PgPool>,
        port: Arc<dyn ComponentPort>,
        scope: ComponentScope,
        settings: Mutex<McpServerSettings>,
        running: Mutex<RunningServer>,
    }

    impl McpServerServiceImpl {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            port: Arc<dyn ComponentPort>,
            scope: ComponentScope,
        ) -> Self {
            Self {
                pool,
                port,
                scope,
                settings: Mutex::new(McpServerSettings::default()),
                running: Mutex::new(RunningServer::default()),
            }
        }

        fn lock_settings(&self) -> Result<std::sync::MutexGuard<'_, McpServerSettings>, McpServerServiceError> {
            self.settings.lock().map_err(|_| McpServerServiceError::Internal("settings lock poisoned".into()))
        }

        fn lock_running(&self) -> Result<std::sync::MutexGuard<'_, RunningServer>, McpServerServiceError> {
            self.running.lock().map_err(|_| McpServerServiceError::Internal("running lock poisoned".into()))
        }
    }

    #[async_trait]
    impl McpServerService for McpServerServiceImpl {
        async fn get_settings(&self) -> Result<McpServerSettingsResponse, McpServerServiceError> {
            let settings = self.lock_settings()?.clone();
            Ok(McpServerSettingsResponse { settings })
        }

        async fn update_settings(
            &self,
            req: UpdateMcpServerSettingsRequest,
        ) -> Result<McpServerSettingsResponse, McpServerServiceError> {
            let mut settings = self.lock_settings()?;
            if let Some(port) = req.port {
                if port < 1024 {
                    return Err(McpServerServiceError::Invalid(
                        "port must be ≥ 1024".into(),
                    ));
                }
                settings.port = port;
            }
            if let Some(auto_start) = req.auto_start {
                settings.auto_start = auto_start;
            }
            Ok(McpServerSettingsResponse { settings: settings.clone() })
        }

        async fn get_status(&self) -> Result<McpServerStatusResponse, McpServerServiceError> {
            let running = self.lock_running()?;
            if running.is_running() {
                let port = running.port;
                Ok(McpServerStatusResponse {
                    state: McpServerState::Running,
                    port: Some(port),
                    endpoint_url: Some(format!("http://0.0.0.0:{port}/mcp")),
                    error: None,
                })
            } else {
                Ok(McpServerStatusResponse {
                    state: McpServerState::Stopped,
                    port: None,
                    endpoint_url: None,
                    error: None,
                })
            }
        }

        async fn start(
            &self,
            req: McpServerStartRequest,
        ) -> Result<McpServerActionResponse, McpServerServiceError> {
            // Check if already running and determine port — release guard before await.
            let port = {
                let running = self.lock_running()?;
                if running.is_running() {
                    return Ok(McpServerActionResponse {
                        state: McpServerState::Running,
                        message: "MCP server is already running".into(),
                    });
                }
                // Determine port: request override → settings → default 9090.
                req.port
                    .unwrap_or_else(|| self.lock_settings().map(|s| s.port).unwrap_or(9090))
                // `running` guard dropped here before the `.await` below.
            };

            let router = orchestrator_mcp_router(
                Arc::clone(&self.pool),
                Arc::clone(&self.port),
                self.scope.clone(),
                OrchestratorMcpServerConfig::default(),
            );

            let addr: SocketAddr = format!("0.0.0.0:{port}").parse().map_err(|e: std::net::AddrParseError| {
                McpServerServiceError::Invalid(e.to_string())
            })?;

            // Await happens here with no MutexGuard held.
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|e| McpServerServiceError::Internal(e.to_string()))?;

            let handle = tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::debug!("MCP server stopped: {e}");
                }
            });

            // Reacquire the guard to store the handle.
            {
                let mut running = self.lock_running()?;
                running.port = port;
                running.handle = Some(handle);
            }

            Ok(McpServerActionResponse {
                state: McpServerState::Running,
                message: format!("MCP server started on port {port}"),
            })
        }

        async fn stop(&self) -> Result<McpServerActionResponse, McpServerServiceError> {
            let mut running = self.lock_running()?;
            if let Some(handle) = running.handle.take() {
                handle.abort();
                running.port = 0;
                Ok(McpServerActionResponse {
                    state: McpServerState::Stopped,
                    message: "MCP server stopped".into(),
                })
            } else {
                Ok(McpServerActionResponse {
                    state: McpServerState::Stopped,
                    message: "MCP server was not running".into(),
                })
            }
        }
    }
}

// ── Public re-export (feature-gated) ─────────────────────────────────────────

#[cfg(feature = "skills-db")]
pub(crate) use inner::McpServerServiceImpl;
