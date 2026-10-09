//! Phase V — `McpServerServiceImpl`.
//!
//! Inbound MCP settings/readiness boundary. The unsafe legacy serving stub
//! is retired; the complete Recipe/chat service is not yet qualified.
//!
//! The eventual server lifetime is instance-owned, not controlled by provider
//! connections or WebUI start/stop buttons. HTTP listening remains host-owned.
//! Missing qualified discovery/chat/provider wiring reports Unavailable.
//! Settings here are process-local compatibility settings, not persisted state.

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

/// Read-only actual listener observation supplied by its host lifecycle owner.
pub trait McpListenerStatus: Send + Sync {
    fn status(&self) -> brassclaw_product_workflow::McpServerStatusResponse;
}

#[cfg(feature = "skills-db")]
mod inner {
    use crate::orchestrator_mcp_server::MCP_CHAT_UNAVAILABLE;
    use async_trait::async_trait;
    use brassclaw_product_workflow::{
        McpServerActionResponse, McpServerService, McpServerServiceError, McpServerSettings,
        McpServerSettingsResponse, McpServerStartRequest, McpServerState, McpServerStatusResponse,
        UpdateMcpServerSettingsRequest,
    };
    use std::sync::Mutex;

    /// Settings/readiness facade while the Recipe/chat transport is unqualified.
    /// It deliberately has no execution, database or listener handle. Startup
    /// must wire the complete qualified chat service before claiming Running.
    pub(crate) struct McpServerServiceImpl {
        settings: Mutex<McpServerSettings>,
        listener: Option<std::sync::Arc<dyn super::McpListenerStatus>>,
    }

    impl McpServerServiceImpl {
        pub(crate) fn new() -> Self {
            Self {
                settings: Mutex::new(McpServerSettings::default()),
                listener: None,
            }
        }
        pub(crate) fn with_listener(
            mut self,
            listener: std::sync::Arc<dyn super::McpListenerStatus>,
        ) -> Self {
            if let Some(port) = listener.status().port {
                self.settings = Mutex::new(McpServerSettings {
                    port,
                    auto_start: true,
                });
            }
            self.listener = Some(listener);
            self
        }
        fn lock_settings(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, McpServerSettings>, McpServerServiceError> {
            self.settings
                .lock()
                .map_err(|_| McpServerServiceError::Internal("settings lock poisoned".into()))
        }
    }

    #[async_trait]
    impl McpServerService for McpServerServiceImpl {
        async fn get_settings(&self) -> Result<McpServerSettingsResponse, McpServerServiceError> {
            Ok(McpServerSettingsResponse {
                settings: self.lock_settings()?.clone(),
            })
        }

        async fn update_settings(
            &self,
            req: UpdateMcpServerSettingsRequest,
        ) -> Result<McpServerSettingsResponse, McpServerServiceError> {
            // Validate the entire request before changing any setting.
            if req.port.is_some_and(|port| port < 1024) {
                return Err(McpServerServiceError::Invalid("port must be ≥ 1024".into()));
            }
            if req.auto_start == Some(false) {
                return Err(McpServerServiceError::Invalid(
                    "the inbound MCP server must run for the instance lifetime".into(),
                ));
            }
            if let Some(listener) = &self.listener
                && req
                    .port
                    .is_some_and(|port| listener.status().port != Some(port))
            {
                return Err(McpServerServiceError::Invalid(
                    "listener port is fixed for this instance lifetime; configure it at startup"
                        .into(),
                ));
            }
            let mut settings = self.lock_settings()?;
            if let Some(port) = req.port {
                settings.port = port;
            }
            Ok(McpServerSettingsResponse {
                settings: settings.clone(),
            })
        }

        async fn get_status(&self) -> Result<McpServerStatusResponse, McpServerServiceError> {
            if let Some(listener) = &self.listener {
                return Ok(listener.status());
            }
            Ok(McpServerStatusResponse {
                state: McpServerState::Error,
                port: None,
                endpoint_url: None,
                error: Some(MCP_CHAT_UNAVAILABLE.into()),
            })
        }

        async fn start(
            &self,
            req: McpServerStartRequest,
        ) -> Result<McpServerActionResponse, McpServerServiceError> {
            if let Some(listener) = &self.listener {
                let status = listener.status();
                if req.port.is_some_and(|port| status.port != Some(port)) {
                    return Err(McpServerServiceError::Invalid(
                        "listener port is fixed at startup".into(),
                    ));
                }
                if status.state == McpServerState::Running {
                    return Ok(McpServerActionResponse {
                        state: status.state,
                        message: "instance-owned MCP listener is running".into(),
                    });
                }
            }
            Err(McpServerServiceError::Unavailable(
                MCP_CHAT_UNAVAILABLE.into(),
            ))
        }

        async fn stop(&self) -> Result<McpServerActionResponse, McpServerServiceError> {
            Err(McpServerServiceError::Invalid("MCP server lifetime belongs to instance startup/shutdown; provider disconnect must not stop it".into()))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn inbound_mcp_lifecycle_reports_unavailable_instead_of_false_running() {
            let service = McpServerServiceImpl::new();
            assert!(matches!(
                service.start(McpServerStartRequest::default()).await,
                Err(McpServerServiceError::Unavailable(_))
            ));
            let status = service.get_status().await.unwrap();
            assert!(matches!(status.state, McpServerState::Error));
            assert!(status.endpoint_url.is_none());
            assert!(status.error.unwrap().contains("ordinary-chat"));
            assert!(matches!(
                service.stop().await,
                Err(McpServerServiceError::Invalid(_))
            ));
        }

        #[tokio::test]
        async fn inbound_mcp_always_on_setting_rejects_partial_updates() {
            let service = McpServerServiceImpl::new();
            assert!(service.get_settings().await.unwrap().settings.auto_start);
            assert!(matches!(
                service
                    .update_settings(UpdateMcpServerSettingsRequest {
                        port: Some(9091),
                        auto_start: Some(false)
                    })
                    .await,
                Err(McpServerServiceError::Invalid(_))
            ));
            assert_eq!(service.get_settings().await.unwrap().settings.port, 9090);
        }
    }
}

// ── No-op spawner (used as default when the host has not wired a real one) ───

/// A [`McpListenerSpawner`] that always returns an error.
///
/// Explicit unavailable host adapter. It never starts a listener. The new
/// server must be qualified at instance startup before a real spawner is used.
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
pub use brassclaw_product_workflow::{
    McpServerServiceError, McpServerState, McpServerStatusResponse,
};

#[cfg(feature = "skills-db")]
pub(crate) use inner::McpServerServiceImpl;
