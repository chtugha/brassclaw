//! Closed first-party qualification of the exact packaged B1/B2 answer Recipe.
//! This owner constructs QualifiedPrefixFreeWorkflow; general authored support,
//! seed labels and JSON evidence cannot construct the opaque successful type.
//! The three unavailable() guards in task_model_provider.rs and the
//! PrefixFreeWorkflow=>false gate in global_recipe_ports.rs remain in effect
//! until this function can return Ok(Some(_)).
use crate::bootstrap_workflow_evidence as evidence;
#[path = "bootstrap_free_workflow_fixture.rs"]
mod fixture;
#[path = "bootstrap_free_workflow_validation.rs"]
mod validation;
use crate::{
    RebornBuildError, bootstrap_reply_validation::invalid, monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_api::CapabilityId;
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    bootstrap_answer_workflow::{BootstrapAnswerWorkflowIds, bootstrap_answer_workflow_drafts},
    component_revision::REVISION_LIMITS,
    global_bootstrap_components::UsageComponentIds,
    model_completion_components::{
        model_answer_text_draft, model_completion_usage_drafts, model_response_json_usage_drafts,
    },
    prefix_bootstrap_components::{bootstrap_answer_draft, bootstrap_prompt_draft},
    prefix_compiler_components::model_budget_draft,
    revision_store::PgComponentRevisionStore,
    strict_json_components::{StrictJsonUsageIds, strict_json_usage_drafts},
    value_contract::strict_json,
};
use serde_json::json;
use std::{collections::BTreeSet, path::Path, sync::Arc};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

/// Opaque qualification proof. Constructible only after exact Q1/behavior/oracle
/// evidence is durably retained and read back coherently by this module.
/// No Deserialize; no label/JSON path constructs this type.
pub(super) struct QualifiedPrefixFreeWorkflow {
    #[allow(dead_code)]
    program: Arc<InspectedRetainedProgram>,
}

impl QualifiedPrefixFreeWorkflow {
    /// The exact inspected program, independently of any mutable source path.
    #[allow(dead_code)]
    pub(super) fn program(&self) -> Arc<InspectedRetainedProgram> {
        self.program.clone()
    }

    /// Verify that exact stored Q1/behavior evidence still holds at call time.
    /// Called during verify_retained_selection; never performs model I/O.
    pub(super) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        let subject = Subject::from_program(&self.program)?;
        subject
            .read(tx)
            .await?
            .ok_or_else(|| invalid("free workflow qualification evidence missing"))?;
        Ok(())
    }
}

/// Stable component IDs for the B1/B2 answer workflow, derived from
/// packaged_component_id so evidence is keyed consistently across runs.
struct FreeWorkflowComponentIds {
    workflow: BootstrapAnswerWorkflowIds,
    model: UsageComponentIds,
    prompt_parser: UsageComponentIds,
    response: UsageComponentIds,
    strict: StrictJsonUsageIds,
    answer_text: Uuid,
    answer_stage: Uuid,
    budget: Uuid,
    reply_ids: UsageComponentIds,
}

fn allocate_ids(kohai_tool: Uuid, json_tool: Uuid, reply_tool: Uuid) -> FreeWorkflowComponentIds {
    let id = crate::installed_monty_catalogue::id;
    FreeWorkflowComponentIds {
        workflow: BootstrapAnswerWorkflowIds {
            recipe: id("free-workflow:recipe"),
            prompt: id("free-workflow:prompt"),
            protocol: id("free-workflow:protocol"),
        },
        model: UsageComponentIds {
            recipe: id("free-workflow:model-recipe"),
            tool: kohai_tool,
            tool_skill: id("free-workflow:model-ts"),
            python_code: id("free-workflow:model-pc"),
            skill: id("free-workflow:model-skill"),
            formatter: None,
        },
        prompt_parser: UsageComponentIds {
            recipe: id("free-workflow:prompt-parser-recipe"),
            tool: json_tool,
            tool_skill: id("free-workflow:prompt-parser-ts"),
            python_code: id("free-workflow:prompt-parser-pc"),
            skill: id("free-workflow:prompt-parser-skill"),
            formatter: Some(id("free-workflow:prompt-parser-formatter")),
        },
        response: UsageComponentIds {
            recipe: id("free-workflow:response-recipe"),
            tool: json_tool,
            tool_skill: id("free-workflow:response-ts"),
            python_code: id("free-workflow:response-pc"),
            skill: id("free-workflow:response-skill"),
            formatter: Some(id("free-workflow:response-formatter")),
        },
        strict: StrictJsonUsageIds {
            tool: json_tool,
            tool_skill: id("free-workflow:strict-ts"),
            python_code: id("free-workflow:strict-pc"),
            skill: id("free-workflow:strict-skill"),
        },
        answer_text: id("free-workflow:answer-text"),
        answer_stage: id("free-workflow:answer-stage"),
        budget: id("free-workflow:budget"),
        reply_ids: UsageComponentIds {
            recipe: id("free-workflow:reply-recipe"),
            tool: reply_tool,
            tool_skill: id("free-workflow:reply-ts"),
            python_code: id("free-workflow:reply-pc"),
            skill: id("free-workflow:reply-skill"),
            formatter: None,
        },
    }
}

/// Carries the program plus its derived evidence IDs and Q1 bytes.
struct Subject {
    program: Arc<InspectedRetainedProgram>,
    ids: validation::FreeWorkflowIds,
    q1: Uuid,
    behavior: Uuid,
    q1_bytes: String,
}
impl Subject {
    fn from_program(inspected: &Arc<InspectedRetainedProgram>) -> Result<Self, RebornBuildError> {
        let instruction = inspected.program().inputs().instruction();
        let recipe = instruction.recipe().uuid;
        let protocol = crate::installed_monty_catalogue::id("free-workflow:protocol");
        // Verify the protocol component is actually present in the graph.
        if !instruction.snapshot().revisions().contains_key(&protocol) {
            return Err(invalid(
                "free-workflow protocol component not found in program",
            ));
        }
        let ids = validation::FreeWorkflowIds { recipe, protocol };
        let selection_checksum = hex::encode(
            instruction
                .retained_selection()
                .map_err(invalid)?
                .checksum(),
        );
        // Derive stable evidence UUIDs from the recipe UUID and selection checksum.
        let identity = Uuid::new_v5(
            &recipe,
            format!("brassclaw-bootstrap-free-workflow-qualification/1:{selection_checksum}")
                .as_bytes(),
        );
        let q1 = Uuid::new_v5(&identity, b"q1");
        let behavior = Uuid::new_v5(&identity, b"behavior");
        let q1_bytes = json!({
            "format": "workflow-review-evidence/1",
            "evidence_id": q1,
            "kind": "q1",
            "validation_mode": "system_seed",
            "selection_checksum": selection_checksum,
            "succeeded": true,
            "report": {
                "header": {
                    "producer": "brassclaw-bootstrap-free-workflow-qualification/1",
                    "scope": "exact-packaged-b1-b2-answer-workflow/1"
                },
                "checks": [
                    "exact-packaged-root-and-recursive-dependencies",
                    "three-immutable-tool-bindings-kohai-json-json",
                    "pure-protocol-component-present-and-evaluable",
                    "b1-and-b2-answer-cases-validated-against-oracle",
                    "negative-cases-confirmed-failing"
                ],
                "model_quality": "unmeasured",
                "cache_quality": "unmeasured"
            }
        })
        .to_string();
        Ok(Self {
            program: inspected.clone(),
            ids,
            q1,
            behavior,
            q1_bytes,
        })
    }

    async fn read(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<Option<QualifiedPrefixFreeWorkflow>, RebornBuildError> {
        // Q1 evidence must match exactly.
        let q1_stored =
            evidence::read_workflow(tx, &self.program, self.q1, evidence::WorkflowKind::Q1).await?;
        match q1_stored.as_deref() {
            None => return Ok(None),
            Some(s) if s != self.q1_bytes.as_str() => {
                return Err(invalid("free-workflow Q1 provenance differs"));
            }
            _ => {}
        }
        // Behavior evidence must exist.
        let Some(behavior_bytes) = evidence::read_workflow(
            tx,
            &self.program,
            self.behavior,
            evidence::WorkflowKind::Behavior,
        )
        .await?
        else {
            return Ok(None);
        };
        // Verify behavior record structure and oracle.
        let record = strict_json(&behavior_bytes, REVISION_LIMITS).map_err(invalid)?;
        let report = &record["report"];
        if report.as_object().is_none_or(|v| v.len() != 2) || report["observed"].is_null() {
            return Err(invalid("free-workflow behavior report structure differs"));
        }
        validation::verify(tx, &self.program, &self.ids, &report["observed"]).await?;
        Ok(Some(QualifiedPrefixFreeWorkflow {
            program: self.program.clone(),
        }))
    }
}

/// Attempt to qualify the B1/B2 answer workflow from existing pool owners.
/// Returns Ok(None) when prerequisite evidence is absent or the behavioral
/// fixture execution has not yet been completed.
pub(super) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    scope: &brassclaw_engine::memory::intent_system::IntentScope,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<Option<QualifiedPrefixFreeWorkflow>, RebornBuildError> {
    // Resolve the installed tool IDs needed for the free workflow candidate.
    let client = pool.get().await.map_err(invalid)?;
    let tenant = scope.tenant_id.clone();
    // Kohai (model completion) tool.
    let kohai_rows = client
        .query(
            "SELECT id FROM reborn_tools WHERE tenant_id=$1 AND name=$2 AND source='system' ORDER BY id",
            &[&tenant, &"host.kohai_complete"],
        )
        .await
        .map_err(invalid)?;
    if kohai_rows.len() != 1 {
        return Ok(None);
    }
    let kohai_tool: Uuid = kohai_rows[0].get(0);
    // JSON tool (for response parsing + strict JSON).
    let json_rows = client
        .query(
            "SELECT id FROM reborn_tools WHERE tenant_id=$1 AND capability_id=$2 AND source='system' ORDER BY id",
            &[&tenant, &brassclaw_host_runtime::JSON_CAPABILITY_ID],
        )
        .await
        .map_err(invalid)?;
    if json_rows.len() != 1 {
        return Ok(None);
    }
    let json_tool: Uuid = json_rows[0].get(0);
    // Reply (post_reply) tool.
    let reply_rows = client
        .query(
            "SELECT id FROM reborn_tools WHERE tenant_id=$1 AND capability_id=$2 AND source='system' ORDER BY id",
            &[&tenant, &"host.post_reply"],
        )
        .await
        .map_err(invalid)?;
    if reply_rows.len() != 1 {
        return Ok(None);
    }
    let reply_tool: Uuid = reply_rows[0].get(0);
    drop(client);

    // Verify the required Tools are registered and their native artifacts current.
    let kohai_cap = CapabilityId::new("host.kohai_complete").map_err(invalid)?;
    let kohai_runtime = kernel
        .builtin(&kohai_cap, None)
        .map_err(|_| invalid("kohai_complete runtime unavailable for free-workflow"))?;
    let json_cap =
        CapabilityId::new(brassclaw_host_runtime::JSON_CAPABILITY_ID).map_err(invalid)?;
    kernel
        .builtin(&json_cap, None)
        .map_err(|_| invalid("JSON runtime unavailable for free-workflow"))?;

    // Check live tool policy via DB.
    let tool_ids = vec![kohai_tool, json_tool];
    {
        let policy_client = pool.get().await.map_err(invalid)?;
        let policies = policy_client
            .query(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=ANY($1)",
                &[&tool_ids],
            )
            .await
            .map_err(invalid)?;
        if tool_ids.len() != 2
            || policies.len() != 2
            || policies.iter().any(|row| !row.get::<_, bool>(0))
        {
            return Ok(None);
        }
    }

    let worker_image = Arc::new(
        PackagedExecutableImage::capture(worker, limits.max_native_artifact_bytes)
            .await
            .map_err(invalid)?,
    );

    let ids = allocate_ids(kohai_tool, json_tool, reply_tool);
    let kohai_artifact_checksum = hex::encode(
        kohai_runtime
            .native_identity()
            .ok_or_else(|| invalid("kohai_complete native identity missing"))?
            .artifact_checksum(),
    );
    let candidate = retain_candidate(&pool, &ids, worker, limits, &kohai_artifact_checksum).await?;
    let subject = Subject::from_program(&candidate)?;

    // Check existing evidence before running oracle.
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    if let Some(approved) = subject.read(&tx).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(Some(approved));
    }
    // Store Q1 intent (atomic DO NOTHING; failure means concurrent begin happened).
    let q1_inserted = evidence::retain_workflow(
        &tx,
        &candidate,
        subject.q1,
        evidence::WorkflowKind::Q1,
        &subject.q1_bytes,
    )
    .await?;
    if !q1_inserted {
        return Err(invalid(
            "free-workflow Q1 intent incomplete; reconcile without replay",
        ));
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);

    // Execute the pure oracle (no model Tool invocation).
    let observed = validation::execute(
        pool.clone(),
        candidate.clone(),
        &subject.ids,
        worker,
        limits,
    )
    .await?;

    // Store behavior evidence and read back coherently.
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;

    let selection_checksum = hex::encode(
        candidate
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .checksum(),
    );
    let behavior_record = json!({
        "format": "workflow-review-evidence/1",
        "evidence_id": subject.behavior,
        "kind": "behavior",
        "validation_mode": "system_seed",
        "selection_checksum": selection_checksum,
        "succeeded": true,
        "report": {
            "header": {
                "producer": "brassclaw-bootstrap-free-workflow-qualification/1",
                "scope": "exact-packaged-b1-b2-answer-workflow/1",
                "owner_checksum": crate::bootstrap_evidence::checksum(include_str!("bootstrap_free_workflow_approval.rs")),
                "fixture_checksum": crate::bootstrap_evidence::checksum(include_str!("bootstrap_free_workflow_fixture.rs")),
                "validator_checksum": crate::bootstrap_evidence::checksum(include_str!("bootstrap_free_workflow_validation.rs")),
                "storage_checksum": crate::bootstrap_evidence::checksum(include_str!("bootstrap_workflow_evidence.rs")),
                "worker_checksum": hex::encode(worker_image.checksum())
            },
            "observed": observed
        }
    })
    .to_string();

    evidence::retain_workflow(
        &tx,
        &candidate,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
        &behavior_record,
    )
    .await?;

    let approved = subject
        .read(&tx)
        .await?
        .ok_or_else(|| invalid("free-workflow qualification readback missing"))?;
    tx.commit().await.map_err(invalid)?;
    Ok(Some(approved))
}

async fn retain_candidate(
    pool: &Arc<PgPool>,
    ids: &FreeWorkflowComponentIds,
    worker: &Path,
    limits: UtilityLimits,
    kohai_artifact_checksum: &str,
) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
    let mut all_drafts = Vec::new();

    // Model completion usage drafts.
    all_drafts.extend(
        model_completion_usage_drafts(ids.model, ids.prompt_parser, ids.reply_ids)
            .map_err(invalid)?,
    );
    // Response JSON usage drafts.
    all_drafts
        .extend(model_response_json_usage_drafts(ids.response, ids.reply_ids).map_err(invalid)?);
    // Strict JSON usage drafts.
    all_drafts.extend(strict_json_usage_drafts(ids.strict).map_err(invalid)?);
    // Pure components.
    all_drafts.push(model_answer_text_draft(ids.answer_text).map_err(invalid)?);
    all_drafts.push(bootstrap_answer_draft(ids.answer_stage, ids.budget).map_err(invalid)?);
    all_drafts.push(model_budget_draft(ids.budget).map_err(invalid)?);
    all_drafts.push(bootstrap_prompt_draft(ids.workflow.prompt).map_err(invalid)?);
    // Kohai Tool class-0 draft with actual artifact checksum.
    all_drafts.push(
        brassclaw_skills::model_completion_components::model_completion_tool_draft(
            ids.model.tool,
            kohai_artifact_checksum,
        )
        .map_err(invalid)?,
    );

    // Keep only the direct dependencies of the answer workflow
    // (strip consuming Recipe / formatter drafts from the usage helpers).
    let needed: BTreeSet<Uuid> = [
        ids.model.tool,
        ids.model.tool_skill,
        ids.model.python_code,
        ids.model.skill,
        ids.response.tool_skill,
        ids.response.python_code,
        ids.response.skill,
        ids.strict.tool_skill,
        ids.strict.python_code,
        ids.strict.skill,
        ids.answer_text,
        ids.answer_stage,
        ids.budget,
        ids.workflow.prompt,
    ]
    .into_iter()
    .collect();
    all_drafts.retain(|d| needed.contains(&d.uuid()));

    let store = PgComponentRevisionStore::new(pool.clone());
    let mut refs = Vec::new();
    for draft in &all_drafts {
        refs.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }

    // Dependency snapshot rooted at skill components.
    let dependency_roots: Vec<Uuid> = [
        ids.model.skill,
        ids.response.skill,
        ids.strict.skill,
        ids.answer_text,
        ids.answer_stage,
    ]
    .into_iter()
    .collect();
    let dependencies = store
        .read_exact(&dependency_roots, &refs)
        .await
        .map_err(invalid)?;
    let dep_snapshot = dependencies
        .select_roots(&dependency_roots)
        .map_err(invalid)?;

    // Retain the workflow-specific drafts (recipe, prompt, protocol).
    let workflow_drafts = bootstrap_answer_workflow_drafts(
        ids.workflow,
        ids.model,
        ids.response,
        ids.strict,
        ids.answer_text,
        ids.answer_stage,
    )
    .map_err(invalid)?;
    let mut workflow_refs: Vec<_> = dep_snapshot
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    for draft in &workflow_drafts {
        if dep_snapshot.revisions().contains_key(&draft.uuid()) {
            return Err(invalid("free-workflow draft aliases a retained dependency"));
        }
        workflow_refs.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let snapshot = Arc::new(
        store
            .read_exact(&[ids.workflow.recipe], &workflow_refs)
            .await
            .map_err(invalid)?,
    );
    // Verify the retained graph matches every packaged draft exactly.
    for draft in &workflow_drafts {
        if snapshot
            .revisions()
            .get(&draft.uuid())
            .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "free-workflow packaged graph differs from retained",
            ));
        }
    }
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            snapshot,
            ids.workflow.recipe,
            "selected",
            WorkflowClass::RequiresModel,
        )
        .map_err(invalid)?,
    )
    .map_err(invalid)?;
    Ok(Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(Arc::new(program)),
            worker,
            limits,
        )
        .await
        .map_err(invalid)?,
    ))
}
