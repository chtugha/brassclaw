//! Fresh task-local bindings against an already captured immutable view.
//!
//! Selection/integrity stay with the catalogue owner. This constructor never
//! reads latest, matches intent, executes a Tool or copies another task's ports.
use std::{collections::BTreeMap, sync::Arc};

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
    monty_kernel::PgSelectedToolPolicy,
    pg_monty_admission::{CatalogueReference, PgMontyAdmission},
};

impl InstalledMontyCatalogue {
    /// The caller already owns the coherent selection and its provenance.
    /// Definitions are retained; every policy, response owner, answer map and
    /// execution port is bound anew to this exact host/admission. This helper
    /// supplies no child admission or approval and cannot validate provenance.
    pub(super) fn bind_captured_view(
        self: &Arc<Self>,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        view: Arc<CatalogueGeneration>,
        resolution: IntentResolution,
        query: String,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError> {
        let mut selections = Vec::new();
        let model_prefix_reference = std::sync::OnceLock::new();
        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
        {
            let RetainedProgram::Tools(program) = selected.program() else {
                return Err(failed());
            };
            let model_responses = program
                .bindings()
                .values()
                .any(|binding| binding.capability_id() == "host.kohai_complete")
                .then(|| {
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
                let runtime = if capability.as_str() == "host.post_reply" {
                    self.kernel
                        .reply(host.clone(), policy)
                        .map_err(|_| failed())?
                } else if capability.as_str() == "host.kohai_complete" {
                    let prefix = brassclaw_reborn::monty_model_capability::RecipeModelPrefix::new(
                        model_prefix_reference.get().ok_or_else(failed)?.0.clone(),
                        model_prefix_reference.get().ok_or_else(failed)?.1.clone(),
                    )
                    .map_err(|_| failed())?;
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
                    self.kernel
                        .builtin_with_policy(&capability, expected, policy)
                        .map_err(|_| failed())?
                };
                runtimes.insert(step.clone(), Arc::new(runtime));
            }
            let tools = Arc::new(SelectedTools {
                host: host.clone(),
                admission: admission.clone(),
                program: program.clone(),
                runtimes,
                mounts: self.memory_mounts.clone(),
                answers: tokio::sync::Mutex::new(BTreeMap::new()),
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
        // Hash once at capture, never on each host operation or after a wait.
        let reference = CatalogueReference {
            id: view.generation,
            checksum: Sha256::digest(view.bytes.as_bytes()).into(),
        };
        let task = Arc::new(TaskCatalogue {
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
