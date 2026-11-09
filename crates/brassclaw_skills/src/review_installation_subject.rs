//! Queue-free retained installation data. These seals authenticate immutable
//! bytes/graphs only; Composition must verify its actual package and runtime.
//! No authored submission/request, own approval or activation is written here.
use super::{Reference, SubmissionStoreError as Error, database, hex, revision_input};
use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::RetainedComponentSnapshot,
    revision_store::PgComponentRevisionStore,
    value_contract::{ContractLimits, strict_json},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_postgres::Transaction;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationRecordRef {
    pub id: Uuid,
    pub checksum: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationPackageContext {
    pub factory_checksum: String,
    pub worker_checksum: String,
    pub association_checksum: String,
    pub parser_completion: InstallationRecordRef,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationProfile {
    Structure,
    Pure,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: String,
    evidence_id: Uuid,
    kind: String,
    validation_mode: String,
    association_checksum: String,
    components: Vec<Reference>,
    succeeded: bool,
    reviewed_evidence: Vec<Value>,
    report: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectRecord {
    format: String,
    subject_id: Uuid,
    package_selector: String,
    candidate: Value,
    dependencies: Vec<Value>,
    base_comparison: Value,
    package_context: InstallationPackageContext,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionRecord {
    format: String,
    selection_id: Uuid,
    subject_id: Uuid,
    subject_checksum: String,
    candidate: Reference,
    validator_selection_bytes: String,
    validator_selection_checksum: String,
    scope: String,
    profile: InstallationProfile,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
pub struct InstallationRecordDraft {
    reference: InstallationRecordRef,
    bytes: String,
}
impl InstallationRecordDraft {
    pub fn reference(&self) -> &InstallationRecordRef {
        &self.reference
    }
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
}
pub struct RetainedInstallationValidatorSubject {
    value: Value,
    candidate: ComponentRevisionRef,
    reference: InstallationRecordRef,
    context: InstallationPackageContext,
}
impl RetainedInstallationValidatorSubject {
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn candidate(&self) -> ComponentRevisionRef {
        self.candidate
    }
    pub fn reference(&self) -> &InstallationRecordRef {
        &self.reference
    }
    pub fn package_context(&self) -> &InstallationPackageContext {
        &self.context
    }
}
pub struct RetainedInstallationValidatorSelection {
    reference: InstallationRecordRef,
    subject: InstallationRecordRef,
    bytes: String,
    validator_bytes: String,
    selection: Value,
    snapshot: Arc<RetainedComponentSnapshot>,
    profile: InstallationProfile,
}
impl RetainedInstallationValidatorSelection {
    pub fn reference(&self) -> &InstallationRecordRef {
        &self.reference
    }
    pub fn subject(&self) -> &InstallationRecordRef {
        &self.subject
    }
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
    pub fn validator_bytes(&self) -> &str {
        &self.validator_bytes
    }
    pub fn selection(&self) -> &Value {
        &self.selection
    }
    pub fn snapshot(&self) -> Arc<RetainedComponentSnapshot> {
        self.snapshot.clone()
    }
    pub fn profile(&self) -> InstallationProfile {
        self.profile
    }
}
fn checksum(bytes: &str) -> String {
    hex(Sha256::digest(bytes.as_bytes()).into())
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn draft(
    id: Uuid,
    context: &InstallationPackageContext,
    refs: Vec<Reference>,
    report: Value,
) -> Result<InstallationRecordDraft, Error> {
    if id.is_nil()
        || !valid_hash(&context.factory_checksum)
        || !valid_hash(&context.worker_checksum)
        || !valid_hash(&context.association_checksum)
        || context.parser_completion.id.is_nil()
        || !valid_hash(&context.parser_completion.checksum)
    {
        return Err(Error::Invalid);
    }
    let bytes = serde_json::to_string(&Envelope {
        format: "component-review-evidence/1".into(),
        evidence_id: id,
        kind: "behavior".into(),
        validation_mode: "system_seed".into(),
        association_checksum: context.association_checksum.clone(),
        components: refs,
        succeeded: false,
        reviewed_evidence: vec![],
        report,
    })
    .map_err(|_| Error::Invalid)?;
    Ok(InstallationRecordDraft {
        reference: InstallationRecordRef {
            id,
            checksum: checksum(&bytes),
        },
        bytes,
    })
}
/// The actual package owner chooses this real retained root. This function
/// authenticates data; it does not certify that a caller-selected graph is S.
pub fn subject_record(
    graph: &RetainedComponentSnapshot,
    candidate: ComponentRevisionRef,
    context: InstallationPackageContext,
    limits: ContractLimits,
) -> Result<InstallationRecordDraft, Error> {
    subject_record_scoped(graph, candidate, context, limits, false)
}
/// Private installation source-workflow fixture data. All qualification and
/// activation flags remain false; Composition owns the exact fixture gallery.
/// This format is disjoint from the historical structural-presence subject.
pub fn pure_fixture_subject_record(
    graph: &RetainedComponentSnapshot,
    candidate: ComponentRevisionRef,
    context: InstallationPackageContext,
    limits: ContractLimits,
) -> Result<InstallationRecordDraft, Error> {
    subject_record_scoped(graph, candidate, context, limits, true)
}
fn subject_record_scoped(
    graph: &RetainedComponentSnapshot,
    candidate: ComponentRevisionRef,
    context: InstallationPackageContext,
    limits: ContractLimits,
    pure_fixture: bool,
) -> Result<InstallationRecordDraft, Error> {
    let root = graph
        .revisions()
        .get(&candidate.uuid)
        .filter(|r| r.reference() == candidate)
        .ok_or(Error::Invalid)?;
    if candidate.class_code != 22 {
        return Err(Error::Invalid);
    }
    let refs = graph
        .revisions()
        .values()
        .map(|r| Reference::from(r.reference()))
        .collect::<Vec<_>>();
    let mut identity = json!({"format":"installation-validator-subject-identity/1","candidate":Reference::from(candidate),"components":refs,"package_context":context});
    if pure_fixture {
        identity["fixture_scope"] = json!("installation-pure-fixture-subject/1");
    }
    let identity = identity.to_string();
    let id = Uuid::new_v5(&candidate.uuid, identity.as_bytes());
    let report = SubjectRecord {
        format: if pure_fixture {
            "installation-pure-fixture-subject/1"
        } else {
            "installation-validator-subject-record/1"
        }
        .into(),
        subject_id: id,
        package_selector: if pure_fixture {
            "validator:pure-qualification:workflow"
        } else {
            "validator:structural-presence"
        }
        .into(),
        candidate: revision_input(root, limits)?,
        dependencies: graph
            .revisions()
            .values()
            .filter(|r| r.reference().uuid != candidate.uuid)
            .map(|r| revision_input(r, limits))
            .collect::<Result<_, _>>()?,
        base_comparison: Value::Null,
        package_context: context.clone(),
        q1_qualification: false,
        semantic_approval: false,
        catalogue_activation: false,
    };
    draft(
        id,
        &context,
        refs,
        serde_json::to_value(report).map_err(|_| Error::Invalid)?,
    )
}
pub fn selection_record(
    subject: &RetainedInstallationValidatorSubject,
    validator_bytes: &str,
    profile: InstallationProfile,
    limits: ContractLimits,
) -> Result<InstallationRecordDraft, Error> {
    let inner = strict_json(validator_bytes, limits).map_err(|_| Error::Invalid)?;
    let refs = serde_json::from_value::<Vec<Reference>>(inner["components"].clone())
        .map_err(|_| Error::Invalid)?;
    let identity =
        json!({"format":"installation-validator-selection-identity/1","subject":subject.reference,
        "validator_checksum":checksum(validator_bytes),"profile":profile})
        .to_string();
    let id = Uuid::new_v5(&subject.reference.id, identity.as_bytes());
    let report = SelectionRecord {
        format: "installation-validator-selection-record/1".into(),
        selection_id: id,
        subject_id: subject.reference.id,
        subject_checksum: subject.reference.checksum.clone(),
        candidate: subject.candidate.into(),
        validator_selection_bytes: validator_bytes.to_owned(),
        validator_selection_checksum: checksum(validator_bytes),
        scope: "controlled-installation-preparation".into(),
        profile,
        q1_qualification: false,
        semantic_approval: false,
        catalogue_activation: false,
    };
    draft(
        id,
        &subject.context,
        refs,
        serde_json::to_value(report).map_err(|_| Error::Invalid)?,
    )
}
async fn read_envelope(
    tx: &Transaction<'_>,
    id: Uuid,
    limits: ContractLimits,
) -> Result<Option<(String, Envelope)>, Error> {
    super::require_snapshot(tx).await?;
    let Some(row)=tx.query_opt("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",&[&id]).await.map_err(database)? else {return Ok(None)};
    let bytes: String = row.get(0);
    if checksum(&bytes) != row.get::<_, String>(1) {
        return Err(Error::Invalid);
    }
    let envelope: Envelope =
        serde_json::from_value(strict_json(&bytes, limits).map_err(|_| Error::Invalid)?)
            .map_err(|_| Error::Invalid)?;
    if envelope.format != "component-review-evidence/1"
        || envelope.evidence_id != id
        || envelope.kind != "behavior"
        || envelope.validation_mode != "system_seed"
        || envelope.succeeded
        || !envelope.reviewed_evidence.is_empty()
    {
        return Err(Error::Invalid);
    }
    Ok(Some((bytes, envelope)))
}
pub async fn read_subject_in_transaction(
    tx: &Transaction<'_>,
    id: Uuid,
    limits: ContractLimits,
) -> Result<Option<RetainedInstallationValidatorSubject>, Error> {
    let Some((bytes, envelope)) = read_envelope(tx, id, limits).await? else {
        return Ok(None);
    };
    let report: SubjectRecord =
        serde_json::from_value(envelope.report.clone()).map_err(|_| Error::Invalid)?;
    let pure_fixture = match (report.format.as_str(), report.package_selector.as_str()) {
        ("installation-validator-subject-record/1", "validator:structural-presence") => false,
        ("installation-pure-fixture-subject/1", "validator:pure-qualification:workflow") => true,
        _ => return Err(Error::Invalid),
    };
    if report.subject_id != id
        || !report.base_comparison.is_null()
        || report.q1_qualification
        || report.semantic_approval
        || report.catalogue_activation
    {
        return Err(Error::Invalid);
    }
    let candidate: Reference = serde_json::from_value(report.candidate["reference"].clone())
        .map_err(|_| Error::Invalid)?;
    let candidate = candidate.parsed()?;
    let refs = envelope
        .components
        .iter()
        .map(Reference::parsed)
        .collect::<Result<Vec<_>, _>>()?;
    let graph = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[candidate.uuid],
        &refs,
        crate::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await?;
    let expected = subject_record_scoped(
        &graph,
        candidate,
        report.package_context.clone(),
        limits,
        pure_fixture,
    )?;
    if expected.reference.id != id || expected.bytes != bytes {
        return Err(Error::Invalid);
    }
    let parser=tx.query_opt("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",&[&report.package_context.parser_completion.id]).await.map_err(database)?.ok_or(Error::Invalid)?;
    let parser_bytes: String = parser.get(0);
    if checksum(&parser_bytes) != parser.get::<_, String>(1)
        || checksum(&parser_bytes) != report.package_context.parser_completion.checksum
    {
        return Err(Error::Invalid);
    }
    let reference = InstallationRecordRef {
        id,
        checksum: checksum(&bytes),
    };
    let value = json!({"format":"installation-validator-subject/1","subject_id":id,"subject_bytes":bytes,
        "subject_checksum":reference.checksum,"candidate":report.candidate,"dependencies":report.dependencies,"base_comparison":null});
    crate::value_contract::validate_data_bounds(&value, limits).map_err(|_| Error::Capacity)?;
    Ok(Some(RetainedInstallationValidatorSubject {
        value,
        candidate,
        reference,
        context: report.package_context,
    }))
}
pub async fn read_selection_in_transaction(
    tx: &Transaction<'_>,
    id: Uuid,
    limits: ContractLimits,
) -> Result<Option<RetainedInstallationValidatorSelection>, Error> {
    let Some((bytes, envelope)) = read_envelope(tx, id, limits).await? else {
        return Ok(None);
    };
    let report: SelectionRecord =
        serde_json::from_value(envelope.report).map_err(|_| Error::Invalid)?;
    if report.format != "installation-validator-selection-record/1"
        || report.selection_id != id
        || report.scope != "controlled-installation-preparation"
        || report.q1_qualification
        || report.semantic_approval
        || report.catalogue_activation
    {
        return Err(Error::Invalid);
    }
    let subject = read_subject_in_transaction(tx, report.subject_id, limits)
        .await?
        .ok_or(Error::Invalid)?;
    if subject.reference.checksum != report.subject_checksum
        || subject.candidate != report.candidate.parsed()?
        || checksum(&report.validator_selection_bytes) != report.validator_selection_checksum
    {
        return Err(Error::Invalid);
    }
    let (selection, snapshot) = super::selection::verify_program_selection(
        tx,
        &report.validator_selection_bytes,
        subject.candidate.class_code,
        limits,
    )
    .await?;
    let expected = selection_record(
        &subject,
        &report.validator_selection_bytes,
        report.profile,
        limits,
    )?;
    if expected.bytes != bytes {
        return Err(Error::Invalid);
    }
    Ok(Some(RetainedInstallationValidatorSelection {
        reference: expected.reference,
        subject: subject.reference,
        bytes,
        validator_bytes: report.validator_selection_bytes,
        selection,
        snapshot,
        profile: report.profile,
    }))
}
