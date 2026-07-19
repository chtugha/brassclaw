use std::sync::Arc;

use brassclaw_product_adapters::ProjectionStream;
use brassclaw_product_workflow::{
    ConnectableChannelsProductFacade, RebornServices as ProductRebornServices, RebornServicesApi,
    RebornServicesError, RebornServicesErrorCode, RebornServicesErrorKind,
};

use crate::{
    RebornBuildError, RebornProductAuthServices, RebornReadiness, RebornRuntime,
    RebornWebuiAutomationFacade,
    lifecycle::{RebornLocalLifecycleFacade, RebornLocalSkillsProductFacade},
    webui_extension_credentials::ProductAuthExtensionCredentialSetup,
};

/// WebUI-facing Reborn service bundle for host composition.
///
/// This bundle deliberately exposes facade-shaped product handles consumed
/// by WebChat v2 and the optional product-auth OAuth routes. HTTP
/// routing, auth middleware, static assets, and SSE transport stay in the
/// WebUI crate (or, when the `webui-v2-beta` feature is on, the
/// [`crate::webui_serve`] module in this crate); lower runtime handles stay
/// behind the existing Reborn runtime / composition services.
#[derive(Clone)]
pub struct RebornWebuiBundle {
    pub api: Arc<dyn RebornServicesApi>,
    pub product_auth: Option<Arc<RebornProductAuthServices>>,
    pub readiness: RebornReadiness,
}

impl std::fmt::Debug for RebornWebuiBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RebornWebuiBundle")
            .field("api", &"Arc<dyn RebornServicesApi>")
            .field("product_auth", &self.product_auth.is_some())
            .field("readiness", &self.readiness)
            .finish()
    }
}

/// Compose the WebUI-facing product facade from an already-built Reborn runtime.
///
/// This function does not create a second turn coordinator, thread service,
/// host runtime or route server. It reuses the runtime's existing task-level
/// composition and attaches the runtime-owned projection stream unless the
/// caller supplies a custom stream.
pub fn build_webui_services(
    runtime: &RebornRuntime,
    event_stream: Option<Arc<dyn ProjectionStream>>,
) -> Result<RebornWebuiBundle, RebornBuildError> {
    build_webui_services_with_connectable_channels(runtime, event_stream, None)
}

pub(crate) fn build_webui_services_with_connectable_channels(
    runtime: &RebornRuntime,
    event_stream: Option<Arc<dyn ProjectionStream>>,
    connectable_channels: Option<Arc<dyn ConnectableChannelsProductFacade>>,
) -> Result<RebornWebuiBundle, RebornBuildError> {
    let services = runtime.services();
    let automation_facade = services
        .host_runtime
        .as_ref()
        .map(|host_runtime| Arc::new(RebornWebuiAutomationFacade::new(Arc::clone(host_runtime))));

    let mut api = ProductRebornServices::new(
        runtime.webui_thread_service(),
        runtime.webui_turn_coordinator(),
    )
    .with_approval_interactions(runtime.webui_approval_interaction_service())
    .with_auth_interactions(runtime.webui_auth_interaction_service());
    if let Some(skill_activation_source) = runtime.webui_skill_activation_source() {
        let activation_recorder = Arc::clone(&skill_activation_source);
        let activation_clearer = skill_activation_source;
        api = api.with_skill_activation_hooks(
            move |scope, accepted_message_ref, message| {
                activation_recorder
                    .record_user_message(scope.clone(), accepted_message_ref.clone(), message)
                    .map_err(|_| RebornServicesError {
                        code: RebornServicesErrorCode::Internal,
                        kind: RebornServicesErrorKind::Internal,
                        status_code: 500,
                        retryable: false,
                        field: None,
                        validation_code: None,
                    })
            },
            move |scope, accepted_message_ref| {
                activation_clearer
                    .clear_accepted_message(scope, accepted_message_ref)
                    .map_err(|_| RebornServicesError {
                        code: RebornServicesErrorCode::Internal,
                        kind: RebornServicesErrorKind::Internal,
                        status_code: 500,
                        retryable: false,
                        field: None,
                        validation_code: None,
                    })
            },
        );
    }
    if let Some(local_runtime) = &services.local_runtime {
        let mut lifecycle_facade =
            RebornLocalLifecycleFacade::new(local_runtime.skill_management.clone());
        if let Some(extension_management) = &local_runtime.extension_management {
            lifecycle_facade =
                lifecycle_facade.with_extension_management(extension_management.clone());
        }
        if let Some(runtime_http_egress) = &local_runtime.runtime_http_egress {
            lifecycle_facade =
                lifecycle_facade.with_runtime_http_egress(runtime_http_egress.clone());
        }
        api = api.with_lifecycle_product_facade(Arc::new(lifecycle_facade));
        api = api.with_skills_facade(Arc::new(RebornLocalSkillsProductFacade::new(
            local_runtime.skill_management.clone(),
        )));
    }
    if let Some(product_auth) = &services.product_auth {
        api = api.with_extension_credentials(Arc::new(ProductAuthExtensionCredentialSetup::new(
            Arc::clone(product_auth),
        )));
    }
    if let Some(automation_facade) = automation_facade {
        api = api.with_automation_product_facade(automation_facade);
    }
    if let Some(connectable_channels) = connectable_channels {
        api = api.with_connectable_channels_facade(connectable_channels);
    }
    api = api.with_event_stream(event_stream.unwrap_or_else(|| runtime.webui_event_stream()));

    // Compose the operator LLM-config settings service when the runtime was
    // assembled with a boot config. The secret store stays private to this
    // crate; the service is the only facade-shaped handle that leaves.
    #[cfg(feature = "root-llm-provider")]
    if let Some(boot) = runtime.webui_boot_config() {
        let keys = crate::LlmKeyStore::new(runtime.services().secret_store());
        let mut llm_config = crate::RebornLlmConfigService::new(boot.clone(), keys);
        if let Some(mut adapter) = runtime.webui_llm_reload_adapter() {
            // Wire the per-provider budget refresh callback on provider change so
            // live budget slots update without a restart when the active provider
            // is switched through the settings UI.
            #[cfg(feature = "libsql")]
            {
                let budget_slot = runtime.live_context_budget();
                let max_output_slot = runtime.live_max_output();
                let total_input_slot = runtime.live_total_input();
                let inline_control_slot = runtime.live_inline_control();
                let context_window_slot = runtime.live_context_window();
                let token_store = services
                    .local_runtime
                    .as_ref()
                    .map(|lr| Arc::clone(&lr.token_settings_store));
                let owner_id = runtime.actor_user_id_string();
                // Load the provider registry once at wiring time so the
                // on_provider_changed callback can read context_window_tokens
                // from an in-memory snapshot rather than hitting disk on every
                // provider switch in the UI.
                let boot_registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
                    .ok()
                    .map(Arc::new);
                // All five live slots share this outer guard: if budget_slot or
                // token_store is absent, the entire callback is skipped. This is
                // intentional — without a store none of the slots can refresh.
                if let (Some(slot), Some(store)) = (budget_slot, token_store) {
                    let on_change: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(
                        move |provider_id: &str| {
                            let slot = slot.clone();
                            let max_out = max_output_slot.clone();
                            let total_in = total_input_slot.clone();
                            let inline_ctl = inline_control_slot.clone();
                            let ctx_win = context_window_slot.clone();
                            let store = Arc::clone(&store);
                            let owner = owner_id.clone();
                            let pid = provider_id.to_string();
                            let registry = boot_registry.clone();
                            tokio::spawn(async move {
                                use brassclaw_product_workflow::TokenSettingsStore as _;
                                match store.get_provider_token_settings(&owner, &pid).await {
                                    Ok(row) => {
                                        slot.set(row.conversation_history);
                                        if let Some(s) = &max_out {
                                            s.set(row.max_output);
                                        }
                                        if let Some(s) = &total_in {
                                            s.set(row.total_input);
                                        }
                                        if let Some(s) = &inline_ctl {
                                            s.set(row.inline_control);
                                        }
                                    }
                                    Err(_) => {
                                        tracing::debug!(
                                            provider = %pid,
                                            "on_provider_changed: failed to load token settings, live slots not updated"
                                        );
                                    }
                                }
                                if let Some(s) = &ctx_win {
                                    if let Some(reg) = &registry {
                                        let window = reg
                                            .find(&pid)
                                            .and_then(|d| d.context_window_tokens)
                                            .map(|v| v as usize);
                                        s.set(window);
                                    } else {
                                        tracing::debug!(
                                            provider = %pid,
                                            "on_provider_changed: provider registry not available at boot, context_window slot not updated"
                                        );
                                    }
                                }
                            });
                        },
                    );
                    adapter = adapter.with_on_provider_changed(on_change);
                }
            }
            llm_config = llm_config
                .with_reload_trigger(Arc::new(adapter) as Arc<dyn crate::LlmReloadTrigger>);
        }
        if let Some(session) = runtime.webui_llm_session() {
            llm_config = llm_config.with_nearai_session(session);
        }
        if let Some(states) = runtime.webui_nearai_login_states() {
            llm_config = llm_config.with_nearai_login_states(states);
        }
        api = api.with_llm_config_service(Arc::new(llm_config));
    }

    // Wire the safety configuration store — libsql (local-dev) or postgres (production).
    #[cfg(feature = "postgres")]
    if let Some(safety_config_store) = &services.pg_safety_config_store {
        tracing::debug!("wiring PgSafetyConfigStore into WebUI API");
        api = api.with_safety_config_store(Arc::clone(safety_config_store));
    }
    #[cfg(feature = "libsql")]
    if let Some(safety_config_store) = &services.safety_config_store {
        tracing::debug!("wiring SafetyConfigStore into WebUI API");
        api = api.with_safety_config_store(Arc::clone(safety_config_store));
    }

    // Wire the token settings store — libsql (local-dev) or postgres (production).
    #[cfg(feature = "postgres")]
    if let Some(token_settings_store) = &services.pg_token_settings_store {
        tracing::debug!("wiring PgTokenSettingsStore into WebUI API");
        api = api.with_token_settings_store(Arc::clone(token_settings_store)
            as Arc<dyn brassclaw_product_workflow::TokenSettingsStore>);
    }
    #[cfg(feature = "libsql")]
    if let Some(token_settings_store) = &services.token_settings_store {
        tracing::debug!("wiring TokenSettingsStore into WebUI API");
        api = api.with_token_settings_store(Arc::clone(token_settings_store)
            as Arc<dyn brassclaw_product_workflow::TokenSettingsStore>);
    }

    // Wire the engine `Store` through `StoreBackedReductionRuleStore` and
    // `StoreBackedRecipeStore` — postgres (production) or libsql (local-dev).
    // This is what backs `/tokens/reduction-rules/*` and the Recipe Manager WebUI.
    #[cfg(feature = "postgres")]
    if let Some(memory_doc_store) = services.pg_memory_doc_store.clone() {
        let dyn_store: Arc<dyn brassclaw_engine::traits::store::Store> =
            Arc::clone(&memory_doc_store) as Arc<dyn brassclaw_engine::traits::store::Store>;
        let reduction_rule_store = crate::reduction_rules_store::StoreBackedReductionRuleStore::open(
            Arc::clone(&dyn_store),
        );
        api = api.with_reduction_rule_store(
            Arc::new(reduction_rule_store)
                as Arc<dyn brassclaw_product_workflow::ReductionRuleStore>,
        );
        api = api.with_reduction_rules_cache_invalidator(Arc::new(
            |_project_id: &str, _user_id: &str| {
                brassclaw_engine::executor::orchestrator::invalidate_reduction_rules_cache();
            },
        ));
        let recipe_store = crate::recipe_store::StoreBackedRecipeStore::open(Arc::clone(&dyn_store));
        api = api.with_recipe_store(
            Arc::new(recipe_store)
                as Arc<dyn brassclaw_product_workflow::RecipeStore>,
        );
        tracing::debug!("ReductionRuleStore + RecipeStore wired through PgMemoryDocStore");
    }
    #[cfg(feature = "libsql")]
    if let Some(memory_doc_store) = services.memory_doc_store.clone() {
        let dyn_store: Arc<dyn brassclaw_engine::traits::store::Store> =
            Arc::clone(&memory_doc_store) as Arc<dyn brassclaw_engine::traits::store::Store>;
        let reduction_rule_store = crate::reduction_rules_store::StoreBackedReductionRuleStore::open(
            Arc::clone(&dyn_store),
        );
        api = api.with_reduction_rule_store(
            Arc::new(reduction_rule_store)
                as Arc<dyn brassclaw_product_workflow::ReductionRuleStore>,
        );
        api = api.with_reduction_rules_cache_invalidator(Arc::new(
            |_project_id: &str, _user_id: &str| {
                brassclaw_engine::executor::orchestrator::invalidate_reduction_rules_cache();
            },
        ));
        let recipe_store = crate::recipe_store::StoreBackedRecipeStore::open(Arc::clone(&dyn_store));
        api = api.with_recipe_store(
            Arc::new(recipe_store)
                as Arc<dyn brassclaw_product_workflow::RecipeStore>,
        );
        tracing::debug!(
            "ReductionRuleStore + RecipeStore wired through MemoryDocLibSqlStore"
        );
    }

    #[cfg(all(feature = "libsql", feature = "root-llm-provider"))]
    if let Some(budget) = runtime.live_context_budget() {
        api = api.with_live_context_budget_setter(Arc::new(move |v| budget.set(v)));
    }
    #[cfg(all(feature = "libsql", feature = "root-llm-provider"))]
    if let Some(slot) = runtime.live_max_output() {
        api = api.with_live_max_output_setter(Arc::new(move |v| slot.set(v)));
    }
    #[cfg(all(feature = "libsql", feature = "root-llm-provider"))]
    if let Some(slot) = runtime.live_total_input() {
        api = api.with_live_total_input_setter(Arc::new(move |v| slot.set(v)));
    }
    #[cfg(all(feature = "libsql", feature = "root-llm-provider"))]
    if let Some(slot) = runtime.live_inline_control() {
        api = api.with_live_inline_control_setter(Arc::new(move |v| slot.set(v)));
    }

    // Wire the extension registry and capability permission store for Tools API
    if let Some(local_runtime) = &services.local_runtime {
        tracing::debug!("wiring ExtensionRegistry into WebUI API");
        api = api.with_extension_registry(Arc::clone(&local_runtime.extension_registry)
            as Arc<dyn brassclaw_host_api::CapabilityRegistry>);

        // Wire capability permission store when available (local-dev with libsql)
        #[cfg(feature = "libsql")]
        if let Some(safety_config_store) = &services.safety_config_store {
            tracing::debug!("wiring CapabilityPermissionStore into WebUI API");
            api = api.with_capability_permission_store(Arc::clone(safety_config_store)
                as Arc<dyn brassclaw_product_workflow::CapabilityPermissionStore>);
        }
    } else {
        tracing::debug!("ExtensionRegistry is None - tools endpoints will return empty list");
    }

    Ok(RebornWebuiBundle {
        api: Arc::new(api),
        product_auth: services.product_auth.clone(),
        readiness: services.readiness,
    })
}
