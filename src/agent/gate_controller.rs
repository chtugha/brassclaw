//! Gate controller implementations for tool execution approval.
//!
//! P0.3: Simplified auto-approving gate controller that unblocks tool execution
//! while providing structure for future real approval implementation.

use std::collections::HashMap;
use std::sync::Arc;

use brassclaw_engine::gate::{GateController, GatePauseRequest, GateResolution};
use tokio::sync::RwLock;

/// Auto-approving gate controller for P0.3.
///
/// This controller automatically approves all tool execution requests,
/// unblocking tool execution while providing structure for future real
/// approval implementation.
///
/// Features:
/// - Logs all approval requests for debugging
/// - Stores pending requests for inspection
/// - Auto-approves all requests
/// - Provides clear upgrade path to real approval
///
/// Future work (P0.4+):
/// - Implement real user approval flow
/// - Add notification system
/// - Implement timeout handling
/// - Support "always approve" feature
/// - Add approval audit logging
pub struct AutoApprovingGateController {
    /// Pending approval requests, keyed by call_id.
    /// Stored for debugging and inspection.
    pending_requests: Arc<RwLock<HashMap<String, GatePauseRequest>>>,
}

impl AutoApprovingGateController {
    /// Create a new auto-approving gate controller.
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create as an Arc<dyn GateController>, the form most call sites need.
    pub fn arc() -> Arc<dyn GateController> {
        Arc::new(Self::new())
    }

    /// Get the number of pending requests (for debugging).
    pub async fn pending_count(&self) -> usize {
        self.pending_requests.read().await.len()
    }

    /// Clear all pending requests (for cleanup).
    pub async fn clear_pending(&self) {
        self.pending_requests.write().await.clear();
    }
}

impl Default for AutoApprovingGateController {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl GateController for AutoApprovingGateController {
    async fn pause(&self, request: GatePauseRequest) -> GateResolution {
        // Log the approval request for debugging
        tracing::info!(
            thread_id = %request.thread_id,
            user_id = %request.user_id,
            gate_name = %request.gate_name,
            action_name = %request.action_name,
            call_id = %request.call_id,
            resume_kind = ?request.resume_kind,
            "Auto-approving tool execution (P0.3 stub - real approval in P0.4+)"
        );

        // Store request for debugging/inspection
        self.pending_requests
            .write()
            .await
            .insert(request.call_id.clone(), request);

        // Auto-approve all requests
        GateResolution::Approved { always: false }
    }

    async fn cancel_thread(&self, thread_id: brassclaw_engine::ThreadId) {
        // Remove any pending requests for this thread
        let mut pending = self.pending_requests.write().await;
        pending.retain(|_, req| req.thread_id != thread_id);

        tracing::debug!(
            thread_id = %thread_id,
            "Cancelled pending approval requests for thread"
        );
    }
}

/// Configuration for gate controller mode.
///
/// Allows switching between different gate controller behaviors
/// for testing and development.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GateMode {
    /// Auto-approve all requests (P0.3 default).
    #[default]
    AutoApprove,
    /// Cancel all requests (P0.2 behavior).
    Cancel,
    /// Real approval flow (future: P0.4+).
    #[allow(dead_code)]
    Real,
}

impl GateMode {
    /// Create a gate controller for this mode.
    pub fn create_controller(&self) -> Arc<dyn GateController> {
        match self {
            Self::AutoApprove => AutoApprovingGateController::arc(),
            Self::Cancel => brassclaw_engine::gate::CancellingGateController::arc(),
            Self::Real => {
                // Future: implement real approval controller
                tracing::warn!(
                    "Real approval mode not yet implemented, falling back to auto-approve"
                );
                AutoApprovingGateController::arc()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::gate::ResumeKind;
    use brassclaw_engine::{ConversationId, ThreadId};

    #[tokio::test]
    async fn test_auto_approve_controller() {
        let controller = AutoApprovingGateController::new();

        let request = GatePauseRequest {
            thread_id: ThreadId::new(),
            user_id: "test_user".to_string(),
            gate_name: "approval".to_string(),
            action_name: "shell".to_string(),
            call_id: "call_123".to_string(),
            parameters: serde_json::json!({"command": "ls"}),
            resume_kind: ResumeKind::Approval { allow_always: true },
            conversation_id: Some(ConversationId(uuid::Uuid::new_v4())),
        };

        let resolution = controller.pause(request).await;

        assert!(matches!(
            resolution,
            GateResolution::Approved { always: false }
        ));

        assert_eq!(controller.pending_count().await, 1);
    }

    #[tokio::test]
    async fn test_cancel_thread() {
        let controller = AutoApprovingGateController::new();
        let thread_id = ThreadId::new();

        let request = GatePauseRequest {
            thread_id,
            user_id: "test_user".to_string(),
            gate_name: "approval".to_string(),
            action_name: "shell".to_string(),
            call_id: "call_123".to_string(),
            parameters: serde_json::json!({"command": "ls"}),
            resume_kind: ResumeKind::Approval { allow_always: true },
            conversation_id: None,
        };

        controller.pause(request).await;
        assert_eq!(controller.pending_count().await, 1);

        controller.cancel_thread(thread_id).await;
        assert_eq!(controller.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_gate_mode_auto_approve() {
        let mode = GateMode::AutoApprove;
        let controller = mode.create_controller();

        let request = GatePauseRequest {
            thread_id: ThreadId::new(),
            user_id: "test_user".to_string(),
            gate_name: "approval".to_string(),
            action_name: "shell".to_string(),
            call_id: "call_123".to_string(),
            parameters: serde_json::json!({"command": "ls"}),
            resume_kind: ResumeKind::Approval { allow_always: true },
            conversation_id: None,
        };

        let resolution = controller.pause(request).await;
        assert!(matches!(
            resolution,
            GateResolution::Approved { always: false }
        ));
    }

    #[tokio::test]
    async fn test_gate_mode_cancel() {
        let mode = GateMode::Cancel;
        let controller = mode.create_controller();

        let request = GatePauseRequest {
            thread_id: ThreadId::new(),
            user_id: "test_user".to_string(),
            gate_name: "approval".to_string(),
            action_name: "shell".to_string(),
            call_id: "call_123".to_string(),
            parameters: serde_json::json!({"command": "ls"}),
            resume_kind: ResumeKind::Approval { allow_always: true },
            conversation_id: None,
        };

        let resolution = controller.pause(request).await;
        assert!(matches!(resolution, GateResolution::Cancelled));
    }
}

// Made with Bob
