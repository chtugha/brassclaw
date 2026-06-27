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
