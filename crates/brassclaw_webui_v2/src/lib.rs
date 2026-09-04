//! Reborn WebChat v2 HTTP route surface.
//!
//! This crate ships the minimal native WebUI v2 route set on top of the
//! [`brassclaw_product_workflow::RebornServicesApi`] facade.
//!
//! ## Boundaries
//!
//! - Handlers consume only [`RebornServicesApi`] for chat, run/gate,
//!   extension, and automation reads. They never reach into the dispatcher,
//!   `HostRuntime`, run-state, DB stores, or any runtime lane.
//! - Auth and CORS are **not** enforced here. Host composition runs the
//!   bearer-token middleware that builds a [`WebUiAuthenticatedCaller`] and
//!   injects it as an `Extension` before traffic reaches these handlers.
//! - The [`IngressRouteDescriptor`] set returned by [`webui_v2_routes`] is
//!   the canonical contract the host composes against: mount path, method,
//!   auth scheme, body / rate limit, streaming mode, audit class, and the
//!   allowed effect path. Adding a new route here requires a matching
//!   descriptor.
//!
//! ## Streaming
//!
//! `stream_events` is exposed as SSE. The current
//! [`RebornServicesApi::stream_events`] is drain-only, so the handler
//! drains once, renders each product envelope into a
//! [`WebChatV2EventFrame`] SSE message with the projection cursor as the
//! SSE id, then polls at a low cadence for newly-arrived events. When the
//! facade gains a real subscription API the handler can migrate without
//! changing the descriptor or browser-visible event schema.
//!
//! Beyond the route descriptor's per-caller request rate limit, the
//! handler caps the number of *concurrent* SSE streams a single
//! `(tenant, user)` may hold and closes any single stream after a fixed
//! maximum lifetime so leaked guards or stuck pollers cannot wedge a
//! caller's slot indefinitely.
//!
//! [`RebornServicesApi`]: brassclaw_product_workflow::RebornServicesApi
//! [`WebChatV2EventFrame`]: crate::WebChatV2EventFrame
//! [`WebUiAuthenticatedCaller`]: brassclaw_product_workflow::WebUiAuthenticatedCaller
//! [`IngressRouteDescriptor`]: brassclaw_host_api::ingress::IngressRouteDescriptor

#![forbid(unsafe_code)]

mod descriptors;
mod error;
mod handlers;
mod router;
mod schema;
mod sse_capacity;

pub use descriptors::{
    WEBUI_V2_ROUTE_ACTIVATE_EXTENSION, WEBUI_V2_ROUTE_CANCEL_RUN,
    WEBUI_V2_ROUTE_COMPLETE_NEARAI_WALLET_LOGIN, WEBUI_V2_ROUTE_COUNT_VALIDATION_QUEUE,
    WEBUI_V2_ROUTE_CREATE_THREAD, WEBUI_V2_ROUTE_DELETE_COMPONENT,
    WEBUI_V2_ROUTE_DELETE_INTENT_INPUTS, WEBUI_V2_ROUTE_DELETE_LLM_PROVIDER,
    WEBUI_V2_ROUTE_DELETE_THREAD, WEBUI_V2_ROUTE_GET_COMPONENT_AUDIT_STATUS,
    WEBUI_V2_ROUTE_GET_EXTENSION_SETUP, WEBUI_V2_ROUTE_GET_INTERCEPTOR_CONFIG,
    WEBUI_V2_ROUTE_GET_LLM_CONFIG, WEBUI_V2_ROUTE_GET_RECIPE, WEBUI_V2_ROUTE_GET_SETTINGS_ACTIONS,
    WEBUI_V2_ROUTE_GET_SETTINGS_EXTENSIONS, WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM,
    WEBUI_V2_ROUTE_GET_SETTINGS_MONTY_VM_STATUS, WEBUI_V2_ROUTE_GET_SETTINGS_ORCHESTRATORS,
    WEBUI_V2_ROUTE_EXPORT_SKILL,
    WEBUI_V2_ROUTE_GET_SETTINGS_SCAFFOLDS, WEBUI_V2_ROUTE_GET_SETTINGS_SKILLS,
    WEBUI_V2_ROUTE_GET_SETTINGS_TOOLS, WEBUI_V2_ROUTE_GET_TIMELINE, WEBUI_V2_ROUTE_GET_TOOL_SKILL,
    WEBUI_V2_ROUTE_INSTALL_EXTENSION, WEBUI_V2_ROUTE_INSTALL_SKILL,
    WEBUI_V2_ROUTE_LIST_AUTOMATIONS, WEBUI_V2_ROUTE_LIST_CONNECTABLE_CHANNELS,
    WEBUI_V2_ROUTE_LIST_EXTENSION_REGISTRY, WEBUI_V2_ROUTE_LIST_EXTENSIONS,
    WEBUI_V2_ROUTE_LIST_INTENT_INPUTS, WEBUI_V2_ROUTE_LIST_LLM_MODELS,
    WEBUI_V2_ROUTE_LIST_PREFIXES, WEBUI_V2_ROUTE_LIST_RECIPES, WEBUI_V2_ROUTE_LIST_SKILLS,
    WEBUI_V2_ROUTE_LIST_THREADS, WEBUI_V2_ROUTE_LIST_TOOL_SKILLS, WEBUI_V2_ROUTE_LIST_TOOLS,
    WEBUI_V2_ROUTE_LIST_VALIDATION_QUEUE, WEBUI_V2_ROUTE_POST_SETTINGS_MONTY_VM_RESTART,
    WEBUI_V2_ROUTE_PUT_CHAT_PREFERENCE, WEBUI_V2_ROUTE_PUT_SETTINGS_MONTY_VM,
    WEBUI_V2_ROUTE_RE_REVIEW_COMPONENT, WEBUI_V2_ROUTE_RECORD_RECIPE_OUTCOME,
    WEBUI_V2_ROUTE_REGENERATE_PREFIX, WEBUI_V2_ROUTE_REJECT_COMPONENT,
    WEBUI_V2_ROUTE_REMOVE_EXTENSION, WEBUI_V2_ROUTE_REMOVE_SKILL, WEBUI_V2_ROUTE_RESOLVE_GATE,
    WEBUI_V2_ROUTE_SEND_COMPONENT_TO_REVISION, WEBUI_V2_ROUTE_SEND_MESSAGE,
    WEBUI_V2_ROUTE_SET_ACTIVE_LLM, WEBUI_V2_ROUTE_SETUP_EXTENSION,
    WEBUI_V2_ROUTE_START_CODEX_LOGIN, WEBUI_V2_ROUTE_START_NEARAI_LOGIN,
    WEBUI_V2_ROUTE_STREAM_EVENTS, WEBUI_V2_ROUTE_STREAM_EVENTS_WS,
    WEBUI_V2_ROUTE_TEST_LLM_CONNECTION, WEBUI_V2_ROUTE_UPDATE_INTERCEPTOR_CONFIG,
    WEBUI_V2_ROUTE_UPDATE_TOOL_PERMISSION, WEBUI_V2_ROUTE_UPSERT_INTENT_INPUT,
    WEBUI_V2_ROUTE_UPSERT_LLM_PROVIDER, WEBUI_V2_ROUTE_VALIDATE_COMPONENT,
    is_webui_v2_llm_config_route_id, webui_v2_routes,
};
pub use error::{WebUiV2HttpError, WebUiV2HttpErrorBody};
pub use handlers::{
    activate_extension, cancel_run, complete_nearai_wallet_login, create_thread,
    delete_llm_provider, delete_thread, get_extension_setup, get_llm_config, get_timeline,
    install_extension, install_skill, list_automations, list_connectable_channels,
    list_extension_registry, list_extensions, list_llm_models, list_skills, list_threads,
    remove_extension, remove_skill, resolve_gate, send_message, set_active_llm, setup_extension,
    start_codex_login, start_nearai_login, stream_events, stream_events_ws, test_llm_connection,
    upsert_llm_provider,
};
pub use router::{
    WebUiV2RouteOptions, WebUiV2State, webui_v2_router, webui_v2_router_with_options,
};
pub use schema::{WebChatV2Event, WebChatV2EventFrame};
