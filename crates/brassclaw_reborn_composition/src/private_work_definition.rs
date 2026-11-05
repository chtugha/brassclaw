//! Immutable preparation bytes for the existing private Work branch. Raw domain
//! inputs remain composition evidence, never Turnstate or an authority flag.
use std::sync::Arc;

use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, orchestrator_contract::GlobalRootDefinition,
};
use brassclaw_turns::private_work::{ClaimedPrivateWork, PrivateWorkRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{RebornBuildError, installed_monty_catalogue::PreparationCapture};

fn invalid() -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: "private preparation original is invalid".into(),
    }
}
pub(crate) fn checksum(bytes: &str) -> [u8; 32] {
    Sha256::digest(bytes.as_bytes()).into()
}
pub(crate) fn decode_checksum(hex_bytes: &str) -> Result<[u8; 32], RebornBuildError> {
    hex::decode(hex_bytes)
        .map_err(|_| invalid())?
        .try_into()
        .map_err(|_| invalid())
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "purpose", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DomainCapture {
    ValidatorStructural {
        capture_bytes: String,
        capture_checksum: [u8; 32],
    },
    PrefixSourceOnly {
        job_id: Uuid,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "format", deny_unknown_fields)]
enum Wire {
    #[serde(rename = "private-work-definition/1")]
    V1 {
        work_id: Uuid,
        run_id: Uuid,
        request_checksum: [u8; 32],
        preparation_bytes: String,
        preparation_checksum: String,
        worker_checksum: [u8; 32],
        root_revision_bytes: String,
        root_revision_version: u64,
        workflow_checksum: [u8; 32],
        input_checksum: [u8; 32],
        domain: DomainCapture,
    },
}

pub(crate) struct DefinitionCapture<'a> {
    pub(crate) request: &'a PrivateWorkRequest,
    pub(crate) request_checksum: [u8; 32],
    pub(crate) preparation: PreparationCapture,
    pub(crate) worker_checksum: [u8; 32],
    pub(crate) root: &'a brassclaw_skills::component_revision::RetainedComponentRevision,
    pub(crate) workflow_checksum: [u8; 32],
    pub(crate) input_checksum: [u8; 32],
    pub(crate) domain: DomainCapture,
}

pub(crate) struct PrivateWorkDefinition {
    wire: Wire,
    root: Arc<GlobalRootDefinition>,
    preparation: PreparationCapture,
    bytes: String,
    checksum: [u8; 32],
}
impl PrivateWorkDefinition {
    pub(crate) fn capture(capture: DefinitionCapture<'_>) -> Result<Self, RebornBuildError> {
        let DefinitionCapture {
            request,
            request_checksum,
            preparation,
            worker_checksum,
            root,
            workflow_checksum,
            input_checksum,
            domain,
        } = capture;
        let wire = Wire::V1 {
            work_id: request.work_id(),
            run_id: request.run_id().as_uuid(),
            request_checksum,
            preparation_bytes: preparation.bytes().to_owned(),
            preparation_checksum: preparation.checksum().to_owned(),
            worker_checksum,
            root_revision_bytes: root.draft().exact_bytes().to_owned(),
            root_revision_version: root.reference().version,
            workflow_checksum,
            input_checksum,
            domain,
        };
        let bytes = serde_json::to_string(&wire).map_err(|_| invalid())?;
        let digest = checksum(&bytes);
        Self::restore(request, request_checksum, &bytes, digest)
    }
    pub(crate) fn restore(
        request: &PrivateWorkRequest,
        expected_request_checksum: [u8; 32],
        bytes: &str,
        expected: [u8; 32],
    ) -> Result<Self, RebornBuildError> {
        if checksum(bytes) != expected {
            return Err(invalid());
        }
        let wire: Wire = serde_json::from_str(bytes).map_err(|_| invalid())?;
        let Wire::V1 {
            work_id,
            run_id,
            request_checksum,
            preparation_bytes,
            preparation_checksum,
            root_revision_bytes,
            root_revision_version,
            workflow_checksum,
            domain,
            ..
        } = &wire;
        if *work_id != request.work_id()
            || *run_id != request.run_id().as_uuid()
            || *request_checksum != expected_request_checksum
        {
            return Err(invalid());
        }
        let preparation =
            PreparationCapture::restore_original(preparation_bytes, preparation_checksum)
                .map_err(|_| invalid())?;
        if request
            .original_preparation_checksum()
            .is_some_and(|original| Some(original) != decode_checksum(preparation_checksum).ok())
            || request
                .original_workflow_checksum()
                .is_some_and(|original| original != *workflow_checksum)
        {
            return Err(invalid());
        }
        match (domain, request.original()) {
            (
                DomainCapture::PrefixSourceOnly { job_id },
                brassclaw_turns::private_work::PrivateOriginalRef::PrefixSourceOnly {
                    job_id: original,
                    ..
                },
            ) if job_id == original => {}
            (
                DomainCapture::ValidatorStructural {
                    capture_bytes,
                    capture_checksum,
                },
                brassclaw_turns::private_work::PrivateOriginalRef::ValidatorStructural { .. },
            ) if checksum(capture_bytes) == *capture_checksum => {}
            _ => return Err(invalid()),
        }
        let revision = ComponentRevisionDraft::from_json(root_revision_bytes)
            .map_err(|_| invalid())?
            .at_version(*root_revision_version)
            .map_err(|_| invalid())?;
        let root = Arc::new(GlobalRootDefinition::from_revision(&revision).map_err(|_| invalid())?);
        Ok(Self {
            wire,
            root,
            preparation,
            bytes: bytes.to_owned(),
            checksum: expected,
        })
    }
    pub(crate) fn bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub(crate) fn root(&self) -> &GlobalRootDefinition {
        &self.root
    }
    pub(crate) fn preparation(&self) -> &PreparationCapture {
        &self.preparation
    }
    pub(crate) fn worker_checksum(&self) -> [u8; 32] {
        let Wire::V1 {
            worker_checksum, ..
        } = &self.wire;
        *worker_checksum
    }
    pub(crate) fn workflow_checksum(&self) -> [u8; 32] {
        let Wire::V1 {
            workflow_checksum, ..
        } = &self.wire;
        *workflow_checksum
    }
    pub(crate) fn input_checksum(&self) -> [u8; 32] {
        let Wire::V1 { input_checksum, .. } = &self.wire;
        *input_checksum
    }
    pub(crate) fn domain(&self) -> &DomainCapture {
        let Wire::V1 { domain, .. } = &self.wire;
        domain
    }
    pub(crate) fn task_input(
        &self,
        claimed: &ClaimedPrivateWork,
    ) -> brassclaw_monty_host::service::PrivateTaskInput {
        brassclaw_monty_host::service::PrivateTaskInput::new(
            claimed.request().work_id(),
            claimed.request().run_id().as_uuid(),
            self.workflow_checksum(),
            self.input_checksum(),
        )
    }
}
