//! Phase V — `McpServerServiceImpl`.
//!
//! Implements [`brassclaw_product_workflow::McpServerService`]: manages the
//! lifecycle of the Orchestrator MCP Server (start / stop / status / settings).
//!
//! The server runs as a tokio background task.  Socket binding and the HTTP
//! serve loop are **not** owned here — they belong in the host binary
//! (CLI or ingress crate) per the
//! `reborn_product_api_crates_do_not_bind_http_ingress` architecture contract.
//! The composition crate exposes a [`McpListenerSpawner`] trait; the host
//! binary provides the concrete impl and passes it at construction time via
//! [`McpServerServiceImpl::new`].
//!
//! Settings are kept in a `Mutex<McpServerSettings>` (in-process only; no DB
//! row needed — the port and auto_start are operator-level runtime toggles).
//!
//! # Feature gate
//!
//! Compiled only when the `skills-db` feature is enabled (the MCP server
//! projection layer it delegates to is `skills-db`-gated).

#![allow(dead_code)]
#![forbid(unsafe_code)]

/// Trait that binds a TCP socket on the given port and drives the HTTP serve
/// loop for the supplied router.
///
/// **This trait must be implemented in a host-owned crate** (e.g.
/// `brassclaw_reborn_webui_ingress` or the CLI binary).  The composition crate
/// only defines the trait surface here — it never binds sockets or drives
/// a serve loop directly, in compliance with
/// `reborn_product_api_crates_do_not_bind_http_ingress`.
///
/// Not feature-gated — the trait surface is shared by any caller, not just the
/// `skills-db` gated code path.
#[async_trait::async_trait]
pub trait McpListenerSpawner: Send + Sync {
    /// Bind a TCP socket on `port` and spawn the HTTP serve loop for
    /// `router`.  Returns the port that was actually bound (useful when the
    /// caller requested port 0 for OS assignment) and a `JoinHandle` for the
    /// serve task.
    async fn bind_and_serve(
        &self,
        port: u16,
        router: axum::Router,
    ) -> Result<(u16, tokio::task::JoinHandle<()>), brassclaw_product_workflow::McpServerServiceError>;
}

#[cfg(feature = "skills-db")]
mod inner {
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

    use super::McpListenerSpawner;

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
    /// Socket binding and the HTTP serve loop are delegated to the injected
    /// [`McpListenerSpawner`] — which must be a host-owned impl from
    /// `brassclaw_reborn_webui_ingress` or the CLI binary.
    pub(crate) struct McpServerServiceImpl {
        pool: Arc<PgPool>,
        port: Arc<dyn ComponentPort>,
        scope: ComponentScope,
        settings: Mutex<McpServerSettings>,
        running: Mutex<RunningServer>,
        spawner: Arc<dyn McpListenerSpawner>,
    }

    impl McpServerServiceImpl {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            port: Arc<dyn ComponentPort>,
            scope: ComponentScope,
            spawner: Arc<dyn McpListenerSpawner>,
        ) -> Self {
            Self {
                pool,
                port,
                scope,
                settings: Mutex::new(McpServerSettings::default()),
                running: Mutex::new(RunningServer::default()),
                spawner,
            }
        }

        fn lock_settings(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, McpServerSettings>, McpServerServiceError> {
            self.settings
                .lock()
                .map_err(|_| McpServerServiceError::Internal("settings lock poisoned".into()))
        }

        fn lock_running(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, RunningServer>, McpServerServiceError> {
            self.running
                .lock()
                .map_err(|_| McpServerServiceError::Internal("running lock poisoned".into()))
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
                    return Err(McpServerServiceError::Invalid("port must be ≥ 1024".into()));
                }
                settings.port = port;
            }
            if let Some(auto_start) = req.auto_start {
                settings.auto_start = auto_start;
            }
            Ok(McpServerSettingsResponse {
                settings: settings.clone(),
            })
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

            // Delegate socket binding and the HTTP serve loop to the host-owned spawner.
            // The spawner lives in brassclaw_reborn_webui_ingress or the CLI binary,
            // which are permitted to bind sockets and drive serve loops.
            let (bound_port, handle) = self.spawner.bind_and_serve(port, router).await?;

            // Reacquire the guard to store the handle.
            {
                let mut running = self.lock_running()?;
                running.port = bound_port;
                running.handle = Some(handle);
            }

            Ok(McpServerActionResponse {
                state: McpServerState::Running,
                message: format!("MCP server started on port {bound_port}"),
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

// ── No-op spawner (used as default when the host has not wired a real one) ───

/// A [`McpListenerSpawner`] that always returns an error.
///
/// Used as the default when the host binary has not injected a concrete spawner
/// (e.g. in tests or when the MCP server is disabled by configuration).
/// Calling [`McpServerService::start`] with this spawner will return
/// `McpServerServiceError::Internal` with a clear message.
pub struct NoopMcpListenerSpawner;

#[async_trait::async_trait]
impl McpListenerSpawner for NoopMcpListenerSpawner {
    async fn bind_and_serve(
        &self,
        _port: u16,
        _router: axum::Router,
    ) -> Result<(u16, tokio::task::JoinHandle<()>), brassclaw_product_workflow::McpServerServiceError>
    {
        Err(brassclaw_product_workflow::McpServerServiceError::Internal(
            "no MCP listener spawner configured — wire a DefaultMcpListenerSpawner from the host binary".into(),
        ))
    }
}

// ── Public re-exports ────────────────────────────────────────────────────────

/// Re-exported so that host-owned ingress crates can satisfy the
/// [`McpListenerSpawner`] contract without taking a direct dependency on
/// `brassclaw_product_workflow` (which is architecturally forbidden from the
/// ingress crate per `reborn_crate_dependency_boundaries_hold`).
pub use brassclaw_product_workflow::McpServerServiceError;

#[cfg(feature = "skills-db")]
pub(crate) use inner::McpServerServiceImpl;
