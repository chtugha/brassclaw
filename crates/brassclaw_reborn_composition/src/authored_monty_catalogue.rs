//! Coherent activation consumes existing trusted usage approvals. It performs
//! assembly/runner checks, never re-audits or executes a Recipe's steps.
use crate::installed_monty_catalogue::{InstalledMontyCatalogue, id};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        intent_system::seed_retained_recipe_intents_in_transaction,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_product_workflow::{
    ActivateCatalogueRequest, CatalogueActivationReceipt, CatalogueRecipeSelection,
    RecipeStoreError,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    association_review_store::retain_authored_association_reviews,
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

pub(crate) struct CatalogueGeneration {
    pub(crate) generation: Uuid,
    pub(crate) bytes: String,
    pub(crate) prefix: String,
    pub(crate) programs: Vec<Arc<InspectedRetainedProgram>>,
    pub(crate) recipes: Vec<CatalogueRecipeSelection>,
    pub(crate) disabled_recipes: BTreeSet<Uuid>,
}
impl CatalogueGeneration {
    pub(crate) fn routing_enabled(&self, program: &InspectedRetainedProgram) -> bool {
        !self
            .disabled_recipes
            .contains(&program.program().inputs().instruction().recipe().uuid)
    }
}
fn invalid(reason: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Invalid(reason.to_string())
}
fn storage(_: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Unavailable("catalogue database operation failed".into())
}
fn identifier(value: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| invalid("canonical non-nil UUID required"))
}
fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// Read the small policy envelope first, then validate the complete immutable
/// document with its retained bounds. Current settings never resize a retained
/// generation during restoration.
pub(crate) fn catalogue_document(bytes: &str) -> Result<Value, RecipeStoreError> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        format: String,
        preparation_policy: Option<brassclaw_host_api::MontyExecutionLimits>,
    }
    let envelope: Envelope = serde_json::from_str(bytes).map_err(invalid)?;
    let mut limits = brassclaw_skills::component_revision::REVISION_LIMITS;
    if matches!(
        envelope.format.as_str(),
        "authored-monty-catalogue/2" | "authored-monty-catalogue/3"
    ) {
        let policy = envelope
            .preparation_policy
            .ok_or_else(|| invalid("retained catalogue preparation policy missing"))?;
        policy.validate().map_err(invalid)?;
        limits.max_bytes = usize::try_from(policy.max_catalogue_bytes).map_err(invalid)?;
        limits.max_nodes = usize::try_from(policy.max_value_nodes).map_err(invalid)?;
    } else if envelope.preparation_policy.is_some() {
        return Err(invalid("unexpected catalogue preparation policy"));
    }
    brassclaw_skills::value_contract::strict_json(bytes, limits).map_err(invalid)
}

impl InstalledMontyCatalogue {
    /// An operator activates a coherent set, rather than one component at a
    /// time. Unchanged usages retain their existing approval identifiers.
    pub(crate) async fn activate_authored(
        &self,
        actor: &str,
        request: ActivateCatalogueRequest,
    ) -> Result<CatalogueActivationReceipt, RecipeStoreError> {
        if actor.trim().is_empty() {
            return Err(invalid("invalid catalogue activation actor"));
        }
        let activation = identifier(&request.activation_id)?;
        let expected = identifier(&request.expected_generation)?;
        let bytes = serde_json::to_string(&request).map_err(invalid)?;
        let _serial = self.activation.lock().await;
        let policy = self.preparation_policy().await.map_err(|_| {
            RecipeStoreError::Unavailable("acknowledged catalogue settings unavailable".into())
        })?;
        let mut client = self.pool.get().await.map_err(storage)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(storage)?;
        // Serialize with another process/owner; its CAS winner must be observed,
        // never overwritten after an unknown commit.
        tx.query_one("SELECT pg_advisory_xact_lock(734215980137::bigint)", &[])
            .await
            .map_err(storage)?;
        if let Some(row) = tx.query_opt("SELECT actor,request_bytes,request_checksum,catalogue_id FROM brassclaw_monty_catalogue_activations WHERE activation_id=$1", &[&activation]).await.map_err(storage)? {
            if row.get::<_, String>(0) != actor || row.get::<_, String>(1) != bytes || row.get::<_, String>(2) != digest(&bytes) {
                return Err(invalid("activation identifier already belongs to different content"));
            }
            let generation: Uuid = row.get(3);
            let current: Uuid = tx.query_one("SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton", &[]).await.map_err(storage)?.get(0);
            tx.commit().await.map_err(storage)?;
            drop(client);
            // An old idempotent receipt never republishes an old generation.
            if current == generation { self.restore_authored_locked().await?; }
            return Ok(CatalogueActivationReceipt { activation_id: activation.to_string(), generation: generation.to_string(), catalogue_activated: current == generation });
        }
        let current: Uuid = tx.query_one("SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton FOR UPDATE", &[]).await.map_err(storage)?.get(0);
        if current != expected {
            return Err(RecipeStoreError::Conflict(
                "catalogue generation compare-and-swap conflict".into(),
            ));
        }
        crate::component_deployment::verify_publication(&tx, current, &request, policy).await?;
        if actor.len() as u64 > u64::from(policy.max_catalogue_actor_bytes)
            || bytes.len() as u64 > policy.max_catalogue_bytes
        {
            return Err(invalid("catalogue activation exceeds preparation capacity"));
        }
        let prior = self.active.read().await.clone();
        if prior.generation != current {
            return Err(RecipeStoreError::Conflict(
                "runtime catalogue differs from committed generation".into(),
            ));
        }
        let disabled = match &request.disabled_recipe_ids {
            None => prior.disabled_recipes.clone(),
            Some(ids) => {
                let selected: BTreeSet<_> = ids
                    .iter()
                    .map(|s| identifier(s))
                    .collect::<Result<_, _>>()?;
                if selected.len() != ids.len() {
                    return Err(invalid("duplicate routing disable"));
                }
                let known: BTreeSet<_> = prior
                    .recipes
                    .iter()
                    .chain(request.recipes.iter())
                    .map(|r| identifier(&r.recipe_id))
                    .collect::<Result<_, _>>()?;
                if selected.iter().any(|r| {
                    *r != id("reply:recipe")
                        && !known.contains(r)
                        && !prior.disabled_recipes.contains(r)
                }) {
                    return Err(invalid("routing choice requires a known ordinary Recipe"));
                }
                selected
            }
        };
        let view = self
            .prepare_with_policy(
                &tx,
                &request.recipes,
                policy,
                "authored-monty-catalogue/3",
                disabled,
                None,
            )
            .await?;
        let advertised: Vec<_> = std::iter::once(self.reply.as_ref())
            .chain(std::iter::once(self.history.as_ref()))
            .chain(view.programs.iter().map(Arc::as_ref))
            .filter(|p| view.routing_enabled(p))
            .collect();
        let contracts = crate::mcp_recipe_catalogue::McpRecipeDiscovery::contracts(
            view.generation,
            &advertised,
        )
        .map_err(invalid)?;
        for contract in contracts.values() {
            for candidate in std::iter::once(&self.reply)
                .chain(view.programs.iter())
                .filter(|p| view.routing_enabled(p))
            {
                let instruction = candidate.program().inputs().instruction();
                for template in &instruction.variant().intent_examples {
                    let document = instruction.snapshot().revisions()[&instruction.recipe().uuid]
                        .draft()
                        .document();
                    if template == &contract.declaration.sentence
                        && document["mcp_call"]["name"] == json!(contract.declaration.name)
                        && document["mcp_call"]["variant_key"]
                            == json!(instruction.variant().variant_key)
                    {
                        continue;
                    }
                    if crate::mcp_command_qualification::may_overlap(
                        &contract.declaration.sentence,
                        template,
                    ) {
                        return Err(invalid("MCP command conflicts with active routing"));
                    }
                }
            }
        }
        let _publication = self.discovery_publication.lock().await;
        let discovery_expected = self.mcp_discovery.snapshot().ok().map(|v| v.generation());
        tx.execute("INSERT INTO brassclaw_monty_boot_catalogues(catalogue_id,catalogue_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(catalogue_id) DO NOTHING", &[&view.generation, &view.bytes, &digest(&view.bytes)]).await.map_err(storage)?;
        let actual: String = tx
            .query_one(
                "SELECT catalogue_bytes FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1",
                &[&view.generation],
            )
            .await
            .map_err(storage)?
            .get(0);
        if actual != view.bytes {
            return Err(invalid("catalogue identity collision"));
        }
        for program in &view.programs {
            seed_retained_recipe_intents_in_transaction(
                &tx,
                &self.qualification_scope,
                program.program().inputs().instruction(),
            )
            .await
            .map_err(invalid)?;
        }
        let routing: Vec<_> = std::iter::once(&self.reply)
            .chain(view.programs.iter())
            .filter(|p| view.routing_enabled(p))
            .map(|p| p.program().inputs().instruction())
            .collect();
        brassclaw_engine::memory::intent_system::RetainedIntentEligibility::from_instructions(
            &routing,
        )
        .map_err(invalid)?;
        crate::mcp_command_qualification::verify_routing_metadata(
            &tx,
            &self.qualification_scope,
            &routing,
        )
        .await
        .map_err(invalid)?;
        tx.execute("INSERT INTO brassclaw_monty_catalogue_activations(activation_id,actor,request_bytes,request_checksum,catalogue_id) VALUES($1,$2,$3,$4,$5)", &[&activation, &actor, &bytes, &digest(&bytes), &view.generation]).await.map_err(storage)?;
        tx.execute("UPDATE brassclaw_monty_active_catalogue SET catalogue_id=$1 WHERE singleton AND catalogue_id=$2", &[&view.generation, &expected]).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        let generation = view.generation;
        self.mcp_discovery
            .publish_unadvertised_installed(discovery_expected, generation, &advertised)
            .map_err(invalid)?;
        drop(advertised);
        *self.active.write().await = Arc::new(view);
        drop(_publication);
        drop(client);
        self.refresh_active_discovery().await.map_err(invalid)?;
        Ok(CatalogueActivationReceipt {
            activation_id: activation.to_string(),
            generation: generation.to_string(),
            catalogue_activated: true,
        })
    }

    pub(crate) async fn prepare_authored(
        &self,
        tx: &Transaction<'_>,
        recipes: &[CatalogueRecipeSelection],
        policy: brassclaw_host_api::MontyExecutionLimits,
    ) -> Result<CatalogueGeneration, RecipeStoreError> {
        let disabled = self.active.read().await.disabled_recipes.clone();
        self.prepare_with_policy(
            tx,
            recipes,
            policy,
            "authored-monty-catalogue/3",
            disabled,
            None,
        )
        .await
    }

    async fn prepare_with_policy(
        &self,
        tx: &Transaction<'_>,
        recipes: &[CatalogueRecipeSelection],
        policy: brassclaw_host_api::MontyExecutionLimits,
        format: &str,
        disabled_recipes: BTreeSet<Uuid>,
        retained_policy: Option<&Value>,
    ) -> Result<CatalogueGeneration, RecipeStoreError> {
        policy.validate().map_err(invalid)?;
        if let Some(document) = retained_policy {
            let recorded = serde_json::from_value::<brassclaw_host_api::MontyExecutionLimits>(
                document.clone(),
            )
            .map_err(invalid)?;
            if format == "authored-monty-catalogue/1" || recorded != policy {
                return Err(invalid("retained catalogue preparation policy differs"));
            }
        }
        if recipes.len() as u64 > u64::from(policy.max_catalogue_recipes) {
            return Err(invalid("catalogue recipe capacity exceeded"));
        }
        let preparation = brassclaw_monty_host::utility::UtilityLimits::from_execution(policy)
            .map_err(invalid)?;
        let mut programs = Vec::new();
        let mut selected = BTreeMap::<Uuid, ComponentRevisionRef>::new();
        let mut documents = BTreeMap::new();
        for packaged in [&self.reply, &self.history] {
            for revision in packaged
                .program()
                .inputs()
                .instruction()
                .snapshot()
                .revisions()
                .values()
            {
                selected.insert(revision.reference().uuid, revision.reference());
                documents.insert(
                    revision.reference().uuid,
                    revision.draft().exact_bytes().to_owned(),
                );
            }
        }
        let mut roots = BTreeSet::new();
        let mut retained_bytes = 0usize;
        for recipe in recipes {
            let root = identifier(&recipe.recipe_id)?;
            if !roots.insert(root)
                || root == id("reply:recipe")
                || root == id("history:recipe")
                || recipe.components.len() as u64
                    > u64::from(policy.max_catalogue_components_per_recipe)
            {
                return Err(invalid("duplicate/reserved recipe or excessive graph"));
            }
            let refs: Vec<_> = recipe
                .components
                .iter()
                .map(crate::pg_review_submission::reference)
                .collect::<Result<_, _>>()?;
            let snapshot = Arc::new(
                PgComponentRevisionStore::read_exact_in_transaction(tx, &[root], &refs)
                    .await
                    .map_err(invalid)?,
            );
            for revision in snapshot.revisions().values() {
                let reference = revision.reference();
                if selected
                    .insert(reference.uuid, reference)
                    .is_some_and(|old| old != reference)
                {
                    return Err(invalid("conflicting component revisions in catalogue"));
                }
                documents.insert(reference.uuid, revision.draft().exact_bytes().to_owned());
            }
            let variants = snapshot.revisions()[&root].draft().document()["variants"]
                .as_array()
                .ok_or_else(|| invalid("recipe variants missing"))?;
            if variants.is_empty() {
                return Err(invalid("activated Recipe requires a selected variant"));
            }
            if variants.len() as u64 > u64::from(policy.max_catalogue_variants_per_recipe) {
                return Err(invalid("recipe variant capacity exceeded"));
            }
            let graph_bytes: usize = snapshot
                .revisions()
                .values()
                .try_fold(0usize, |total, r| {
                    total.checked_add(r.draft().exact_bytes().len())
                })
                .ok_or_else(|| invalid("retained catalogue byte count overflow"))?;
            retained_bytes = graph_bytes
                .checked_mul(variants.len())
                .and_then(|bytes| retained_bytes.checked_add(bytes))
                .filter(|bytes| *bytes as u64 <= policy.max_catalogue_bytes)
                .ok_or_else(|| invalid("retained catalogue exceeds technical memory capacity"))?;
            let mut used_approvals = BTreeSet::new();
            let mut covered = BTreeSet::new();
            for variant in variants {
                let key = variant["variant_key"]
                    .as_str()
                    .ok_or_else(|| invalid("variant identity missing"))?;
                // The supported runner determines eligibility. Model work is not
                // inferred deterministic merely because an operator labels it so.
                let compiled = compile_retained_recipe(
                    snapshot.clone(),
                    root,
                    key,
                    match recipe.workflow_class {
                        brassclaw_product_workflow::CatalogueWorkflowClass::Deterministic => {
                            WorkflowClass::Deterministic
                        }
                        brassclaw_product_workflow::CatalogueWorkflowClass::RequiresModel => {
                            WorkflowClass::RequiresModel
                        }
                    },
                )
                .map_err(invalid)?;
                let program = prepare_retained_tool_program(compiled).map_err(invalid)?;
                let mut canonical_generation = None;
                for owner in &self.generation_approvals {
                    if let Some(formatter) = owner.canonical_coverage(&program).map_err(invalid)? {
                        owner.verify_in_transaction(tx).await.map_err(invalid)?;
                        covered.insert(formatter);
                        canonical_generation = Some(owner);
                    }
                }
                let mut records = BTreeMap::new();
                let mut approvals = BTreeMap::new();

                for (step, binding) in program.bindings() {
                    for r in binding.combination() {
                        covered.insert(r.uuid);
                    }
                    if let Some(bootstrap) = self.bootstrap_usage(binding) {
                        bootstrap.verify_in_transaction(tx).await.map_err(invalid)?;
                        let skill = binding.skill().uuid.to_string();
                        if let Some(supplied) = recipe.association_approvals.get(&skill) {
                            if identifier(supplied)? != bootstrap.approval_id() {
                                return Err(invalid("selected bootstrap approval differs"));
                            }
                            used_approvals.insert(skill);
                        }

                        approvals.insert(
                            step.clone(),
                            match bootstrap {
                                crate::installed_monty_catalogue::BootstrapUsageApproval::Reply(owner)=>owner.usage_approval(binding),
                                crate::installed_monty_catalogue::BootstrapUsageApproval::Generation(owner)=>owner.usage_approval(binding),
                                crate::installed_monty_catalogue::BootstrapUsageApproval::Compiler(owner)=>owner.usage_approval(binding),
                                crate::installed_monty_catalogue::BootstrapUsageApproval::Encoding(owner)=>owner.usage_approval(binding),
                            }.ok_or_else(|| invalid("bootstrap usage differs"))?,
                        );
                    } else {
                        let skill = binding.skill().uuid.to_string();
                        let approval = identifier(
                            recipe
                                .association_approvals
                                .get(&skill)
                                .ok_or_else(|| invalid("selected usage approval missing"))?,
                        )?;
                        used_approvals.insert(skill);
                        records.insert(
                            step.clone(),
                            retain_authored_association_reviews(
                                tx,
                                approval,
                                binding.association(),
                                binding.combination(),
                            )
                            .await
                            .map_err(invalid)?,
                        );
                    }
                    if program.inputs().instruction().class() == WorkflowClass::Deterministic
                        && matches!(
                            binding.capability_id(),
                            "builtin.shell" | "builtin.spawn_subagent" | "host.kohai_complete"
                        )
                    {
                        return Err(invalid(
                            "shell, subagent and model workflows require the model tier",
                        ));
                    }
                    self.verify_implementation(tx, &program, binding)
                        .await
                        .map_err(invalid)?;
                }
                for (step, record) in &records {
                    approvals.insert(step.clone(), record.approval());
                }
                let program = program
                    .with_association_approvals(&approvals)
                    .map_err(invalid)?;
                let inspected = InspectedRetainedProgram::inspect_with_limits(
                    RetainedProgram::Tools(Arc::new(program)),
                    &self.worker,
                    preparation,
                )
                .await
                .map_err(invalid)?;
                if let Some(owner) = canonical_generation {
                    owner
                        .require_selected_program(&inspected)
                        .map_err(invalid)?;
                }
                if inspected.preload_order().is_empty()
                    || inspected
                        .program()
                        .program()
                        .steplist
                        .iter()
                        .any(|step| inspected.invocation(&step.step_id).is_none())
                {
                    return Err(invalid(
                        "activated PythonCode requires the qualified preload interface",
                    ));
                }
                programs.push(Arc::new(inspected));
            }
            // Standalone executable components need their own supported
            // approval mechanism. Do not treat a valid schema as approval.
            if snapshot
                .revisions()
                .values()
                .any(|r| r.reference().uuid != root && !covered.contains(&r.reference().uuid))
            {
                return Err(invalid(
                    "dependency lacks retained component approval coverage",
                ));
            }
            if recipe
                .association_approvals
                .keys()
                .any(|k| !used_approvals.contains(k))
            {
                return Err(invalid("unused or mismatched association approval"));
            }
        }
        brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage::validate_catalogue(
            programs.iter().map(|program| program.program().inputs()),
        )
        .map_err(invalid)?;
        let prefix = documents.values().cloned().collect::<Vec<_>>().join("\n\n");
        let selections: Vec<Value> = programs
            .iter()
            .map(|p| {
                let s = p
                    .program()
                    .inputs()
                    .instruction()
                    .retained_selection()
                    .map_err(invalid)?;
                serde_json::from_str(s.exact_bytes()).map_err(invalid)
            })
            .collect::<Result<_, _>>()?;
        let mut document = json!({"format":"authored-monty-catalogue/1", "base":self.generation,
            "recipes":recipes, "recipe_selections":selections,
            "reply_selection":serde_json::from_str::<Value>(self.reply.program().inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes()).map_err(invalid)?,
            "prefix_checksum":digest(&prefix)});
        if format != "authored-monty-catalogue/1" {
            document["format"] = json!(format);
            // Historical manifests can omit settings introduced afterwards.
            // Decode their defaults for execution, but retain the exact stored
            // fields for identity. New activation writes the complete policy.
            document["preparation_policy"] = match retained_policy {
                Some(original) => original.clone(),
                None => serde_json::to_value(policy).map_err(invalid)?,
            };
        }
        if format == "authored-monty-catalogue/3" {
            document["disabled_recipe_ids"] = json!(disabled_recipes);
        } else if !disabled_recipes.is_empty() {
            return Err(invalid(
                "historical catalogue cannot carry routing overrides",
            ));
        }
        let bytes = document.to_string();
        catalogue_document(&bytes)?;
        Ok(CatalogueGeneration {
            generation: id(&digest(&bytes)),
            bytes,
            prefix,
            programs,
            recipes: recipes.to_vec(),
            disabled_recipes,
        })
    }

    pub(crate) async fn restore_authored(&self) -> Result<(), RecipeStoreError> {
        let _serial = self.activation.lock().await;
        self.restore_authored_locked().await
    }
    async fn restore_authored_locked(&self) -> Result<(), RecipeStoreError> {
        let compatibility_policy = self.preparation_policy().await.map_err(|_| {
            RecipeStoreError::Unavailable("acknowledged catalogue settings unavailable".into())
        })?;
        let mut client = self.pool.get().await.map_err(storage)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(storage)?;
        tx.execute("INSERT INTO brassclaw_monty_active_catalogue(singleton,catalogue_id) VALUES(true,$1) ON CONFLICT(singleton) DO NOTHING", &[&self.generation]).await.map_err(storage)?;
        let row = tx.query_one("SELECT c.catalogue_id,c.catalogue_bytes,c.checksum FROM brassclaw_monty_active_catalogue a JOIN brassclaw_monty_boot_catalogues c USING(catalogue_id) WHERE singleton", &[]).await.map_err(storage)?;
        let generation: Uuid = row.get(0);
        let bytes: String = row.get(1);
        if digest(&bytes) != row.get::<_, String>(2) {
            return Err(invalid("active catalogue integrity failed"));
        }
        if generation == self.generation {
            tx.commit().await.map_err(storage)?;
            return Ok(());
        }
        let document = catalogue_document(&bytes)?;
        if (document["format"] != "authored-monty-catalogue/1"
            && document["format"] != "authored-monty-catalogue/2"
            && document["format"] != "authored-monty-catalogue/3")
            || document["base"] != json!(self.generation)
        {
            return Err(invalid(
                "active catalogue requires an explicit compatible installation upgrade",
            ));
        }
        let recipes: Vec<CatalogueRecipeSelection> =
            serde_json::from_value(document["recipes"].clone()).map_err(invalid)?;
        let format = document["format"]
            .as_str()
            .ok_or_else(|| invalid("catalogue format missing"))?;
        let disabled = if format == "authored-monty-catalogue/3" {
            let ids: Vec<String> =
                serde_json::from_value(document["disabled_recipe_ids"].clone()).map_err(invalid)?;
            let selected: BTreeSet<_> = ids
                .iter()
                .map(|s| identifier(s))
                .collect::<Result<_, _>>()?;
            if selected.len() != ids.len() {
                return Err(invalid("duplicate retained routing disable"));
            }
            selected
        } else {
            BTreeSet::new()
        };
        let retained = format != "authored-monty-catalogue/1";
        let policy = if retained {
            serde_json::from_value(document["preparation_policy"].clone()).map_err(invalid)?
        } else {
            // Compatibility for historical manifests that did not record a
            // preparation policy. Do not claim these have the new retention
            // contract; preserve their exact existing format/identity.
            compatibility_policy
        };
        let view = self
            .prepare_with_policy(
                &tx,
                &recipes,
                policy,
                format,
                disabled,
                retained.then_some(&document["preparation_policy"]),
            )
            .await?;
        if view.generation != generation || view.bytes != bytes {
            return Err(invalid("active catalogue selection differs"));
        }
        tx.commit().await.map_err(storage)?;
        *self.active.write().await = Arc::new(view);
        drop(client);
        self.refresh_active_discovery().await.map_err(invalid)
    }

    async fn verify_implementation(
        &self,
        tx: &Transaction<'_>,
        program: &RetainedToolProgram,
        binding: &brassclaw_engine::memory::retained_tools::RetainedToolBinding,
    ) -> Result<(), crate::RebornBuildError> {
        let rows = tx.query("SELECT id,capability_id FROM reborn_tools WHERE tenant_id=$1 AND (id=$2 OR capability_id=$3)",
            &[&self.qualification_scope.tenant_id, &binding.tool().uuid, &binding.capability_id()]).await
            .map_err(|_| crate::RebornBuildError::InvalidConfig { reason:"Tool registration identity read failed".into() })?;
        if rows.len() != 1
            || rows[0].get::<_, Uuid>(0) != binding.tool().uuid
            || rows[0].get::<_, String>(1) != binding.capability_id()
        {
            return Err(crate::RebornBuildError::InvalidConfig {
                reason: "missing or conflicting Tool registration identity".into(),
            });
        }
        let doc = program.inputs().instruction().snapshot().revisions()[&binding.tool().uuid]
            .draft()
            .document();
        if matches!(
            binding.capability_id(),
            "host.post_reply" | "host.kohai_complete"
        ) {
            verify_packaged_task_implementation(
                &self._image,
                binding.capability_id(),
                binding.association().callable(),
                &doc["implementation"],
            )
            .await?;
        } else {
            let expected = serde_json::from_value(doc["implementation"]["identity"].clone())
                .map_err(|_| crate::RebornBuildError::InvalidConfig {
                    reason: "retained native implementation identity missing".into(),
                })?;
            let capability = self
                .kernel
                .builtin(
                    &brassclaw_host_api::CapabilityId::new(binding.capability_id())?,
                    Some(expected),
                )
                .map_err(|e| crate::RebornBuildError::InvalidConfig {
                    reason: e.to_string(),
                })?;
            capability.verify_native_artifact().await.map_err(|e| {
                crate::RebornBuildError::InvalidConfig {
                    reason: e.to_string(),
                }
            })?;
        }
        Ok(())
    }
}

// Task-bound adapters are compiled into the captured executable rather than
// registered as reusable singleton builtin handlers. Verify their exact ABI
// declaration AND the retained executable bytes before accepting a selection.
// This supplies no association approval, activation or Tool permission.
async fn verify_packaged_task_implementation(
    image: &Arc<brassclaw_host_runtime::NativeExecutableImage>,
    capability: &str,
    callable: &str,
    implementation: &Value,
) -> Result<(), crate::RebornBuildError> {
    let invalid = || crate::RebornBuildError::InvalidConfig {
        reason: "retained task implementation differs".into(),
    };
    let (format, adapter) = match capability {
        "host.post_reply" => ("packaged-task-reply/1", "TaskReplyCapability/1"),
        "host.kohai_complete" => ("packaged-task-model/1", "TaskModelCapability/1"),
        _ => return Err(invalid()),
    };
    if callable != capability
        || implementation
            != &json!({
                "format":format, "artifact_checksum":hex::encode(image.checksum()), "adapter":adapter
            })
    {
        return Err(invalid());
    }
    image.verify().await.map_err(|_| invalid())
}

impl InstalledMontyCatalogue {
    pub(crate) async fn verify_selected_approvals(
        &self,
        tx: &Transaction<'_>,
        selected: &InspectedRetainedProgram,
    ) -> Result<(), RecipeStoreError> {
        let RetainedProgram::Tools(program) = selected.program() else {
            return Err(invalid("unsupported selected runner"));
        };
        let pins = program.inputs().instruction().association_approvals();
        if pins.len() != program.bindings().len() {
            return Err(invalid("incomplete approval pins"));
        }
        for (step, binding) in program.bindings() {
            let pin = pins
                .get(step)
                .ok_or_else(|| invalid("approval pin missing"))?;
            let approval_id = identifier(
                pin["approval_id"]
                    .as_str()
                    .ok_or_else(|| invalid("approval identity missing"))?,
            )?;
            let expected = pin["checksum"]
                .as_str()
                .ok_or_else(|| invalid("approval checksum missing"))?;
            if let Some(bootstrap) = self.bootstrap_usage(binding) {
                bootstrap.verify_in_transaction(tx).await.map_err(invalid)?;
                let approval = bootstrap
                    .usage_approval(binding)
                    .ok_or_else(|| invalid("bootstrap usage missing"))?;
                if approval.approval_id() != approval_id
                    || digest(approval.exact_bytes()) != expected
                {
                    return Err(invalid("bootstrap approval pin differs"));
                }
            } else {
                let record = retain_authored_association_reviews(
                    tx,
                    approval_id,
                    binding.association(),
                    binding.combination(),
                )
                .await
                .map_err(invalid)?;
                if digest(record.approval().exact_bytes()) != expected {
                    return Err(invalid("authored approval pin differs"));
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn refresh_active_discovery(
        &self,
    ) -> Result<(), crate::mcp_recipe_catalogue::McpDiscoveryError> {
        let _publication = self.discovery_publication.lock().await;
        let view = self.active.read().await.clone();
        let programs: Vec<_> = std::iter::once(self.reply.as_ref())
            .chain(std::iter::once(self.history.as_ref()))
            .chain(view.programs.iter().map(Arc::as_ref))
            .filter(|p| view.routing_enabled(p))
            .collect();
        let routing: Vec<_> = std::iter::once(self.reply.as_ref())
            .chain(view.programs.iter().map(Arc::as_ref))
            .filter(|p| view.routing_enabled(p))
            .collect();
        let current = self.mcp_discovery.snapshot().ok();
        if current.as_ref().is_some_and(|v| {
            v.generation() == view.generation && v.qualification_checksum().is_some()
        }) {
            return Ok(());
        }
        let expected = current.map(|v| v.generation());
        match crate::mcp_command_qualification::qualify_authored_commands(
            &self.pool,
            &self.qualification_scope,
            view.generation,
            &programs,
            &routing,
            self,
        )
        .await
        {
            Ok(proof) => self.mcp_discovery.publish_qualified_installed(
                expected,
                view.generation,
                &programs,
                &proof,
            ),
            Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Unqualified) => self
                .mcp_discovery
                .publish_unadvertised_installed(expected, view.generation, &programs),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod packaged_task_implementation_tests {
    use super::*;

    #[tokio::test]
    async fn packaged_model_qualification_verifies_actual_image_and_exact_adapter_contract() {
        let image = brassclaw_host_runtime::NativeExecutableImage::capture(
            brassclaw_host_api::MontyExecutionLimits::default().max_native_artifact_bytes,
        )
        .await
        .unwrap();
        let model = brassclaw_skills::model_completion_components::model_completion_tool_draft(
            Uuid::new_v4(),
            &hex::encode(image.checksum()),
        )
        .unwrap();
        let declaration = &model.document()["implementation"];
        verify_packaged_task_implementation(
            &image,
            "host.kohai_complete",
            "host.kohai_complete",
            declaration,
        )
        .await
        .unwrap();
        for field in ["format", "artifact_checksum", "adapter"] {
            let mut wrong = declaration.clone();
            wrong[field] = json!("different");
            assert!(
                verify_packaged_task_implementation(
                    &image,
                    "host.kohai_complete",
                    "host.kohai_complete",
                    &wrong
                )
                .await
                .is_err()
            );
        }
        let mut extra = declaration.clone();
        extra["unreviewed"] = json!(true);
        assert!(
            verify_packaged_task_implementation(
                &image,
                "host.kohai_complete",
                "host.kohai_complete",
                &extra
            )
            .await
            .is_err()
        );
        assert!(
            verify_packaged_task_implementation(
                &image,
                "host.kohai_complete",
                "host.post_reply",
                declaration
            )
            .await
            .is_err()
        );
        assert!(
            verify_packaged_task_implementation(
                &image,
                "host.post_reply",
                "host.post_reply",
                declaration
            )
            .await
            .is_err()
        );
        assert!(
            verify_packaged_task_implementation(
                &image,
                "builtin.model",
                "builtin.model",
                declaration
            )
            .await
            .is_err()
        );
        let reply = json!({"format":"packaged-task-reply/1", "artifact_checksum":hex::encode(image.checksum()), "adapter":"TaskReplyCapability/1"});
        verify_packaged_task_implementation(&image, "host.post_reply", "host.post_reply", &reply)
            .await
            .unwrap();
    }
}
