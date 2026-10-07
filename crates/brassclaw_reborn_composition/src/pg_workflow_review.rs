//! Exact selected-variant structural and step-execution observations.
//! These are not semantic approval, root-task completion, protected-root trust,
//! association approval or an active catalogue. No public success-write API.

use std::{collections::BTreeMap, sync::Arc};

use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedRecipeExecution, RetainedStepFailure, port_answer_checksum, typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_pg::PgPool;
use brassclaw_skills::{component_revision::REVISION_LIMITS, value_contract::validate_data_bounds};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use super::pg_retained_q1::{
    ExpectedStepResult, RetainedReviewPersistenceError, StructuralReviewFailure,
};

/// Expectations are authoring data. Only comparisons with executor-owned
/// observations determine the persisted result, including negative cases.
pub(crate) struct WorkflowStepExpectation {
    pub(crate) inputs: Value,
    pub(crate) tool: Option<(Value, PortAnswer)>,
    pub(crate) result: ExpectedStepResult,
}
pub(crate) enum ExpectedWorkflowTermination {
    CompletedSteps,
    FailedStep {
        step: String,
        failure: RetainedStepFailure,
    },
}

/// Private construction retains the exact inspected program and actual derived
/// evidence. No arbitrary JSON, approval flag or latest-version lookup enters.
pub(crate) struct WorkflowReview {
    inspected: Arc<InspectedRetainedProgram>,
    id: Uuid,
    selection: String,
    selection_checksum: String,
    kind: &'static str,
    bytes: String,
    checksum: String,
}
impl WorkflowReview {
    pub(crate) fn structural(
        inspected: Arc<InspectedRetainedProgram>,
    ) -> Result<Self, StructuralReviewFailure> {
        let mut sources = BTreeMap::new();
        let program = inspected.program();
        if program.program().steplist.is_empty() {
            return Err(StructuralReviewFailure::Preparation(
                "nonempty Recipe workflow required",
            ));
        }
        for step in &program.program().steplist {
            let reference = program
                .inputs()
                .components()
                .get(&step.step_id)
                .ok_or(StructuralReviewFailure::Integrity)?;
            let source = inspected
                .source_checks()
                .get(&reference.uuid)
                .filter(|source| source.component() == *reference)
                .ok_or(StructuralReviewFailure::Integrity)?;
            sources.insert(
                step.step_id.clone(),
                json!({"python_code_uuid":reference.uuid,"observations":source.observations()}),
            );
        }
        Self::record(
            inspected,
            "q1",
            true,
            json!({
                "scope":"selected-variant-structure", "semantic_approval":false,
                "task_completion":false, "sources":sources,
            }),
        )
    }

    pub(crate) fn behavior(
        execution: &RetainedRecipeExecution,
        expected: &BTreeMap<String, WorkflowStepExpectation>,
        termination: ExpectedWorkflowTermination,
    ) -> Result<Self, StructuralReviewFailure> {
        let inspected = execution.inspected_program();
        let order = execution
            .observation_order()
            .ok_or(StructuralReviewFailure::Observation)?;
        let selected: Vec<_> = inspected
            .program()
            .program()
            .steplist
            .iter()
            .map(|step| step.step_id.clone())
            .collect();
        let expected_order = match &termination {
            ExpectedWorkflowTermination::CompletedSteps => selected.as_slice(),
            ExpectedWorkflowTermination::FailedStep { step, .. } => {
                let position = selected
                    .iter()
                    .position(|id| id == step)
                    .ok_or(StructuralReviewFailure::Observation)?;
                &selected[..=position]
            }
        };
        // This adapter currently executes each selected occurrence once. Do not
        // manufacture proof for missing/reordered steps, branches or retries.
        if order.is_empty()
            || order != expected_order
            || expected.len() != order.len()
            || execution.observations().len() != order.len()
            || execution.transport_failure().is_some()
        {
            return Err(StructuralReviewFailure::Observation);
        }
        let mut succeeded = true;
        let mut observations = Vec::new();
        for (index, step) in order.iter().enumerate() {
            let observed = execution
                .observations()
                .get(step)
                .filter(|case| case.settled() && case.observation_error().is_none())
                .ok_or(StructuralReviewFailure::Observation)?;
            let wanted = expected
                .get(step)
                .ok_or(StructuralReviewFailure::Observation)?;
            let input = typed_value_checksum(&wanted.inputs).map_err(|_| {
                StructuralReviewFailure::Preparation("expected inputs exceed capacity")
            })?;
            let (arguments, answer) = match &wanted.tool {
                Some((arguments, answer)) => (
                    Some(typed_value_checksum(arguments).map_err(|_| {
                        StructuralReviewFailure::Preparation(
                            "expected Tool arguments exceed capacity",
                        )
                    })?),
                    Some(port_answer_checksum(answer).map_err(|_| {
                        StructuralReviewFailure::Preparation(
                            "expected Tool answer exceeds capacity",
                        )
                    })?),
                ),
                None => (None, None),
            };
            let (result, failure) = match &wanted.result {
                ExpectedStepResult::Return(value) => (
                    Some(typed_value_checksum(value).map_err(|_| {
                        StructuralReviewFailure::Preparation("expected result exceeds capacity")
                    })?),
                    None,
                ),
                ExpectedStepResult::Failure(failure) => (None, Some(*failure)),
            };
            let terminal = match &termination {
                ExpectedWorkflowTermination::CompletedSteps => None,
                ExpectedWorkflowTermination::FailedStep { failure, .. }
                    if index + 1 == order.len() =>
                {
                    Some(*failure)
                }
                ExpectedWorkflowTermination::FailedStep { .. } => None,
            };
            // A negative expectation cannot count a failure at another step as
            // an observed successful workflow, even if its checksum matches.
            let passed = observed.input_checksum() == input
                && observed.arguments_checksum() == arguments
                && observed.answer_checksum() == answer
                && observed.result_checksum() == result
                && observed.failure() == failure
                && failure == terminal;
            succeeded &= passed;
            observations.push(json!({
                "step_id":step, "inputs_checksum":hex(observed.input_checksum()),
                "arguments_checksum":observed.arguments_checksum().map(hex),
                "answer_checksum":observed.answer_checksum().map(hex),
                "result_checksum":observed.result_checksum().map(hex),
                "failure":observed.failure().map(RetainedStepFailure::reason_kind),
                "expected":{"inputs_checksum":hex(input),"arguments_checksum":arguments.map(hex),
                    "answer_checksum":answer.map(hex),"result_checksum":result.map(hex),
                    "failure":failure.map(RetainedStepFailure::reason_kind)},
                "succeeded":passed,
            }));
        }
        Self::record(
            inspected,
            "behavior",
            succeeded,
            json!({
                "scope":"selected-variant-step-execution", "semantic_approval":false,
                "task_completion":false, "execution_order":order, "observations":observations,
            }),
        )
    }

    fn record(
        inspected: Arc<InspectedRetainedProgram>,
        kind: &'static str,
        succeeded: bool,
        report: Value,
    ) -> Result<Self, StructuralReviewFailure> {
        if inspected
            .program()
            .inputs()
            .instruction()
            .recipe()
            .class_code
            != 21
        {
            return Err(StructuralReviewFailure::Preparation(
                "Recipe-variant evidence cannot certify protected roots",
            ));
        }
        let selected = inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| {
                StructuralReviewFailure::Preparation(
                    "retained workflow selection is invalid or exceeds capacity",
                )
            })?;
        let selection = selected.exact_bytes().to_owned();
        let selection_checksum = hex(selected.checksum());
        let id = Uuid::new_v4();
        let evidence = json!({"format":"workflow-review-evidence/1", "evidence_id":id,
            "kind":kind,"validation_mode":"authored", "selection_checksum":selection_checksum,
            "succeeded":succeeded,"report":report});
        validate_data_bounds(&evidence, REVISION_LIMITS).map_err(|_| {
            StructuralReviewFailure::Preparation("workflow evidence exceeds capacity")
        })?;
        let bytes = evidence.to_string();
        if bytes.len() > REVISION_LIMITS.max_bytes {
            return Err(StructuralReviewFailure::Preparation(
                "serialized workflow evidence exceeds capacity",
            ));
        }
        let checksum = digest(&bytes);
        Ok(Self {
            inspected,
            id,
            selection,
            selection_checksum,
            kind,
            bytes,
            checksum,
        })
    }
    pub(crate) fn id(&self) -> Uuid {
        self.id
    }
    pub(crate) fn evidence(&self) -> &str {
        &self.bytes
    }
}

pub(crate) async fn persist_workflow_review(
    pool: &PgPool,
    review: Arc<WorkflowReview>,
) -> Result<(), RetainedReviewPersistenceError<WorkflowReview>> {
    persist(pool, &review)
        .await
        .map_err(|failure| RetainedReviewPersistenceError {
            failure,
            retained: review,
        })
}
async fn persist(pool: &PgPool, review: &WorkflowReview) -> Result<(), StructuralReviewFailure> {
    let mut client = pool
        .get()
        .await
        .map_err(StructuralReviewFailure::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(StructuralReviewFailure::Database)?;
    let instruction = review.inspected.program().inputs().instruction();
    let graph = instruction.snapshot();
    let mut ids = Vec::new();
    let mut versions = Vec::new();
    let mut classes = Vec::new();
    let mut checksums = Vec::new();
    for revision in graph.revisions().values() {
        let reference = revision.reference();
        ids.push(reference.uuid);
        versions.push(
            i64::try_from(reference.version).map_err(|_| StructuralReviewFailure::Integrity)?,
        );
        classes.push(
            i16::try_from(reference.class_code).map_err(|_| StructuralReviewFailure::Integrity)?,
        );
        checksums.push(hex(reference.checksum));
    }
    let complete: bool = tx.query_one("SELECT count(*) = cardinality($1::uuid[]) FROM
        unnest($1::uuid[],$2::bigint[],$3::smallint[],$4::text[]) AS e(id,version,class_code,checksum)
        JOIN reborn_component_revisions r ON r.component_id=e.id AND r.version=e.version
            AND r.class_code=e.class_code AND r.checksum=e.checksum", &[&ids,&versions,&classes,&checksums])
        .await.map_err(StructuralReviewFailure::Database)?.get(0);
    if !complete || ids.is_empty() {
        return Err(StructuralReviewFailure::Integrity);
    }
    let recipe = instruction.recipe();
    let version = i64::try_from(recipe.version).map_err(|_| StructuralReviewFailure::Integrity)?;
    tx.execute("INSERT INTO reborn_workflow_review_evidence
        (evidence_id,recipe_id,recipe_version,selection_bytes,selection_checksum,evidence_kind,evidence_bytes,checksum)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (evidence_id) DO NOTHING",
        &[&review.id,&recipe.uuid,&version,&review.selection,&review.selection_checksum,&review.kind,&review.bytes,&review.checksum])
        .await.map_err(StructuralReviewFailure::Database)?;
    let existing = tx.query_one("SELECT selection_bytes,selection_checksum,evidence_bytes,checksum FROM reborn_workflow_review_evidence WHERE evidence_id=$1", &[&review.id])
        .await.map_err(StructuralReviewFailure::Database)?;
    if existing.get::<_, &str>(0) != review.selection
        || existing.get::<_, &str>(1) != review.selection_checksum
        || existing.get::<_, &str>(2) != review.bytes
        || existing.get::<_, &str>(3) != review.checksum
    {
        return Err(StructuralReviewFailure::Integrity);
    }
    tx.commit().await.map_err(StructuralReviewFailure::Database)
}
fn hex(checksum: [u8; 32]) -> String {
    checksum.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn digest(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
