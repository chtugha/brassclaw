//! Task-scoped provider preparation using the existing provider/catalogue and
//! live dispatch guard. There is no publication of old task clients as current
//! desired state, no model submission here and no second task registry.
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_llm::{ProviderRole, RetainedLlmProvider, SessionManager};
use brassclaw_loop_support::{HostManagedModelError, HostManagedModelErrorKind};
use brassclaw_pg::PgPool;
use brassclaw_reborn::model_gateway::TaskModelProviderSource;
use brassclaw_turns::run_profile::{
    AgentLoopHostError, AgentLoopHostErrorKind, LoopModelRouteSnapshot, LoopTaskModelSelection,
};

use crate::{
    LlmKeyStore, RebornProviderAdmin,
    provider_admin::{TaskCredentialSnapshot, TaskProviderSelections},
};

#[derive(Clone)]
pub(crate) struct TaskProviderConfiguration {
    pool: PgPool,
    tenant: String,
    keys: LlmKeyStore,
    primary: Option<Arc<brassclaw_llm::LlmReloadHandle>>,
    sempai: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
    #[cfg(feature = "skills-db")]
    memory: Option<Arc<brassclaw_memory::EmbeddingProviderSourceSlot>>,
    #[cfg(feature = "skills-db")]
    encoding: Option<EncodingDependencies>,
    /// Authenticated PG bridge owner for native Codex per-task credential
    /// consumption. Absent for non-root runtimes and non-Codex protocols.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    codex_native_owner: Option<Arc<crate::pg_auth_product_services::PgAuthProductServices>>,
}
#[cfg(feature = "skills-db")]
#[derive(Clone)]
struct EncodingDependencies {
    worker: Arc<crate::source_inspection::ContainedSourceInspection>,
    governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
}

impl TaskProviderConfiguration {
    pub(crate) fn new(pool: PgPool, tenant: &str, keys: LlmKeyStore) -> Self {
        Self {
            pool,
            tenant: tenant.into(),
            keys,
            primary: None,
            sempai: None,
            #[cfg(feature = "skills-db")]
            memory: None,
            #[cfg(feature = "skills-db")]
            encoding: None,
            #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
            codex_native_owner: None,
        }
    }

    /// Supply the native Codex credential owner for per-task client builds.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    pub(crate) fn with_codex_native_owner(
        mut self,
        owner: Option<Arc<crate::pg_auth_product_services::PgAuthProductServices>>,
    ) -> Self {
        self.codex_native_owner = owner;
        self
    }
    #[cfg(feature = "skills-db")]
    pub(crate) fn with_final_encoding(
        mut self,
        worker: Option<Arc<crate::source_inspection::ContainedSourceInspection>>,
        governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
    ) -> Self {
        self.encoding = worker.map(|worker| EncodingDependencies { worker, governor });
        self
    }

    #[cfg(feature = "skills-db")]
    pub(crate) fn with_memory_source(
        mut self,
        source: Option<Arc<brassclaw_memory::EmbeddingProviderSourceSlot>>,
    ) -> Self {
        self.memory = source;
        self
    }

    pub(crate) fn with_published_clients(
        mut self,
        primary: Arc<brassclaw_llm::LlmReloadHandle>,
        sempai: Arc<brassclaw_llm::SwappableLlmProvider>,
    ) -> Self {
        self.primary = Some(primary);
        self.sempai = Some(sempai);
        self
    }

    pub(crate) fn with_session(self, session: Arc<SessionManager>) -> Arc<TaskProviderSources> {
        Arc::new(TaskProviderSources {
            configuration: self,
            session,
            identity: Arc::new(()),
        })
    }
}

pub(crate) struct TaskProviderSources {
    configuration: TaskProviderConfiguration,
    session: Arc<SessionManager>,
    identity: Arc<()>,
}

impl TaskProviderSources {
    pub(crate) fn role(self: &Arc<Self>, role: ProviderRole) -> Arc<dyn TaskModelProviderSource> {
        Arc::new(TaskRoleSource {
            owner: self.clone(),
            role,
            #[cfg(feature = "skills-db")]
            review: None,
        })
    }
    #[cfg(feature = "skills-db")]
    pub(crate) fn review_role(
        self: &Arc<Self>,
        admission: ReviewAdmission,
    ) -> Arc<dyn TaskModelProviderSource> {
        Arc::new(TaskRoleSource {
            owner: self.clone(),
            role: ProviderRole::Sempai,
            review: Some(admission),
        })
    }
}

struct TaskRoleSource {
    owner: Arc<TaskProviderSources>,
    role: ProviderRole,
    #[cfg(feature = "skills-db")]
    review: Option<ReviewAdmission>,
}
#[cfg(feature = "skills-db")]
#[derive(Clone)]
pub(crate) struct ReviewAdmission {
    pub(crate) owner: uuid::Uuid,
    pub(crate) context: brassclaw_turns::run_profile::LoopRunContext,
    pub(crate) ownership: crate::global_monty_owner::GlobalOwnerCheck,
    pub(crate) closed: Arc<std::sync::atomic::AtomicBool>,
}
#[cfg(feature = "skills-db")]
impl ReviewAdmission {
    async fn check(
        &self,
        pool: &PgPool,
        binding: &str,
        model: &str,
        reference: &str,
    ) -> Result<(), brassclaw_llm::LlmError> {
        let reject = || brassclaw_llm::LlmError::FinalRequestRejected {
            reason: brassclaw_llm::final_request_guard::FinalRequestError::Binding,
        };
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(reject());
        }
        self.ownership.check().await.map_err(|_| reject())?;
        let client = pool.get().await.map_err(|_| reject())?;
        let row=client.query_opt("SELECT w.model_binding_bytes,w.model_identity,w.prefix_bytes,w.phase,e.event_bytes::jsonb->'scope' FROM brassclaw_monty_review_work w JOIN brassclaw_monty_review_events e ON e.run_id=w.source_run_id WHERE w.attempt_id=$1 AND w.owner_id=$2",
            &[&self.context.run_id.as_uuid(),&self.owner]).await.map_err(|_| reject())?.ok_or_else(reject)?;
        let event: serde_json::Value = row.get(4);
        if row.get::<_, Option<String>>(0).as_deref() != Some(binding)
            || row.get::<_, String>(1) != model
            || row.get::<_, String>(2) != reference
            || row.get::<_, String>(3) != "model_dispatching"
            || event["tenant_id"] != self.context.scope.tenant_id.as_str()
            || event["agent_id"] != serde_json::json!(self.context.scope.agent_id)
            || event["project_id"] != serde_json::json!(self.context.scope.project_id)
            || self.closed.load(std::sync::atomic::Ordering::Acquire)
        {
            tracing::warn!(
                binding_matches = row.get::<_, Option<String>>(0).as_deref() == Some(binding),
                model_matches = row.get::<_, String>(1) == model,
                prefix_matches = row.get::<_, String>(2) == reference,
                phase_matches = row.get::<_, String>(3) == "model_dispatching",
                tenant_matches = event["tenant_id"] == self.context.scope.tenant_id.as_str(),
                agent_matches = event["agent_id"] == serde_json::json!(self.context.scope.agent_id),
                project_matches =
                    event["project_id"] == serde_json::json!(self.context.scope.project_id),
                "native review model binding mismatch"
            );
            return Err(reject());
        }
        Ok(())
    }
}
#[cfg(feature = "skills-db")]
struct ReviewPolicy {
    base: Arc<dyn brassclaw_llm::ProviderDispatchGuard>,
    admission: ReviewAdmission,
    pool: PgPool,
    binding: String,
    model: String,
    reference: Arc<str>,
}
#[cfg(feature = "skills-db")]
impl std::fmt::Debug for ReviewPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReviewPolicy")
    }
}
#[cfg(feature = "skills-db")]
#[async_trait]
impl brassclaw_llm::ProviderDispatchGuard for ReviewPolicy {
    async fn check(
        &self,
        facts: brassclaw_llm::ProviderRequestFacts<'_>,
    ) -> Result<brassclaw_llm::ProviderDispatchState, brassclaw_llm::LlmError> {
        self.admission
            .check(&self.pool, &self.binding, &self.model, &self.reference)
            .await?;
        self.base.check(facts).await
    }
}
#[cfg(feature = "skills-db")]
struct TaskPrefixSource {
    binding: Arc<dyn LoopTaskModelSelection>,
    role: ProviderRole,
    policy: brassclaw_skills::recipe_model_input::RecipeModelInput,
}
/// Lazily resolves only when a dependent model usage actually needs its prefix.
/// Model-free tasks are not blocked by an absent provider or ungenerated prefix.
#[cfg(feature = "skills-db")]
pub(crate) fn recipe_prefix(
    host: &brassclaw_reborn::monty_task_host::MontyTaskHost,
    role: ProviderRole,
) -> Option<brassclaw_reborn::monty_model_capability::RecipeModelPrefix> {
    host.task_model_selection()
        .map(|binding| prefix_from_binding(binding, role))
}
#[cfg(feature = "skills-db")]
pub(crate) fn prefix_from_binding(
    binding: Arc<dyn LoopTaskModelSelection>,
    role: ProviderRole,
) -> brassclaw_reborn::monty_model_capability::RecipeModelPrefix {
    prefix_from_policy(
        binding,
        role,
        brassclaw_skills::recipe_model_input::RecipeModelInput::Default,
    )
}
#[cfg(feature = "skills-db")]
pub(crate) fn prefix_from_policy(
    binding: Arc<dyn LoopTaskModelSelection>,
    role: ProviderRole,
    policy: brassclaw_skills::recipe_model_input::RecipeModelInput,
) -> brassclaw_reborn::monty_model_capability::RecipeModelPrefix {
    brassclaw_reborn::monty_model_capability::RecipeModelPrefix::at_final_boundary(Arc::new(
        TaskPrefixSource {
            binding,
            role,
            policy,
        },
    ))
}
#[cfg(feature = "skills-db")]
pub(crate) fn recipe_prefix_for_instruction(
    host: &brassclaw_reborn::monty_task_host::MontyTaskHost,
    role: ProviderRole,
    instruction: &brassclaw_engine::memory::retained_instruction::RetainedRecipeInstruction,
) -> Result<Option<brassclaw_reborn::monty_model_capability::RecipeModelPrefix>, AgentLoopHostError>
{
    use brassclaw_skills::recipe_model_input::RecipeModelInput;
    if !crate::global_recipe_ports::model_input_supported(instruction) {
        return Err(host_unavailable());
    }
    match host.task_model_selection() {
        Some(binding) => Ok(Some(prefix_from_policy(
            binding,
            role,
            instruction.model_input().clone(),
        ))),
        None if instruction.model_input() == &RecipeModelInput::Default => Ok(None),
        None => Err(host_unavailable()),
    }
}
#[cfg(feature = "skills-db")]
fn role_profile(role: ProviderRole) -> Result<&'static str, HostManagedModelError> {
    match role {
        ProviderRole::Kohai => Ok("base-prompt"),
        ProviderRole::Sempai => Ok("sempai"),
        _ => Err(unavailable()),
    }
}
#[cfg(feature = "skills-db")]
#[async_trait]
impl brassclaw_reborn::monty_model_capability::RecipeModelPrefixSource for TaskPrefixSource {
    async fn resolve(
        &self,
    ) -> Result<
        brassclaw_reborn::monty_model_capability::ResolvedRecipeModelPrefix,
        AgentLoopHostError,
    > {
        let selected = self
            .binding
            .as_any()
            .downcast_ref::<TaskSelection>()
            .ok_or_else(host_unavailable)?;
        let configuration = &selected.owner.configuration;
        let route = selected
            .snapshot
            .role_model_route(self.role)
            .map_err(|_| host_unavailable())?;
        use brassclaw_skills::recipe_model_input::RecipeModelInput;
        use brassclaw_turns::run_profile::RecipePrefixSelection;
        let (profile, choice) = match &self.policy {
            RecipeModelInput::Default => (
                role_profile(self.role).map_err(|_| host_unavailable())?,
                RecipePrefixSelection::Default,
            ),
            RecipeModelInput::Selected { profile } => {
                (profile.as_str(), RecipePrefixSelection::Selected)
            }
            RecipeModelInput::PrefixFreeWorkflow { .. } => return Err(host_unavailable()),
        };
        let id = *selected
            .prefixes
            .get(&(
                route.provider_id.clone(),
                route.model_id.clone(),
                profile.into(),
            ))
            .ok_or_else(host_unavailable)?;
        let prepared = configuration
            .encoding
            .as_ref()
            .ok_or_else(host_unavailable)?
            .worker
            .prepare()
            .await
            .map_err(|_| host_unavailable())?;
        let owner = crate::prefix_acceptances::PgPrefixAcceptances::new(
            Arc::new(configuration.pool.clone()),
            configuration.tenant.clone(),
        );
        let selection = owner
            .resolve(id, prepared.revision)
            .await
            .map_err(|_| host_unavailable())?;
        if selection.provider_id != route.provider_id
            || selection.model != route.model_id
            || selection.profile != profile
            || selection.worker_sha256 != hex::encode(prepared.worker.checksum())
        {
            return Err(host_unavailable());
        }
        let content = owner
            .reference(id, prepared.revision)
            .await
            .map_err(|_| host_unavailable())?;
        brassclaw_reborn::monty_model_capability::ResolvedRecipeModelPrefix::prefix(
            generation_reference(profile, &selection.generation, id),
            content,
            choice,
            brassclaw_reborn::monty_model_capability::RecipePrefixInsertion::FinalBoundary,
        )
    }
}

// Mutable client/credential caches are private to one original task family.
// Only an authenticated native parent handoff shares this object with children.
struct TaskSelection {
    owner: Arc<TaskProviderSources>,
    snapshot: TaskProviderSelections,
    #[cfg(feature = "skills-db")]
    prefix_capture: Option<crate::task_model_capture::PrefixFreeCaptureRecord>,
    prepared: tokio::sync::Mutex<Prepared>,
    #[cfg(feature = "skills-db")]
    prefixes: BTreeMap<(String, String, String), uuid::Uuid>,
}

#[cfg(feature = "skills-db")]
fn generation_reference(profile: &str, generation: &str, qualification: uuid::Uuid) -> String {
    format!("prefix-generation:{profile}:{generation}:qualification:{qualification}")
}

/// Resolve only the original task's captured map. The suffix is not approval:
/// the caller independently resolves the immutable qualification and checks the
/// entire canonical reference, exact pair and profile before using any content.
#[cfg(feature = "skills-db")]
fn original_prefix(
    selected: &TaskSelection,
    role: ProviderRole,
    provider: &str,
    model: &str,
    input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
) -> Result<
    (
        String,
        uuid::Uuid,
        brassclaw_turns::run_profile::RecipePrefixSelection,
    ),
    HostManagedModelError,
> {
    use brassclaw_turns::run_profile::RecipePrefixSelection;
    let choice = input.map_or(RecipePrefixSelection::Default, |input| input.selection);
    match choice {
        RecipePrefixSelection::PrefixFreeWorkflow => Err(unavailable()),
        RecipePrefixSelection::Default => {
            let profile = role_profile(role)?.to_owned();
            let id = *selected
                .prefixes
                .get(&(provider.to_owned(), model.to_owned(), profile.clone()))
                .ok_or_else(unavailable)?;
            Ok((profile, id, choice))
        }
        RecipePrefixSelection::Selected => {
            let input = input.ok_or_else(unavailable)?;
            let (_, suffix) = input
                .reference
                .rsplit_once(":qualification:")
                .ok_or_else(unavailable)?;
            let id = uuid::Uuid::parse_str(suffix).map_err(|_| unavailable())?;
            let mut matches = selected
                .prefixes
                .iter()
                .filter(|((p, m, _), selected)| p == provider && m == model && **selected == id);
            let ((_, _, profile), _) = matches.next().ok_or_else(unavailable)?;
            if matches.next().is_some() {
                return Err(unavailable());
            }
            Ok((profile.clone(), id, choice))
        }
    }
}

#[derive(Default)]
struct Prepared {
    credentials: BTreeMap<String, Arc<TaskCredentialSnapshot>>,
    roles: BTreeMap<&'static str, PreparedModel>,
    #[cfg(feature = "skills-db")]
    embedding: Option<PreparedTaskEmbedding>,
}
struct PreparedModel {
    retained: RetainedLlmProvider,
    #[cfg(feature = "skills-db")]
    guard: Arc<dyn brassclaw_llm::ProviderDispatchGuard>,
    #[cfg(feature = "skills-db")]
    endpoint: Option<reqwest::Url>,
}

#[cfg(feature = "skills-db")]
struct PreparedTaskEmbedding {
    provider: Arc<dyn brassclaw_memory::EmbeddingProvider>,
    guard: Arc<dyn brassclaw_embeddings::EmbeddingDispatchGuard>,
    // Only task-private clients release their cache when the family ends.
    // Shared published caches retain their existing source owner's lifecycle.
    private_cache: Option<Arc<brassclaw_embeddings::CachedEmbeddingProvider>>,
}

#[cfg(feature = "skills-db")]
impl Drop for PreparedTaskEmbedding {
    fn drop(&mut self) {
        if let Some(cache) = &self.private_cache {
            cache.retire();
        }
    }
}

fn unavailable() -> HostManagedModelError {
    HostManagedModelError::safe(
        HostManagedModelErrorKind::ConfigurationError,
        "original task provider selection is unavailable",
    )
}
#[cfg(feature = "skills-db")]
fn binding_unavailable(stage: &'static str) -> HostManagedModelError {
    tracing::warn!(
        stage,
        "native model-prefix binding rejected before submission"
    );
    unavailable()
}
fn host_unavailable() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "original task provider selection is unavailable",
    )
}

#[cfg(feature = "skills-db")]
fn embedding_unavailable() -> brassclaw_memory::EmbeddingError {
    brassclaw_memory::EmbeddingError::ProviderUnavailable {
        reason: "original task embedding selection is unavailable".into(),
    }
}

/// Original family client only. This performs no embedding inference and never
/// reselects the current role. The caller must also retain the task/scope fence.
#[cfg(feature = "skills-db")]
pub(crate) async fn task_embedding_provider(
    host: &brassclaw_reborn::monty_task_host::MontyTaskHost,
) -> Result<Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>, brassclaw_memory::EmbeddingError>
{
    host.check_dispatch_open()
        .map_err(|_| embedding_unavailable())?;
    let Some(binding) = host.task_model_selection() else {
        // Explicit immutable/qualification hosts without model dependencies
        // cannot gain a current global embedding provider.
        return Ok(None);
    };
    if binding
        .as_any()
        .is::<brassclaw_turns::run_profile::InheritedUnavailableModelSelection>()
    {
        return Ok(None);
    }
    let selected = binding
        .as_any()
        .downcast_ref::<TaskSelection>()
        .ok_or_else(embedding_unavailable)?;
    selected
        .snapshot
        .validate(host.run_context().scope.tenant_id.as_str())
        .map_err(|_| embedding_unavailable())?;
    let provider = prepare_embedding(selected).await?;
    host.check_dispatch_open()
        .map_err(|_| embedding_unavailable())?;
    Ok(provider)
}

#[cfg(feature = "skills-db")]
async fn prepare_embedding(
    selected: &TaskSelection,
) -> Result<Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>, brassclaw_memory::EmbeddingError>
{
    use brassclaw_memory::EmbeddingProviderSource as _;
    let configuration = &selected.owner.configuration;
    if !selected.snapshot.role_configured("embedding") {
        return Ok(None);
    }
    let mut prepared = selected.prepared.lock().await;
    if prepared.embedding.is_none() {
        let credential = selected
            .snapshot
            .provider_id(ProviderRole::Embedding)
            .and_then(|id| prepared.credentials.get(id))
            .cloned();
        let retained = RebornProviderAdmin::new(configuration.pool.clone(), &configuration.tenant)
            .prepare_task_provider(
                &selected.snapshot,
                ProviderRole::Embedding,
                &configuration.keys,
                credential,
            )
            .await
            .map_err(|_| embedding_unavailable())?;
        let captured = retained.captured;
        prepared
            .credentials
            .insert(captured.revision.provider_id.clone(), retained.credential);
        let model = captured.revision.model.clone();
        let dimension = usize::try_from(
            captured
                .definition
                .embedding_models
                .0
                .get(&model)
                .ok_or_else(embedding_unavailable)?
                .dimension
                .get(),
        )
        .map_err(|_| embedding_unavailable())?;
        let guard: Arc<dyn brassclaw_embeddings::EmbeddingDispatchGuard> =
            crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
                configuration.pool.clone(),
                &configuration.tenant,
                configuration.keys.clone(),
                &captured,
            )
            .map_err(|_| embedding_unavailable())?;
        guard.check().await.map_err(|_| embedding_unavailable())?;
        let shared = match &configuration.memory {
            Some(source) => {
                source
                    .retained(&captured.revision.configuration_ref)
                    .await?
            }
            None => None,
        };
        let (provider, private_cache) = match shared {
            Some(provider) => (provider, None),
            None => {
                let client = crate::factory::build_captured_embedding_provider(
                    &configuration.tenant,
                    captured,
                    selected.owner.session.clone(),
                    guard.clone(),
                )
                .await
                .map_err(|_| embedding_unavailable())?;
                (client.provider, Some(client.cache))
            }
        };
        if provider.model_name() != model || provider.dimension() != dimension {
            if let Some(cache) = private_cache {
                cache.retire();
            }
            return Err(embedding_unavailable());
        }
        prepared.embedding = Some(PreparedTaskEmbedding {
            provider,
            guard,
            private_cache,
        });
    }
    let embedding = prepared
        .embedding
        .as_ref()
        .ok_or_else(embedding_unavailable)?;
    embedding
        .guard
        .check()
        .await
        .map_err(|_| embedding_unavailable())?;
    Ok(Some(embedding.provider.clone()))
}

impl LoopTaskModelSelection for TaskSelection {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_metadata(&self) -> Result<serde_json::Value, AgentLoopHostError> {
        #[cfg(feature = "skills-db")]
        let prefixes: Vec<_> = self.prefixes.iter().map(|((provider,model,profile),id)|
            serde_json::json!({"provider":provider,"model":model,"profile":profile,"qualification":id})).collect();
        #[cfg(not(feature = "skills-db"))]
        let prefixes = Vec::<serde_json::Value>::new();
        let mut document = serde_json::json!({
            "selection": serde_json::to_value(&self.snapshot).map_err(|_| host_unavailable())?,
            "prefix_bindings": prefixes,
        });
        document.sort_all_objects();
        use sha2::Digest;
        #[cfg(feature = "skills-db")]
        let capture = self
            .prefix_capture
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| host_unavailable())?;
        #[cfg(not(feature = "skills-db"))]
        let capture: Option<serde_json::Value> = None;
        let mut subject = serde_json::json!([
            self.snapshot.checksum().map_err(|_| host_unavailable())?,
            document["prefix_bindings"],
        ]);
        let mut format = "brassclaw-task-provider-binding/2";
        if let Some(mut capture) = capture {
            #[cfg(feature = "skills-db")]
            if !self.prefixes.is_empty() {
                return Err(host_unavailable());
            }
            capture.sort_all_objects();
            subject
                .as_array_mut()
                .ok_or_else(host_unavailable)?
                .push(capture.clone());
            document["prefix_capture"] = capture;
            format = "brassclaw-task-provider-binding/3";
        }
        let checksum = hex::encode(sha2::Sha256::digest(
            serde_json::to_vec(&subject).map_err(|_| host_unavailable())?,
        ));
        document["format"] = serde_json::json!(format);
        document["checksum"] = serde_json::json!(checksum);
        Ok(document)
    }
    fn matches_capture_context(
        &self,
        capture: &dyn brassclaw_turns::run_profile::LoopModelCaptureContext,
    ) -> bool {
        #[cfg(feature = "skills-db")]
        return capture
            .as_any()
            .downcast_ref::<crate::task_model_capture::PrefixFreeCapture>()
            .and_then(|capture| capture.record(&self.owner.configuration.tenant).ok())
            .is_some_and(|record| self.prefix_capture.as_ref() == Some(&record));
        #[cfg(not(feature = "skills-db"))]
        {
            let _ = capture;
            false
        }
    }
    fn model_route(&self) -> Result<LoopModelRouteSnapshot, AgentLoopHostError> {
        self.snapshot.model_route().map_err(|_| host_unavailable())
    }
    fn role_model_route(&self, role: &str) -> Result<LoopModelRouteSnapshot, AgentLoopHostError> {
        let role = match role {
            "kohai" => ProviderRole::Kohai,
            "sempai" => ProviderRole::Sempai,
            _ => return Err(host_unavailable()),
        };
        self.snapshot
            .role_model_route(role)
            .map_err(|_| host_unavailable())
    }

    fn role_configured(&self, role: &str) -> bool {
        self.snapshot.role_configured(role)
    }
}

#[async_trait]
impl TaskModelProviderSource for TaskRoleSource {
    async fn capture(
        &self,
        parent: Option<Arc<dyn LoopTaskModelSelection>>,
        capture: Option<Arc<dyn brassclaw_turns::run_profile::LoopModelCaptureContext>>,
    ) -> Result<Arc<dyn LoopTaskModelSelection>, HostManagedModelError> {
        #[cfg(feature = "skills-db")]
        let prefix_capture = capture
            .as_ref()
            .map(|capture| {
                capture
                    .as_any()
                    .downcast_ref::<crate::task_model_capture::PrefixFreeCapture>()
                    .ok_or_else(unavailable)?
                    .record(&self.owner.configuration.tenant)
                    .map_err(|_| unavailable())
            })
            .transpose()?;
        #[cfg(not(feature = "skills-db"))]
        if capture.is_some() {
            return Err(unavailable());
        }
        if let Some(parent) = parent {
            if capture
                .as_ref()
                .is_some_and(|capture| !parent.matches_capture_context(capture.as_ref()))
            {
                return Err(unavailable());
            }
            if parent
                .as_any()
                .is::<brassclaw_turns::run_profile::InheritedUnavailableModelSelection>()
            {
                // Less capability, never a new role selection or an SDK client.
                return Ok(parent);
            }
            let selected = parent
                .as_any()
                .downcast_ref::<TaskSelection>()
                .ok_or_else(unavailable)?;
            if !Arc::ptr_eq(&selected.owner.identity, &self.owner.identity) {
                return Err(unavailable());
            }
            selected
                .snapshot
                .validate(&self.owner.configuration.tenant)
                .map_err(|_| unavailable())?;
            return Ok(parent);
        }
        let configuration = &self.owner.configuration;
        let snapshot = RebornProviderAdmin::new(configuration.pool.clone(), &configuration.tenant)
            .capture_task_provider_selections(&configuration.keys)
            .await
            .map_err(|_| unavailable())?;
        #[cfg(feature = "skills-db")]
        let prefixes = if prefix_capture.is_some() {
            BTreeMap::new()
        } else {
            crate::prefix_acceptances::PgPrefixAcceptances::new(
                Arc::new(configuration.pool.clone()),
                configuration.tenant.clone(),
            )
            .bindings()
            .await
            .map_err(|_| unavailable())?
        };
        Ok(Arc::new(TaskSelection {
            owner: self.owner.clone(),
            snapshot,
            #[cfg(feature = "skills-db")]
            prefix_capture,
            prepared: tokio::sync::Mutex::new(Prepared::default()),
            #[cfg(feature = "skills-db")]
            prefixes,
        }))
    }

    async fn prepare(
        &self,
        selection: &Arc<dyn LoopTaskModelSelection>,
    ) -> Result<RetainedLlmProvider, HostManagedModelError> {
        let selected = selection
            .as_any()
            .downcast_ref::<TaskSelection>()
            .ok_or_else(unavailable)?;
        if !Arc::ptr_eq(&selected.owner.identity, &self.owner.identity) {
            return Err(unavailable());
        }
        let role = match self.role {
            ProviderRole::Kohai => "kohai",
            ProviderRole::Sempai => "sempai",
            ProviderRole::Embedding => return Err(unavailable()),
        };
        let mut prepared = selected.prepared.lock().await;
        if let Some(provider) = prepared.roles.get(role) {
            return Ok(provider.retained.clone());
        }
        let configuration = &self.owner.configuration;
        let admin = RebornProviderAdmin::new(configuration.pool.clone(), &configuration.tenant);
        // All role preparation is serialized within this task family. Find a
        // previously authenticated same-provider credential, including our own
        // trusted legacy envelope upgrade, without reading a replacement role.
        let credential = selected
            .snapshot
            .provider_id(self.role)
            .and_then(|id| prepared.credentials.get(id))
            .cloned();
        let retained = admin
            .prepare_task_provider(
                &selected.snapshot,
                self.role,
                &configuration.keys,
                credential,
            )
            .await
            .map_err(|_| unavailable())?;
        let provider_id = retained.captured.revision.provider_id.clone();
        // Keep the authenticated origin/live evidence before later fallible
        // config/client preparation. A failure cannot lose our migration proof.
        prepared
            .credentials
            .insert(provider_id.clone(), retained.credential);
        #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
        let codex_native_owner = configuration.codex_native_owner.clone();
        #[cfg(not(all(feature = "postgres", feature = "root-llm-provider")))]
        let codex_native_owner: Option<
            std::sync::Arc<crate::pg_auth_product_services::PgAuthProductServices>,
        > = None;
        let resolved = crate::llm_catalog::resolve_captured_provider_selection(
            &configuration.pool,
            &configuration.tenant,
            &configuration.keys,
            retained.captured,
            codex_native_owner,
        )
        .map_err(|_| unavailable())?;
        let model = resolved.revision.model.clone();
        #[cfg(feature = "skills-db")]
        let endpoint = resolved
            .config
            .provider
            .as_ref()
            .and_then(|config| brassclaw_llm::final_chat_endpoint(config).ok());
        #[cfg(feature = "skills-db")]
        let final_policy = resolved.guard.clone();
        let publication = brassclaw_llm::ProviderPublication::Configured {
            selection: resolved.revision,
        };
        let current = match self.role {
            ProviderRole::Kohai => configuration
                .primary
                .as_ref()
                .map(|slot| slot.published_primary_provider()),
            ProviderRole::Sempai => configuration
                .sempai
                .as_ref()
                .map(|slot| slot.published_provider()),
            ProviderRole::Embedding => None,
        };
        // Reuse the existing real client/cache/circuit when it is exactly this
        // selection. A historical task may build a private old client, but must
        // never publish that client over a later desired/effective revision.
        let provider = match current {
            Some((provider, actual)) if actual == publication => provider,
            _ => match resolved.codex_host_source {
                Some(source) => brassclaw_llm::build_static_provider_chain_with_codex_host_session(
                    &resolved.config,
                    self.owner.session.clone(),
                    resolved.guard,
                    source,
                )
                .await
                .map_err(|_| unavailable())?,
                None => brassclaw_llm::build_static_provider_chain_with_guard(
                    &resolved.config,
                    self.owner.session.clone(),
                    resolved.guard,
                )
                .await
                .map_err(|_| unavailable())?,
            },
        };
        let retained = RetainedLlmProvider {
            provider,
            provider_id,
            model,
            publication,
        };
        prepared.roles.insert(
            role,
            PreparedModel {
                retained: retained.clone(),
                #[cfg(feature = "skills-db")]
                guard: final_policy,
                #[cfg(feature = "skills-db")]
                endpoint,
            },
        );
        Ok(retained)
    }
    #[cfg(feature = "skills-db")]
    async fn preflight_compatibility(
        &self,
        binding: &Arc<dyn LoopTaskModelSelection>,
        max_output_tokens: Option<u32>,
        input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
    ) -> Result<(), HostManagedModelError> {
        let retained = self.prepare(binding).await?;
        let selected = binding
            .as_any()
            .downcast_ref::<TaskSelection>()
            .ok_or_else(unavailable)?;
        let role = match self.role {
            ProviderRole::Kohai => "kohai",
            ProviderRole::Sempai => "sempai",
            _ => return Err(unavailable()),
        };
        let (profile, _, choice) = original_prefix(
            selected,
            self.role,
            &retained.provider_id,
            &retained.model,
            input,
        )?;
        let policy = match choice {
            brassclaw_turns::run_profile::RecipePrefixSelection::Default => {
                brassclaw_skills::recipe_model_input::RecipeModelInput::Default
            }
            brassclaw_turns::run_profile::RecipePrefixSelection::Selected => {
                brassclaw_skills::recipe_model_input::RecipeModelInput::Selected { profile }
            }
            brassclaw_turns::run_profile::RecipePrefixSelection::PrefixFreeWorkflow => {
                return Err(unavailable());
            }
        };
        let resolved = prefix_from_policy(binding.clone(), self.role, policy)
            .resolve()
            .await
            .map_err(|_| unavailable())?;
        if input.is_some_and(|input| input != &resolved.input_reference()) {
            return Err(unavailable());
        }
        let guard = selected
            .prepared
            .lock()
            .await
            .roles
            .get(role)
            .ok_or_else(unavailable)?
            .guard
            .clone();
        let state = guard
            .check_preparation(brassclaw_llm::ProviderRequestFacts {
                model: &retained.model,
                max_output_tokens,
            })
            .await
            .map_err(|_| unavailable())?;
        if state.provider_id != retained.provider_id
            || state.model != retained.model
            || state.model_token_limits.is_none()
            || state.configured_context_window_tokens.is_none()
        {
            return Err(unavailable());
        }
        Ok(())
    }
    #[cfg(feature = "skills-db")]
    async fn final_request_guard(
        &self,
        binding: &Arc<dyn LoopTaskModelSelection>,
        request: &brassclaw_loop_support::HostManagedModelRequest,
        input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
    ) -> Result<brassclaw_reborn::model_gateway::TaskFinalModelRequest, HostManagedModelError> {
        let selected = binding
            .as_any()
            .downcast_ref::<TaskSelection>()
            .ok_or_else(unavailable)?;
        if !Arc::ptr_eq(&selected.owner.identity, &self.owner.identity) {
            return Err(unavailable());
        }
        let retained = self
            .prepare(binding)
            .await
            .map_err(|_| binding_unavailable("prefix-client-preparation"))?;
        let role = match self.role {
            ProviderRole::Kohai => "kohai",
            ProviderRole::Sempai => "sempai",
            _ => return Err(unavailable()),
        };
        let (profile, id, choice) = original_prefix(
            selected,
            self.role,
            &retained.provider_id,
            &retained.model,
            input,
        )?;
        let configuration = &self.owner.configuration;
        let encoding = configuration.encoding.as_ref().ok_or_else(unavailable)?;
        let prepared = encoding
            .worker
            .prepare()
            .await
            .map_err(|_| binding_unavailable("prefix-worker-preparation"))?;
        let owner = Arc::new(crate::prefix_acceptances::PgPrefixAcceptances::new(
            Arc::new(configuration.pool.clone()),
            configuration.tenant.clone(),
        ));
        let selection = owner
            .resolve(id, prepared.revision)
            .await
            .map_err(|_| binding_unavailable("prefix-acceptance-resolution"))?;
        let endpoint = owner
            .endpoint(id)
            .await
            .map_err(|_| binding_unavailable("prefix-accepted-endpoint"))?;
        let reference = owner
            .reference(id, prepared.revision)
            .await
            .map_err(|_| binding_unavailable("prefix-original-reference"))?;
        let request_output_tokens = owner
            .request_output_tokens(id)
            .await
            .map_err(|_| binding_unavailable("prefix-output-reservation"))?;
        if selection.provider_id != retained.provider_id
            || selection.model != retained.model
            || selection.profile != profile
            || selection.worker_sha256 != hex::encode(prepared.worker.checksum())
        {
            return Err(unavailable());
        }
        let binding_bytes =
            serde_json::to_string(&binding.retained_metadata().map_err(|_| unavailable())?)
                .map_err(|_| unavailable())?;
        let (scope, user_id) = match &self.review {
            Some(review) => {
                if request.run_id != review.context.run_id
                    || request.turn_id != review.context.turn_id
                {
                    return Err(unavailable());
                }
                review
                    .check(
                        &configuration.pool,
                        &binding_bytes,
                        &retained.model,
                        &reference,
                    )
                    .await
                    .map_err(|_| binding_unavailable("prefix-review-admission"))?;
                (
                    review.context.scope.clone(),
                    review
                        .context
                        .actor
                        .as_ref()
                        .map(|actor| actor.user_id.clone())
                        .or_else(|| review.context.scope.explicit_owner_user_id().cloned())
                        .ok_or_else(unavailable)?,
                )
            }
            None => {
                let client = configuration.pool.get().await.map_err(|_| unavailable())?;
                let row = client.query_opt("SELECT scope,turn_id,phase,task_model_selection,native_actor FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                    &[&request.run_id.as_uuid()]).await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
                let scope: brassclaw_turns::TurnScope =
                    serde_json::from_value(row.get(0)).map_err(|_| unavailable())?;
                if scope.tenant_id.as_str() != configuration.tenant
                    || row.get::<_, uuid::Uuid>(1) != request.turn_id.as_uuid()
                    || row.get::<_, String>(2) != "started"
                    || row.get::<_, Option<serde_json::Value>>(3)
                        != Some(binding.retained_metadata().map_err(|_| unavailable())?)
                {
                    return Err(unavailable());
                }
                let actor: Option<brassclaw_turns::TurnActor> = serde_json::from_value(
                    row.get::<_, Option<serde_json::Value>>(4)
                        .ok_or_else(unavailable)?,
                )
                .map_err(|_| unavailable())?;
                let user = actor
                    .map(|actor| actor.user_id)
                    .or_else(|| scope.explicit_owner_user_id().cloned())
                    .ok_or_else(unavailable)?;
                (scope, user)
            }
        };
        let scope = brassclaw_host_api::ResourceScope {
            tenant_id: scope.tenant_id.clone(),
            user_id,
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            thread_id: Some(scope.thread_id),
            // This is the contained measurement's resource invocation, never a
            // replacement model work identity, attempt or deduplication key.
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        let model = selected.prepared.lock().await;
        let model = model.roles.get(role).ok_or_else(unavailable)?;
        if model.endpoint.as_ref() != Some(&endpoint) {
            return Err(binding_unavailable("prefix-client-endpoint"));
        }
        let policy: Arc<dyn brassclaw_llm::ProviderDispatchGuard> = match &self.review {
            Some(review) => Arc::new(ReviewPolicy {
                base: model.guard.clone(),
                admission: review.clone(),
                pool: configuration.pool.clone(),
                binding: binding_bytes,
                model: retained.model.clone(),
                reference: reference.clone(),
            }),
            None => model.guard.clone(),
        };
        let binding_reference = generation_reference(&profile, &selection.generation, id);
        if input.is_some_and(|input| input.reference != binding_reference) {
            return Err(unavailable());
        }
        let counter = crate::final_model_encoding::ContainedFinalRequestMeasurer::new(
            selection,
            scope,
            Arc::new(crate::model_encoding::ContainedModelEncoding::new(
                Arc::new(configuration.pool.clone()),
                encoding.worker.clone(),
            )),
            owner,
            encoding.governor.clone(),
        )
        .map_err(|_| binding_unavailable("prefix-counter-construction"))?;
        let guard = brassclaw_llm::final_request_guard::FinalRequestGuard::new(
            retained.provider_id,
            retained.model,
            endpoint,
            policy,
            Arc::new(counter),
        )
        .map(Arc::new)
        .map_err(|_| binding_unavailable("prefix-guard-construction"))?;
        Ok(brassclaw_reborn::model_gateway::TaskFinalModelRequest {
            guard,
            binding: brassclaw_turns::run_profile::LoopModelInputReference {
                selection: choice,
                reference: binding_reference,
            },
            input: brassclaw_reborn::model_gateway::TaskFinalModelInput::Prefix { reference },
            request_output_tokens: Some(
                std::num::NonZeroU32::new(request_output_tokens)
                    .ok_or_else(|| binding_unavailable("prefix-output-reservation"))?,
            ),
        })
    }
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "task_model_provider_tests.rs"]
pub(crate) mod tests;
