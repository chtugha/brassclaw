//! Installation-owned public Recipe *candidate* population. This is immutable
//! draft retention and source/interface preparation, never activation, approval,
//! ordinary routing or advertising. It cannot publish a discovery generation.
use std::{collections::BTreeMap, path::Path, sync::Arc};

use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::RevisionError,
    global_bootstrap_components::{
        GlobalBootstrapUsage, UsageComponentIds, public_reply_drafts, verify_packaged_usage,
    },
    revision_store::PgComponentRevisionStore,
};
use uuid::Uuid;

pub(crate) use crate::bootstrap_reply_validation::RestrictedPolicy as RestrictedBootstrapPolicy;
use crate::{RebornBuildError, mcp_recipe_catalogue::McpRecipeDiscovery};

fn invalid(reason: impl ToString) -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: reason.to_string(),
    }
}

/// Reuse the canonical reply Recipe/Skill identities. The old installed task
/// selection remains exact; the package candidate never changes an active head.
pub(crate) async fn retain_public_reply_candidate(
    pool: Arc<PgPool>,
    private: &InspectedRetainedProgram,
    worker: &Path,
) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
    let RetainedProgram::Tools(program) = private.program() else {
        return Err(invalid("public reply requires the retained Tool adapter"));
    };
    if program.bindings().len() != 1 {
        return Err(invalid("public reply requires one exact usage"));
    }
    let binding = program
        .bindings()
        .values()
        .next()
        .expect("one binding checked");
    let original = program.inputs().instruction();
    let ids = UsageComponentIds {
        recipe: original.recipe().uuid,
        python_code: binding.python().uuid,
        tool_skill: binding.tool_skill().uuid,
        tool: binding.tool().uuid,
        skill: binding.skill().uuid,
        formatter: None,
    };
    if verify_packaged_usage(ids, GlobalBootstrapUsage::PostReply, original.snapshot()).is_err() {
        for expected in public_reply_drafts(ids).map_err(invalid)? {
            if original
                .snapshot()
                .revisions()
                .get(&expected.uuid())
                .is_none_or(|actual| actual.draft().exact_bytes() != expected.exact_bytes())
            {
                return Err(invalid(
                    "reply migration input is neither exact packaged revision",
                ));
            }
        }
    }
    let store = PgComponentRevisionStore::new(pool);
    let mut references = BTreeMap::from([(ids.tool, binding.tool())]);
    for draft in public_reply_drafts(ids).map_err(invalid)? {
        let reference = store.retain_packaged_draft(&draft).await.map_err(invalid)?;
        references.insert(reference.uuid, reference);
    }
    let references: Vec<_> = references.into_values().collect();
    let snapshot = Arc::new(
        store
            .read_exact(&[ids.recipe], &references)
            .await
            .map_err(invalid)?,
    );
    // Compare exact package bytes again after storage, not a source/status label.
    for expected in public_reply_drafts(ids).map_err(invalid)? {
        if snapshot.revisions()[&expected.uuid()].draft().exact_bytes() != expected.exact_bytes() {
            return Err(invalid(RevisionError::Invalid(
                "public candidate package differs",
            )));
        }
    }
    let prepared = prepare_retained_tool_program(
        compile_retained_recipe(
            snapshot,
            ids.recipe,
            "selected",
            WorkflowClass::Deterministic,
        )
        .map_err(invalid)?,
    )
    .map_err(invalid)?;
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(RetainedProgram::Tools(Arc::new(prepared)), worker)
            .await
            .map_err(invalid)?,
    );
    let selection = inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let identity = Uuid::new_v5(&ids.recipe, &selection.checksum());
    let commands = McpRecipeDiscovery::contracts(identity, &[&inspected]).map_err(invalid)?;
    if commands.len() != 1 || !commands.contains_key("publish_literal_reply") {
        return Err(invalid("public reply candidate declaration is missing"));
    }
    Ok(inspected)
}

#[cfg(test)]
pub(crate) async fn assert_public_candidate_retention(
    pool: Arc<PgPool>,
    private: &InspectedRetainedProgram,
    generation: Uuid,
    scope: &brassclaw_engine::memory::intent_system::IntentScope,
) {
    use brassclaw_engine::memory::intent_system::{
        IntentResolution, RetainedIntentEligibility, resolve_catalogue_intent_in_transaction,
    };
    use brassclaw_skills::component_revision::ComponentRevisionDraft;
    use serde_json::{Value, json};
    use tokio_postgres::IsolationLevel;
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let candidate = retain_public_reply_candidate(pool.clone(), private, &worker)
        .await
        .unwrap();
    let successor = candidate.program().inputs().instruction();
    crate::bootstrap_reply_approval::assert_bootstrap_readback(pool.clone(), candidate.clone())
        .await;
    let original = private.program().inputs().instruction();
    assert_eq!(successor.recipe().uuid, original.recipe().uuid);
    assert_eq!(successor.recipe(), original.recipe());
    let active_selection: Value =
        serde_json::from_str(original.retained_selection().unwrap().exact_bytes()).unwrap();
    assert!(active_selection["association_approvals"]["0:2"]["approval_id"].is_string());
    assert!(active_selection["association_approvals"]["0:2"]["checksum"].is_string());
    let RetainedProgram::Tools(candidate_program) = candidate.program() else {
        unreachable!()
    };
    let RetainedProgram::Tools(original_program) = private.program() else {
        unreachable!()
    };
    assert_eq!(
        candidate_program.bindings()["0:2"].python(),
        original_program.bindings()["0:2"].python()
    );
    assert_eq!(
        candidate_program.bindings()["0:2"].tool_skill(),
        original_program.bindings()["0:2"].tool_skill()
    );
    let again = retain_public_reply_candidate(pool.clone(), private, &worker)
        .await
        .unwrap();
    assert_eq!(
        again
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .checksum(),
        successor.retained_selection().unwrap().checksum()
    );
    // A later operator draft must survive package reuse, including its head.
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut operator_document: Value = serde_json::from_str(
        successor.snapshot().revisions()[&successor.recipe().uuid]
            .draft()
            .exact_bytes(),
    )
    .unwrap();
    operator_document["document"]["description"] = json!("Operator-owned draft: preserve me");
    let operator_draft = ComponentRevisionDraft::from_json(&operator_document.to_string()).unwrap();
    let operator = store
        .stage(&operator_draft, successor.recipe().version)
        .await
        .unwrap();
    crate::bootstrap_reply_approval::assert_rejects_unapproved_recipe_successor(
        pool.clone(),
        candidate.clone(),
        operator,
    )
    .await;
    let retained = retain_public_reply_candidate(pool.clone(), private, &worker)
        .await
        .unwrap();
    assert_eq!(
        retained.program().inputs().instruction().recipe(),
        successor.recipe()
    );
    assert_eq!(
        store
            .read_revision(operator.uuid, operator.version)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        operator_draft.exact_bytes()
    );
    let client = pool.get().await.unwrap();
    let head: i64 = client
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&operator.uuid],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(head as u64, operator.version);
    drop(client);
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let eligible = RetainedIntentEligibility::from_instructions(&[original]).unwrap();
    assert!(matches!(
        resolve_catalogue_intent_in_transaction(
            &tx,
            scope,
            "publish literal reply Ready.",
            &eligible
        )
        .await
        .unwrap(),
        IntentResolution::Match { .. }
    ));
    let row = tx
        .query_one(
            "SELECT catalogue_bytes FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1",
            &[&generation],
        )
        .await
        .unwrap();
    let active: Value = serde_json::from_str(row.get(0)).unwrap();
    let active_reply: Value = serde_json::from_str(
        active["components"][successor.recipe().uuid.to_string()]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        active_reply["document"]["mcp_call"]["name"],
        "publish_literal_reply"
    );
    assert_eq!(active["reply_selection"], active_selection);
    tx.commit().await.unwrap();
    // An inspected candidate is deliberately not a discovery/approval proof.
    let discovery = McpRecipeDiscovery::new();
    assert!(
        discovery
            .publish_installed(None, generation, &[&candidate])
            .is_err()
    );
    assert!(discovery.snapshot().is_err());
    let document = successor.snapshot().revisions()[&successor.recipe().uuid]
        .draft()
        .document();
    assert_eq!(
        document["mcp_qualification"]["success_examples"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        document["input_layouts"]["selected"]["task_inputs"]["answer"],
        json!({"type":"string","required":true,"checks":[]})
    );
}
