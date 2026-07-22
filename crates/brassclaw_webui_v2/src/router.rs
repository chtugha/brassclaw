//! Convenience constructor for an axum [`Router`] wired to the
//! WebChat v2 handlers.
//!
//! Host composition is free to ignore this and mount each handler directly
//! against its own router; the descriptors in [`crate::descriptors`] are
//! the canonical contract. This module exists so handler-level tests can
//! drive the full route table without re-stating the path/method table.

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post, put};
use brassclaw_product_workflow::RebornServicesApi;

use crate::descriptors::{
    WEBUI_V2_PATTERN_ACTIVATE_EXTENSION, WEBUI_V2_PATTERN_CANCEL_RUN,
    WEBUI_V2_PATTERN_CHAT_PREFERENCE, WEBUI_V2_PATTERN_COMPLETE_NEARAI_WALLET_LOGIN,
    WEBUI_V2_PATTERN_CREATE_THREAD, WEBUI_V2_PATTERN_DELETE_COMPONENT,
    WEBUI_V2_PATTERN_DELETE_LLM_PROVIDER, WEBUI_V2_PATTERN_DELETE_THREAD,
    WEBUI_V2_PATTERN_GET_COMPONENT_AUDIT_STATUS, WEBUI_V2_PATTERN_GET_INTERCEPTOR_CONFIG,
    WEBUI_V2_PATTERN_GET_LLM_CONFIG, WEBUI_V2_PATTERN_GET_RECIPE, WEBUI_V2_PATTERN_GET_TIMELINE,
    WEBUI_V2_PATTERN_GET_TOOL_SKILL, WEBUI_V2_PATTERN_INSTALL_EXTENSION,
    WEBUI_V2_PATTERN_INSTALL_SKILL, WEBUI_V2_PATTERN_LIST_AUTOMATIONS,
    WEBUI_V2_PATTERN_LIST_CONNECTABLE_CHANNELS, WEBUI_V2_PATTERN_LIST_EXTENSION_REGISTRY,
    WEBUI_V2_PATTERN_LIST_EXTENSIONS, WEBUI_V2_PATTERN_LIST_LLM_MODELS,
    WEBUI_V2_PATTERN_LIST_RECIPES, WEBUI_V2_PATTERN_LIST_SKILLS, WEBUI_V2_PATTERN_LIST_TOOL_SKILLS,
    WEBUI_V2_PATTERN_LIST_TOOLS, WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR,
    WEBUI_V2_PATTERN_RE_REVIEW_COMPONENT, WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR,
    WEBUI_V2_PATTERN_RECORD_RECIPE_OUTCOME, WEBUI_V2_PATTERN_REJECT_COMPONENT,
    WEBUI_V2_PATTERN_REJECT_RECIPE, WEBUI_V2_PATTERN_REJECT_TOOL_SKILL,
    WEBUI_V2_PATTERN_REMOVE_EXTENSION, WEBUI_V2_PATTERN_REMOVE_SKILL,
    WEBUI_V2_PATTERN_REQUEST_RECIPE_REVIEW, WEBUI_V2_PATTERN_REQUEST_TOOL_SKILL_REVIEW,
    WEBUI_V2_PATTERN_RESOLVE_GATE, WEBUI_V2_PATTERN_SEND_COMPONENT_TO_REVISION,
    WEBUI_V2_PATTERN_SEND_MESSAGE, WEBUI_V2_PATTERN_SET_ACTIVE_LLM,
    WEBUI_V2_PATTERN_SETTINGS_ACTIONS, WEBUI_V2_PATTERN_SETTINGS_EXTENSIONS,
    WEBUI_V2_PATTERN_SETTINGS_MONTY_VM, WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_RESTART,
    WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_STATUS, WEBUI_V2_PATTERN_SETTINGS_ORCHESTRATORS,
    WEBUI_V2_PATTERN_SETTINGS_SCAFFOLDS, WEBUI_V2_PATTERN_SETTINGS_SKILLS,
    WEBUI_V2_PATTERN_SETTINGS_TOOLS, WEBUI_V2_PATTERN_SETUP_EXTENSION,
    WEBUI_V2_PATTERN_START_CODEX_LOGIN, WEBUI_V2_PATTERN_START_NEARAI_LOGIN,
    WEBUI_V2_PATTERN_STREAM_EVENTS, WEBUI_V2_PATTERN_STREAM_EVENTS_WS,
    WEBUI_V2_PATTERN_TEST_LLM_CONNECTION, WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION,
    WEBUI_V2_PATTERN_VALIDATE_COMPONENT, WEBUI_V2_PATTERN_VALIDATE_RECIPE,
    WEBUI_V2_PATTERN_VALIDATE_TOOL_SKILL, WEBUI_V2_PATTERN_VALIDATION_QUEUE,
    WEBUI_V2_PATTERN_VALIDATION_QUEUE_COUNT,
};
use crate::handlers;
use crate::sse_capacity::{DEFAULT_SSE_MAX_CONCURRENT_PER_CALLER, SseCapacity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebUiV2RouteOptions {
    pub mount_llm_config_routes: bool,
}

impl WebUiV2RouteOptions {
    pub const fn all() -> Self {
        Self {
            mount_llm_config_routes: true,
        }
    }

    pub const fn without_llm_config_routes() -> Self {
        Self {
            mount_llm_config_routes: false,
        }
    }
}

/// Shared state injected into every WebChat v2 handler.
///
/// Handlers receive a single facade so they can never reach into the
/// dispatcher, run-state, or any runtime lane directly. The state also
/// owns the [`SseCapacity`] gate that bounds concurrent SSE streams per
/// `(tenant, user)`; cloning the state shares the same gate so all
/// handler invocations enforce one cap process-wide.
#[derive(Clone)]
pub struct WebUiV2State {
    services: Arc<dyn RebornServicesApi>,
    sse_capacity: Arc<SseCapacity>,
}

impl WebUiV2State {
    /// Build state with the default per-caller SSE concurrency cap
    /// ([`DEFAULT_SSE_MAX_CONCURRENT_PER_CALLER`]).
    pub fn new(services: Arc<dyn RebornServicesApi>) -> Self {
        Self::with_sse_concurrency_limit(services, DEFAULT_SSE_MAX_CONCURRENT_PER_CALLER)
    }

    /// Build state with a custom per-caller SSE concurrency cap. Use
    /// from host composition or tests that want to tune the ceiling.
    pub fn with_sse_concurrency_limit(
        services: Arc<dyn RebornServicesApi>,
        max_concurrent_streams_per_caller: usize,
    ) -> Self {
        Self {
            services,
            sse_capacity: Arc::new(SseCapacity::new(max_concurrent_streams_per_caller)),
        }
    }

    pub fn services(&self) -> &Arc<dyn RebornServicesApi> {
        &self.services
    }

    pub(crate) fn sse_capacity(&self) -> &Arc<SseCapacity> {
        &self.sse_capacity
    }
}

/// Build a [`Router`] mounting the WebChat v2 routes against the supplied
/// facade. Path patterns match
/// [`crate::descriptors::webui_v2_routes`] exactly; host composition is
/// expected to apply its own auth / CORS / body-limit middleware in front
/// of this router.
pub fn webui_v2_router(state: WebUiV2State) -> Router {
    webui_v2_router_with_options(state, WebUiV2RouteOptions::all())
}

pub fn webui_v2_router_with_options(state: WebUiV2State, options: WebUiV2RouteOptions) -> Router {
    let mut router = Router::new()
        // GET and POST share the `/api/webchat/v2/threads` path
        // (`WEBUI_V2_PATTERN_CREATE_THREAD == WEBUI_V2_PATTERN_LIST_THREADS`);
        // mount both verbs in one `.route()` so axum's matcher
        // dispatches by method.
        .route(
            WEBUI_V2_PATTERN_CREATE_THREAD,
            post(handlers::create_thread).get(handlers::list_threads),
        )
        .route(
            WEBUI_V2_PATTERN_DELETE_THREAD,
            delete(handlers::delete_thread),
        )
        .route(WEBUI_V2_PATTERN_SEND_MESSAGE, post(handlers::send_message))
        .route(WEBUI_V2_PATTERN_GET_TIMELINE, get(handlers::get_timeline))
        .route(WEBUI_V2_PATTERN_STREAM_EVENTS, get(handlers::stream_events))
        .route(
            WEBUI_V2_PATTERN_STREAM_EVENTS_WS,
            get(handlers::stream_events_ws),
        )
        .route(WEBUI_V2_PATTERN_CANCEL_RUN, post(handlers::cancel_run))
        .route(WEBUI_V2_PATTERN_RESOLVE_GATE, post(handlers::resolve_gate))
        .route(
            WEBUI_V2_PATTERN_LIST_AUTOMATIONS,
            get(handlers::list_automations),
        )
        .route(
            WEBUI_V2_PATTERN_LIST_CONNECTABLE_CHANNELS,
            get(handlers::list_connectable_channels),
        )
        .route(
            WEBUI_V2_PATTERN_LIST_EXTENSIONS,
            get(handlers::list_extensions),
        )
        .route(
            WEBUI_V2_PATTERN_LIST_EXTENSION_REGISTRY,
            get(handlers::list_extension_registry),
        )
        .route(
            WEBUI_V2_PATTERN_INSTALL_EXTENSION,
            post(handlers::install_extension),
        )
        .route(
            WEBUI_V2_PATTERN_ACTIVATE_EXTENSION,
            post(handlers::activate_extension),
        )
        .route(
            WEBUI_V2_PATTERN_REMOVE_EXTENSION,
            post(handlers::remove_extension),
        )
        .route(
            WEBUI_V2_PATTERN_SETUP_EXTENSION,
            get(handlers::get_extension_setup).post(handlers::setup_extension),
        )
        .route(WEBUI_V2_PATTERN_LIST_TOOLS, get(handlers::list_tools))
        .route(
            WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION,
            put(handlers::update_tool_permission),
        )
        .route(WEBUI_V2_PATTERN_LIST_SKILLS, get(handlers::list_skills))
        .route(
            WEBUI_V2_PATTERN_INSTALL_SKILL,
            post(handlers::install_skill),
        )
        .route(
            WEBUI_V2_PATTERN_REMOVE_SKILL,
            delete(handlers::remove_skill),
        )
        // Phase 7 — Recipe-Skill-Tool library surface. The listing
        // endpoints catalogue what the agent has already learned;
        // per-id GETs back the Recipe Manager detail pane; the
        // validation queue + status transition routes drive the
        // post-extraction review tab; outcomes feed the engine
        // `MetricRecorder` for Wilson + tier math.
        .route(WEBUI_V2_PATTERN_LIST_RECIPES, get(handlers::list_recipes))
        .route(
            WEBUI_V2_PATTERN_LIST_TOOL_SKILLS,
            get(handlers::list_tool_skills),
        )
        .route(WEBUI_V2_PATTERN_GET_RECIPE, get(handlers::get_recipe))
        .route(
            WEBUI_V2_PATTERN_GET_TOOL_SKILL,
            get(handlers::get_tool_skill),
        )
        .route(
            WEBUI_V2_PATTERN_VALIDATION_QUEUE,
            get(handlers::list_validation_queue),
        )
        .route(
            WEBUI_V2_PATTERN_VALIDATION_QUEUE_COUNT,
            get(handlers::count_validation_queue),
        )
        .route(
            WEBUI_V2_PATTERN_VALIDATE_RECIPE,
            put(handlers::validate_recipe),
        )
        .route(WEBUI_V2_PATTERN_REJECT_RECIPE, put(handlers::reject_recipe))
        .route(
            WEBUI_V2_PATTERN_REQUEST_RECIPE_REVIEW,
            put(handlers::request_recipe_review),
        )
        .route(
            WEBUI_V2_PATTERN_VALIDATE_TOOL_SKILL,
            put(handlers::validate_tool_skill),
        )
        .route(
            WEBUI_V2_PATTERN_REJECT_TOOL_SKILL,
            put(handlers::reject_tool_skill),
        )
        .route(
            WEBUI_V2_PATTERN_REQUEST_TOOL_SKILL_REVIEW,
            put(handlers::request_tool_skill_review),
        )
        .route(
            WEBUI_V2_PATTERN_RECORD_RECIPE_OUTCOME,
            post(handlers::record_recipe_outcome),
        )
        // Phase 3 (Step 3.5) — Generalized component validation routes.
        // Old recipe/tool_skill-specific routes are kept as aliases (removed in Phase 7).
        .route(
            WEBUI_V2_PATTERN_VALIDATE_COMPONENT,
            put(handlers::validate_component),
        )
        .route(
            WEBUI_V2_PATTERN_REJECT_COMPONENT,
            put(handlers::reject_component),
        )
        .route(
            WEBUI_V2_PATTERN_SEND_COMPONENT_TO_REVISION,
            put(handlers::send_component_to_revision),
        )
        .route(
            WEBUI_V2_PATTERN_RE_REVIEW_COMPONENT,
            put(handlers::re_review_component),
        )
        .route(
            WEBUI_V2_PATTERN_DELETE_COMPONENT,
            delete(handlers::delete_component),
        )
        .route(
            WEBUI_V2_PATTERN_GET_COMPONENT_AUDIT_STATUS,
            get(handlers::get_component_audit_status),
        )
        // Safety configuration endpoints
        .route(
            "/api/webchat/v2/safety/sensitive-paths",
            get(handlers::safety::get_sensitive_paths)
                .put(handlers::safety::update_sensitive_paths),
        )
        .route(
            "/api/webchat/v2/safety/workspace-rules",
            get(handlers::safety::get_workspace_rules)
                .put(handlers::safety::update_workspace_rules),
        )
        .route(
            "/api/webchat/v2/safety/blocked-paths",
            get(handlers::safety::get_blocked_paths).put(handlers::safety::update_blocked_paths),
        )
        // Per-provider token settings endpoint
        .route(
            "/api/webchat/v2/providers/{provider_id}/tokens",
            get(handlers::tokens::get_provider_token_settings)
                .put(handlers::tokens::update_provider_token_settings),
        )
        // Reduction-rule endpoints — once-per-user/per-project storage
        // backing `__get_reduction_rules__()` in the orchestrator. Mutating
        // routes invalidate the engine cache via the composer-wired hook
        // on `RebornServices`; the GET path is read-only and hits the
        // store every time so a fresh operator-authored rule surfaces in
        // the WebUI on the next refresh without a server restart.
        // 400-by-contract: a caller without an authenticated `project_id`
        // is rejected at the handler boundary, so the orchestrator cache
        // never sees rules filed under an empty bucket.
        .route(
            "/api/webchat/v2/tokens/reduction-rules",
            get(handlers::reduction_rules::list_reduction_rules)
                .put(handlers::reduction_rules::replace_reduction_rules),
        )
        .route(
            "/api/webchat/v2/tokens/reduction-rules/author",
            post(handlers::reduction_rules::author_reduction_rule),
        )
        // Phase 5.5 — Interceptor configuration routes.
        // GET and POST share `/api/webchat/v2/interceptor/config`.
        .route(
            WEBUI_V2_PATTERN_GET_INTERCEPTOR_CONFIG,
            get(handlers::get_interceptor_config).post(handlers::update_interceptor_config),
        )
        .route(
            WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR,
            post(handlers::reassemble_interceptor),
        )
        .route(
            WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR,
            post(handlers::prewarm_interceptor),
        )
        // Phase 6 — Settings UI routes (10-tab editor).
        // Note: /api/settings/monty-vm/restart and /api/settings/monty-vm/status
        // must be mounted BEFORE /api/settings/monty-vm so axum's router resolves
        // the more specific paths first.
        .route(
            WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_RESTART,
            post(handlers::post_settings_monty_vm_restart),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_STATUS,
            get(handlers::get_settings_monty_vm_status),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_SKILLS,
            get(handlers::get_settings_skills),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_TOOLS,
            get(handlers::get_settings_tools),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_EXTENSIONS,
            get(handlers::get_settings_extensions),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_ACTIONS,
            get(handlers::get_settings_actions),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_ORCHESTRATORS,
            get(handlers::get_settings_orchestrators),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_SCAFFOLDS,
            get(handlers::get_settings_scaffolds),
        )
        .route(
            WEBUI_V2_PATTERN_SETTINGS_MONTY_VM,
            get(handlers::get_settings_monty_vm).put(handlers::put_settings_monty_vm),
        )
        .route(
            WEBUI_V2_PATTERN_CHAT_PREFERENCE,
            put(handlers::put_chat_preference),
        );
    if options.mount_llm_config_routes {
        router = router
            // `WEBUI_V2_PATTERN_GET_LLM_CONFIG == WEBUI_V2_PATTERN_UPSERT_LLM_PROVIDER`
            // (`/llm/providers`); mount GET + POST in one `.route()`.
            .route(
                WEBUI_V2_PATTERN_GET_LLM_CONFIG,
                get(handlers::get_llm_config).post(handlers::upsert_llm_provider),
            )
            .route(
                WEBUI_V2_PATTERN_DELETE_LLM_PROVIDER,
                post(handlers::delete_llm_provider),
            )
            .route(
                WEBUI_V2_PATTERN_SET_ACTIVE_LLM,
                post(handlers::set_active_llm),
            )
            .route(
                WEBUI_V2_PATTERN_TEST_LLM_CONNECTION,
                post(handlers::test_llm_connection),
            )
            .route(
                WEBUI_V2_PATTERN_LIST_LLM_MODELS,
                post(handlers::list_llm_models),
            )
            .route(
                WEBUI_V2_PATTERN_START_NEARAI_LOGIN,
                post(handlers::start_nearai_login),
            )
            .route(
                WEBUI_V2_PATTERN_COMPLETE_NEARAI_WALLET_LOGIN,
                post(handlers::complete_nearai_wallet_login),
            )
            .route(
                WEBUI_V2_PATTERN_START_CODEX_LOGIN,
                post(handlers::start_codex_login),
            );
    }
    router.with_state(state)
}
