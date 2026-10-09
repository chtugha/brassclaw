//! Exact structural Q1 and observed behavior producers for the supported
//! retained single-Tool adapter. No client JSON/pass flag establishes success.
//! Semantic agreement, authenticated human Q2 and activation remain separate.
//! This is not the controlled system-seed provenance adapter.

use std::{collections::BTreeMap, sync::Arc};

use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedStepFailure, port_answer_checksum,
        typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_pg::PgPool;
use brassclaw_skills::{component_revision::REVISION_LIMITS, value_contract::validate_data_bounds};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

const MAX_REVIEW_BYTES: usize = 64 * 1024 * 1024;

#[derive(thiserror::Error)]
pub(crate) enum StructuralReviewFailure {
    #[error("retained structural review: {0}")]
    Preparation(&'static str),
    #[error("retained structural review database operation failed")]
    Database(#[source] tokio_postgres::Error),
    #[error("retained structural review database connection failed")]
    Connection(#[source] deadpool_postgres::PoolError),
    #[error("retained structural review revision or record integrity failed")]
    Integrity,
    #[error("retained behavior observation is missing, pending or incomplete")]
    Observation,
}
impl std::fmt::Debug for StructuralReviewFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}
fn database(error: tokio_postgres::Error) -> StructuralReviewFailure {
    StructuralReviewFailure::Database(error)
}

struct Record {
    id: Uuid,
    bytes: String,
    checksum: String,
}

/// Privately produced actual observations, retaining their exact selected graph.
/// Evidence IDs identify Q1 records only, never combination approval or grants.
pub(crate) struct StructuralReviewSet {
    inspected: Arc<InspectedRetainedProgram>,
    records: BTreeMap<Uuid, Record>,
}
impl StructuralReviewSet {
    pub(crate) fn prepare(
        inspected: Arc<InspectedRetainedProgram>,
    ) -> Result<Self, StructuralReviewFailure> {
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(StructuralReviewFailure::Preparation(
                "association review requires an explicitly prepared usage",
            ));
        };
        if program.bindings().is_empty() {
            return Err(StructuralReviewFailure::Preparation(
                "association review has no selected usage",
            ));
        }
        let mut records = BTreeMap::new();
        let mut total = 0usize;
        for binding in program.bindings().values() {
            if records.contains_key(&binding.skill().uuid) {
                continue;
            }
            let source = inspected
                .source_checks()
                .get(&binding.python().uuid)
                .filter(|source| source.component() == binding.python())
                .ok_or(StructuralReviewFailure::Integrity)?;
            // A usage can dispatch through retained Python dependencies. Its
            // review must preserve their real parser observations as well as
            // the entry source; component checksums alone are not those facts.
            let sources: BTreeMap<_, _> = binding
                .combination()
                .iter()
                .filter(|component| component.class_code == 22)
                .map(|component| {
                    inspected
                        .source_checks()
                        .get(&component.uuid)
                        .filter(|source| source.component() == *component)
                        .map(|source| (component.uuid, source.observations()))
                        .ok_or(StructuralReviewFailure::Integrity)
                })
                .collect::<Result<_, _>>()?;
            let id = Uuid::new_v4();
            let components: Vec<_> = binding
                .combination()
                .iter()
                .map(|component| {
                    json!({"uuid":component.uuid,"class_code":component.class_code,
                    "version":component.version,"checksum":hex(component.checksum)})
                })
                .collect();
            let record = json!({
                "format":"component-review-evidence/1", "evidence_id":id,
                "kind":"q1", "validation_mode":"authored",
                "association_checksum":digest(binding.association().exact_bytes()),
                "components":components, "succeeded":true, "reviewed_evidence":[],
                "report":{
                    "format":"retained-binding-source-review/1", "scope":"structural-only",
                    "semantic_approval":false, "python_code_uuid":binding.python().uuid,
                    "observations":source.observations(),
                    "sources":sources,
                },
            });
            validate_data_bounds(&record, REVISION_LIMITS).map_err(|_| {
                StructuralReviewFailure::Preparation("review exceeds technical record capacity")
            })?;
            let bytes = record.to_string();
            if bytes.len() > REVISION_LIMITS.max_bytes {
                return Err(StructuralReviewFailure::Preparation(
                    "serialized review exceeds technical record capacity",
                ));
            }
            total = total
                .checked_add(bytes.len())
                .filter(|total| *total <= MAX_REVIEW_BYTES)
                .ok_or(StructuralReviewFailure::Preparation(
                    "reviews exceed aggregate technical capacity",
                ))?;
            records.insert(
                binding.skill().uuid,
                Record {
                    id,
                    checksum: digest(&bytes),
                    bytes,
                },
            );
        }
        Ok(Self { inspected, records })
    }

    pub(crate) fn inspected(&self) -> &InspectedRetainedProgram {
        &self.inspected
    }
    pub(crate) fn references(&self) -> BTreeMap<Uuid, Uuid> {
        self.records
            .iter()
            .map(|(skill, record)| (*skill, record.id))
            .collect()
    }
    /// Private audit bytes for the trusted review UI/supervisor, not model state.
    pub(crate) fn evidence(&self, skill: Uuid) -> Option<&str> {
        self.records.get(&skill).map(|record| record.bytes.as_str())
    }
}
fn digest(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
fn hex(checksum: [u8; 32]) -> String {
    checksum.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Actual record identities/bytes survive any storage or ambiguous commit error.
/// Retry this same set; never manufacture a new successful Q1 record on recovery.
pub(crate) struct RetainedReviewPersistenceError<T> {
    pub(crate) failure: StructuralReviewFailure,
    pub(crate) retained: Arc<T>,
}
impl<T> std::fmt::Debug for RetainedReviewPersistenceError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}
impl<T> std::fmt::Display for RetainedReviewPersistenceError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.failure)
    }
}
impl<T> std::error::Error for RetainedReviewPersistenceError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}
pub(crate) type StructuralReviewPersistenceError =
    RetainedReviewPersistenceError<StructuralReviewSet>;

pub(crate) async fn persist_structural_reviews(
    pool: &PgPool,
    reviews: Arc<StructuralReviewSet>,
) -> Result<(), StructuralReviewPersistenceError> {
    persist(pool, &reviews.inspected, &reviews.records)
        .await
        .map_err(|failure| StructuralReviewPersistenceError {
            failure,
            retained: reviews,
        })
}

async fn persist(
    pool: &PgPool,
    inspected: &InspectedRetainedProgram,
    records: &BTreeMap<Uuid, Record>,
) -> Result<(), StructuralReviewFailure> {
    let mut client = pool
        .get()
        .await
        .map_err(StructuralReviewFailure::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(database)?;
    let graph = inspected.program().inputs().instruction().snapshot();
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
    // The inspected graph has already parsed and verified its exact checksums
    // and complete dependencies. Compare against actual immutable stored pairs
    // without copying their potentially large source documents a second time.
    // Draft heads/latest status deliberately do not alter this exact review.
    let complete: bool = tx.query_one(
        "SELECT count(*) = cardinality($1::uuid[]) FROM
         unnest($1::uuid[],$2::bigint[],$3::smallint[],$4::text[]) AS e(id,version,class_code,checksum)
         JOIN reborn_component_revisions r ON r.component_id=e.id AND r.version=e.version
          AND r.class_code=e.class_code AND r.checksum=e.checksum",
        &[&ids,&versions,&classes,&checksums],
    ).await.map_err(database)?.get(0);
    if !complete || ids.is_empty() {
        return Err(StructuralReviewFailure::Integrity);
    }
    for record in records.values() {
        tx.execute(
            "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
             VALUES ($1,$2,$3) ON CONFLICT (evidence_id) DO NOTHING",
            &[&record.id, &record.bytes, &record.checksum],
        )
        .await
        .map_err(database)?;
        let existing = tx
            .query_one(
                "SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",
                &[&record.id],
            )
            .await
            .map_err(database)?;
        if existing.get::<_, String>(0) != record.bytes
            || existing.get::<_, String>(1) != record.checksum
        {
            return Err(StructuralReviewFailure::Integrity);
        }
    }
    tx.commit().await.map_err(database)
}

/// Authored expected values/classification, not observations or approval flags.
/// The producer compares them to the actual executor-owned completed step.
pub(crate) enum ExpectedStepResult {
    Return(serde_json::Value),
    Failure(RetainedStepFailure),
}
pub(crate) struct BehavioralExpectation {
    pub(crate) inputs: serde_json::Value,
    pub(crate) arguments: serde_json::Value,
    pub(crate) answer: PortAnswer,
    pub(crate) result: ExpectedStepResult,
}

/// One actual verification case per selected usage, including repeated uses.
/// This does not establish whole-Recipe completion, semantic consistency,
/// authenticated human Q2, controlled system-seed provenance or activation.
pub(crate) struct BehavioralReviewSet {
    inspected: Arc<InspectedRetainedProgram>,
    records: BTreeMap<Uuid, Record>,
}
impl BehavioralReviewSet {
    pub(crate) fn prepare(
        execution: &RetainedRecipeExecution,
        expectations: &BTreeMap<String, BehavioralExpectation>,
    ) -> Result<Self, StructuralReviewFailure> {
        let inspected = execution.inspected_program();
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(StructuralReviewFailure::Preparation(
                "behavior evidence requires an explicitly prepared Tool usage",
            ));
        };
        if expectations.is_empty() || expectations.len() > 4096 {
            return Err(StructuralReviewFailure::Preparation(
                "nonempty bounded behavioral cases required",
            ));
        }
        let mut cases: BTreeMap<Uuid, Vec<serde_json::Value>> = BTreeMap::new();
        for (step, expected) in expectations {
            let binding = program
                .bindings()
                .get(step)
                .ok_or(StructuralReviewFailure::Observation)?;
            let observed = execution
                .observations()
                .get(step)
                .filter(|observed| observed.settled() && observed.observation_error().is_none())
                .ok_or(StructuralReviewFailure::Observation)?;
            let actual_arguments = observed
                .arguments_checksum()
                .ok_or(StructuralReviewFailure::Observation)?;
            let actual_answer = observed
                .answer_checksum()
                .ok_or(StructuralReviewFailure::Observation)?;
            let expected_inputs = typed_value_checksum(&expected.inputs).map_err(|_| {
                StructuralReviewFailure::Preparation("expected inputs exceed technical capacity")
            })?;
            let expected_arguments = typed_value_checksum(&expected.arguments).map_err(|_| {
                StructuralReviewFailure::Preparation("expected arguments exceed technical capacity")
            })?;
            let expected_answer = port_answer_checksum(&expected.answer).map_err(|_| {
                StructuralReviewFailure::Preparation("expected answer exceeds technical capacity")
            })?;
            let (expected_result, expected_failure) = match &expected.result {
                ExpectedStepResult::Return(value) => (
                    Some(typed_value_checksum(value).map_err(|_| {
                        StructuralReviewFailure::Preparation(
                            "expected result exceeds technical capacity",
                        )
                    })?),
                    None,
                ),
                ExpectedStepResult::Failure(kind) => (None, Some(*kind)),
            };
            let succeeded = observed.input_checksum() == expected_inputs
                && actual_arguments == expected_arguments
                && actual_answer == expected_answer
                && observed.result_checksum() == expected_result
                && observed.failure() == expected_failure;
            cases.entry(binding.skill().uuid).or_default().push(json!({
                "step_id":step,
                "inputs_checksum":hex(observed.input_checksum()),
                "arguments_checksum":hex(actual_arguments),
                "answer_checksum":hex(actual_answer),
                "result_checksum":observed.result_checksum().map(hex),
                "failure":observed.failure().map(RetainedStepFailure::reason_kind),
                "expected":{
                    "inputs_checksum":hex(expected_inputs), "arguments_checksum":hex(expected_arguments),
                    "answer_checksum":hex(expected_answer), "result_checksum":expected_result.map(hex),
                    "failure":expected_failure.map(RetainedStepFailure::reason_kind),
                },
                "succeeded":succeeded,
            }));
        }
        let bindings: BTreeMap<_, _> = program
            .bindings()
            .values()
            .map(|binding| (binding.skill().uuid, binding))
            .collect();
        let mut records = BTreeMap::new();
        let mut total = 0usize;
        for (skill, observations) in cases {
            let binding = bindings[&skill];
            let id = Uuid::new_v4();
            let components: Vec<_> = binding
                .combination()
                .iter()
                .map(|component| {
                    json!({
                        "uuid":component.uuid,"class_code":component.class_code,
                        "version":component.version,"checksum":hex(component.checksum),
                    })
                })
                .collect();
            let record = json!({
                "format":"component-review-evidence/1", "evidence_id":id,
                "kind":"behavior", "validation_mode":"authored",
                "association_checksum":digest(binding.association().exact_bytes()),
                "components":components,
                "succeeded":observations.iter().all(|case| case["succeeded"] == true),
                "reviewed_evidence":[],
                "report":{
                    "format":"retained-usage-behavior/1", "scope":"selected-tool-usages",
                    "semantic_approval":false, "workflow_completion":false,
                    "value_fingerprint":"typed-value-sha256/1",
                    "answer_fingerprint":"port-answer-sha256/1", "observations":observations,
                },
            });
            validate_data_bounds(&record, REVISION_LIMITS).map_err(|_| {
                StructuralReviewFailure::Preparation("behavior record exceeds technical capacity")
            })?;
            let bytes = record.to_string();
            if bytes.len() > REVISION_LIMITS.max_bytes {
                return Err(StructuralReviewFailure::Preparation(
                    "serialized behavior record exceeds technical capacity",
                ));
            }
            total = total
                .checked_add(bytes.len())
                .filter(|total| *total <= MAX_REVIEW_BYTES)
                .ok_or(StructuralReviewFailure::Preparation(
                    "behavior records exceed aggregate capacity",
                ))?;
            records.insert(
                skill,
                Record {
                    id,
                    checksum: digest(&bytes),
                    bytes,
                },
            );
        }
        Ok(Self { inspected, records })
    }
    pub(crate) fn references(&self) -> BTreeMap<Uuid, Uuid> {
        self.records
            .iter()
            .map(|(skill, record)| (*skill, record.id))
            .collect()
    }
    pub(crate) fn evidence(&self, skill: Uuid) -> Option<&str> {
        self.records.get(&skill).map(|record| record.bytes.as_str())
    }
}

pub(crate) async fn persist_behavioral_reviews(
    pool: &PgPool,
    reviews: Arc<BehavioralReviewSet>,
) -> Result<(), RetainedReviewPersistenceError<BehavioralReviewSet>> {
    persist(pool, &reviews.inspected, &reviews.records)
        .await
        .map_err(|failure| RetainedReviewPersistenceError {
            failure,
            retained: reviews,
        })
}
