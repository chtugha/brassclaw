//! Immutable workflow proof storage only; no oracle or approval grant.
use crate::bootstrap_evidence::{checksum, require_coherent};
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use tokio_postgres::Transaction;
use uuid::Uuid;

/// Workflow observations use the existing selected-variant evidence store,
/// separately from one-Tool association evidence. These storage helpers cannot
/// approve a workflow; its closed installation owner supplies the actual oracle.
#[derive(Clone, Copy)]
pub(super) enum WorkflowKind {
    Q1,
    Behavior,
}
impl WorkflowKind {
    fn name(&self) -> &'static str {
        match self {
            Self::Q1 => "q1",
            Self::Behavior => "behavior",
        }
    }
}
struct WorkflowFields {
    recipe: Uuid,
    version: i64,
    selection: String,
    checksum: String,
}
fn workflow_fields(
    inspected: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
) -> Result<WorkflowFields, RebornBuildError> {
    let instruction = inspected.program().inputs().instruction();
    let recipe = instruction.recipe();
    if recipe.class_code != 21 {
        return Err(invalid("bootstrap workflow evidence requires a Recipe"));
    }
    let selection = instruction.retained_selection().map_err(invalid)?;
    Ok(WorkflowFields {
        recipe: recipe.uuid,
        version: i64::try_from(recipe.version).map_err(invalid)?,
        selection: selection.exact_bytes().to_owned(),
        checksum: hex::encode(selection.checksum()),
    })
}
fn workflow_record(
    bytes: &str,
    id: Uuid,
    kind: &WorkflowKind,
    fields: &WorkflowFields,
) -> Result<(), RebornBuildError> {
    let record = brassclaw_skills::value_contract::strict_json(
        bytes,
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .map_err(invalid)?;
    if record.as_object().is_none_or(|v| v.len() != 7)
        || record["format"] != "workflow-review-evidence/1"
        || record["evidence_id"] != id.to_string()
        || record["kind"] != kind.name()
        || record["selection_checksum"] != fields.checksum
        || record["validation_mode"] != "system_seed"
        || record["succeeded"] != true
    {
        return Err(invalid("bootstrap workflow evidence envelope differs"));
    }
    Ok(())
}
pub(super) async fn read_workflow(
    tx: &Transaction<'_>,
    inspected: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
    id: Uuid,
    kind: WorkflowKind,
) -> Result<Option<String>, RebornBuildError> {
    require_coherent(tx).await?;
    let fields = workflow_fields(inspected)?;
    let Some(row)=tx.query_opt("SELECT recipe_id,recipe_version,selection_bytes,selection_checksum,evidence_kind,evidence_bytes,checksum FROM reborn_workflow_review_evidence WHERE evidence_id=$1",&[&id]).await.map_err(invalid)? else {return Ok(None);};
    let bytes: String = row.get(5);
    if row.get::<_, Uuid>(0) != fields.recipe
        || row.get::<_, i64>(1) != fields.version
        || row.get::<_, String>(2) != fields.selection
        || row.get::<_, String>(3) != fields.checksum
        || row.get::<_, String>(4) != kind.name()
        || row.get::<_, String>(6) != checksum(&bytes)
    {
        return Err(invalid("bootstrap workflow evidence selection differs"));
    }
    workflow_record(&bytes, id, &kind, &fields)?;
    Ok(Some(bytes))
}
/// Q1 returns false for an existing identity; that is never permission to rerun
/// an incomplete qualification. Behavior insertion has no conflict overwrite.
pub(super) async fn retain_workflow(
    tx: &Transaction<'_>,
    inspected: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
    id: Uuid,
    kind: WorkflowKind,
    bytes: &str,
) -> Result<bool, RebornBuildError> {
    require_coherent(tx).await?;
    let fields = workflow_fields(inspected)?;
    workflow_record(bytes, id, &kind, &fields)?;
    let query = match kind {
        WorkflowKind::Q1 => {
            "INSERT INTO reborn_workflow_review_evidence(evidence_id,recipe_id,recipe_version,selection_bytes,selection_checksum,evidence_kind,evidence_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(evidence_id) DO NOTHING"
        }
        WorkflowKind::Behavior => {
            "INSERT INTO reborn_workflow_review_evidence(evidence_id,recipe_id,recipe_version,selection_bytes,selection_checksum,evidence_kind,evidence_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6,$7,$8)"
        }
    };
    Ok(tx
        .execute(
            query,
            &[
                &id,
                &fields.recipe,
                &fields.version,
                &fields.selection,
                &fields.checksum,
                &kind.name(),
                &bytes,
                &checksum(bytes),
            ],
        )
        .await
        .map_err(invalid)?
        == 1)
}
