//! Retain the packaged global-root draft before runtime admission. This is not
//! protected-root approval, a system-seed provenance writer or VM activation.
//! No legacy row/status is converted into trust and no operator selection moves.
use std::{collections::BTreeSet, sync::Arc};

use brassclaw_pg::PgPool;
use brassclaw_skills::{
    orchestrator_contract::{GlobalRootDefinition, global_root_draft},
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
};
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
