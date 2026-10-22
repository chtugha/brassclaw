//! Coherent catalogue of packaged workflows and explicitly activated authored
//! workflows. Row labels never establish approval. Each ordinary task captures
//! its immutable generation before matching or execution.
use std::{collections::BTreeMap, path::Path, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedToolInvocation, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        intent_system::{
            IntentResolution, IntentScope, RetainedIntentEligibility,
            resolve_catalogue_intent_in_transaction, seed_retained_recipe_intents_in_transaction,
        },
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolBinding, RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate,
    RuntimeKind, TrustClass,
};
use brassclaw_host_runtime::{
    NativeExecutableImage, RetainedFirstPartyCapability, RuntimeCapabilityOutcome,
    RuntimeCapabilityRequest,
};
use brassclaw_loop_support::SystemBundleSource;
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskInput},
};
use brassclaw_pg::PgPool;
use brassclaw_reborn::{
    monty_model_capability::RecipeModelResponseStore, monty_task_host::MontyTaskHost,
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    global_bootstrap_components::{
        GlobalBootstrapUsage, UsageComponentIds, usage_drafts, verify_packaged_usage,
    },
    revision_store::PgComponentRevisionStore,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use brassclaw_turns::run_profile::AgentLoopDriverError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::{
    RebornBuildError,
    global_recipe_ports::{MontyIntentSelection, MontyTaskCatalogue, SelectedMontyRecipe},
    global_task_factory::MontyCatalogueProvider,
    monty_kernel::{MontyKernelSnapshot, PgSelectedToolPolicy},
    pg_monty_admission::{CatalogueReference, PgMontyAdmission},
};

#[path = "task_catalogue_binding.rs"]
mod task_binding;

fn invalid(reason: impl Into<String>) -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: reason.into(),
    }
}
fn failure() -> PortFailure {
    PortFailure::new("recipe_composition_failed").expect("static reason")
}
fn failed() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "monty_catalogue_capture_failed".into(),
    }
}
pub(crate) fn id(name: &str) -> Uuid {
    brassclaw_skills::global_bootstrap_components::packaged_component_id(name)
}

fn reference_value(
    reference: brassclaw_skills::association_contract::ComponentRevisionRef,
) -> Value {
    json!({"uuid":reference.uuid,"class_code":reference.class_code,
        "version":reference.version,"checksum":hex::encode(reference.checksum)})
}

pub(crate) enum BootstrapUsageApproval<'a> {
    Reply(&'a crate::bootstrap_reply_approval::BootstrapReplyApproval),
    Generation(&'a crate::bootstrap_generation_approval::BootstrapGenerationApproval),
    Compiler(&'a crate::bootstrap_compiler_approval::BootstrapCompilerApproval),
    Encoding(&'a crate::bootstrap_encoding_approval::BootstrapEncodingApproval),
}
impl BootstrapUsageApproval<'_> {
    pub(crate) fn approval_id(&self) -> Uuid {
        match self {
            Self::Reply(a) => a.approval_id(),
            Self::Generation(a) => a.approval_id(),
            Self::Compiler(a) => a.approval_id(),
            Self::Encoding(a) => a.approval_id(),
        }
    }
    pub(crate) fn usage_approval(
        &self,
        binding: &brassclaw_engine::memory::retained_tools::RetainedToolBinding,
    ) -> Option<&brassclaw_skills::association_contract::AssociationApprovalDeclaration> {
        match self {
            Self::Reply(a) => a.usage_approval(binding),
            Self::Generation(a) => a.usage_approval(binding),
            Self::Compiler(a) => a.usage_approval(binding),
            Self::Encoding(a) => a.usage_approval(binding),
        }
    }
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        match self {
            Self::Reply(a) => a.verify_in_transaction(tx).await,
            Self::Generation(a) => a.verify_in_transaction(tx).await,
            Self::Compiler(a) => a.verify_in_transaction(tx).await,
            Self::Encoding(a) => a.verify_in_transaction(tx).await,
        }
    }
}

pub(crate) struct InstalledMontyCatalogue {
    initial_preparation: brassclaw_host_api::MontyExecutionLimits,
    preparation_settings:
        std::sync::RwLock<Option<Arc<dyn brassclaw_product_workflow::MontyVmSettingsStore>>>,
    pub(crate) pool: Arc<PgPool>,
    pub(crate) kernel: Arc<dyn MontyKernelSnapshot>,
    pub(crate) reply: Arc<InspectedRetainedProgram>,
    pub(crate) history: Arc<InspectedRetainedProgram>,
    pub(crate) memory_mounts: MountView,
    pub(crate) generation: Uuid,
    pub(crate) reference_sources: crate::prefix_reference_seeds::Package,
    pub(crate) root_reference: brassclaw_skills::association_contract::ComponentRevisionRef,
    pub(crate) mcp_discovery: Arc<crate::mcp_recipe_catalogue::McpRecipeDiscovery>,
    pub(crate) qualification_scope: IntentScope,
    pub(crate) worker: std::path::PathBuf,
    pub(crate) active:
        tokio::sync::RwLock<Arc<crate::authored_monty_catalogue::CatalogueGeneration>>,
    pub(crate) activation: tokio::sync::Mutex<()>,
    #[cfg(test)]
    pub(crate) capture_gate:
        tokio::sync::Mutex<Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>>,
    captured_prefixes: std::sync::Mutex<BTreeMap<String, std::sync::Weak<TaskCatalogue>>>,
    pub(crate) discovery_publication: tokio::sync::Mutex<()>,
    // Retain the bootstrap subject independently of its approved active selection.
    _public_reply_candidate: Arc<InspectedRetainedProgram>,
    pub(super) _generation_candidates: [Arc<InspectedRetainedProgram>; 2],
    pub(super) _source_compiler_candidates: crate::prefix_compiler_candidates::CompilerCandidates,
    pub(super) _measurement_candidate: Arc<InspectedRetainedProgram>,
    pub(super) _measurement_program: Option<Arc<InspectedRetainedProgram>>,
    pub(super) measurement_qualification:
        Option<crate::bootstrap_measurement_approval::BootstrapMeasurementQualification>,
    pub(super) _compiler_program: Option<Arc<InspectedRetainedProgram>>,
    pub(crate) compiler_approval:
        Option<crate::bootstrap_compiler_approval::BootstrapCompilerApproval>,
    pub(crate) generation_approvals:
        Vec<crate::bootstrap_generation_approval::BootstrapGenerationApproval>,
    pub(crate) encoding_approvals:
        Vec<crate::bootstrap_encoding_approval::BootstrapEncodingApproval>,
    pub(crate) reply_approval: Option<crate::bootstrap_reply_approval::BootstrapReplyApproval>,
    // Retain the actual executable, independently of a mutable source pathname.
    pub(crate) _image: Arc<NativeExecutableImage>,
}

impl InstalledMontyCatalogue {
    pub(crate) fn bootstrap_usage(
        &self,
        binding: &brassclaw_engine::memory::retained_tools::RetainedToolBinding,
    ) -> Option<BootstrapUsageApproval<'_>> {
        if let Some(approval) = self
            .reply_approval
            .as_ref()
            .filter(|a| a.usage_approval(binding).is_some())
        {
            return Some(BootstrapUsageApproval::Reply(approval));
        }
        self.generation_approvals
            .iter()
            .find(|a| a.usage_approval(binding).is_some())
            .map(BootstrapUsageApproval::Generation)
            .or_else(|| {
                self.compiler_approval
                    .as_ref()
                    .filter(|a| a.usage_approval(binding).is_some())
                    .map(BootstrapUsageApproval::Compiler)
            })
            .or_else(|| {
                self.encoding_approvals
                    .iter()
                    .find(|a| a.usage_approval(binding).is_some())
                    .map(BootstrapUsageApproval::Encoding)
            })
    }

    pub(crate) fn bind_preparation_settings(
        &self,
        source: Arc<dyn brassclaw_product_workflow::MontyVmSettingsStore>,
    ) -> Result<(), RebornBuildError> {
        let mut slot = self
            .preparation_settings
            .write()
            .map_err(|_| invalid("preparation settings unavailable"))?;
        if slot.is_some() {
            return Err(invalid("preparation settings already bound"));
        }
        *slot = Some(source);
        Ok(())
    }

    pub(crate) async fn preparation_limits(
        &self,
    ) -> Result<brassclaw_monty_host::utility::UtilityLimits, RebornBuildError> {
        brassclaw_monty_host::utility::UtilityLimits::from_execution(
            self.preparation_policy().await?,
        )
        .map_err(invalid)
    }

    pub(crate) async fn preparation_policy(
        &self,
    ) -> Result<brassclaw_host_api::MontyExecutionLimits, RebornBuildError> {
        let source = self
            .preparation_settings
            .read()
            .map_err(|_| invalid("preparation settings unavailable"))?
            .clone();
        let Some(source) = source else {
            return Ok(self.initial_preparation);
        };
        let settings = source
            .get("default", "default")
            .await
            .map_err(|_| invalid("effective preparation settings unavailable"))?;
        settings.execution_limits.validate().map_err(invalid)?;
        Ok(settings.execution_limits)
    }
    /// Fresh capture/source projection must still name the active generation.
    /// Caller holds activation serialization and a read-only repeatable snapshot.
    pub(crate) async fn verify_source_selection(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        view: &crate::authored_monty_catalogue::CatalogueGeneration,
    ) -> Result<(), AgentLoopDriverError> {
        self.verify_retained_selection(tx, view).await?;
        let active_generation: Uuid = tx
            .query_one(
                "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
                &[],
            )
            .await
            .map_err(|_| failed())?
            .get(0);
        if active_generation != view.generation {
            return Err(failed());
        }
        Ok(())
    }

    /// Verify exact stored bytes, revisions and existing approvals for a view
    /// already selected by the catalogue owner. Replacement/withdrawal does not
    /// invalidate that retained selection. This neither selects a new task nor
    /// proves child provenance, restores execution, or grants Tool permission.
    /// The caller supplies the same coherent read-only transaction as capture.
    pub(crate) async fn verify_retained_selection(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        view: &crate::authored_monty_catalogue::CatalogueGeneration,
    ) -> Result<(), AgentLoopDriverError> {
        let row = tx.query_one("SELECT catalogue_bytes,checksum FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1", &[&view.generation])
            .await.map_err(|_| failed())?;
        if row.get::<_, String>(0) != view.bytes
            || row.get::<_, String>(1) != hex::encode(Sha256::digest(view.bytes.as_bytes()))
        {
            return Err(failed());
        }
        if let Some(approval) = &self.reply_approval {
            approval
                .verify_in_transaction(tx)
                .await
                .map_err(|_| failed())?;
        }
        for approval in &self.generation_approvals {
            approval
                .verify_in_transaction(tx)
                .await
                .map_err(|_| failed())?;
        }
        if let Some(qualification) = &self.measurement_qualification {
            qualification
                .verify_in_transaction(tx)
                .await
                .map_err(|_| failed())?;
        }

        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
        {
            let instruction = selected.program().inputs().instruction();
            let refs: Vec<_> = instruction
                .snapshot()
                .revisions()
                .values()
                .map(|revision| revision.reference())
                .collect();
            PgComponentRevisionStore::read_exact_in_transaction(
                tx,
                &[instruction.recipe().uuid],
                &refs,
            )
            .await
            .map_err(|_| failed())?;
        }
        for selected in &view.programs {
            self.verify_selected_approvals(tx, selected)
                .await
                .map_err(|_| failed())?;
        }
        Ok(())
    }

    pub(crate) fn mcp_discovery(&self) -> Arc<crate::mcp_recipe_catalogue::McpRecipeDiscovery> {
        self.mcp_discovery.clone()
    }

    pub(crate) async fn boot(
        pool: Arc<PgPool>,
        kernel: Arc<dyn MontyKernelSnapshot>,
        worker: &Path,
        scope: &IntentScope,
        memory_mounts: MountView,
        root: brassclaw_skills::association_contract::ComponentRevisionRef,
        preparation_policy: brassclaw_host_api::MontyExecutionLimits,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let preparation =
            brassclaw_monty_host::utility::UtilityLimits::from_execution(preparation_policy)
                .map_err(invalid)?;
        let image = NativeExecutableImage::capture_with_verification(
            preparation.max_native_artifact_bytes,
            brassclaw_host_runtime::NativeVerificationLimits::new(
                preparation.bounds.max_stdout_bytes,
                preparation.process.response_timeout,
            )
            .map_err(|error| invalid(error.to_string()))?,
        )
        .await
        .map_err(|error| invalid(error.to_string()))?;
        let memory = kernel
            .builtin(&CapabilityId::new("builtin.memory_write")?, None)
            .map_err(|error| invalid(error.to_string()))?;
        let native = memory
            .native_identity()
            .ok_or_else(|| invalid("memory implementation lacks native registration"))?;
        memory
            .verify_native_artifact()
            .await
            .map_err(|error| invalid(error.to_string()))?;
        crate::prefix_profile_seeds::retain(pool.as_ref()).await?;
        let reference_sources = crate::prefix_reference_seeds::retain(pool.clone(), scope).await?;
        let generation_tool =
            crate::builtin_bootstrap::seed_prefix_generation_tool(pool.clone(), &scope.tenant_id)
                .await
                .map_err(|error| invalid(error.to_string()))?;
        let generation_candidates = crate::prefix_generation_candidates::retain(
            pool.clone(),
            kernel.as_ref(),
            generation_tool,
            worker,
            preparation,
        )
        .await?;
        let json_tool =
            crate::builtin_bootstrap::seed_json_primitive_tool(pool.clone(), &scope.tenant_id)
                .await
                .map_err(|error| invalid(error.to_string()))?;
        kernel
            .builtin(
                &CapabilityId::new(brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID)
                    .map_err(|error| invalid(error.to_string()))?,
                None,
            )
            .map_err(|error| invalid(error.to_string()))?
            .verify_native_artifact()
            .await
            .map_err(|error| invalid(error.to_string()))?;
        let model_encoding_tool =
            crate::builtin_bootstrap::seed_model_encoding_tool(pool.clone(), &scope.tenant_id)
                .await
                .map_err(|error| invalid(error.to_string()))?;
        let encoding_candidates = crate::prefix_encoding_candidates::retain(
            pool.clone(),
            kernel.as_ref(),
            model_encoding_tool,
            worker,
            preparation,
        )
        .await?;
        let encoding_approvals = Box::pin(crate::bootstrap_encoding_approval::qualify(
            pool.clone(),
            kernel.clone(),
            &encoding_candidates,
            worker,
        ))
        .await?;
        let source_compiler_candidates = crate::prefix_compiler_candidates::retain(
            pool.clone(),
            kernel.as_ref(),
            json_tool,
            generation_tool,
            worker,
            preparation,
        )
        .await?;
        let generation_approvals = Box::pin(crate::bootstrap_generation_approval::qualify(
            pool.clone(),
            kernel.clone(),
            &generation_candidates,
            worker,
        ))
        .await?;
        let compiler_approval = Box::pin(crate::bootstrap_compiler_approval::qualify(
            pool.clone(),
            kernel.clone(),
            &source_compiler_candidates.source,
            &generation_approvals,
            worker,
        ))
        .await?;
        let compiler_program = match &compiler_approval {
            Some(approval) => Some(approval.prepare_private_program().await?),
            None => None,
        };
        let measurement_candidate = crate::prefix_measurement_candidates::retain(
            pool.clone(),
            kernel.as_ref(),
            json_tool,
            model_encoding_tool,
            worker,
            preparation,
        )
        .await?;
        let measurement_qualification = Box::pin(crate::bootstrap_measurement_approval::qualify(
            pool.clone(),
            kernel.clone(),
            &measurement_candidate,
            compiler_approval.as_ref(),
            &encoding_approvals,
            worker,
        ))
        .await?;
        let measurement_program = measurement_qualification
            .as_ref()
            .map(|qualification| qualification.program());
        let store = PgComponentRevisionStore::new(pool.clone());
        let client = pool
            .get()
            .await
            .map_err(|_| invalid("cannot resolve installed Tool identities"))?;
        let mut tools = BTreeMap::new();
        for (name, capability) in [
            ("host.post_reply", "host.post_reply"),
            ("memory_write", "builtin.memory_write"),
        ] {
            let rows = client.query("SELECT id FROM reborn_tools WHERE tenant_id=$1 AND source='system' AND capability_id=$2 ORDER BY id",
                &[&scope.tenant_id, &capability]).await.map_err(|_| invalid("cannot resolve installed Tool identities"))?;
            if rows.len() != 1 {
                return Err(invalid(format!(
                    "installed {name} Tool identity is missing or ambiguous"
                )));
            }
            tools.insert(capability, rows[0].get::<_, Uuid>(0));
        }
        drop(client);
        let mut prepared = Vec::new();
        for history in [false, true] {
            let capability = if history {
                "builtin.memory_write"
            } else {
                "host.post_reply"
            };
            let name = if history { "history" } else { "reply" };
            let ids = UsageComponentIds {
                recipe: id(&format!("{name}:recipe")),
                python_code: id(&format!("{name}:code")),
                tool_skill: id(&format!("{name}:binding")),
                tool: tools[capability],
                skill: id(&format!("{name}:skill")),
                formatter: history.then(|| id("history:formatter")),
            };
            let usage = if history {
                GlobalBootstrapUsage::SaveHistory
            } else {
                GlobalBootstrapUsage::PostReply
            };
            let drafts = usage_drafts(ids, usage).map_err(|error| invalid(error.to_string()))?;
            let mut inputs = drafts
                .iter()
                .find(|draft| draft.uuid() == ids.python_code)
                .ok_or_else(|| invalid("packaged usage code is missing"))?
                .document()["input_contract"]
                .clone();
            if history {
                inputs["target"] = json!({"type":"string","required":true,"checks":[]});
            }
            let implementation = if history {
                json!({"format":"native-first-party/1", "identity":native})
            } else {
                json!({"format":"packaged-task-reply/1", "artifact_checksum":hex::encode(image.checksum()),
                    "adapter":"TaskReplyCapability/1"})
            };
            let tool = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1", "uuid":ids.tool,
                "class_code":0,"document":{"capability_id":capability,
                    "callable":if history {"host.memory_write"} else {"host.post_reply"},
                    "input_contract":inputs,"implementation":implementation},"dependencies":[],"association":null}).to_string())
                .map_err(|error| invalid(error.to_string()))?;
            let mut refs = Vec::new();
            for draft in std::iter::once(tool).chain(drafts) {
                refs.push(
                    store
                        .retain_packaged_draft(&draft)
                        .await
                        .map_err(|error| invalid(error.to_string()))?,
                );
            }
            let snapshot = Arc::new(
                store
                    .read_exact(&[ids.recipe], &refs)
                    .await
                    .map_err(|error| invalid(error.to_string()))?,
            );
            verify_packaged_usage(ids, usage, &snapshot)
                .map_err(|error| invalid(error.to_string()))?;
            let program = Arc::new(
                prepare_retained_tool_program(
                    compile_retained_recipe(
                        snapshot,
                        ids.recipe,
                        "selected",
                        WorkflowClass::Deterministic,
                    )
                    .map_err(|error| invalid(error.to_string()))?,
                )
                .map_err(|error| invalid(error.to_string()))?,
            );
            let inspected = Arc::new(
                InspectedRetainedProgram::inspect_with_limits(
                    RetainedProgram::Tools(program),
                    worker,
                    preparation,
                )
                .await
                .map_err(|error| invalid(error.to_string()))?,
            );
            prepared.push(inspected);
        }
        let history = prepared
            .pop()
            .ok_or_else(|| invalid("installed history Recipe is missing"))?;
        let reply = prepared
            .pop()
            .ok_or_else(|| invalid("installed reply Recipe is missing"))?;
        let public_reply_candidate =
            crate::public_recipe_population::retain_public_reply_candidate(
                pool.clone(),
                &reply,
                worker,
            )
            .await?;
        // Policy initialization precedes contained qualification, without enabling
        // an existing blocked Tool. Activation is published only after evidence.
        let client = pool
            .get()
            .await
            .map_err(|_| invalid("Tool settings unavailable"))?;
        for tool in tools.values() {
            client.execute("INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING", &[tool])
                .await.map_err(|_| invalid("instance Tool setting initialization failed"))?;
        }
        drop(client);
        let public_reply_approval = Box::pin(crate::bootstrap_reply_approval::qualify(
            pool.clone(),
            kernel.clone(),
            public_reply_candidate.clone(),
            worker,
        ))
        .await?;
        if let Some(approval) = &public_reply_approval {
            tracing::info!(approval_id=%approval.approval_id(), selection_checksum=%hex::encode(approval.selection_checksum()),
                "public reply bootstrap qualification retained");
        }
        let reply = match &public_reply_approval {
            Some(approval) => approval.activated_program(pool.clone()).await?,
            None => {
                let client = pool
                    .get()
                    .await
                    .map_err(|_| invalid("activation history unavailable"))?;
                let active: bool = client.query_one(
                    "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_boot_catalogues WHERE catalogue_bytes::jsonb #>> '{reply_selection,recipe,uuid}'=$1 AND catalogue_bytes::jsonb #> '{reply_selection,association_approvals}' IS NOT NULL)",
                    &[&id("reply:recipe").to_string()],
                ).await.map_err(|_| invalid("activation history invalid"))?.get(0);
                if active {
                    return Err(invalid(
                        "an activated public reply cannot downgrade while its successor awaits qualification",
                    ));
                }
                reply // Never-activated candidate remains pending under a first-time block.
            }
        };
        let mut documents = BTreeMap::new();
        let mut references = BTreeMap::new();
        for selected in [&reply, &history] {
            for revision in selected
                .program()
                .inputs()
                .instruction()
                .snapshot()
                .revisions()
                .values()
            {
                references.insert(
                    revision.reference().uuid,
                    reference_value(revision.reference()),
                );
                if documents
                    .insert(
                        revision.reference().uuid,
                        revision.draft().exact_bytes().to_owned(),
                    )
                    .is_some_and(|old| old != revision.draft().exact_bytes())
                {
                    return Err(invalid(
                        "installed catalogue has conflicting component selections",
                    ));
                }
            }
        }
        // The knowledge prefix is derived from this exact component library,
        // without hardcoded prose, model calls or artificial token truncation.
        let prefix = documents.values().cloned().collect::<Vec<_>>().join("\n\n");
        let generation_bytes =
            json!({"format":"installation-monty-catalogue/1", "validation_mode":"system_seed",
            "components":documents,"references":references,"root":reference_value(root),
            "prefix_checksum":hex::encode(Sha256::digest(prefix.as_bytes())),
            "artifact_checksum":hex::encode(image.checksum()),
            "reply_selection":serde_json::from_str::<Value>(reply.program().inputs().instruction().retained_selection().map_err(|e| invalid(e.to_string()))?.exact_bytes()).map_err(|e| invalid(e.to_string()))?})
            .to_string();
        let generation = id(&hex::encode(Sha256::digest(generation_bytes.as_bytes())));
        let checksum = hex::encode(Sha256::digest(generation_bytes.as_bytes()));
        let mut client = pool
            .get()
            .await
            .map_err(|_| invalid("installation catalogue database unavailable"))?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| invalid("installation catalogue transaction failed"))?;
        tx.execute("INSERT INTO brassclaw_monty_boot_catalogues(catalogue_id,catalogue_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(catalogue_id) DO NOTHING",
            &[&generation, &generation_bytes, &checksum]).await.map_err(|_| invalid("installation catalogue retention failed"))?;
        let actual = tx.query_one("SELECT catalogue_bytes,checksum FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1", &[&generation])
            .await.map_err(|_| invalid("installation catalogue read failed"))?;
        if actual.get::<_, String>(0) != generation_bytes || actual.get::<_, String>(1) != checksum
        {
            return Err(invalid("installation catalogue integrity failed"));
        }
        if let Some(approval) = &public_reply_approval {
            approval.verify_in_transaction(&tx).await?;
        }
        for selected in [&reply, &history] {
            let instruction = selected.program().inputs().instruction();
            let refs: Vec<_> = instruction
                .snapshot()
                .revisions()
                .values()
                .map(|r| r.reference())
                .collect();
            PgComponentRevisionStore::read_exact_in_transaction(
                &tx,
                &[instruction.recipe().uuid],
                &refs,
            )
            .await
            .map_err(|e| invalid(e.to_string()))?;
        }
        seed_retained_recipe_intents_in_transaction(
            &tx,
            scope,
            reply.program().inputs().instruction(),
        )
        .await
        .map_err(|e| invalid(e.to_string()))?;
        if public_reply_approval.is_some() {
            crate::mcp_command_qualification::verify_routing_metadata(
                &tx,
                scope,
                &[reply.program().inputs().instruction()],
            )
            .await
            .map_err(|e| invalid(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|_| invalid("installation catalogue commit failed"))?;
        drop(client);
        // Discovery uses exactly this qualified normal-chat catalogue, never
        // raw Skill/Recipe tables or a parallel latest-version selection.
        let mcp_discovery = Arc::new(crate::mcp_recipe_catalogue::McpRecipeDiscovery::new());
        let qualified = crate::mcp_command_qualification::qualify_installed_commands(
            &pool,
            scope,
            generation,
            &[&reply, &history],
            &[&reply],
            public_reply_approval.as_ref(),
        )
        .await;
        match qualified {
            Ok(qualified) => mcp_discovery
                .publish_qualified_installed(None, generation, &[&reply, &history], &qualified)
                .map_err(|error| invalid(error.to_string()))?,
            Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Unqualified) => {
                mcp_discovery
                    .publish_unadvertised_installed(None, generation, &[&reply, &history])
                    .map_err(|e| invalid(e.to_string()))?;
                tracing::info!("MCP discovery awaits complete normal-command evidence");
            }
            Err(error) => return Err(invalid(error.to_string())),
        }
        let active = Arc::new(crate::authored_monty_catalogue::CatalogueGeneration {
            generation,
            bytes: generation_bytes.clone(),
            prefix: prefix.clone(),
            programs: Vec::new(),
            recipes: Vec::new(),
            disabled_recipes: Default::default(),
        });
        let catalogue = Arc::new(Self {
            initial_preparation: preparation_policy,
            preparation_settings: std::sync::RwLock::new(None),
            captured_prefixes: std::sync::Mutex::new(BTreeMap::new()),
            #[cfg(test)]
            capture_gate: tokio::sync::Mutex::new(None),
            worker: worker.to_owned(),
            active: tokio::sync::RwLock::new(active),
            activation: tokio::sync::Mutex::new(()),
            discovery_publication: tokio::sync::Mutex::new(()),
            mcp_discovery,
            qualification_scope: scope.clone(),
            _public_reply_candidate: public_reply_candidate,
            _generation_candidates: generation_candidates,
            _source_compiler_candidates: source_compiler_candidates,
            _measurement_candidate: measurement_candidate,
            _measurement_program: measurement_program,
            measurement_qualification,
            generation_approvals,
            compiler_approval,
            encoding_approvals,
            _compiler_program: compiler_program,
            reply_approval: public_reply_approval,
            pool,
            kernel,
            reply,
            history,
            memory_mounts,
            generation,
            root_reference: root,
            reference_sources,
            _image: image,
        });
        catalogue
            .restore_authored()
            .await
            .map_err(|e| invalid(e.to_string()))?;
        Ok(catalogue)
    }
}

#[async_trait]
impl SystemBundleSource for InstalledMontyCatalogue {
    async fn get_system_bundle_for_run(
        &self,
        user: &str,
        project: &str,
        run: &str,
    ) -> Result<String, brassclaw_turns::run_profile::AgentLoopHostError> {
        use brassclaw_turns::run_profile::{AgentLoopHostError, AgentLoopHostErrorKind};
        let unavailable = || {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::Unavailable,
                "retained task prefix is unavailable",
            )
        };
        let task = self
            .captured_prefixes
            .lock()
            .map_err(|_| unavailable())?
            .get(run)
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(unavailable)?;
        if task.prefix_user != user || task.prefix_project != project {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "retained task prefix scope differs",
            ));
        }
        Ok(task.view.prefix.clone())
    }
    async fn get_system_bundle(&self, _user_id: &str, _project_id: &str) -> String {
        self.active.read().await.prefix.clone()
    }
}

struct TaskCatalogue {
    reference: CatalogueReference,
    prefix_user: String,
    prefix_project: String,
    reply: SelectedMontyRecipe,
    history: SelectedMontyRecipe,
    incoming: Vec<SelectedMontyRecipe>,
    resolution: IntentResolution,
    query: String,
    view: Arc<crate::authored_monty_catalogue::CatalogueGeneration>,
    _package: Arc<InstalledMontyCatalogue>,
}

#[async_trait]
impl MontyCatalogueProvider for Arc<InstalledMontyCatalogue> {
    async fn capture_internal(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<
        Option<Arc<dyn crate::global_recipe_ports::InternalRecipeSource>>,
        AgentLoopDriverError,
    > {
        let context = host.run_context();
        if !context.trusted_internal_turn {
            return Ok(None);
        }
        let Some(definition) = crate::prefix_job_definitions::Store::new(self.pool.clone())
            .get(context.run_id.as_uuid())
            .await
            .map_err(|_| failed())?
        else {
            return Ok(None);
        };
        let reference = format!("prefix-job:{}", definition.id);
        if definition.operation == "measure" {
            if input.user_input != reference
                || definition.scope != context.scope
                || definition.created_by
                    != context.actor.as_ref().ok_or_else(failed)?.user_id.as_str()
                || definition.mode != "target"
            {
                return Err(failed());
            }
            return crate::prefix_measurement_capture::select(
                self, host, admission, definition, reference,
            )
            .await
            .map(Some)
            .map_err(|_| failed());
        }
        let revision = definition.revision.to_string();
        if input.user_input != reference
            || definition.scope != context.scope
            || definition.created_by != context.actor.as_ref().ok_or_else(failed)?.user_id.as_str()
            || definition.operation != "generate"
            || definition.mode != "source_only"
            || !matches!(
                definition.profile.as_str(),
                "base-prompt" | "prefix-maker" | "sempai"
            )
            || definition.input["profile"] != definition.profile
            || definition.input["revision"] != revision.as_str()
        {
            return Err(failed());
        }
        crate::prefix_job_execution::validate_snapshot(
            &definition.profile,
            definition.input["snapshot_bytes"]
                .as_str()
                .ok_or_else(failed)?,
        )
        .map_err(|_| failed())?;
        let inspected = self._compiler_program.as_ref().ok_or_else(failed)?.clone();
        if inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| failed())?
            .exact_bytes()
            != definition.program_bytes
        {
            return Err(failed());
        }
        let approval = self.compiler_approval.as_ref().ok_or_else(failed)?;
        let mut client = self.pool.get().await.map_err(|_| failed())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failed())?;
        approval
            .verify_in_transaction(&tx)
            .await
            .map_err(|_| failed())?;
        approval
            .require_selected_program(&inspected)
            .map_err(|_| failed())?;
        tx.commit().await.map_err(|_| failed())?;
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(failed());
        };
        let tools = bootstrap_compiler_tools(
            self.pool.clone(),
            self.kernel.clone(),
            host,
            admission,
            program.clone(),
            false,
        )
        .map_err(|_| failed())?;
        Ok(Some(Arc::new(crate::prefix_job_execution::Invocation {
            definition,
            reference,
            inspected,
            tools,
            pool: self.pool.clone(),
        })))
    }
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError> {
        let _serial = self.activation.lock().await;
        let view = self.active.read().await.clone();
        let context = host.run_context();
        let scope = IntentScope {
            tenant_id: context.scope.tenant_id.to_string(),
            user_id: context
                .actor
                .as_ref()
                .ok_or_else(failed)?
                .user_id
                .to_string(),
            agent_id: context
                .scope
                .agent_id
                .as_ref()
                .ok_or_else(failed)?
                .to_string(),
            project_id: context
                .scope
                .project_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "default".into()),
        };
        let mut client = self.pool.get().await.map_err(|_| failed())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failed())?;
        self.verify_source_selection(&tx, &view)
            .await
            .map_err(|_| failed())?;
        // MCP still enters through ordinary chat. Its durable transport receipt
        // fences the gap between command admission and this task's matching;
        // stale commands must not become a different Recipe or a Tier-2 prompt.
        if let Some(call) = tx
            .query_opt(
                "SELECT tool_name,command,command_checksum,phase FROM brassclaw_mcp_chat_calls WHERE call_id=$1",
                &[&context.run_id.as_uuid()],
            )
            .await
            .map_err(|_| failed())?
        {
            let discovery = self.mcp_discovery.snapshot().map_err(|_| failed())?;
            if discovery.generation() != view.generation
                || call.get::<_, String>(1) != input.user_input
                || call.get::<_, i16>(3) != 1
                || hex::encode(
                    discovery
                        .validate_command(&call.get::<_, String>(0), &input.user_input)
                        .map_err(|_| failed())?,
                ) != call.get::<_, String>(2)
            {
                return Err(failed());
            }
        }
        let instructions: Vec<_> = std::iter::once(&self.reply)
            .chain(view.programs.iter())
            .filter(|p| view.routing_enabled(p))
            .map(|p| p.program().inputs().instruction())
            .collect();
        if scope == self.qualification_scope {
            crate::mcp_command_qualification::verify_routing_metadata(&tx, &scope, &instructions)
                .await
                .map_err(|_| failed())?;
        }
        let eligible =
            RetainedIntentEligibility::from_instructions(&instructions).map_err(|_| failed())?;
        let resolution =
            resolve_catalogue_intent_in_transaction(&tx, &scope, &input.user_input, &eligible)
                .await
                .map_err(|_| failed())?;
        tx.commit().await.map_err(|_| failed())?;
        drop(client);
        drop(_serial);
        #[cfg(test)]
        if let Some((captured, release)) = self.capture_gate.lock().await.take() {
            captured.notify_one();
            release.notified().await;
        }
        self.bind_captured_view(host, admission, view, resolution, input.user_input.clone())
    }
}

fn copy_selection(selected: &SelectedMontyRecipe) -> SelectedMontyRecipe {
    SelectedMontyRecipe {
        normal_match: None,
        inspected: selected.inspected.clone(),
        inputs: selected.inputs.clone(),
        tools: selected.tools.clone(),
    }
}

/// The contained installation validator reuses the exact production dispatcher,
/// kernel reply registration, current global policy and durable effect journal.
pub(crate) fn bootstrap_reply_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    let binding = program
        .bindings()
        .get("0:2")
        .ok_or_else(|| invalid("bootstrap reply binding missing"))?;
    if binding.capability_id() != "host.post_reply" || program.bindings().len() != 1 {
        return Err(invalid("bootstrap only accepts the packaged reply usage"));
    }
    let descriptor = crate::monty_kernel::reply_package()?
        .capabilities
        .into_iter()
        .next()
        .ok_or_else(|| invalid("bootstrap reply descriptor missing"))?;
    let policy = Arc::new(PgSelectedToolPolicy {
        host: host.clone(),
        pool,
        tool: binding.tool().uuid,
        capability: descriptor.id.clone(),
        effects: descriptor.effects,
        mounts: MountView::default(),
    });
    let policy = Arc::new(crate::public_recipe_population::RestrictedBootstrapPolicy {
        live: policy,
        denied,
    });
    let runtime = kernel.reply(host.clone(), policy)?;
    Ok(Arc::new(SelectedTools {
        host,
        admission,
        program,
        runtimes: BTreeMap::from([("0:2".to_owned(), Arc::new(runtime))]),
        mounts: MountView::default(),
        answers: tokio::sync::Mutex::new(BTreeMap::new()),
        model_responses: None,
    }))
}

// Closed generation qualification shares the actual dispatcher, journal and
// live policy. The caller must first verify the exact packaged subject.
pub(crate) fn bootstrap_generation_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    let (_step, binding) = program
        .bindings()
        .iter()
        .next()
        .filter(|_| program.bindings().len() == 1)
        .ok_or_else(|| invalid("generation qualification requires one usage"))?;
    if binding.capability_id() != brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID {
        return Err(invalid("generation qualification binding differs"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, denied)
}

// Same native dispatcher/journal as ordinary selected usages. The installation
// qualification owner separately proves exact packaged source and behavior.
pub(super) fn bootstrap_encoding_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 1
        || program.bindings().get("0:2").is_none_or(|b| {
            b.capability_id() != brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID
        })
    {
        return Err(invalid("encoding qualification binding differs"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, denied)
}

#[cfg(test)]
pub(crate) fn response_parser_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 1
        || program.bindings().get("0:2").is_none_or(|binding| {
            binding.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID
        })
    {
        return Err(invalid("response parser candidate binding differs"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, false)
}

pub(super) fn bootstrap_measurement_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 2
        || program
            .bindings()
            .get("0:5")
            .is_none_or(|b| b.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID)
        || program.bindings().get("0:7").is_none_or(|b| {
            b.capability_id() != brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID
        })
    {
        return Err(invalid("measurement candidate binding differs"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, denied)
}

// Transport/policy construction only. The closed producer above checks its exact
// subject; development tests below cannot manufacture bootstrap approval.
#[cfg(all(test, feature = "root-llm-provider"))]
pub(crate) fn model_recipe_candidate_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    model: (
        brassclaw_reborn::monty_model_capability::RecipeModelPrefix,
        Arc<dyn RecipeModelResponseStore>,
    ),
    application_checksum: [u8; 32],
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.inputs().instruction().class()
        != brassclaw_engine::memory::retained_instruction::WorkflowClass::RequiresModel
        || program.bindings().len() != 4
        || program
            .bindings()
            .values()
            .filter(|b| b.capability_id() == "builtin.json")
            .count()
            != 2
        || program
            .bindings()
            .values()
            .filter(|b| b.capability_id() == "host.post_reply")
            .count()
            != 1
        || program
            .bindings()
            .values()
            .filter(|b| b.capability_id() == "host.kohai_complete")
            .count()
            != 1
    {
        return Err(invalid("model candidate bindings differ"));
    }
    let (prefix, responses) = model;
    let mut prefix = Some(prefix);
    let mut runtimes = BTreeMap::new();
    for (step, binding) in program.bindings() {
        let capability = CapabilityId::new(binding.capability_id())
            .map_err(|error| invalid(error.to_string()))?;
        let special = matches!(
            capability.as_str(),
            "host.post_reply" | "host.kohai_complete"
        );
        let descriptor = match capability.as_str() {
            "host.post_reply" => crate::monty_kernel::reply_package()?
                .capabilities
                .into_iter()
                .next(),
            "host.kohai_complete" => crate::monty_kernel::model_package()?
                .capabilities
                .into_iter()
                .next(),
            _ => Some(
                kernel
                    .builtin(&capability, None)
                    .map_err(|error| invalid(error.to_string()))?
                    .descriptor()
                    .clone(),
            ),
        }
        .ok_or_else(|| invalid("model candidate descriptor missing"))?;
        let document = program.inputs().instruction().snapshot().revisions()[&binding.tool().uuid]
            .draft()
            .document();
        if special
            && document["implementation"]["artifact_checksum"] != hex::encode(application_checksum)
        {
            return Err(invalid("model candidate application identity differs"));
        }
        let policy = Arc::new(PgSelectedToolPolicy {
            host: host.clone(),
            pool: pool.clone(),
            tool: binding.tool().uuid,
            capability: capability.clone(),
            effects: descriptor.effects,
            mounts: MountView::default(),
        });
        let runtime = match capability.as_str() {
            "host.post_reply" => kernel.reply(host.clone(), policy)?,
            "host.kohai_complete" => kernel.model(
                host.clone(),
                prefix
                    .take()
                    .ok_or_else(|| invalid("model candidate prefix already bound"))?,
                responses.clone(),
                policy,
            )?,
            _ => kernel
                .builtin_with_policy(
                    &capability,
                    serde_json::from_value(document["implementation"]["identity"].clone())
                        .map_err(|error| invalid(error.to_string()))?,
                    policy,
                )
                .map_err(|error| invalid(error.to_string()))?,
        };
        runtimes.insert(step.clone(), Arc::new(runtime));
    }
    Ok(Arc::new(SelectedTools {
        host,
        admission,
        program,
        runtimes,
        mounts: MountView::default(),
        answers: tokio::sync::Mutex::new(BTreeMap::new()),
        model_responses: Some(responses),
    }))
}

fn bootstrap_builtin_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    let mut runtimes = BTreeMap::new();
    for (step, binding) in program.bindings() {
        let capability = CapabilityId::new(binding.capability_id())
            .map_err(|error| invalid(error.to_string()))?;
        let descriptor = kernel
            .builtin(&capability, None)
            .map_err(|error| invalid(error.to_string()))?
            .descriptor()
            .clone();
        let policy = Arc::new(PgSelectedToolPolicy {
            host: host.clone(),
            pool: pool.clone(),
            tool: binding.tool().uuid,
            capability: capability.clone(),
            effects: descriptor.effects,
            mounts: MountView::default(),
        });
        let policy = Arc::new(crate::public_recipe_population::RestrictedBootstrapPolicy {
            live: policy,
            denied,
        });
        let document = program.inputs().instruction().snapshot().revisions()[&binding.tool().uuid]
            .draft()
            .document();
        let expected = serde_json::from_value(document["implementation"]["identity"].clone())
            .map_err(|error| invalid(error.to_string()))?;
        let runtime = kernel
            .builtin_with_policy(&capability, expected, policy)
            .map_err(|error| invalid(error.to_string()))?;
        runtimes.insert(step.clone(), Arc::new(runtime));
    }
    Ok(Arc::new(SelectedTools {
        host,
        admission,
        program,
        runtimes,
        mounts: MountView::default(),
        answers: tokio::sync::Mutex::new(BTreeMap::new()),
        model_responses: None,
    }))
}

pub(crate) fn bootstrap_compiler_tools(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    denied: bool,
) -> Result<Arc<dyn RetainedToolPort>, RebornBuildError> {
    if program.bindings().len() != 5
        || program
            .bindings()
            .values()
            .filter(|binding| binding.capability_id() == "builtin.json")
            .count()
            != 4
        || program
            .bindings()
            .values()
            .filter(|binding| {
                binding.capability_id() == brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID
            })
            .count()
            != 1
    {
        return Err(invalid("compiler bootstrap fixture bindings differ"));
    }
    bootstrap_builtin_tools(pool, kernel, host, admission, program, denied)
}

#[async_trait]
impl MontyTaskCatalogue for TaskCatalogue {
    fn catalogue_reference(&self) -> Option<CatalogueReference> {
        Some(self.reference)
    }
    async fn refresh_command_qualification(&self) {
        if let Err(error) = self._package.refresh_active_discovery().await {
            tracing::warn!(%error, "MCP qualification refresh failed");
        }
    }
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure> {
        // Matching was committed with capture, before any activation can change
        // physical routing. Child execution never looks up latest or rematches.
        if query != self.query {
            return Err(failure());
        }
        match &self.resolution {
            IntentResolution::NoMatch => Ok(MontyIntentSelection::NoMatch),
            IntentResolution::Disambiguation { .. } => Ok(MontyIntentSelection::Disambiguation),
            IntentResolution::Match {
                component_id,
                step_link,
                input_text,
                ..
            } => {
                let original = std::iter::once(&self.reply)
                    .chain(self.incoming.iter())
                    .find(|p| {
                        let i = p.inspected.program().inputs().instruction();
                        i.recipe().uuid == *component_id
                            && i.variant().step_link == *step_link
                            && i.variant().intent_examples.contains(input_text)
                    })
                    .ok_or_else(failure)?;
                let mut selected = copy_selection(original);
                selected.inputs = Some(
                    selected
                        .inspected
                        .program()
                        .inputs()
                        .bind_variant_example(input_text, query, &json!({}))
                        .map_err(|_| failure())?,
                );
                selected.normal_match = Some(Box::new(
                    crate::normal_match_evidence::NormalMatchEvidence::from_committed_match(
                        self.view.generation,
                        query,
                        &self.resolution,
                        selected.inspected.program().inputs().instruction(),
                        selected.inputs.as_ref().ok_or_else(failure)?,
                    )
                    .ok_or_else(failure)?,
                ));
                Ok(MontyIntentSelection::Match(selected))
            }
        }
    }
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure> {
        // These exact artifacts and qualification owner were retained at task
        // start; no latest lookup or matching happens during a child/wait.
        match name {
            "host-post-reply" => Ok(copy_selection(&self.reply)),
            "host-save-history" => Ok(copy_selection(&self.history)),
            _ => Err(failure()),
        }
    }
}

struct SelectedTools {
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    runtimes: BTreeMap<String, Arc<RetainedFirstPartyCapability>>,
    mounts: MountView,
    // A lost answer commit retains the actual result; it cannot justify replay.
    answers: tokio::sync::Mutex<BTreeMap<String, Value>>,
    model_responses: Option<Arc<dyn RecipeModelResponseStore>>,
}
#[async_trait]
impl RetainedToolPort for SelectedTools {
    fn has_uncommitted_native_results(&self) -> bool {
        self.model_responses
            .as_ref()
            .is_some_and(|owner| owner.has_uncommitted_results())
    }

    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        let error = || PortAnswer::TerminalError {
            reason_kind: "retained_tool_failed".into(),
        };
        let Some(selected) = self.program.bindings().get(invocation.step_id()) else {
            return error();
        };
        let Some(runtime) = self.runtimes.get(invocation.step_id()) else {
            return error();
        };
        if selected.tool() != binding.tool()
            || selected.python() != binding.python()
            || runtime.descriptor().id.as_str() != binding.capability_id()
        {
            return error();
        }
        let context = self.host.run_context();
        let Some(actor) = context.actor.as_ref() else {
            return error();
        };
        let mut execution = match ExecutionContext::local_default(
            actor.user_id.clone(),
            ExtensionId::new("monty-recipe").expect("static identity"),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            self.mounts.clone(),
        ) {
            Ok(value) => value,
            Err(_) => return error(),
        };
        execution.tenant_id = context.scope.tenant_id.clone();
        execution.agent_id = context.scope.agent_id.clone();
        execution.project_id = context.scope.project_id.clone();
        execution.thread_id = Some(context.thread_id.clone());
        execution.resource_scope.tenant_id = execution.tenant_id.clone();
        execution.resource_scope.agent_id = execution.agent_id.clone();
        execution.resource_scope.project_id = execution.project_id.clone();
        execution.resource_scope.thread_id = execution.thread_id.clone();
        let record = match self
            .admission
            .begin_tool_invocation(
                execution.invocation_id,
                &self.program,
                invocation.step_id(),
                &arguments,
                invocation.data_limits(),
            )
            .await
        {
            Ok(record) => record,
            Err(_) => return error(),
        };
        let trust = TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: runtime.descriptor().effects.clone(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::AdminConfig,
            evaluated_at: chrono::Utc::now(),
        };
        let answer = match self
            .host
            .invoke_retained_tool(
                runtime,
                RuntimeCapabilityRequest::new(
                    execution,
                    runtime.descriptor().id.clone(),
                    ResourceEstimate::default(),
                    arguments,
                    trust,
                ),
            )
            .await
        {
            Ok(RuntimeCapabilityOutcome::Completed(completed)) => PortAnswer::Return {
                value: completed.output.into(),
            },
            Ok(RuntimeCapabilityOutcome::Failed(failed)) => PortAnswer::TerminalError {
                reason_kind: format!("retained_tool_{}", failed.kind.as_str()),
            },
            _ => error(),
        };
        let retained = match serde_json::to_value(&answer) {
            Ok(value) => value,
            Err(_) => return error(),
        };
        self.answers
            .lock()
            .await
            .insert(invocation.step_id().to_owned(), retained);
        if record.record_answer(&answer).await.is_err() {
            return PortAnswer::TerminalError {
                reason_kind: "tool_answer_persistence_failed".into(),
            };
        }
        answer
    }
}
