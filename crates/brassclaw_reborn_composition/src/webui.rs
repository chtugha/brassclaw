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
pub async fn build_webui_services(
    runtime: &RebornRuntime,
    event_stream: Option<Arc<dyn ProjectionStream>>,
) -> Result<RebornWebuiBundle, RebornBuildError> {
    build_webui_services_with_connectable_channels(runtime, event_stream, None).await
}

pub(crate) async fn build_webui_services_with_connectable_channels(
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

    // Compose the operator LLM-config settings service.
    // Requires both root-llm-provider and postgres — the service is DB-exclusive
    // after the V047/V048 migration.
    #[cfg(all(feature = "root-llm-provider", feature = "postgres"))]
    if let Some(pool) = services.pg_pool.clone() {
        let keys = crate::LlmKeyStore::new(runtime.services().secret_store());
        let tenant_id = runtime.webui_tenant_id().to_string();
        let pg_repo = Arc::new(crate::pg_provider_repo::PgProviderRepo::new(
            pool.as_ref().clone(),
            tenant_id.clone(),
        ));
        // Seed builtin providers on every service start (idempotent).
        if let Err(e) = seed_builtin_providers(&pg_repo).await {
            tracing::warn!(
                error = %e,
                "builtin provider seeding failed; providers may be \
                 missing from the settings UI until the next restart"
            );
        }
        let mut llm_config =
            crate::RebornLlmConfigService::new(keys, pool.clone(), pg_repo, tenant_id);
        if let Some(adapter) = runtime.webui_llm_reload_adapter() {
            llm_config = llm_config
                .with_reload_trigger(Arc::new(adapter) as Arc<dyn crate::LlmReloadTrigger>);
        }
        if let Some(session) = runtime.webui_llm_session() {
            llm_config = llm_config.with_nearai_session(session);
        }
        if let Some(states) = runtime.webui_nearai_login_states() {
            llm_config = llm_config.with_nearai_login_states(states);
        }
        if let Some(swappable) = runtime.sempai_swappable() {
            llm_config = llm_config.with_sempai_swappable(swappable);
        }
        if let Some(mode) = runtime.interceptor_mode() {
            llm_config = llm_config.with_interceptor_mode(mode);
        }
        api = api.with_llm_config_service(Arc::new(llm_config));
    }

    // Seed the built-in host.* component stack (Phase C.2). Idempotent; runs
    // on every service start. Only needs postgres (independent of
    // root-llm-provider) — the host.* Tools + Recipes back the
    // Orchestrator↔Executioner surface regardless of which LLM provider is
    // configured.
    #[cfg(feature = "postgres")]
    if let Some(pool) = services.pg_pool.clone() {
        let host_tenant_id = runtime.webui_tenant_id().to_string();
        if let Err(e) =
            crate::seed_builtin_host::seed_builtin_host_components(pool, &host_tenant_id).await
        {
            tracing::warn!(
                error = %e,
                "builtin host component seeding failed; host.* capabilities may be \
                 missing until the next restart"
            );
        }
    }

    // Wire the safety configuration store (Postgres path).
    #[cfg(feature = "postgres")]
    if let Some(safety_config_store) = &services.pg_safety_config_store {
        tracing::debug!("wiring PgSafetyConfigStore into WebUI API");
        api = api.with_safety_config_store(Arc::clone(safety_config_store)
            as Arc<dyn brassclaw_product_workflow::SafetyConfigStore>);
    }

    // Wire the token settings store (Postgres path).
    #[cfg(feature = "postgres")]
    if let Some(token_settings_store) = &services.pg_token_settings_store {
        tracing::debug!("wiring PgTokenSettingsStore into WebUI API");
        api = api.with_token_settings_store(Arc::clone(token_settings_store)
            as Arc<dyn brassclaw_product_workflow::TokenSettingsStore>);
    }

    // Wire the engine `Store` through `StoreBackedReductionRuleStore` (postgres).
    // Step 5.3: RecipeStore is now wired through PgRecipeStoreFacade (reborn_recipes table)
    // instead of the old StoreBackedRecipeStore (MemoryDoc-backed).
    #[cfg(feature = "postgres")]
    if let Some(memory_doc_store) = services.pg_memory_doc_store.clone() {
        let dyn_store: Arc<dyn brassclaw_engine::traits::store::Store> =
            Arc::clone(&memory_doc_store) as Arc<dyn brassclaw_engine::traits::store::Store>;
        let reduction_rule_store =
            crate::reduction_rules_store::StoreBackedReductionRuleStore::open(Arc::clone(
                &dyn_store,
            ));
        api = api.with_reduction_rule_store(Arc::new(reduction_rule_store)
            as Arc<dyn brassclaw_product_workflow::ReductionRuleStore>);
        api = api.with_reduction_rules_cache_invalidator(Arc::new(
            |_project_id: &str, _user_id: &str| {
                brassclaw_engine::executor::orchestrator::invalidate_reduction_rules_cache();
            },
        ));
        tracing::debug!("ReductionRuleStore wired through PgMemoryDocStore");
    }
    // Wire PgRecipeStoreFacade as the RecipeStore (reborn_recipes table).
    // Postgres is mandatory — the MemoryDoc-backed fallback has been removed.
    // When no PG pool is available the RecipeStore is not wired (recipe endpoints
    // return "unavailable") rather than silently falling back to legacy storage.
    #[cfg(feature = "postgres")]
    if let Some(pool) = services.pg_pool.as_ref() {
        let tenant_id = runtime.webui_tenant_id();
        let facade = crate::pg_recipe_store::PgRecipeStoreFacade::new(
            Arc::clone(pool),
            tenant_id,
            "default",
        );
        api =
            api.with_recipe_store(
                Arc::new(facade) as Arc<dyn brassclaw_product_workflow::RecipeStore>
            );
        tracing::debug!("RecipeStore wired through PgRecipeStoreFacade (reborn_recipes)");
    }

    // Wire the extension registry for Tools API
    if let Some(local_runtime) = &services.local_runtime {
        tracing::debug!("wiring ExtensionRegistry into WebUI API");
        api = api.with_extension_registry(Arc::clone(&local_runtime.extension_registry)
            as Arc<dyn brassclaw_host_api::CapabilityRegistry>);
    } else {
        tracing::debug!("ExtensionRegistry is None - tools endpoints will return empty list");
    }

    // Wire the interceptor configuration service (Phase 5.5, postgres +
    // root-llm-provider only).  When the pool is available, the service
    // provides snapshot/update/reassemble/prewarm over brassclaw_config.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    if let Some(pool) = services.pg_pool.clone() {
        let tenant_id = runtime.webui_tenant_id().to_string();
        let mut interceptor_svc =
            crate::interceptor_config_service::RebornInterceptorConfigService::new(pool, tenant_id);
        if let Some(mode) = runtime.interceptor_mode() {
            interceptor_svc = interceptor_svc.with_interceptor_mode(mode);
        }
        if let Some(gateway) = runtime.sempai_gateway() {
            interceptor_svc = interceptor_svc.with_sempai_gateway(gateway);
        }
        api = api.with_interceptor_config_service(Arc::new(interceptor_svc)
            as Arc<dyn brassclaw_product_workflow::InterceptorConfigService>);
    }

    // Wire the Monty VM settings store (postgres path).
    // When the pool is available, GET/PUT /api/settings/monty-vm persists to DB.
    #[cfg(feature = "postgres")]
    if let Some(pool) = services.pg_pool.as_ref() {
        let tenant_id = runtime.webui_tenant_id();
        let agent_id = runtime.webui_agent_id();
        let monty_vm_store = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            Arc::clone(pool),
            tenant_id,
            agent_id,
        );
        api = api
            .with_monty_vm_settings_store(Arc::new(monty_vm_store)
                as Arc<dyn brassclaw_product_workflow::MontyVmSettingsStore>);
        tracing::debug!("MontyVmSettingsStore wired through PgMontyVmSettingsStore");
    }

    // Wire the chat preference store (postgres path).
    // When the pool is available, PUT /api/chat/preferences/{key} persists to DB.
    #[cfg(feature = "postgres")]
    if let Some(pool) = services.pg_pool.as_ref() {
        let pref_store =
            crate::pg_user_preference_store::PgUserPreferenceStore::new(Arc::clone(pool));
        api = api.with_chat_preference_store(
            Arc::new(pref_store) as Arc<dyn brassclaw_product_workflow::ChatPreferenceStore>
        );
        tracing::debug!("ChatPreferenceStore wired through PgUserPreferenceStore");
    }

    // Wire the intent inputs store (postgres + skills-db path).
    // When the pool is available, GET/PUT/DELETE /api/settings/intent-inputs persists to DB.
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    if let Some(pool) = services.pg_pool.as_ref() {
        let tenant_id = runtime.webui_tenant_id().to_string();
        let agent_id = runtime.webui_agent_id().to_string();
        let intent_store = crate::pg_intent_inputs_store::PgIntentInputsStore::new(
            Arc::clone(pool),
            tenant_id,
            agent_id,
        );
        api = api.with_intent_inputs_store(
            Arc::new(intent_store) as Arc<dyn brassclaw_product_workflow::IntentInputsStore>
        );
        tracing::debug!("IntentInputsStore wired through PgIntentInputsStore");
    }

    Ok(RebornWebuiBundle {
        api: Arc::new(api),
        product_auth: services.product_auth.clone(),
        readiness: services.readiness,
    })
}

/// Seed (or update) builtin provider definitions into the DB.
///
/// Called on every service start — `upsert_builtin` is idempotent so existing
/// rows are updated with structural fields from the current binary's
/// `providers.json` while operator-owned fields (base_url, model, description,
/// api_key_required, token_budget) are preserved.
///
/// This ensures new builtins added in a binary upgrade are automatically
/// available after restart without requiring a manual migration.
///
/// Non-fatal: individual provider seed failures are logged as warnings and do
/// not prevent service startup.
#[cfg(feature = "postgres")]
pub(crate) async fn seed_builtin_providers(
    pg_repo: &crate::pg_provider_repo::PgProviderRepo,
) -> Result<(), crate::pg_provider_repo::PgProviderRepoError> {
    use brassclaw_llm::ProviderDefinition;
    use std::collections::HashMap;

    let registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
        .map_err(|e| crate::pg_provider_repo::PgProviderRepoError::Db(e.to_string()))?;

    // Load existing builtin rows for the Rust-side merge so we can preserve
    // operator-owned fields (base_url, model, description, etc.).
    let existing = pg_repo.load_all().await?;
    let existing_map: HashMap<String, ProviderDefinition> = existing
        .into_iter()
        .filter(|(_, is_builtin)| *is_builtin)
        .map(|(def, _)| (def.id.clone(), def))
        .collect();

    let mut seeded = 0usize;
    for new_def in registry.all() {
        // Merge: start from the new binary's canonical definition;
        // overlay the operator-owned fields from any existing builtin row.
        let merged = if let Some(existing_def) = existing_map.get(&new_def.id) {
            let mut merged = new_def.clone();
            // Operator-owned fields — preserve what the operator last set.
            // Note: api_key_required is intentionally NOT copied from the existing
            // DB row; the canonical binary definition is always authoritative for
            // whether a key is structurally required.  Copying the DB value would
            // re-seed any corruption introduced by a previous buggy save.
            merged.default_base_url = existing_def.default_base_url.clone();
            merged.default_model = existing_def.default_model.clone();
            merged.description = existing_def.description.clone();
            merged.token_budget = existing_def.token_budget.clone();
            merged
        } else {
            new_def.clone()
        };

        match pg_repo.upsert_builtin(merged).await {
            Ok(true) => seeded += 1,
            Ok(false) => tracing::warn!(
                provider_id = %new_def.id,
                "builtin provider skipped: a non-builtin row with the same id exists; \
                 the builtin will not overwrite it"
            ),
            Err(e) => tracing::warn!(
                provider_id = %new_def.id,
                error = %e,
                "failed to seed builtin provider"
            ),
        }
    }

    tracing::debug!(count = seeded, "seeded builtin LLM providers into DB");
    Ok(())
}
