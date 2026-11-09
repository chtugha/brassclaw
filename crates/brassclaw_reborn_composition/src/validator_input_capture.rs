//! Original authored validator input custody. No Work, claim, admission,
//! approval or settings owner lives here; B02 retains the resulting bytes.
use super::*;
use crate::{
    RebornBuildError, bootstrap_reply_validation::invalid,
    installed_monty_catalogue::PreparationCapture,
};
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_skills::value_contract::strict_json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    submission_id: Uuid,
    subject_checksum: String,
    validator_selection_bytes: String,
    validator_selection_checksum: String,
    validator_bytes: String,
    validator_checksum: String,
    input_bytes: String,
    input_checksum: String,
    preparation_bytes: String,
    preparation_checksum: String,
    worker_checksum: String,
}
fn checksum(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}
pub(super) fn limits(preparation: &PreparationCapture) -> ContractLimits {
    let bounds = preparation.limits().bounds;
    ContractLimits {
        max_depth: bounds.max_value_depth,
        max_nodes: bounds.max_value_nodes,
        max_bytes: bounds.max_value_bytes,
    }
}
pub(crate) struct ValidatorInputCapture {
    wire: Wire,
    inputs: brassclaw_host_api::JsonValue,
    preparation: PreparationCapture,
    bytes: String,
    checksum: String,
}
impl ValidatorInputCapture {
    /// Integrity of a historical captured preparation, without current worker
    /// inspection, requalification or recomputation using today's settings.
    pub(crate) fn verify_historical(
        subject: &RetainedValidatorSubject,
        selection: &RetainedValidatorSelection,
        definition: &crate::private_work_definition::PrivateWorkDefinition,
    ) -> Result<(), RebornBuildError> {
        let crate::private_work_definition::DomainCapture::ValidatorStructural {
            capture_bytes,
            capture_checksum,
        } = definition.domain()
        else {
            return Err(invalid("historical validator capture purpose differs"));
        };
        let wire: Wire = serde_json::from_str(capture_bytes).map_err(invalid)?;
        if checksum(capture_bytes) != hex::encode(capture_checksum)
            || serde_json::to_string(&wire).map_err(invalid)? != *capture_bytes
            || wire.format != "validator-input-capture/1"
            || wire.submission_id != subject.submission_id()
            || wire.subject_checksum != subject.subject_checksum()
            || wire.validator_selection_bytes != selection.bytes()
            || wire.validator_selection_checksum != checksum(selection.bytes())
            || wire.validator_bytes != selection.validator_bytes()
            || wire.validator_checksum != checksum(selection.validator_bytes())
            || wire.validator_checksum != hex::encode(definition.workflow_checksum())
            || wire.input_checksum != checksum(&wire.input_bytes)
            || wire.input_checksum != hex::encode(definition.input_checksum())
            || wire.preparation_bytes != definition.preparation().bytes()
            || wire.preparation_checksum != definition.preparation().checksum()
            || wire.worker_checksum != hex::encode(definition.worker_checksum())
        {
            return Err(invalid(
                "historical validator capture differs from original",
            ));
        }
        let inputs =
            strict_json(&wire.input_bytes, limits(definition.preparation())).map_err(invalid)?;
        let program = RetainedValidatorCoverage::restore_preparation(selection).map_err(invalid)?;
        let coverage = RetainedValidatorCoverage::from_inputs(program.inputs()).map_err(invalid)?;
        coverage.require_subject(subject).map_err(invalid)?;
        let bound = program
            .inputs()
            .bind_task_inputs_with_limits(&inputs, limits(definition.preparation()))
            .map_err(invalid)?;
        if inputs != bound {
            return Err(invalid(
                "historical validator inputs differ from captured binding",
            ));
        }
        if selection.source_qualification().is_some() {
            let expected = coverage
                .subject_task_inputs(subject, program.inputs(), limits(definition.preparation()))
                .map_err(invalid)?;
            if expected != inputs
                || serde_json::to_string(&expected).map_err(invalid)? != wire.input_bytes
            {
                return Err(invalid(
                    "historical Source input differs from complete original subject",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn inputs(&self) -> &brassclaw_host_api::JsonValue {
        &self.inputs
    }
    pub(crate) fn input_checksum(&self) -> &str {
        &self.wire.input_checksum
    }
    pub(crate) fn validator_checksum(&self) -> &str {
        &self.wire.validator_checksum
    }
    pub(crate) fn preparation(&self) -> &PreparationCapture {
        &self.preparation
    }
    pub(crate) fn bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn checksum(&self) -> &str {
        &self.checksum
    }
}
pub(crate) struct PreparedValidatorReview {
    pub(super) subject: Arc<RetainedValidatorSubject>,
    pub(super) selection: Arc<RetainedValidatorSelection>,
    pub(super) inspected: Arc<InspectedRetainedProgram>,
    pub(super) coverage: RetainedValidatorCoverage,
    capture: ValidatorInputCapture,
}
impl PreparedValidatorReview {
    /// Transaction-local data capture of W already inspected under the same
    /// acknowledged preparation. The qualification owner is the only caller.
    pub(crate) fn capture_qualified(
        subject: Arc<RetainedValidatorSubject>,
        selection: Arc<RetainedValidatorSelection>,
        inspected: Arc<InspectedRetainedProgram>,
        coverage: RetainedValidatorCoverage,
        preparation: PreparationCapture,
        worker_checksum: [u8; 32],
    ) -> Result<Arc<Self>, RebornBuildError> {
        if selection.source_qualification().is_none()
            || selection.submission_id() != subject.submission_id()
            || selection.subject_checksum() != subject.subject_checksum()
            || inspected
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?
                .exact_bytes()
                != selection.validator_bytes()
        {
            return Err(invalid("qualified Source capture differs"));
        }
        let inputs =
            subject_validator_task_inputs(&coverage, &subject, &inspected, limits(&preparation))
                .map_err(invalid)?;
        let input_bytes = serde_json::to_string(inputs.as_value()).map_err(invalid)?;
        let wire = Wire {
            format: "validator-input-capture/1".into(),
            submission_id: subject.submission_id(),
            subject_checksum: subject.subject_checksum().to_owned(),
            validator_selection_bytes: selection.bytes().to_owned(),
            validator_selection_checksum: checksum(selection.bytes()),
            validator_bytes: selection.validator_bytes().to_owned(),
            validator_checksum: checksum(selection.validator_bytes()),
            input_checksum: checksum(&input_bytes),
            input_bytes,
            preparation_bytes: preparation.bytes().to_owned(),
            preparation_checksum: preparation.checksum().to_owned(),
            worker_checksum: hex::encode(worker_checksum),
        };
        let bytes = serde_json::to_string(&wire).map_err(invalid)?;
        let capture = ValidatorInputCapture {
            wire,
            inputs,
            preparation,
            checksum: checksum(&bytes),
            bytes,
        };
        Ok(Arc::new(Self {
            subject,
            selection,
            inspected,
            coverage,
            capture,
        }))
    }
    /// Fresh task preparation is captured by the catalogue exactly once before
    /// this call. No caller-provided inputs or separate revision claims exist.
    #[cfg(test)]
    pub(crate) async fn prepare(
        subject: Arc<RetainedValidatorSubject>,
        selection: Arc<RetainedValidatorSelection>,
        worker: Arc<PackagedExecutableImage>,
        preparation: PreparationCapture,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let (worker, inspected, coverage) =
            Self::inspect(&subject, &selection, worker, &preparation).await?;
        let inputs =
            subject_validator_task_inputs(&coverage, &subject, &inspected, limits(&preparation))
                .map_err(invalid)?;
        let input_bytes = serde_json::to_string(inputs.as_value()).map_err(invalid)?;
        let wire = Wire {
            format: "validator-input-capture/1".into(),
            submission_id: subject.submission_id(),
            subject_checksum: subject.subject_checksum().to_owned(),
            validator_selection_bytes: selection.bytes().to_owned(),
            validator_selection_checksum: checksum(selection.bytes()),
            validator_bytes: selection.validator_bytes().to_owned(),
            validator_checksum: checksum(selection.validator_bytes()),
            input_checksum: checksum(&input_bytes),
            input_bytes,
            preparation_bytes: preparation.bytes().to_owned(),
            preparation_checksum: preparation.checksum().to_owned(),
            worker_checksum: hex::encode(worker.checksum()),
        };
        let bytes = serde_json::to_string(&wire).map_err(invalid)?;
        let capture = ValidatorInputCapture {
            wire,
            inputs,
            preparation,
            checksum: checksum(&bytes),
            bytes,
        };
        Ok(Arc::new(Self {
            subject,
            selection,
            inspected,
            coverage,
            capture,
        }))
    }
    /// Restore only exact original metadata. B02 must independently prove Work
    /// ownership/current claim and supply the retained original worker image.
    pub(crate) async fn restore_original(
        subject: Arc<RetainedValidatorSubject>,
        selection: Arc<RetainedValidatorSelection>,
        worker: Arc<PackagedExecutableImage>,
        original_preparation: PreparationCapture,
        capture_bytes: &str,
        capture_checksum: &str,
    ) -> Result<Arc<Self>, RebornBuildError> {
        if checksum(capture_bytes) != capture_checksum {
            return Err(invalid("validator input capture checksum differs"));
        }
        // A flat, closed typed envelope: serde rejects duplicate, unknown and
        // missing fields. Nested typed data is decoded separately below under
        // the original capture's technical limits, never today's settings.
        let wire: Wire = serde_json::from_str(capture_bytes).map_err(invalid)?;
        if wire.format != "validator-input-capture/1"
            || serde_json::to_string(&wire).map_err(invalid)? != capture_bytes
            || wire.submission_id != subject.submission_id()
            || wire.subject_checksum != subject.subject_checksum()
            || wire.validator_selection_bytes != selection.bytes()
            || wire.validator_selection_checksum != checksum(selection.bytes())
            || wire.validator_bytes != selection.validator_bytes()
            || wire.validator_checksum != checksum(selection.validator_bytes())
            || wire.preparation_bytes != original_preparation.bytes()
            || wire.preparation_checksum != original_preparation.checksum()
            || wire.worker_checksum != hex::encode(worker.checksum())
            || wire.input_checksum != checksum(&wire.input_bytes)
        {
            return Err(invalid(
                "validator original subject/selection/preparation/worker differs",
            ));
        }
        let preparation = PreparationCapture::restore_original(
            &wire.preparation_bytes,
            &wire.preparation_checksum,
        )
        .map_err(invalid)?;
        let (_, inspected, coverage) =
            Self::inspect(&subject, &selection, worker, &preparation).await?;
        let original = strict_json(&wire.input_bytes, limits(&preparation)).map_err(invalid)?;
        let bound = inspected
            .program()
            .inputs()
            .bind_task_inputs_with_limits(&original, limits(&preparation))
            .map_err(invalid)?;
        let expected =
            subject_validator_task_inputs(&coverage, &subject, &inspected, limits(&preparation))
                .map_err(invalid)?;
        if original != bound
            || original != *expected.as_value()
            || serde_json::to_string(expected.as_value()).map_err(invalid)? != wire.input_bytes
        {
            return Err(invalid(
                "validator original typed inputs differ from sealed subject",
            ));
        }
        let capture = ValidatorInputCapture {
            wire,
            inputs: original.into(),
            preparation,
            bytes: capture_bytes.to_owned(),
            checksum: capture_checksum.to_owned(),
        };
        Ok(Arc::new(Self {
            subject,
            selection,
            inspected,
            coverage,
            capture,
        }))
    }
    async fn inspect(
        subject: &RetainedValidatorSubject,
        selection: &RetainedValidatorSelection,
        worker: Arc<PackagedExecutableImage>,
        preparation: &PreparationCapture,
    ) -> Result<
        (
            Arc<PackagedExecutableImage>,
            Arc<InspectedRetainedProgram>,
            RetainedValidatorCoverage,
        ),
        RebornBuildError,
    > {
        if subject.submission_id() != selection.submission_id()
            || subject.subject_checksum() != selection.subject_checksum()
        {
            return Err(invalid("validator original subject selection differs"));
        }
        let program = RetainedValidatorCoverage::restore_preparation(selection).map_err(invalid)?;
        let coverage = RetainedValidatorCoverage::from_inputs(program.inputs()).map_err(invalid)?;
        coverage.require_subject(subject).map_err(invalid)?;
        let worker =
            super::verified_worker(&worker, preparation.limits().max_native_artifact_bytes).await?;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                &worker.path(),
                preparation.limits(),
            )
            .await
            .map_err(invalid)?,
        );
        Ok((worker, inspected, coverage))
    }
    pub(crate) fn input_capture(&self) -> &ValidatorInputCapture {
        &self.capture
    }
    pub(crate) fn inspected(&self) -> Arc<InspectedRetainedProgram> {
        self.inspected.clone()
    }
    pub(crate) fn subject(&self) -> &RetainedValidatorSubject {
        &self.subject
    }
    pub(crate) fn retained_subject(&self) -> Arc<RetainedValidatorSubject> {
        self.subject.clone()
    }
    pub(crate) fn selection(&self) -> Arc<RetainedValidatorSelection> {
        self.selection.clone()
    }
}

#[cfg(test)]
pub(crate) async fn assert_original_capture(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    subject: Arc<RetainedValidatorSubject>,
    selection: Arc<RetainedValidatorSelection>,
    settings: &dyn brassclaw_product_workflow::MontyVmSettingsStore,
) {
    let original = catalogue.capture_task_preparation().await.unwrap();
    let prepared = PreparedValidatorReview::prepare(
        subject.clone(),
        selection.clone(),
        catalogue.worker_image.clone(),
        original.clone(),
    )
    .await
    .unwrap();
    let capture = prepared.input_capture();
    // Inspect this owner's actual retained envelope directly; callers need the
    // complete bytes/checksum, not separate getters for its internal fields.
    assert_eq!(
        capture.input_checksum(),
        checksum(&capture.wire.input_bytes)
    );
    assert_eq!(capture.wire.validator_selection_bytes, selection.bytes());
    assert_eq!(
        capture.wire.validator_selection_checksum,
        checksum(selection.bytes())
    );
    assert_eq!(capture.wire.validator_bytes, selection.validator_bytes());
    assert_eq!(
        capture.validator_checksum(),
        checksum(selection.validator_bytes())
    );
    assert_eq!(capture.preparation().revision(), original.revision());
    assert_eq!(capture.preparation().bytes(), original.bytes());
    assert_eq!(
        capture.wire.worker_checksum,
        hex::encode(catalogue.worker_image.checksum())
    );
    // These existing real profiles contain the actual SourceInspection binding.
    // Their common input capture must never grant entry to Structural Q1.
    assert!(
        ReviewAdmissionSource::from_prepared(prepared.clone(), catalogue.pool.clone()).is_err()
    );
    let current = settings.get("default", "default").await.unwrap();
    let mut reduced = current.execution_limits;
    reduced.max_source_bytes /= 2;
    reduced.max_stdout_bytes /= 2;
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(serde_json::json!({
                "expected_revision":current.revision,"execution_limits":reduced,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let newer = catalogue.capture_task_preparation().await.unwrap();
    assert!(newer.revision() > original.revision());
    assert_ne!(newer.bytes(), original.bytes());
    let restored = PreparedValidatorReview::restore_original(
        subject.clone(),
        selection.clone(),
        catalogue.worker_image.clone(),
        original.clone(),
        capture.bytes(),
        capture.checksum(),
    )
    .await
    .unwrap();
    assert_eq!(restored.input_capture().bytes(), capture.bytes());
    assert_eq!(
        restored.input_capture().inputs().as_value(),
        capture.inputs().as_value()
    );
    assert_eq!(
        restored.input_capture().preparation().bytes(),
        original.bytes()
    );
    assert!(
        PreparedValidatorReview::restore_original(
            subject.clone(),
            selection.clone(),
            catalogue.worker_image.clone(),
            newer,
            capture.bytes(),
            capture.checksum()
        )
        .await
        .is_err(),
        "current preparation cannot replace the original"
    );
    let mut variants = Vec::new();
    let mut changed: Wire = serde_json::from_str(capture.bytes()).unwrap();
    changed.validator_selection_bytes.push(' ');
    changed.validator_selection_checksum = checksum(&changed.validator_selection_bytes);
    variants.push(serde_json::to_string(&changed).unwrap());
    let mut changed: Wire = serde_json::from_str(capture.bytes()).unwrap();
    changed.worker_checksum = "00".repeat(32);
    variants.push(serde_json::to_string(&changed).unwrap());
    variants.push(capture.bytes().replace(
        &format!("\"input_checksum\":\"{}\",", capture.input_checksum()),
        "",
    ));
    let mut changed: Wire = serde_json::from_str(capture.bytes()).unwrap();
    changed.input_bytes = serde_json::json!({"subject_json":"{}"}).to_string();
    changed.input_checksum = checksum(&changed.input_bytes);
    variants.push(serde_json::to_string(&changed).unwrap());
    variants.push(format!(
        "{{\"format\":\"validator-input-capture/1\",{}",
        &capture.bytes()[1..]
    ));
    for bytes in variants {
        assert!(
            PreparedValidatorReview::restore_original(
                subject.clone(),
                selection.clone(),
                catalogue.worker_image.clone(),
                original.clone(),
                &bytes,
                &checksum(&bytes)
            )
            .await
            .is_err(),
            "changed/incomplete/duplicate original must fail before effects"
        );
    }
}
