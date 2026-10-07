//! Exact structural Q1 producer for the supported retained single-Tool adapter.
//! No client JSON/pass flag enters this producer. Semantic agreement, actual
//! behavioral validation, authenticated human Q2 and activation are separate.
//! This is not the controlled system-seed provenance adapter.

use std::{collections::BTreeMap, sync::Arc};

use brassclaw_engine::executor::{
    retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
};
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
pub(crate) struct StructuralReviewPersistenceError {
    pub(crate) failure: StructuralReviewFailure,
    pub(crate) retained: Arc<StructuralReviewSet>,
}
impl std::fmt::Debug for StructuralReviewPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}
impl std::fmt::Display for StructuralReviewPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.failure)
    }
}
impl std::error::Error for StructuralReviewPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

pub(crate) async fn persist_structural_reviews(
    pool: &PgPool,
    reviews: Arc<StructuralReviewSet>,
) -> Result<(), StructuralReviewPersistenceError> {
    persist(pool, &reviews)
        .await
        .map_err(|failure| StructuralReviewPersistenceError {
            failure,
            retained: reviews,
        })
}

async fn persist(
    pool: &PgPool,
    reviews: &StructuralReviewSet,
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
    let graph = reviews
        .inspected
        .program()
        .inputs()
        .instruction()
        .snapshot();
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
    for record in reviews.records.values() {
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
