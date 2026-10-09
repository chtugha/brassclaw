//! Installation-owned catalogue of the packaged typed workflows. Authored
//! additions use their separate validation/activation owner; row labels never
//! enter this catalogue. The immutable package generation is pinned per task.
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
        retained_instruction::{
            WorkflowClass, compile_matched_retained_recipe, compile_retained_recipe,
        },
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
use brassclaw_reborn::monty_task_host::MontyTaskHost;
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
    pg_monty_admission::PgMontyAdmission,
};

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
    Uuid::new_v5(
        &Uuid::from_u128(0x19ddf38d_487e_4732_b008_5a1ec57d2ef9),
        name.as_bytes(),
    )
}

fn reference_value(
    reference: brassclaw_skills::association_contract::ComponentRevisionRef,
) -> Value {
    json!({"uuid":reference.uuid,"class_code":reference.class_code,
        "version":reference.version,"checksum":hex::encode(reference.checksum)})
}

pub(crate) struct InstalledMontyCatalogue {
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    reply: Arc<InspectedRetainedProgram>,
    history: Arc<InspectedRetainedProgram>,
    memory_mounts: MountView,
    generation: Uuid,
    generation_bytes: String,
    prefix: String,
    mcp_discovery: Arc<crate::mcp_recipe_catalogue::McpRecipeDiscovery>,
    qualification_scope: IntentScope,
    // Retain the bootstrap subject independently of its approved active selection.
    _public_reply_candidate: Arc<InspectedRetainedProgram>,
    reply_approval: Option<crate::bootstrap_reply_approval::BootstrapReplyApproval>,
    // Retain the actual executable, independently of a mutable source pathname.
    _image: Arc<NativeExecutableImage>,
}

impl InstalledMontyCatalogue {
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
    ) -> Result<Arc<Self>, RebornBuildError> {
        let image = NativeExecutableImage::capture(1024 * 1024 * 1024)
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
                InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), worker)
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
        Ok(Arc::new(Self {
            mcp_discovery,
            qualification_scope: scope.clone(),
            _public_reply_candidate: public_reply_candidate,
            reply_approval: public_reply_approval,
            pool,
            kernel,
            reply,
            history,
            memory_mounts,
            generation,
            generation_bytes,
            prefix,
            _image: image,
        }))
    }
}

#[async_trait]
impl SystemBundleSource for InstalledMontyCatalogue {
    async fn get_system_bundle(&self, _user_id: &str, _project_id: &str) -> String {
        self.prefix.clone()
    }
}

struct TaskCatalogue {
    pool: Arc<PgPool>,
    host: Arc<MontyTaskHost>,
    reply: SelectedMontyRecipe,
    history: SelectedMontyRecipe,
    // Pins the qualification/artifact owner with all its exact task selections.
    _package: Arc<InstalledMontyCatalogue>,
}

impl TaskCatalogue {
    fn intent_scope(&self) -> Result<IntentScope, PortFailure> {
        let context = self.host.run_context();
        Ok(IntentScope {
            tenant_id: context.scope.tenant_id.to_string(),
            user_id: context
                .actor
                .as_ref()
                .ok_or_else(failure)?
                .user_id
                .to_string(),
            agent_id: context
                .scope
                .agent_id
                .as_ref()
                .ok_or_else(failure)?
                .to_string(),
            project_id: context
                .scope
                .project_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "default".into()),
        })
    }
}

#[async_trait]
impl MontyCatalogueProvider for Arc<InstalledMontyCatalogue> {
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, AgentLoopDriverError> {
        let mut client = self.pool.get().await.map_err(|_| failed())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failed())?;
        let row = tx.query_one("SELECT catalogue_bytes,checksum FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1", &[&self.generation])
            .await.map_err(|_| failed())?;
        if row.get::<_, String>(0) != self.generation_bytes
            || row.get::<_, String>(1)
                != hex::encode(Sha256::digest(self.generation_bytes.as_bytes()))
        {
            return Err(failed());
        }
        if let Some(approval) = &self.reply_approval {
            approval
                .verify_in_transaction(&tx)
                .await
                .map_err(|_| failed())?;
        }
        for selected in [&self.reply, &self.history] {
            let instruction = selected.program().inputs().instruction();
            let refs: Vec<_> = instruction
                .snapshot()
                .revisions()
                .values()
                .map(|revision| revision.reference())
                .collect();
            PgComponentRevisionStore::read_exact_in_transaction(
                &tx,
                &[instruction.recipe().uuid],
                &refs,
            )
            .await
            .map_err(|_| failed())?;
        }
        tx.commit().await.map_err(|_| failed())?;
        drop(client);
        let mut selections = Vec::new();
        for selected in [&self.reply, &self.history] {
            let RetainedProgram::Tools(program) = selected.program() else {
                return Err(failed());
            };
            let binding = program.bindings().values().next().ok_or_else(failed)?;
            let capability = CapabilityId::new(binding.capability_id()).map_err(|_| failed())?;
            let descriptor = if capability.as_str() == "host.post_reply" {
                crate::monty_kernel::reply_package()
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
                mounts: if capability.as_str() == "host.post_reply" {
                    MountView::default()
                } else {
                    self.memory_mounts.clone()
                },
            });
            let runtime = if capability.as_str() == "host.post_reply" {
                self.kernel
                    .reply(host.clone(), policy)
                    .map_err(|_| failed())?
            } else {
                let doc = program.inputs().instruction().snapshot().revisions()
                    [&binding.tool().uuid]
                    .draft()
                    .document();
                let expected = serde_json::from_value(doc["implementation"]["identity"].clone())
                    .map_err(|_| failed())?;
                self.kernel
                    .builtin_with_policy(&capability, expected, policy)
                    .map_err(|_| failed())?
            };
            let tools = Arc::new(SelectedTools {
                host: host.clone(),
                admission: admission.clone(),
                program: program.clone(),
                runtime: Arc::new(runtime),
                mounts: self.memory_mounts.clone(),
                answers: tokio::sync::Mutex::new(BTreeMap::new()),
            });
            selections.push(SelectedMontyRecipe {
                normal_match: None,
                inspected: (*selected).clone(),
                inputs: None,
                tools: Some(tools),
            });
        }
        let history = selections.pop().ok_or_else(failed)?;
        let reply = selections.pop().ok_or_else(failed)?;
        Ok(Arc::new(TaskCatalogue {
            pool: self.pool.clone(),
            host,
            reply,
            history,
            _package: self.clone(),
        }))
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
        runtime: Arc::new(runtime),
        mounts: MountView::default(),
        answers: tokio::sync::Mutex::new(BTreeMap::new()),
    }))
}

#[async_trait]
impl MontyTaskCatalogue for TaskCatalogue {
    async fn refresh_command_qualification(&self) {
        let package = &self._package;
        if package
            .mcp_discovery
            .snapshot()
            .is_ok_and(|snapshot| snapshot.qualification_checksum().is_some())
        {
            return;
        }
        let result = async {
            let qualified = crate::mcp_command_qualification::qualify_installed_commands(
                &package.pool,
                &package.qualification_scope,
                package.generation,
                &[&package.reply, &package.history],
                &[&package.reply],
                package.reply_approval.as_ref(),
            )
            .await?;
            package.mcp_discovery.publish_qualified_installed(
                package
                    .mcp_discovery
                    .snapshot()
                    .ok()
                    .map(|snapshot| snapshot.generation()),
                package.generation,
                &[&package.reply, &package.history],
                &qualified,
            )
        }
        .await;
        match result {
            Ok(()) | Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Unqualified) => {}
            Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Conflict)
                if package.mcp_discovery.snapshot().is_ok() => {}
            Err(error) => {
                tracing::warn!(%error, "MCP command qualification refresh failed; discovery remains unavailable")
            }
        }
    }
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure> {
        let scope = self.intent_scope()?;
        let mut client = self.pool.get().await.map_err(|_| failure())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failure())?;
        let instruction = self.reply.inspected.program().inputs().instruction();
        let eligible =
            RetainedIntentEligibility::from_instructions(&[instruction]).map_err(|_| failure())?;
        // A public command must not fall into Tier 2 after its physical route
        // disappears or changes. This initial check retains the task selection;
        // it never rematches a begun execution or consults latest components.
        if scope == self._package.qualification_scope
            && [&self._package.reply, &self._package.history]
                .iter()
                .any(|selected| {
                    let instruction = selected.program().inputs().instruction();
                    instruction.snapshot().revisions()[&instruction.recipe().uuid]
                        .draft()
                        .document()
                        .get("mcp_call")
                        .is_some()
                })
        {
            crate::mcp_command_qualification::verify_routing_metadata(&tx, &scope, &[instruction])
                .await
                .map_err(|_| failure())?;
        }
        let matched = resolve_catalogue_intent_in_transaction(&tx, &scope, query, &eligible)
            .await
            .map_err(|_| failure())?;
        let mut result = match &matched {
            IntentResolution::NoMatch => MontyIntentSelection::NoMatch,
            IntentResolution::Disambiguation { .. } => MontyIntentSelection::Disambiguation,
            IntentResolution::Match {
                component_id,
                input_text,
                ..
            } => {
                let refs: Vec<_> = instruction
                    .snapshot()
                    .revisions()
                    .values()
                    .map(|r| r.reference())
                    .collect();
                let snapshot = Arc::new(
                    PgComponentRevisionStore::read_exact_in_transaction(
                        &tx,
                        &[*component_id],
                        &refs,
                    )
                    .await
                    .map_err(|_| failure())?,
                );
                let actual = compile_matched_retained_recipe(
                    snapshot,
                    &matched,
                    WorkflowClass::Deterministic,
                )
                .map_err(|_| failure())?;
                let actual = prepare_retained_tool_program(actual).map_err(|_| failure())?;
                let actual = match &self._package.reply_approval {
                    Some(approval) => {
                        approval
                            .verify_in_transaction(&tx)
                            .await
                            .map_err(|_| failure())?;
                        approval.pin_program(actual).map_err(|_| failure())?
                    }
                    None => actual,
                };
                if actual
                    .inputs()
                    .instruction()
                    .retained_selection()
                    .map_err(|_| failure())?
                    .checksum()
                    != instruction
                        .retained_selection()
                        .map_err(|_| failure())?
                        .checksum()
                {
                    return Err(failure());
                }
                let mut selected = copy_selection(&self.reply);
                selected.inputs = Some(
                    selected
                        .inspected
                        .program()
                        .inputs()
                        .bind_variant_example(input_text, query, &json!({}))
                        .map_err(|_| failure())?,
                );
                MontyIntentSelection::Match(selected)
            }
        };
        tx.commit().await.map_err(|_| failure())?;
        if let MontyIntentSelection::Match(selected) = &mut result {
            selected.normal_match = Some(Box::new(
                crate::normal_match_evidence::NormalMatchEvidence::from_committed_match(
                    self._package.generation,
                    query,
                    &matched,
                    selected.inspected.program().inputs().instruction(),
                    selected.inputs.as_ref().ok_or_else(failure)?,
                )
                .ok_or_else(failure)?,
            ));
        }
        Ok(result)
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
    runtime: Arc<RetainedFirstPartyCapability>,
    mounts: MountView,
    // A lost answer commit retains the actual result; it cannot justify replay.
    answers: tokio::sync::Mutex<BTreeMap<String, Value>>,
}
#[async_trait]
impl RetainedToolPort for SelectedTools {
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
        if selected.tool() != binding.tool()
            || selected.python() != binding.python()
            || self.runtime.descriptor().id.as_str() != binding.capability_id()
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
            .begin_tool_invocation(&self.program, invocation.step_id(), &arguments)
            .await
        {
            Ok(record) => record,
            Err(_) => return error(),
        };
        let trust = TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: self.runtime.descriptor().effects.clone(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::AdminConfig,
            evaluated_at: chrono::Utc::now(),
        };
        let answer = match self
            .host
            .invoke_retained_tool(
                &self.runtime,
                RuntimeCapabilityRequest::new(
                    execution,
                    self.runtime.descriptor().id.clone(),
                    ResourceEstimate::default(),
                    arguments,
                    trust,
                ),
            )
            .await
        {
            Ok(RuntimeCapabilityOutcome::Completed(completed)) => PortAnswer::Return {
                value: completed.output,
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
