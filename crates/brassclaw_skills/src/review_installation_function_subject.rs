//! Separate queue-free function-fixture originals. These authenticate retained
//! data only; the package owner must establish actual qualification separately.
use super::*;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Partner {
    role: String,
    component: Value,
    base_comparison: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FunctionSubjectRecord {
    format: String,
    subject_id: Uuid,
    package_selector: String,
    candidate: Value,
    function_partner: Partner,
    dependencies: Vec<Value>,
    base_comparison: Value,
    package_context: InstallationPackageContext,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}

pub(super) fn record(
    graph: &RetainedComponentSnapshot,
    candidate: ComponentRevisionRef,
    partner: ComponentRevisionRef,
    context: InstallationPackageContext,
    limits: ContractLimits,
) -> Result<InstallationRecordDraft, Error> {
    let root = graph
        .revisions()
        .get(&candidate.uuid)
        .filter(|revision| revision.reference() == candidate)
        .ok_or(Error::Invalid)?;
    let other = graph
        .revisions()
        .get(&partner.uuid)
        .filter(|revision| revision.reference() == partner)
        .ok_or(Error::Invalid)?;
    let (skill, code) = match (candidate.class_code, partner.class_code) {
        (1..=3, 22) => (root, other),
        (22, 1..=3) => (other, root),
        _ => return Err(Error::Invalid),
    };
    let association = skill.draft().association().ok_or(Error::Invalid)?;
    if association.skill_uuid() != skill.reference().uuid
        || association.python_code_uuid() != code.reference().uuid
        || graph
            .revisions()
            .get(&association.tool_uuid())
            .is_none_or(|revision| revision.reference().class_code != 0)
        || graph
            .revisions()
            .get(&association.tool_skill_uuid())
            .is_none_or(|revision| revision.reference().class_code != 13)
    {
        return Err(Error::Invalid);
    }
    let closure = graph
        .select_roots(&[candidate.uuid, partner.uuid])
        .map_err(|_| Error::Invalid)?;
    if closure.revisions().len() != graph.revisions().len() {
        return Err(Error::Invalid);
    }
    let refs = graph
        .revisions()
        .values()
        .map(|revision| Reference::from(revision.reference()))
        .collect::<Vec<_>>();
    let identity = json!({"format":"installation-function-subject-identity/1",
        "candidate":Reference::from(candidate),"partner":Reference::from(partner),
        "components":refs,"package_context":context})
    .to_string();
    let id = Uuid::new_v5(&candidate.uuid, identity.as_bytes());
    let report = FunctionSubjectRecord {
        format: "installation-function-fixture-subject/1".into(),
        subject_id: id,
        package_selector: "validator:function-source:workflow".into(),
        candidate: revision_input(root, limits)?,
        function_partner: Partner {
            role: "subject".into(),
            component: revision_input(other, limits)?,
            base_comparison: Value::Null,
        },
        dependencies: graph
            .revisions()
            .values()
            .filter(|revision| ![candidate.uuid, partner.uuid].contains(&revision.reference().uuid))
            .map(|revision| revision_input(revision, limits))
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

pub(super) async fn read(
    tx: &Transaction<'_>,
    id: Uuid,
    bytes: &str,
    envelope: &Envelope,
    limits: ContractLimits,
) -> Result<RetainedInstallationValidatorSubject, Error> {
    let report: FunctionSubjectRecord =
        serde_json::from_value(envelope.report.clone()).map_err(|_| Error::Invalid)?;
    if report.format != "installation-function-fixture-subject/1"
        || report.package_selector != "validator:function-source:workflow"
        || report.subject_id != id
        || report.function_partner.role != "subject"
        || !report.base_comparison.is_null()
        || !report.function_partner.base_comparison.is_null()
        || report.q1_qualification
        || report.semantic_approval
        || report.catalogue_activation
    {
        return Err(Error::Invalid);
    }
    let candidate: Reference = serde_json::from_value(report.candidate["reference"].clone())
        .map_err(|_| Error::Invalid)?;
    let candidate = candidate.parsed()?;
    let partner: Reference =
        serde_json::from_value(report.function_partner.component["reference"].clone())
            .map_err(|_| Error::Invalid)?;
    let partner = partner.parsed()?;
    let refs = envelope
        .components
        .iter()
        .map(Reference::parsed)
        .collect::<Result<Vec<_>, _>>()?;
    let graph = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[candidate.uuid, partner.uuid],
        &refs,
        crate::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await?;
    let expected = record(
        &graph,
        candidate,
        partner,
        report.package_context.clone(),
        limits,
    )?;
    if expected.reference.id != id || expected.bytes != bytes {
        return Err(Error::Invalid);
    }
    verify_parser_context(tx, &report.package_context).await?;
    let reference = InstallationRecordRef {
        id,
        checksum: checksum(bytes),
    };
    let value = json!({"format":"installation-validator-subject/2","subject_id":id,
        "subject_bytes":bytes,"subject_checksum":reference.checksum,"candidate":report.candidate,
        "function_partner":{"role":"subject","component":report.function_partner.component,"base_comparison":null},
        "dependencies":report.dependencies,"base_comparison":null});
    crate::value_contract::validate_data_bounds(&value, limits).map_err(|_| Error::Capacity)?;
    Ok(RetainedInstallationValidatorSubject {
        value,
        candidate,
        reference,
        context: report.package_context,
        function_review: true,
    })
}
