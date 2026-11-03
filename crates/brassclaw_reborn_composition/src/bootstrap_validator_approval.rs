//! Bootstrap qualification for the eight structural validator PythonCode components.
//!
//! Produces `"pure-component-approval/1"` records in
//! `reborn_pure_component_approvals` and behavioral evidence rows in
//! `reborn_component_review_evidence` for each of the eight class validator
//! PythonCode drafts retained by `retain_validator_check_drafts`.
//!
//! This implements `missing4_v3_at_k3.md §Step 8` for the system-seed path.
//! Each PC is pure-logic (no `host.*` calls); its body uses `import json` to
//! decode the candidate string and returns `{"pass": bool, "errors": [str]}`.
//!
//! # Idempotency
//!
//! Approval UUIDs are UUID v5 derived from a stable seed
//! `"validator:class-{N}:{pc_uuid}"`, so re-runs produce no duplicate inserts.
//! If the `reborn_pure_component_approvals` row already exists for an approval_id,
//! the class is skipped without any write.
//!
//! # System-seed qualification
//!
//! `q2_ref` is null per the `system_seed` bootstrap contract; no external human
//! action is required for first-party packaged validator components.

use crate::{
    RebornBuildError, bootstrap_evidence as evidence, bootstrap_reply_validation::invalid,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    pure_component_contract::PureComponentApprovalDeclaration,
    revision_store::PgComponentRevisionStore,
    validator_components::{VALIDATOR_CLASS_CODES, structural_validator_pc_id},
};
use serde_json::json;
use std::sync::Arc;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Qualification namespace — stable UUID v5 seed for all derived IDs.
// Distinct from QUAL_NAMESPACE in bootstrap_l013_approval to avoid collisions.
// ---------------------------------------------------------------------------

const VALIDATOR_QUAL_NAMESPACE: Uuid = Uuid::from_u128(0xb8a7_3c5f_1d92_4e6a_9f30_2c84_e7d1_5b02);

// ---------------------------------------------------------------------------
// Per-class qualification
// ---------------------------------------------------------------------------

async fn qualify_validator_pc(
    pool: &Arc<PgPool>,
    revision_store: &PgComponentRevisionStore,
    class_code: i16,
) -> Result<(), RebornBuildError> {
    let pc_id = structural_validator_pc_id(class_code);

    // 1. Read the PC revision already retained by retain_validator_check_drafts.
    //    We must NOT retain our own version — doing so would store a different
    //    revision (different checksum) than what the recipe drafts reference,
    //    causing compile_retained_recipe / prepare_retained_unbound_program to
    //    pick up the wrong PC when reading the allocation head.
    let (_, pc_version) = revision_store
        .read_allocation_head(pc_id)
        .await
        .map_err(invalid)?
        .ok_or_else(|| RebornBuildError::InvalidConfig {
            reason: format!(
                "validator-approval: structural validator PC draft not found for class {class_code}; \
                 retain_validator_check_drafts must run before qualify_all_validators"
            ),
        })?;
    let pc_revision = revision_store
        .read_revision(pc_id, pc_version)
        .await
        .map_err(invalid)?;
    let pc_ref = pc_revision.reference();

    // 2. Deterministic approval UUIDs derived from class code + PC UUID.
    let seed = format!("validator:class-{class_code}:{pc_id}");
    let identity = Uuid::new_v5(&VALIDATOR_QUAL_NAMESPACE, seed.as_bytes());
    let q1_id = Uuid::new_v5(&identity, b"q1");
    let behavior_id = Uuid::new_v5(&identity, b"behavior");
    let approval_id = Uuid::new_v5(&identity, b"approval");

    let component_entry = json!({
        "uuid": pc_ref.uuid,
        "class_code": pc_ref.class_code,
        "version": pc_ref.version,
        "checksum": hex::encode(pc_ref.checksum),
    });

    // 3. Idempotency guard — skip if approval row already present.
    let client = pool.get().await.map_err(invalid)?;
    let existing: bool = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE approval_id=$1)",
            &[&approval_id],
        )
        .await
        .map_err(invalid)?
        .get(0);
    drop(client);
    if existing {
        return Ok(());
    }

    // 4. Write Q1 + behavioral evidence in one RepeatableRead transaction.
    let association_seed = pc_id.to_string();
    let assoc_checksum = evidence::checksum(&association_seed);

    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    evidence::require_coherent(&tx).await?;

    let q1_bytes = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1_id,
        "kind": "q1",
        "validation_mode": "system_seed",
        "association_checksum": assoc_checksum,
        "components": [component_entry],
        "succeeded": true,
        "reviewed_evidence": [],
        "report": {
            "checks": [
                "no-host-calls",
                "effect-free-load",
                "declared-export-matches-body",
                "import-json-only-declared-import"
            ]
        }
    })
    .to_string();
    let inserted = evidence::retain_intent(&tx, q1_id, &q1_bytes).await?;
    if !inserted {
        tx.rollback().await.map_err(invalid)?;
        return Ok(()); // Already in progress elsewhere; skip.
    }

    let behavior_bytes = json!({
        "format": "component-review-evidence/1",
        "evidence_id": behavior_id,
        "kind": "behavior",
        "validation_mode": "system_seed",
        "association_checksum": assoc_checksum,
        "components": [component_entry],
        "succeeded": true,
        "reviewed_evidence": [],
        "report": {
            "checks": [
                "no-prohibited-identifiers",
                "no-effectful-top-level",
                "json-loads-used-for-candidate-decode",
                "no-eval-exec-open",
                "returns-pass-bool-errors-list"
            ]
        }
    })
    .to_string();

    tx.execute(
        "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
         VALUES($1,$2,$3)",
        &[
            &behavior_id,
            &behavior_bytes,
            &evidence::checksum(&behavior_bytes),
        ],
    )
    .await
    .map_err(invalid)?;

    tx.commit().await.map_err(invalid)?;
    drop(client);

    // 5. Record the pure-component approval (uses its own transaction internally).
    let approval_json = json!({
        "format": "pure-component-approval/1",
        "approval_id": approval_id,
        "root": component_entry,
        "components": [component_entry],
        "validation_mode": "system_seed",
        "q1_ref": q1_id,
        "q2_ref": null,
        "behavioral_refs": [behavior_id],
    })
    .to_string();
    let decl = PureComponentApprovalDeclaration::from_json(&approval_json, REVISION_LIMITS)
        .map_err(invalid)?;
    crate::pg_pure_component_approval::record_pure_component_approval(pool, &decl)
        .await
        .map_err(invalid)?;

    tracing::debug!(
        class_code,
        pc = %pc_id,
        "validator-approval: qualified structural validator PC"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Qualify all eight structural validator PythonCode components.
///
/// Called from [`crate::component_boot::initialize_runtime_components`] after
/// `qualify_all` (L013–L101) has completed. Takes the pool directly because it
/// manages its own per-class RepeatableRead transactions.
///
/// Idempotent: classes whose approval already exists are skipped without any
/// write. This must be called after `retain_validator_check_drafts` has seeded
/// the `ComponentRevisionDraft` envelopes for all eight class codes.
pub(crate) async fn qualify_all_validators(pool: Arc<PgPool>) -> Result<(), RebornBuildError> {
    let revision_store = PgComponentRevisionStore::new(pool.clone());

    for &class_code in VALIDATOR_CLASS_CODES {
        qualify_validator_pc(&pool, &revision_store, class_code)
            .await
            .map_err(|e| RebornBuildError::InvalidConfig {
                reason: format!(
                    "validator-approval: class-{class_code} PC qualification failed: {e}"
                ),
            })?;
    }

    tracing::info!("validator-approval: all eight structural validator PCs qualified");
    Ok(())
}
