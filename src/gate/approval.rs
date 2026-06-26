//! Approval gate — wraps `Tool::requires_approval()`.
//!
//! Replaces the inline approval check in `EffectBridgeAdapter::execute_action()`
//! (steps 1) with a composable gate that handles interactive, autonomous, and
//! container execution modes.

use async_trait::async_trait;
use brassclaw_engine::gate::{ExecutionGate, GateContext, GateDecision};

/// The three approval requirement levels used by V2's gate pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequirement {
    Never,
    UnlessAutoApproved,
    Always,
}

/// Gate that checks `AuthManager::check_action_auth()` for missing credentials.
///
/// Priority: 200 (after approval — no point checking credentials for a denied tool).
///
/// Currently a pass-through — the actual auth check remains inline in
/// `effect_adapter.rs` step 1.7 until Phase 4 migration completes.
pub struct AuthenticationGate;

#[async_trait]
impl ExecutionGate for AuthenticationGate {
    fn name(&self) -> &str {
        "authentication"
    }

    fn priority(&self) -> u32 {
        200
    }

    async fn evaluate(&self, _ctx: &GateContext<'_>) -> GateDecision {
        // The actual auth check is performed via the EffectBridgeAdapter's
        // auth_manager — this gate delegates there during Phase 4 migration.
        // For now, the inline check in effect_adapter.rs step 1.7 remains.
        GateDecision::Allow
    }
}

/// Gate that auto-denies approval-requiring tools on relay channels.
///
/// Fixes v1/v2 inconsistency where relay channels auto-deny was only
/// in v1 dispatcher but not in v2 router.
///
/// Priority: 80 (before approval — no point showing approval UI on channels
/// that can't respond interactively).
pub struct RelayChannelGate;

#[async_trait]
impl ExecutionGate for RelayChannelGate {
    fn name(&self) -> &str {
        "relay_channel"
    }

    fn priority(&self) -> u32 {
        80
    }

    async fn evaluate(&self, ctx: &GateContext<'_>) -> GateDecision {
        let is_relay = ctx.source_channel.ends_with("-relay");
        if !is_relay {
            return GateDecision::Allow;
        }

        if ctx.action_def.requires_approval {
            GateDecision::Deny {
                reason: format!(
                    "Tool '{}' requires approval but relay channel '{}' cannot provide interactive response.",
                    ctx.action_name, ctx.source_channel
                ),
            }
        } else {
            GateDecision::Allow
        }
    }
}

// V1 tests disabled - depend on deleted tools module
#[cfg(all(test, disabled))]
mod tests {
    use super::*;
    use crate::context::JobContext;
    use crate::tools::{Tool, ToolError, ToolOutput};
    use brassclaw_engine::gate::ExecutionMode;
    use brassclaw_engine::types::capability::{ActionDef, EffectType, ModelToolSurface};
    use brassclaw_engine::types::thread::ThreadId;
    use std::collections::HashSet;
    use std::time::Duration;

    struct ApprovalTestTool {
        name: &'static str,
        requirement: ApprovalRequirement,
    }

    #[async_trait]
    impl Tool for ApprovalTestTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "approval test tool"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &JobContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                serde_json::json!({ "ok": true }),
                Duration::from_millis(1),
            ))
        }

        fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
            self.requirement
        }
    }

    fn action_def(name: &str, requires_approval: bool) -> ActionDef {
        ActionDef {
            name: name.into(),
            description: String::new(),
            parameters_schema: serde_json::json!({}),
            effects: vec![EffectType::ReadLocal],
            requires_approval,
            model_tool_surface: ModelToolSurface::FullSchema,
            discovery: None,
        }
    }

    fn ctx<'a>(
        action_def: &'a ActionDef,
        mode: ExecutionMode,
        channel: &'a str,
        auto_approved: &'a HashSet<String>,
        params: &'a serde_json::Value,
    ) -> GateContext<'a> {
        GateContext {
            user_id: "user1",
            thread_id: ThreadId::new(),
            source_channel: channel,
            action_name: &action_def.name,
            call_id: "call_1",
            parameters: params,
            action_def,
            execution_mode: mode,
            auto_approved,
        }
    }

    async fn approval_gate_with_tool(
        name: &'static str,
        requirement: ApprovalRequirement,
    ) -> ApprovalGate {
        let registry = Arc::new(ToolRegistry::new());
        registry
            .register(Arc::new(ApprovalTestTool { name, requirement }))
            .await;
        ApprovalGate::new(registry)
    }

    // ── InteractiveAutoApprove mode ─────────────────────────

    #[tokio::test]
    async fn test_auto_approve_allows_unless_auto_approved_tools() {
        let gate = RelayChannelGate;
        // This test uses RelayChannelGate only to get a gate instance —
        // the actual auto-approve logic is in ApprovalGate which needs
        // a ToolRegistry. Test the mode semantics directly via GateContext.
        let ad = action_def("shell", false); // UnlessAutoApproved mapped here
        let auto = HashSet::new();
        let params = serde_json::json!({});
        let c = ctx(
            &ad,
            ExecutionMode::InteractiveAutoApprove,
            "web",
            &auto,
            &params,
        );
        // RelayChannelGate doesn't care about mode — it only checks channel suffix
        assert!(matches!(gate.evaluate(&c).await, GateDecision::Allow)); // safety: test-only
    }

    #[tokio::test]
    async fn approval_gate_auto_approved_unless_allows() {
        let gate =
            approval_gate_with_tool("test_tool", ApprovalRequirement::UnlessAutoApproved).await;
        let ad = action_def("test_tool", true);
        let auto = HashSet::from(["test_tool".to_string()]);
        let params = serde_json::json!({});
        let c = ctx(&ad, ExecutionMode::Interactive, "web", &auto, &params);

        assert!(matches!(gate.evaluate(&c).await, GateDecision::Allow));
    }

    #[tokio::test]
    async fn approval_gate_auto_approved_always_pauses_interactive() {
        let gate = approval_gate_with_tool("test_tool", ApprovalRequirement::Always).await;
        let ad = action_def("test_tool", true);
        let auto = HashSet::from(["test_tool".to_string()]);
        let params = serde_json::json!({});
        let c = ctx(&ad, ExecutionMode::Interactive, "web", &auto, &params);

        assert!(matches!(
            gate.evaluate(&c).await,
            GateDecision::Pause {
                resume_kind: ResumeKind::Approval {
                    allow_always: false
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn approval_gate_auto_approved_always_pauses_interactive_auto_approve() {
        let gate = approval_gate_with_tool("test_tool", ApprovalRequirement::Always).await;
        let ad = action_def("test_tool", true);
        let auto = HashSet::from(["test_tool".to_string()]);
        let params = serde_json::json!({});
        let c = ctx(
            &ad,
            ExecutionMode::InteractiveAutoApprove,
            "web",
            &auto,
            &params,
        );

        assert!(matches!(
            gate.evaluate(&c).await,
            GateDecision::Pause {
                resume_kind: ResumeKind::Approval {
                    allow_always: false
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn approval_gate_auto_approved_always_denies_autonomous() {
        let gate = approval_gate_with_tool("test_tool", ApprovalRequirement::Always).await;
        let ad = action_def("test_tool", true);
        let auto = HashSet::from(["test_tool".to_string()]);
        let params = serde_json::json!({});
        let c = ctx(&ad, ExecutionMode::Autonomous, "web", &auto, &params);

        assert!(matches!(gate.evaluate(&c).await, GateDecision::Deny { .. }));
    }

    // ── RelayChannelGate ─────────────────────────────────────

    #[tokio::test]
    async fn test_relay_channel_denies_approval_requiring_tools() {
        let gate = RelayChannelGate;
        let ad = action_def("shell", true);
        let auto = HashSet::new();
        let params = serde_json::json!({});
        let c = ctx(
            &ad,
            ExecutionMode::Interactive,
            "slack-relay",
            &auto,
            &params,
        );
        assert!(matches!(gate.evaluate(&c).await, GateDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn test_non_relay_channel_always_allows() {
        let gate = RelayChannelGate;
        let ad = action_def("shell", true);
        let auto = HashSet::new();
        let params = serde_json::json!({});
        let c = ctx(&ad, ExecutionMode::Interactive, "telegram", &auto, &params);
        assert!(matches!(gate.evaluate(&c).await, GateDecision::Allow));
    }

    #[tokio::test]
    async fn test_relay_allows_non_approval_tools() {
        let gate = RelayChannelGate;
        let ad = action_def("echo", false);
        let auto = HashSet::new();
        let params = serde_json::json!({});
        let c = ctx(
            &ad,
            ExecutionMode::Interactive,
            "slack-relay",
            &auto,
            &params,
        );
        assert!(matches!(gate.evaluate(&c).await, GateDecision::Allow));
    }
}
