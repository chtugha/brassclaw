//! Engine v2 bridge — connects `brassclaw_engine` to existing infrastructure.
//!
//! Strategy C: parallel deployment. When `ENGINE_V2=true`, user messages
//! route through the engine instead of the existing agentic loop. All
//! existing behavior is unchanged when the flag is off.

mod action_discovery;
// mod action_projector; // V1 - deleted
// mod capability_projector; // V1 - deleted
mod cost_guard_gate;
// mod effect_adapter; // V1 - deleted
mod effect_adapter_v2;
mod engine_actions;
// mod external_tools; // V1 - deleted
// mod gate_controller; // V1 - deleted
mod llm_adapter;
// mod router; // V1 - deleted
pub mod sandbox;
pub mod skill_migration;
mod store_adapter;
// mod tool_surface; // V1 - deleted
mod user_facing_errors;
mod workspace_reader;

pub use cost_guard_gate::CostGuardBudgetGate;
// pub use external_tools::{ // V1 - deleted
//     EXTERNAL_TOOL_CALLBACK_PREFIX, ExternalToolCatalog, ExternalToolEntry,
//     call_id_from_external_callback, external_tool_callback_id, is_external_tool_callback_id,
// };
pub use workspace_reader::WorkspaceReaderAdapter;

// pub use effect_adapter::EffectBridgeAdapter; // V1 - deleted
pub use effect_adapter_v2::EffectBridgeAdapter as EffectBridgeAdapterV2; // V2 - Reborn
pub use effect_adapter_v2::EffectBridgeAdapter; // V2 - default export

// Re-export engine types needed by consumers
pub use brassclaw_engine::{EffectExecutor, ThreadExecutionContext};
// pub use gate_controller::{BridgeGateController, GateResolutions, PerExecutionContext}; // V1 - deleted
// pub use router::{ // V1 - deleted - all router exports commented out
//     // DTO types
//     AttentionItem,
//     AuthCallbackContinuation,
//     // Typed outcome from v2 bridge handlers
//     BridgeOutcome,
//     EngineMissionDetail,
//     EngineMissionInfo,
//     EngineProjectInfo,
//     EngineStepInfo,
//     EngineThreadDetail,
//     EngineThreadInfo,
//     InlineGateError,
//     InlineGateOutcome,
//     ProjectOverviewEntry,
//     ProjectsOverviewResponse,
//     clear_engine_pending_auth,
//     clear_engine_pending_auth_for_credential,
//     discard_engine_pending_auth_request,
//     // Engine internal action names — used by request validators to
//     // reject caller-supplied tool names that would shadow internal
//     // capability actions (mission_*, skill_*, memory_*, etc.).
//     engine_capability_action_names,
//     // External tool catalog accessor (Responses API)
//     engine_external_tool_catalog,
//     // Query functions
//     fire_engine_mission,
//     get_engine_mission,
//     get_engine_pending_gate,
//     get_engine_project,
//     get_engine_projects_overview,
//     get_engine_thread,
//     get_pending_gate_by_request_id,
//     // Action handlers
//     handle_approval,
//     handle_auth_gate_resolution,
//     handle_clear,
//     handle_exec_approval,
//     handle_expected,
//     handle_external_callback,
//     handle_interrupt,
//     handle_new_thread,
//     handle_pairing_claim,
//     handle_with_engine,
//     has_any_pending_gate,
//     has_pending_auth,
//     // Initialization
//     init_engine,
//     is_engine_v2_enabled,
//     list_engine_missions,
//     list_engine_projects,
//     list_engine_thread_events,
//     list_engine_thread_steps,
//     list_engine_threads,
//     pause_engine_mission,
//     resolve_engine_auth_callback,
//     resolve_gate,
//     resolve_inline_gates_for_credential,
//     resume_engine_mission,
//     resume_paused_missions_for_credential,
//     resume_paused_missions_for_gate_request,
//     transition_engine_pending_auth_request_to_pairing,
//     try_resolve_inline_approval_gate,
// };

// #[cfg(feature = "libsql")]
// pub use router::reset_engine_state; // V1 - deleted

// // `engine_retrospectives_for_test` is a test-only reachability surface —
// // integration tests live in a separate crate, so `#[cfg(test)]` wouldn't
// // expose it. `#[doc(hidden)]` keeps it out of public docs and signals
// // that it is not a supported API.
// #[cfg(feature = "libsql")]
// #[doc(hidden)]
// pub use router::engine_retrospectives_for_test; // V1 - deleted

// #[cfg(feature = "libsql")]
// #[doc(hidden)]
// pub use router::override_engine_project_root_for_test; // V1 - deleted

// // Exposed for caller-level testing of the cross-user thread_id guard
// #[cfg(test)]
// pub(crate) use router::handle_mission_notification; // V1 - deleted

// #[cfg(test)]
// pub(crate) use router::test_support; // V1 - deleted
