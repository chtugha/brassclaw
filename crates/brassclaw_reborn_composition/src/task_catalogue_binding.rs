//! Fresh task-local bindings against an already captured immutable view.
//!
//! Selection/integrity stay with the catalogue owner. This constructor never
//! reads latest, matches intent, executes a Tool or copies another task's ports.
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::retained_recipe::RetainedProgram, memory::intent_system::IntentResolution,
};
use brassclaw_host_api::{CapabilityId, MountView};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_turns::run_profile::AgentLoopDriverError;
use sha2::{Digest, Sha256};

use super::{InstalledMontyCatalogue, SelectedTools, TaskCatalogue, failed};
use crate::{
    authored_monty_catalogue::CatalogueGeneration,
    global_recipe_ports::{MontyTaskCatalogue, SelectedMontyRecipe},
    global_task_factory::CapturedMontyCatalogue,
    monty_kernel::PgSelectedToolPolicy,
    pg_monty_admission::{CatalogueReference, PgMontyAdmission},
};

/// Immutable selection and original native address. No runtimes or prefix
/// fallback are created until the model/family capture has completed.
struct CapturedView {
    owner: Arc<InstalledMontyCatalogue>,
    host: std::sync::Weak<MontyTaskHost>,
    admission: std::sync::Weak<PgMontyAdmission>,
    view: Arc<CatalogueGeneration>,
    reference: CatalogueReference,
    resolution: Option<IntentResolution>,
    query: String,
    bound: std::sync::atomic::AtomicBool,
}
#[async_trait]
impl CapturedMontyCatalogue for CapturedView {
    fn selected_workflow(
        &self,
    ) -> Result<
        Option<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>>,
        AgentLoopDriverError,
    > {
        let Some(resolution) = self.resolution.as_ref() else {
            return Ok(None);
        };
        super::program_for_committed_match(
            std::iter::once(self.owner.reply.as_ref())
                .chain(self.view.programs.iter().map(Arc::as_ref)),
            resolution,
        )
        .map_err(|_| failed())?
        .map(|program| {
            std::iter::once(&self.owner.reply)
                .chain(self.view.programs.iter())
                .find(|p| std::ptr::eq(p.as_ref(), program))
                .cloned()
                .ok_or_else(failed)
        })
        .transpose()
    }
    fn captured_programs(
        &self,
    ) -> Vec<Arc<brassclaw_engine::executor::retained_source::InspectedRetainedProgram>> {
        std::iter::once(self.owner.reply.clone())
            .chain(std::iter::once(self.owner.history.clone()))
            .chain(self.view.programs.iter().cloned())
            .collect()
    }
    async fn bind(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError> {
        if !Arc::ptr_eq(&self.host.upgrade().ok_or_else(failed)?, &host)
            || !Arc::ptr_eq(&self.admission.upgrade().ok_or_else(failed)?, &admission)
        {
            return Err(failed());
        }
        if !admission.task_models_captured() {
            return Err(failed());
        }
        if self.bound.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err(failed());
        }
        // Passive binding must remain possible after cancellation so the
        // existing service can admit/acknowledge that exact cancelled task.
        // Actual dispatch still checks the task fence and live kernel policy.
        let programs = self.captured_programs();
        let credentials = crate::task_tool_credentials::CapturedToolCredentials::capture(
            &host,
            self.owner.kernel.as_ref(),
            self.owner.tool_credential_sources.get().cloned(),
            &programs,
        )
        .await?;
        self.owner
            .bind_captured_view(
                host,
                admission,
                self.view.clone(),
                self.reference,
                (self.resolution.clone(), self.query.clone()),
                credentials,
            )
            .map(|task| task as Arc<dyn MontyTaskCatalogue>)
    }
}

impl InstalledMontyCatalogue {
    pub(super) fn prepare_captured_view(
        self: &Arc<Self>,
        host: &Arc<MontyTaskHost>,
        admission: &Arc<PgMontyAdmission>,
        view: Arc<CatalogueGeneration>,
        resolution: Option<IntentResolution>,
        query: String,
    ) -> Arc<dyn CapturedMontyCatalogue> {
        let reference = CatalogueReference {
            id: view.generation,
            checksum: Sha256::digest(view.bytes.as_bytes()).into(),
        };
        Arc::new(CapturedView {
            owner: self.clone(),
            host: Arc::downgrade(host),
            admission: Arc::downgrade(admission),
            view,
            reference,
            resolution,
            query,
            bound: std::sync::atomic::AtomicBool::new(false),
        })
    }
    /// The caller already owns the coherent selection and its provenance.
    /// Definitions are retained; every policy, response owner, answer map and
    /// execution port is bound anew to this exact host/admission. This helper
    /// supplies no child admission or approval and cannot validate provenance.
    pub(super) fn bind_captured_view(
        self: &Arc<Self>,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        view: Arc<CatalogueGeneration>,
        reference: CatalogueReference,
        (resolution, query): (Option<IntentResolution>, String),
        credentials: crate::task_tool_credentials::CapturedToolCredentials,
    ) -> Result<Arc<TaskCatalogue>, AgentLoopDriverError> {
        if !admission.task_models_captured() {
            return Err(failed());
        }
        let mut selections = Vec::new();
        let model_prefix_reference = std::sync::OnceLock::new();
        #[cfg(feature = "root-llm-provider")]
        let target_prefix =
            crate::task_model_provider::recipe_prefix(&host, brassclaw_llm::ProviderRole::Kohai);
        #[cfg(not(feature = "root-llm-provider"))]
        let target_prefix: Option<
            brassclaw_reborn::monty_model_capability::RecipeModelPrefix,
        > = None;
        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
        {
            let RetainedProgram::Tools(program) = selected.program() else {
                return Err(failed());
            };
            if !crate::global_recipe_ports::model_input_supported(program.inputs().instruction()) {
                return Err(failed());
            }
            let requires_model = program
                .bindings()
                .values()
                .any(|binding| binding.capability_id() == "host.kohai_complete");
            #[cfg(feature = "root-llm-provider")]
            let consumer_prefix = if requires_model {
                crate::task_model_provider::recipe_prefix_for_instruction(
                    &host,
                    brassclaw_llm::ProviderRole::Kohai,
                    program.inputs().instruction(),
                )
                .map_err(|_| failed())?
            } else {
                None
            };
            #[cfg(not(feature = "root-llm-provider"))]
            let consumer_prefix: Option<
                brassclaw_reborn::monty_model_capability::RecipeModelPrefix,
            > = None;
            let model_responses = requires_model.then(|| {
                if let Some(prefix) = &consumer_prefix {
                    return admission.selected_model_response_store(
                        prefix.clone(),
                        host.task_model_selection(),
                    );
                }
                admission.model_response_store(
                    model_prefix_reference
                        .get_or_init(|| {
                            (
                                format!(
                                    "catalogue:{}:sha256:{}",
                                    view.generation,
                                    hex::encode(Sha256::digest(view.prefix.as_bytes()))
                                ),
                                Arc::<str>::from(view.prefix.as_str()),
                            )
                        })
                        .0
                        .clone(),
                )
            });
            let mut runtimes = BTreeMap::new();
            let mut credential_policies = BTreeMap::new();
            for (step, binding) in program.bindings() {
                let capability =
                    CapabilityId::new(binding.capability_id()).map_err(|_| failed())?;
                let descriptor = if capability.as_str() == "host.post_reply" {
                    crate::monty_kernel::reply_package()
                        .map_err(|_| failed())?
                        .capabilities
                        .into_iter()
                        .next()
                        .ok_or_else(failed)?
                } else if capability.as_str() == "host.kohai_complete" {
                    crate::monty_kernel::model_package()
                        .map_err(|_| failed())?
                        .capabilities
                        .into_iter()
                        .next()
                        .ok_or_else(failed)?
                } else {
                    self.kernel
                        .builtin(&capability, None)
                        .map_err(|_| failed())?
                        .descriptor()
                        .clone()
                };
                let policy = Arc::new(PgSelectedToolPolicy {
                    host: host.clone(),
                    pool: self.pool.clone(),
                    tool: binding.tool().uuid,
                    capability: capability.clone(),
                    effects: descriptor.effects.clone(),
                    mounts: if matches!(
                        capability.as_str(),
                        "host.post_reply" | "host.kohai_complete"
                    ) {
                        MountView::default()
                    } else {
                        self.memory_mounts.clone()
                    },
                });
                if !descriptor.runtime_credentials.is_empty() {
                    credential_policies.insert(step.clone(), policy.clone());
                }
                let runtime = if capability.as_str() == "host.post_reply" {
                    self.kernel
                        .reply(host.clone(), policy)
                        .map_err(|_| failed())?
                } else if capability.as_str() == "host.kohai_complete" {
                    let prefix = match &consumer_prefix {
                        Some(prefix) => prefix.clone(),
                        None => brassclaw_reborn::monty_model_capability::RecipeModelPrefix::new(
                            model_prefix_reference.get().ok_or_else(failed)?.0.clone(),
                            model_prefix_reference.get().ok_or_else(failed)?.1.clone(),
                        )
                        .map_err(|_| failed())?,
                    };
                    self.kernel
                        .model(
                            host.clone(),
                            prefix,
                            model_responses.clone().ok_or_else(failed)?,
                            policy,
                        )
                        .map_err(|_| failed())?
                } else {
                    let doc = program.inputs().instruction().snapshot().revisions()
                        [&binding.tool().uuid]
                        .draft()
                        .document();
                    let expected =
                        serde_json::from_value(doc["implementation"]["identity"].clone())
                            .map_err(|_| failed())?;
                    if capability.as_str() == brassclaw_host_runtime::SPAWN_SUBAGENT_CAPABILITY_ID {
                        self.kernel
                            .child_with_policy(
                                expected,
                                self.child_task_port(host.clone(), admission.clone()),
                                policy,
                            )
                            .map_err(|_| failed())?
                    } else {
                        self.kernel
                            .builtin_with_policy(&capability, expected, policy)
                            .map_err(|_| failed())?
                    }
                };
                runtimes.insert(step.clone(), Arc::new(runtime));
            }
            let tools = Arc::new(SelectedTools {
                host: host.clone(),
                admission: admission.clone(),
                program: program.clone(),
                runtimes,
                kernel: self.kernel.clone(),
                credential_policies,
                credentials: credentials.clone(),
                mounts: self.memory_mounts.clone(),
                answers: super::native_answers::NativeToolAnswers::default(),
                model_responses,
            });
            selections.push(SelectedMontyRecipe {
                normal_match: None,
                inspected: (*selected).clone(),
                inputs: None,
                tools: Some(tools),
            });
        }
        let incoming = selections.split_off(2);
        let history = selections.pop().ok_or_else(failed)?;
        let reply = selections.pop().ok_or_else(failed)?;
        // Already hashed once during the immutable preparation, before waits.
        let task = Arc::new(TaskCatalogue {
            credentials,
            reference,
            prefix_user: host
                .run_context()
                .actor
                .as_ref()
                .map(|actor| actor.user_id.as_str())
                .unwrap_or("_system")
                .to_owned(),
            prefix_project: host
                .run_context()
                .scope
                .project_id
                .as_ref()
                .map(|project| project.as_str())
                .unwrap_or("default")
                .to_owned(),
            reply,
            history,
            incoming,
            resolution,
            query,
            view,
            target_prefix,
            _package: self.clone(),
        });
        {
            let mut pinned = self.captured_prefixes.lock().map_err(|_| failed())?;
            pinned.retain(|_, task| task.strong_count() > 0);
            pinned.insert(host.run_context().run_id.to_string(), Arc::downgrade(&task));
        }
        Ok(task)
    }
}
