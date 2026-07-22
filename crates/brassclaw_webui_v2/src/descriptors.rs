//! Host-owned route descriptors for the Reborn WebChat v2 surface.
//!
//! Host composition consumes [`webui_v2_routes`] and mounts the matching
//! handler from [`crate::handlers`] under each descriptor's pattern. The
//! descriptor is the contract: changing a route's policy here changes what
//! host composition enforces before the handler runs.

use brassclaw_host_api::ingress::{
    AllowedEffectPath, AuditTraceClass, BodyLimitPolicy, CorsPolicy, IngressAuthPolicy,
    IngressAuthScheme, IngressPolicy, IngressPolicyParts, IngressRouteDescriptor, ListenerClass,
    RateLimitPolicy, RateLimitScope, StreamingMode, WebSocketOriginPolicy,
};
use brassclaw_host_api::{IngressScopeSource, NetworkMethod};
use std::num::{NonZeroU32, NonZeroU64};

pub const WEBUI_V2_ROUTE_CREATE_THREAD: &str = "webui.v2.create_thread";
pub const WEBUI_V2_ROUTE_DELETE_THREAD: &str = "webui.v2.delete_thread";
pub const WEBUI_V2_ROUTE_SEND_MESSAGE: &str = "webui.v2.send_message";
pub const WEBUI_V2_ROUTE_LIST_THREADS: &str = "webui.v2.list_threads";
pub const WEBUI_V2_ROUTE_GET_TIMELINE: &str = "webui.v2.get_timeline";
pub const WEBUI_V2_ROUTE_STREAM_EVENTS: &str = "webui.v2.stream_events";
pub const WEBUI_V2_ROUTE_STREAM_EVENTS_WS: &str = "webui.v2.stream_events_ws";
pub const WEBUI_V2_ROUTE_CANCEL_RUN: &str = "webui.v2.cancel_run";
pub const WEBUI_V2_ROUTE_RESOLVE_GATE: &str = "webui.v2.resolve_gate";
pub const WEBUI_V2_ROUTE_LIST_AUTOMATIONS: &str = "webui.v2.list_automations";
pub const WEBUI_V2_ROUTE_LIST_CONNECTABLE_CHANNELS: &str = "webui.v2.list_connectable_channels";
pub const WEBUI_V2_ROUTE_LIST_EXTENSIONS: &str = "webui.v2.list_extensions";
pub const WEBUI_V2_ROUTE_LIST_EXTENSION_REGISTRY: &str = "webui.v2.list_extension_registry";
pub const WEBUI_V2_ROUTE_INSTALL_EXTENSION: &str = "webui.v2.install_extension";
pub const WEBUI_V2_ROUTE_ACTIVATE_EXTENSION: &str = "webui.v2.activate_extension";
pub const WEBUI_V2_ROUTE_REMOVE_EXTENSION: &str = "webui.v2.remove_extension";
pub const WEBUI_V2_ROUTE_GET_EXTENSION_SETUP: &str = "webui.v2.get_extension_setup";
pub const WEBUI_V2_ROUTE_SETUP_EXTENSION: &str = "webui.v2.setup_extension";
pub const WEBUI_V2_ROUTE_GET_LLM_CONFIG: &str = "webui.v2.get_llm_config";
pub const WEBUI_V2_ROUTE_UPSERT_LLM_PROVIDER: &str = "webui.v2.upsert_llm_provider";
pub const WEBUI_V2_ROUTE_DELETE_LLM_PROVIDER: &str = "webui.v2.delete_llm_provider";
pub const WEBUI_V2_ROUTE_SET_ACTIVE_LLM: &str = "webui.v2.set_active_llm";
pub const WEBUI_V2_ROUTE_TEST_LLM_CONNECTION: &str = "webui.v2.test_llm_connection";
pub const WEBUI_V2_ROUTE_LIST_LLM_MODELS: &str = "webui.v2.list_llm_models";
pub const WEBUI_V2_ROUTE_START_NEARAI_LOGIN: &str = "webui.v2.start_nearai_login";
pub const WEBUI_V2_ROUTE_COMPLETE_NEARAI_WALLET_LOGIN: &str =
    "webui.v2.complete_nearai_wallet_login";
pub const WEBUI_V2_ROUTE_START_CODEX_LOGIN: &str = "webui.v2.start_codex_login";
pub const WEBUI_V2_ROUTE_LIST_TOOLS: &str = "webui.v2.list_tools";
pub const WEBUI_V2_ROUTE_UPDATE_TOOL_PERMISSION: &str = "webui.v2.update_tool_permission";
pub const WEBUI_V2_ROUTE_LIST_SKILLS: &str = "webui.v2.list_skills";
pub const WEBUI_V2_ROUTE_INSTALL_SKILL: &str = "webui.v2.install_skill";
pub const WEBUI_V2_ROUTE_REMOVE_SKILL: &str = "webui.v2.remove_skill";

// Phase 7 — Recipe-Skill-Tool library. Each mutating route is a
// state-machine transition on a `MemoryDoc.metadata.validation_status`
// field; review endpoints that fail the agent's structured JSON
// extraction fall through to the LLM review mission registered at
// runtime startup.
pub const WEBUI_V2_ROUTE_LIST_RECIPES: &str = "webui.v2.list_recipes";
pub const WEBUI_V2_ROUTE_LIST_TOOL_SKILLS: &str = "webui.v2.list_tool_skills";
pub const WEBUI_V2_ROUTE_GET_RECIPE: &str = "webui.v2.get_recipe";
pub const WEBUI_V2_ROUTE_GET_TOOL_SKILL: &str = "webui.v2.get_tool_skill";
pub const WEBUI_V2_ROUTE_LIST_VALIDATION_QUEUE: &str = "webui.v2.list_validation_queue";
pub const WEBUI_V2_ROUTE_COUNT_VALIDATION_QUEUE: &str = "webui.v2.count_validation_queue";
pub const WEBUI_V2_ROUTE_VALIDATE_RECIPE: &str = "webui.v2.validate_recipe";
pub const WEBUI_V2_ROUTE_REJECT_RECIPE: &str = "webui.v2.reject_recipe";
pub const WEBUI_V2_ROUTE_REQUEST_RECIPE_REVIEW: &str = "webui.v2.request_recipe_review";
pub const WEBUI_V2_ROUTE_VALIDATE_TOOL_SKILL: &str = "webui.v2.validate_tool_skill";
pub const WEBUI_V2_ROUTE_REJECT_TOOL_SKILL: &str = "webui.v2.reject_tool_skill";
pub const WEBUI_V2_ROUTE_REQUEST_TOOL_SKILL_REVIEW: &str = "webui.v2.request_tool_skill_review";
pub const WEBUI_V2_ROUTE_RECORD_RECIPE_OUTCOME: &str = "webui.v2.record_recipe_outcome";

// Phase 3 (Step 3.5) — Generalized component validation routes for all ~20 class codes.
pub const WEBUI_V2_ROUTE_VALIDATE_COMPONENT: &str = "webui.v2.validate_component";
pub const WEBUI_V2_ROUTE_REJECT_COMPONENT: &str = "webui.v2.reject_component";
pub const WEBUI_V2_ROUTE_SEND_COMPONENT_TO_REVISION: &str = "webui.v2.send_component_to_revision";
pub const WEBUI_V2_ROUTE_RE_REVIEW_COMPONENT: &str = "webui.v2.re_review_component";
pub const WEBUI_V2_ROUTE_DELETE_COMPONENT: &str = "webui.v2.delete_component";
pub const WEBUI_V2_ROUTE_GET_COMPONENT_AUDIT_STATUS: &str = "webui.v2.get_component_audit_status";

// Phase 5.5 — Interceptor configuration routes.
pub const WEBUI_V2_ROUTE_GET_INTERCEPTOR_CONFIG: &str = "webui.v2.get_interceptor_config";
pub const WEBUI_V2_ROUTE_UPDATE_INTERCEPTOR_CONFIG: &str = "webui.v2.update_interceptor_config";
pub const WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR: &str = "webui.v2.reassemble_interceptor";
pub const WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR: &str = "webui.v2.prewarm_interceptor";

pub const WEBUI_V2_PATTERN_GET_INTERCEPTOR_CONFIG: &str = "/api/webchat/v2/interceptor/config";
pub const WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR: &str =
    "/api/webchat/v2/interceptor/reassemble";
pub const WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR: &str = "/api/webchat/v2/interceptor/prewarm";

// Phase 6 — Settings UI routes (10-tab editor).
pub const WEBUI_V2_ROUTE_GET_SETTINGS_SKILLS: &str = "webui.v2.get_settings_skills";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_TOOLS: &str = "webui.v2.get_settings_tools";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_EXTENSIONS: &str = "webui.v2.get_settings_extensions";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_ACTIONS: &str = "webui.v2.get_settings_actions";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_ORCHESTRATORS: &str = "webui.v2.get_settings_orchestrators";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_SCAFFOLDS: &str = "webui.v2.get_settings_scaffolds";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM: &str = "webui.v2.get_settings_monty_vm";
pub const WEBUI_V2_ROUTE_PUT_SETTINGS_MONTY_VM: &str = "webui.v2.put_settings_monty_vm";
pub const WEBUI_V2_ROUTE_POST_SETTINGS_MONTY_VM_RESTART: &str =
    "webui.v2.post_settings_monty_vm_restart";
pub const WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM_STATUS: &str =
    "webui.v2.get_settings_monty_vm_status";
pub const WEBUI_V2_ROUTE_PUT_CHAT_PREFERENCE: &str = "webui.v2.put_chat_preference";

pub const WEBUI_V2_PATTERN_SETTINGS_SKILLS: &str = "/api/settings/skills";
pub const WEBUI_V2_PATTERN_SETTINGS_TOOLS: &str = "/api/settings/tools";
pub const WEBUI_V2_PATTERN_SETTINGS_EXTENSIONS: &str = "/api/settings/extensions";
pub const WEBUI_V2_PATTERN_SETTINGS_ACTIONS: &str = "/api/settings/actions";
pub const WEBUI_V2_PATTERN_SETTINGS_ORCHESTRATORS: &str = "/api/settings/orchestrators";
pub const WEBUI_V2_PATTERN_SETTINGS_SCAFFOLDS: &str = "/api/settings/scaffolds";
pub const WEBUI_V2_PATTERN_SETTINGS_MONTY_VM: &str = "/api/settings/monty-vm";
pub const WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_RESTART: &str = "/api/settings/monty-vm/restart";
pub const WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_STATUS: &str = "/api/settings/monty-vm/status";
pub const WEBUI_V2_PATTERN_CHAT_PREFERENCE: &str = "/api/chat/preferences/{key}";

pub const WEBUI_V2_PATTERN_CREATE_THREAD: &str = "/api/webchat/v2/threads";
pub const WEBUI_V2_PATTERN_LIST_THREADS: &str = "/api/webchat/v2/threads";
pub const WEBUI_V2_PATTERN_DELETE_THREAD: &str = "/api/webchat/v2/threads/{thread_id}";
pub const WEBUI_V2_PATTERN_SEND_MESSAGE: &str = "/api/webchat/v2/threads/{thread_id}/messages";
pub const WEBUI_V2_PATTERN_GET_TIMELINE: &str = "/api/webchat/v2/threads/{thread_id}/timeline";
pub const WEBUI_V2_PATTERN_STREAM_EVENTS: &str = "/api/webchat/v2/threads/{thread_id}/events";
pub const WEBUI_V2_PATTERN_STREAM_EVENTS_WS: &str = "/api/webchat/v2/threads/{thread_id}/ws";
pub const WEBUI_V2_PATTERN_CANCEL_RUN: &str =
    "/api/webchat/v2/threads/{thread_id}/runs/{run_id}/cancel";
pub const WEBUI_V2_PATTERN_RESOLVE_GATE: &str =
    "/api/webchat/v2/threads/{thread_id}/runs/{run_id}/gates/{gate_ref}/resolve";
pub const WEBUI_V2_PATTERN_LIST_AUTOMATIONS: &str = "/api/webchat/v2/automations";
pub const WEBUI_V2_PATTERN_LIST_CONNECTABLE_CHANNELS: &str = "/api/webchat/v2/channels/connectable";
pub const WEBUI_V2_PATTERN_LIST_EXTENSIONS: &str = "/api/webchat/v2/extensions";
pub const WEBUI_V2_PATTERN_LIST_EXTENSION_REGISTRY: &str = "/api/webchat/v2/extensions/registry";
pub const WEBUI_V2_PATTERN_INSTALL_EXTENSION: &str = "/api/webchat/v2/extensions/install";
pub const WEBUI_V2_PATTERN_ACTIVATE_EXTENSION: &str =
    "/api/webchat/v2/extensions/{package_id}/activate";
pub const WEBUI_V2_PATTERN_REMOVE_EXTENSION: &str =
    "/api/webchat/v2/extensions/{package_id}/remove";
pub const WEBUI_V2_PATTERN_SETUP_EXTENSION: &str = "/api/webchat/v2/extensions/{package_id}/setup";
pub const WEBUI_V2_PATTERN_GET_LLM_CONFIG: &str = "/api/webchat/v2/llm/providers";
pub const WEBUI_V2_PATTERN_UPSERT_LLM_PROVIDER: &str = "/api/webchat/v2/llm/providers";
pub const WEBUI_V2_PATTERN_DELETE_LLM_PROVIDER: &str =
    "/api/webchat/v2/llm/providers/{provider_id}/delete";
pub const WEBUI_V2_PATTERN_SET_ACTIVE_LLM: &str = "/api/webchat/v2/llm/active";
pub const WEBUI_V2_PATTERN_TEST_LLM_CONNECTION: &str = "/api/webchat/v2/llm/test-connection";
pub const WEBUI_V2_PATTERN_LIST_LLM_MODELS: &str = "/api/webchat/v2/llm/list-models";
pub const WEBUI_V2_PATTERN_START_NEARAI_LOGIN: &str = "/api/webchat/v2/llm/nearai/login";
pub const WEBUI_V2_PATTERN_COMPLETE_NEARAI_WALLET_LOGIN: &str = "/api/webchat/v2/llm/nearai/wallet";
pub const WEBUI_V2_PATTERN_START_CODEX_LOGIN: &str = "/api/webchat/v2/llm/codex/login";
pub const WEBUI_V2_PATTERN_LIST_TOOLS: &str = "/api/webchat/v2/tools";
pub const WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION: &str =
    "/api/webchat/v2/tools/{capability_id}/permission";
pub const WEBUI_V2_PATTERN_LIST_SKILLS: &str = "/api/webchat/v2/skills";
pub const WEBUI_V2_PATTERN_INSTALL_SKILL: &str = "/api/webchat/v2/skills/install";
pub const WEBUI_V2_PATTERN_REMOVE_SKILL: &str = "/api/webchat/v2/skills/{name}";

// Phase 7 — Recipe-Skill-Tool learning pipeline.
//
// The Recipe Library is the agent's persistent cookbook; the validation
// queue surfaces post-extraction rows that need an operator review pass,
// and the outcome endpoint feeds back into the engine `MetricRecorder`
// for Wilson lower-bound + tier reclassifications.
pub const WEBUI_V2_PATTERN_LIST_RECIPES: &str = "/api/webchat/v2/recipes";
pub const WEBUI_V2_PATTERN_LIST_TOOL_SKILLS: &str = "/api/webchat/v2/tool-skills";
pub const WEBUI_V2_PATTERN_GET_RECIPE: &str = "/api/webchat/v2/recipes/{project_id}/{recipe_id}";
pub const WEBUI_V2_PATTERN_GET_TOOL_SKILL: &str =
    "/api/webchat/v2/tool-skills/{project_id}/{skill_id}";
pub const WEBUI_V2_PATTERN_VALIDATE_RECIPE: &str =
    "/api/webchat/v2/recipes/{project_id}/{recipe_id}/validate";
pub const WEBUI_V2_PATTERN_REJECT_RECIPE: &str =
    "/api/webchat/v2/recipes/{project_id}/{recipe_id}/reject";
pub const WEBUI_V2_PATTERN_REQUEST_RECIPE_REVIEW: &str =
    "/api/webchat/v2/recipes/{project_id}/{recipe_id}/review-request";
pub const WEBUI_V2_PATTERN_VALIDATE_TOOL_SKILL: &str =
    "/api/webchat/v2/tool-skills/{project_id}/{skill_id}/validate";
pub const WEBUI_V2_PATTERN_REJECT_TOOL_SKILL: &str =
    "/api/webchat/v2/tool-skills/{project_id}/{skill_id}/reject";
pub const WEBUI_V2_PATTERN_REQUEST_TOOL_SKILL_REVIEW: &str =
    "/api/webchat/v2/tool-skills/{project_id}/{skill_id}/review-request";
pub const WEBUI_V2_PATTERN_RECORD_RECIPE_OUTCOME: &str =
    "/api/webchat/v2/recipes/{project_id}/{recipe_id}/outcomes";
pub const WEBUI_V2_PATTERN_VALIDATION_QUEUE: &str = "/api/webchat/v2/validation-queue";
pub const WEBUI_V2_PATTERN_VALIDATION_QUEUE_COUNT: &str = "/api/webchat/v2/validation-queue/count";

// Generalized component routes — `{class_code}` is the integer class code
// (e.g. 0=Tool, 1=Skill/Rusty, 21=Recipe).
pub const WEBUI_V2_PATTERN_VALIDATE_COMPONENT: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}/validate";
pub const WEBUI_V2_PATTERN_REJECT_COMPONENT: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}/reject";
pub const WEBUI_V2_PATTERN_SEND_COMPONENT_TO_REVISION: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}/send-to-revision";
pub const WEBUI_V2_PATTERN_RE_REVIEW_COMPONENT: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}/re-review";
pub const WEBUI_V2_PATTERN_DELETE_COMPONENT: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}";
pub const WEBUI_V2_PATTERN_GET_COMPONENT_AUDIT_STATUS: &str =
    "/api/webchat/v2/components/{class_code}/{component_id}/audit-status";

/// Return the canonical [`IngressRouteDescriptor`] set for the WebChat v2
/// beta route surface.
///
/// Host composition calls this once at startup, validates the descriptors
/// against its own mount table, and refuses to bind any route whose policy
/// the host cannot enforce.
pub fn webui_v2_routes() -> Vec<IngressRouteDescriptor> {
    vec![
        create_thread_descriptor(),
        delete_thread_descriptor(),
        send_message_descriptor(),
        list_threads_descriptor(),
        get_timeline_descriptor(),
        stream_events_descriptor(),
        stream_events_ws_descriptor(),
        cancel_run_descriptor(),
        resolve_gate_descriptor(),
        list_automations_descriptor(),
        list_connectable_channels_descriptor(),
        list_extensions_descriptor(),
        list_extension_registry_descriptor(),
        install_extension_descriptor(),
        activate_extension_descriptor(),
        remove_extension_descriptor(),
        get_extension_setup_descriptor(),
        setup_extension_descriptor(),
        get_llm_config_descriptor(),
        upsert_llm_provider_descriptor(),
        delete_llm_provider_descriptor(),
        set_active_llm_descriptor(),
        test_llm_connection_descriptor(),
        list_llm_models_descriptor(),
        start_nearai_login_descriptor(),
        complete_nearai_wallet_login_descriptor(),
        start_codex_login_descriptor(),
        list_tools_descriptor(),
        update_tool_permission_descriptor(),
        list_skills_descriptor(),
        install_skill_descriptor(),
        remove_skill_descriptor(),
        list_recipes_descriptor(),
        list_tool_skills_descriptor(),
        get_recipe_descriptor(),
        get_tool_skill_descriptor(),
        list_validation_queue_descriptor(),
        count_validation_queue_descriptor(),
        validate_recipe_descriptor(),
        reject_recipe_descriptor(),
        request_recipe_review_descriptor(),
        validate_tool_skill_descriptor(),
        reject_tool_skill_descriptor(),
        request_tool_skill_review_descriptor(),
        record_recipe_outcome_descriptor(),
        // Phase 3 (Step 3.5) — generalized component validation routes.
        validate_component_descriptor(),
        reject_component_descriptor(),
        send_component_to_revision_descriptor(),
        re_review_component_descriptor(),
        delete_component_descriptor(),
        get_component_audit_status_descriptor(),
        // Phase 5.5 — Interceptor configuration routes.
        get_interceptor_config_descriptor(),
        update_interceptor_config_descriptor(),
        reassemble_interceptor_descriptor(),
        prewarm_interceptor_descriptor(),
        // Phase 6 — Settings UI routes (10-tab editor).
        get_settings_skills_descriptor(),
        get_settings_tools_descriptor(),
        get_settings_extensions_descriptor(),
        get_settings_actions_descriptor(),
        get_settings_orchestrators_descriptor(),
        get_settings_scaffolds_descriptor(),
        get_settings_monty_vm_descriptor(),
        put_settings_monty_vm_descriptor(),
        post_settings_monty_vm_restart_descriptor(),
        get_settings_monty_vm_status_descriptor(),
        put_chat_preference_descriptor(),
    ]
}

/// Returns whether a route id belongs to the operator-wide LLM config surface.
/// Host composition uses this to keep route mounting and descriptor policy
/// filtering in sync when non-operator authenticators leave those routes
/// unmounted.
pub fn is_webui_v2_llm_config_route_id(route_id: &str) -> bool {
    matches!(
        route_id,
        WEBUI_V2_ROUTE_GET_LLM_CONFIG
            | WEBUI_V2_ROUTE_UPSERT_LLM_PROVIDER
            | WEBUI_V2_ROUTE_DELETE_LLM_PROVIDER
            | WEBUI_V2_ROUTE_SET_ACTIVE_LLM
            | WEBUI_V2_ROUTE_TEST_LLM_CONNECTION
            | WEBUI_V2_ROUTE_LIST_LLM_MODELS
            | WEBUI_V2_ROUTE_START_NEARAI_LOGIN
            | WEBUI_V2_ROUTE_COMPLETE_NEARAI_WALLET_LOGIN
            | WEBUI_V2_ROUTE_START_CODEX_LOGIN
    )
}

fn create_thread_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_CREATE_THREAD,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_CREATE_THREAD,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn send_message_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_SEND_MESSAGE,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_SEND_MESSAGE,
        mutation_policy(
            // Message bodies carry user content. 1 MiB is the same cap the
            // existing turn admission layer enforces.
            body_limit_kib(1024),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::TurnCoordinator,
        ),
    )
}

fn delete_thread_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_DELETE_THREAD,
        NetworkMethod::Delete,
        WEBUI_V2_PATTERN_DELETE_THREAD,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn get_timeline_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_TIMELINE,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_TIMELINE,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn stream_events_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_STREAM_EVENTS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_STREAM_EVENTS,
        read_policy(
            stream_rate_limit(),
            AuditTraceClass::StreamingSubscription,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::Sse,
        ),
    )
}

fn cancel_run_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_CANCEL_RUN,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_CANCEL_RUN,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::TurnCoordinator,
        ),
    )
}

fn resolve_gate_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_RESOLVE_GATE,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_RESOLVE_GATE,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::TurnCoordinator,
        ),
    )
}

fn list_threads_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_THREADS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_THREADS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn stream_events_ws_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_STREAM_EVENTS_WS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_STREAM_EVENTS_WS,
        ws_read_policy(
            stream_rate_limit(),
            AuditTraceClass::StreamingSubscription,
            AllowedEffectPath::ProjectionOnly,
        ),
    )
}

fn list_automations_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_AUTOMATIONS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_AUTOMATIONS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
            StreamingMode::None,
        ),
    )
}

fn list_connectable_channels_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_CONNECTABLE_CHANNELS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_CONNECTABLE_CHANNELS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn list_extensions_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_EXTENSIONS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_EXTENSIONS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn list_extension_registry_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_EXTENSION_REGISTRY,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_EXTENSION_REGISTRY,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn install_extension_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_INSTALL_EXTENSION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_INSTALL_EXTENSION,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn activate_extension_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_ACTIVATE_EXTENSION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_ACTIVATE_EXTENSION,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn remove_extension_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REMOVE_EXTENSION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_REMOVE_EXTENSION,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn get_extension_setup_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_EXTENSION_SETUP,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETUP_EXTENSION,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn setup_extension_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_SETUP_EXTENSION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_SETUP_EXTENSION,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn get_llm_config_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_LLM_CONFIG,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_LLM_CONFIG,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn upsert_llm_provider_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_UPSERT_LLM_PROVIDER,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_UPSERT_LLM_PROVIDER,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn delete_llm_provider_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_DELETE_LLM_PROVIDER,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_DELETE_LLM_PROVIDER,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn set_active_llm_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_SET_ACTIVE_LLM,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_SET_ACTIVE_LLM,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn test_llm_connection_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_TEST_LLM_CONNECTION,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_TEST_LLM_CONNECTION,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn list_llm_models_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_LLM_MODELS,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_LIST_LLM_MODELS,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn start_nearai_login_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_START_NEARAI_LOGIN,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_START_NEARAI_LOGIN,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn complete_nearai_wallet_login_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_COMPLETE_NEARAI_WALLET_LOGIN,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_COMPLETE_NEARAI_WALLET_LOGIN,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn start_codex_login_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_START_CODEX_LOGIN,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_START_CODEX_LOGIN,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn list_tools_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_TOOLS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_TOOLS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn update_tool_permission_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_UPDATE_TOOL_PERMISSION,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn list_skills_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_SKILLS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_SKILLS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn install_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_INSTALL_SKILL,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_INSTALL_SKILL,
        mutation_policy(
            body_limit_kib(512),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn remove_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REMOVE_SKILL,
        NetworkMethod::Delete,
        WEBUI_V2_PATTERN_REMOVE_SKILL,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn list_recipes_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_RECIPES,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_RECIPES,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn list_tool_skills_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_TOOL_SKILLS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_TOOL_SKILLS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_recipe_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_RECIPE,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_RECIPE,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_tool_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_TOOL_SKILL,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_TOOL_SKILL,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn list_validation_queue_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_VALIDATION_QUEUE,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_VALIDATION_QUEUE,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn count_validation_queue_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_COUNT_VALIDATION_QUEUE,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_VALIDATION_QUEUE_COUNT,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn validate_recipe_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_VALIDATE_RECIPE,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_VALIDATE_RECIPE,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn reject_recipe_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REJECT_RECIPE,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_REJECT_RECIPE,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn request_recipe_review_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REQUEST_RECIPE_REVIEW,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_REQUEST_RECIPE_REVIEW,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn validate_tool_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_VALIDATE_TOOL_SKILL,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_VALIDATE_TOOL_SKILL,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn reject_tool_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REJECT_TOOL_SKILL,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_REJECT_TOOL_SKILL,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn request_tool_skill_review_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REQUEST_TOOL_SKILL_REVIEW,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_REQUEST_TOOL_SKILL_REVIEW,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn record_recipe_outcome_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_RECORD_RECIPE_OUTCOME,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_RECORD_RECIPE_OUTCOME,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn validate_component_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_VALIDATE_COMPONENT,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_VALIDATE_COMPONENT,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn reject_component_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_REJECT_COMPONENT,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_REJECT_COMPONENT,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn send_component_to_revision_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_SEND_COMPONENT_TO_REVISION,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_SEND_COMPONENT_TO_REVISION,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn re_review_component_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_RE_REVIEW_COMPONENT,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_RE_REVIEW_COMPONENT,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn delete_component_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_DELETE_COMPONENT,
        NetworkMethod::Delete,
        WEBUI_V2_PATTERN_DELETE_COMPONENT,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn get_component_audit_status_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_COMPONENT_AUDIT_STATUS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_COMPONENT_AUDIT_STATUS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_interceptor_config_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_INTERCEPTOR_CONFIG,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_GET_INTERCEPTOR_CONFIG,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
            StreamingMode::None,
        ),
    )
}

fn update_interceptor_config_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_UPDATE_INTERCEPTOR_CONFIG,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_GET_INTERCEPTOR_CONFIG,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn reassemble_interceptor_descriptor() -> IngressRouteDescriptor {
    // Reassemble has a 1/min per-caller rate limit (enforced in the service).
    descriptor(
        WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn prewarm_interceptor_descriptor() -> IngressRouteDescriptor {
    // Pre-warm has a 1/min per-caller rate limit (enforced in the service).
    descriptor(
        WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

// Phase 6 — Settings UI descriptor builder functions.

fn get_settings_skills_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_SKILLS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_SKILLS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_tools_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_TOOLS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_TOOLS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_extensions_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_EXTENSIONS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_EXTENSIONS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_actions_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_ACTIONS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_ACTIONS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_orchestrators_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_ORCHESTRATORS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_ORCHESTRATORS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_scaffolds_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_SCAFFOLDS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_SCAFFOLDS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn get_settings_monty_vm_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_MONTY_VM,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn put_settings_monty_vm_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_PUT_SETTINGS_MONTY_VM,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_SETTINGS_MONTY_VM,
        mutation_policy(
            body_limit_kib(16),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn post_settings_monty_vm_restart_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_POST_SETTINGS_MONTY_VM_RESTART,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_RESTART,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn get_settings_monty_vm_status_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM_STATUS,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_SETTINGS_MONTY_VM_STATUS,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn put_chat_preference_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_PUT_CHAT_PREFERENCE,
        NetworkMethod::Put,
        WEBUI_V2_PATTERN_CHAT_PREFERENCE,
        mutation_policy(
            body_limit_kib(4),
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}

fn ws_read_policy(
    rate_limit: RateLimitPolicy,
    audit: AuditTraceClass,
    effect_path: AllowedEffectPath,
) -> IngressPolicy {
    IngressPolicy::new(IngressPolicyParts {
        listener_class: ListenerClass::LocalGateway,
        auth: bearer_required(),
        scope_source: IngressScopeSource::AuthenticatedCaller,
        body_limit: BodyLimitPolicy::NoBody,
        rate_limit,
        cors: CorsPolicy::SameOriginOnly,
        // WS upgrade is gated by host composition's same-origin
        // check; declared here so the descriptor is the contract a
        // future allowlist-based deployment overrides.
        websocket_origin: WebSocketOriginPolicy::SameOriginRequired,
        streaming: StreamingMode::WebSocket,
        audit,
        effect_path,
    })
    .expect("webui v2 WS read policy must validate") // safety: combination LocalGateway + bearer + AuthenticatedCaller + WebSocket + SameOriginRequired is a permitted shape; other parts are crate-local constants
}

fn descriptor(
    route_id: &str,
    method: NetworkMethod,
    pattern: &str,
    policy: IngressPolicy,
) -> IngressRouteDescriptor {
    IngressRouteDescriptor::new(route_id.to_string(), method, pattern.to_string(), policy)
        .expect("webui v2 route descriptor must validate at startup") // safety: route_id/pattern are crate-local literals known to satisfy IngressRouteId / IngressRoutePattern; policy is constructed by sibling helpers that validate their own inputs
}

fn mutation_policy(
    body_limit: BodyLimitPolicy,
    rate_limit: RateLimitPolicy,
    audit: AuditTraceClass,
    effect_path: AllowedEffectPath,
) -> IngressPolicy {
    IngressPolicy::new(IngressPolicyParts {
        listener_class: ListenerClass::LocalGateway,
        auth: bearer_required(),
        scope_source: IngressScopeSource::AuthenticatedCaller,
        body_limit,
        rate_limit,
        cors: CorsPolicy::SameOriginOnly,
        websocket_origin: WebSocketOriginPolicy::NotApplicable,
        streaming: StreamingMode::None,
        audit,
        effect_path,
    })
    .expect("webui v2 mutation policy must validate") // safety: all parts are crate-local constants; the combination (LocalGateway + bearer required + AuthenticatedCaller + None streaming) is a permitted shape, locked in by the descriptor contract test
}

fn read_policy(
    rate_limit: RateLimitPolicy,
    audit: AuditTraceClass,
    effect_path: AllowedEffectPath,
    streaming: StreamingMode,
) -> IngressPolicy {
    IngressPolicy::new(IngressPolicyParts {
        listener_class: ListenerClass::LocalGateway,
        auth: bearer_required(),
        scope_source: IngressScopeSource::AuthenticatedCaller,
        body_limit: BodyLimitPolicy::NoBody,
        rate_limit,
        cors: CorsPolicy::SameOriginOnly,
        websocket_origin: WebSocketOriginPolicy::NotApplicable,
        streaming,
        audit,
        effect_path,
    })
    .expect("webui v2 read policy must validate") // safety: streaming is either None or Sse (both permitted with bearer + AuthenticatedCaller); other parts are crate-local constants
}

fn bearer_required() -> IngressAuthPolicy {
    IngressAuthPolicy::Required {
        schemes: vec![IngressAuthScheme::BearerToken],
    }
}

fn body_limit_kib(kib: u64) -> BodyLimitPolicy {
    let bytes = kib
        .checked_mul(1024)
        .and_then(NonZeroU64::new)
        .expect("body limit must be non-zero"); // safety: all call sites pass crate-local positive constants (4, 16, 1024); overflow at u64 * 1024 is impossible for these
    BodyLimitPolicy::Limited { max_bytes: bytes }
}

fn mutation_rate_limit() -> RateLimitPolicy {
    rate_limit_per_caller(60, 60)
}

fn read_rate_limit() -> RateLimitPolicy {
    rate_limit_per_caller(120, 60)
}

fn stream_rate_limit() -> RateLimitPolicy {
    // Shared budget for the SSE (`stream_events`) and WebSocket
    // (`stream_events_ws`) routes. SSE sessions are long-lived; the
    // per-tenant/user concurrency cap (3 streams, enforced in
    // `WebUiV2State::SseCapacity`) does the real bounding. The
    // request-rate window here is just for burst protection against
    // reconnect storms.
    //
    // Set to 30/60s — the SSE route additionally accepts `?token=…`
    // because `EventSource` can't set headers, which leaks the
    // bearer into browser history, server access logs, and proxy
    // logs. Keeping the request rate higher than necessary widens
    // the replay surface for a logged token, so the budget is capped
    // at 2x a worst-case exponential-backoff reconnect cycle (≈ 1,
    // 2, 4, 8, 16, 32s per minute = 6 opens) rather than parity with
    // the mutation budget. The WS route doesn't carry the same
    // URL-token risk (headers + `WebSocketOriginPolicy::SameOriginRequired`),
    // but the lower limit costs it nothing — the same reconnect-storm
    // math applies, the same concurrency cap is the real load gate,
    // and using one helper for both keeps the descriptors aligned.
    rate_limit_per_caller(30, 60)
}

fn rate_limit_per_caller(max: u32, window_secs: u32) -> RateLimitPolicy {
    RateLimitPolicy::Limited {
        scope: RateLimitScope::PerCaller,
        max_requests: NonZeroU32::new(max).expect("max_requests must be non-zero"), // safety: all call sites pass crate-local positive constants (12, 60, 120)
        window_seconds: NonZeroU32::new(window_secs).expect("window_seconds must be non-zero"), // safety: all call sites pass crate-local positive constants (60)
    }
}
