//! Retain the packaged global-root draft before runtime admission. This is not
//! protected-root approval, a system-seed provenance writer or VM activation.
//! No legacy row/status is converted into trust and no operator selection moves.
use std::{collections::BTreeSet, sync::Arc};

use brassclaw_pg::PgPool;
use brassclaw_skills::{
    orchestrator_contract::{GlobalRootDefinition, global_root_draft},
    review_submission_store::{
        PgReviewSubmissionStore, RetainedReviewSubmission, ReviewSubmissionDraft,
        SubmissionStoreError,
    },
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// Stable identity distinct from the legacy per-conversation orchestrator row.
const ROOT_ID: Uuid = Uuid::from_u128(0x52a1c2df_fb73_4832_ab11_e47b931f0724);

/// Actual root compiler/transport interface. Tool calls by child PythonCode
/// receive their separately prepared binding; root ports grant no Tool authority.
fn ports() -> BTreeSet<String> {
    [
        "await_next_task",
        "enter_task",
        "finish_task",
        "resolve_intent",
        "resolve_component_by_name",
        "compose_orchestrator",
        "run_program",
        "resolve_reply",
        "visible_capabilities",
        "build_prompt_bundle",
        "stream_model",
        "invoke_capability",
        "append_capability_result_ref",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Boot/upgrade retention is idempotent by exact packaged bytes. Source changes
/// append a draft; boot never approves it, selects latest or replaces a live VM.
/// Return the exact stored definition for subsequent protected-root review.
pub(crate) async fn retain_packaged_global_root(
    pool: Arc<PgPool>,
    source: &str,
) -> Result<GlobalRootDefinition, RevisionStoreError> {
    let store = PgComponentRevisionStore::new(pool);
    let draft = global_root_draft(ROOT_ID, source, &ports())?;
    let reference = store.retain_packaged_draft(&draft).await?;
    let snapshot = store.read_exact(&[ROOT_ID], &[reference]).await?;
    Ok(GlobalRootDefinition::from_revision(
        &snapshot.revisions()[&ROOT_ID],
    )?)
}

/// Retain the complete class-10 review subject separately from its source
/// retention. The actor is authoring provenance only, never a review verdict.
pub(crate) async fn retain_packaged_root_review(
    pool: Arc<PgPool>,
    definition: &GlobalRootDefinition,
) -> Result<RetainedReviewSubmission, SubmissionStoreError> {
    let reference = definition.reference();
    if reference.uuid != ROOT_ID {
        return Err(SubmissionStoreError::Invalid);
    }
    let store = PgComponentRevisionStore::new(pool.clone());
    let candidate = store
        .read_revision(reference.uuid, reference.version)
        .await?;
    if candidate.reference() != reference {
        return Err(SubmissionStoreError::Invalid);
    }
    let base = if reference.version > 1 {
        Some(
            store
                .read_revision(reference.uuid, reference.version - 1)
                .await?
                .reference(),
        )
    } else {
        None
    };
    // Version-8 UUID is an idempotent index, not a signature or approval. The
    // store compares the entire subject bytes to reject any identity collision.
    let mut hasher = Sha256::new();
    hasher.update(b"packaged-global-root-review/1\0");
    hasher.update(reference.uuid.as_bytes());
    hasher.update(reference.version.to_be_bytes());
    hasher.update(reference.checksum);
    let mut id = [0; 16];
    id.copy_from_slice(&hasher.finalize()[..16]);
    id[6] = (id[6] & 0x0f) | 0x80;
    id[8] = (id[8] & 0x3f) | 0x80;
    let draft = ReviewSubmissionDraft::new(
        Uuid::from_bytes(id),
        "packaged-global-root",
        candidate.draft().clone(),
        base,
        Vec::new(),
    )?;
    PgReviewSubmissionStore::new(pool)
        .submit_retained(&draft)
        .await
}
