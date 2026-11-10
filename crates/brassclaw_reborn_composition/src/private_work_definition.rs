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
    #[serde(rename = "original_pure_case/1")]
    OriginalPureCaseV1 {
        case_id: Uuid,
        case_checksum: [u8; 32],
    },
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
    #[serde(rename = "private-work-definition/2")]
    V2 {
        work_id: Uuid,
        run_id: Uuid,
        request_checksum: [u8; 32],
        preparation_bytes: String,
        preparation_checksum: String,
        worker_checksum: [u8; 32],
        root_revision_bytes: String,
        root_revision_version: u64,
        root_preparation_bytes: Option<String>,
        root_preparation_checksum: Option<String>,
        workflow_checksum: [u8; 32],
        input_checksum: [u8; 32],
        domain: DomainCapture,
    },
}

// The borrowed capture form has the exact /2 field order and encoding. Count
// complete escaping before copying any root string or allocating an output.
#[derive(Serialize)]
struct CaptureWire<'a> {
    format: &'static str,
    work_id: Uuid,
    run_id: Uuid,
    request_checksum: [u8; 32],
    preparation_bytes: &'a str,
    preparation_checksum: &'a str,
    worker_checksum: [u8; 32],
    root_revision_bytes: &'a str,
    root_revision_version: u64,
    root_preparation_bytes: Option<&'a str>,
    root_preparation_checksum: Option<&'a str>,
    workflow_checksum: [u8; 32],
    input_checksum: [u8; 32],
    domain: &'a DomainCapture,
}

pub(crate) const PRIVATE_DEFINITION_INSERT: &str = "INSERT INTO brassclaw_monty_private_preparations(work_id,definition_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(work_id) DO NOTHING";
fn checked_definition_size(
    bytes: u64,
    preparation: &PreparationCapture,
) -> Result<usize, RebornBuildError> {
    let policy = preparation.policy();
    if bytes > policy.max_value_bytes {
        return Err(invalid());
    }
    let size = usize::try_from(bytes).map_err(|_| invalid())?;
    let request = brassclaw_pg::wire_capacity::unnamed_typed_request_bytes(
        PRIVATE_DEFINITION_INSERT.len(),
        &[16, size, 32],
    )
    .ok_or_else(invalid)?;
    if u64::try_from(request).map_err(|_| invalid())? > policy.max_ipc_frame_bytes {
        return Err(invalid());
    }
    brassclaw_pg::wire_capacity::binary_data_row_bytes(&[Some(size), Some(32)])
        .ok_or_else(invalid)?;
    Ok(size)
}
struct CheckedBuffer {
    bytes: Vec<u8>,
    expected: usize,
}
impl std::io::Write for CheckedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|next| next > self.expected)
        {
            return Err(std::io::Error::other("private definition capacity"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_capture(
    wire: &CaptureWire<'_>,
    preparation: &PreparationCapture,
) -> Result<String, RebornBuildError> {
    let length =
        brassclaw_common::json::count_json_bytes(wire, Some(preparation.policy().max_value_bytes))
            .map_err(|_| invalid())?;
    let expected = checked_definition_size(length, preparation)?;
    let mut buffer = CheckedBuffer {
        bytes: Vec::new(),
        expected,
    };
    buffer
        .bytes
        .try_reserve_exact(expected)
        .map_err(|_| invalid())?;
    brassclaw_common::json::write_json(&mut buffer, wire).map_err(|_| invalid())?;
    if buffer.bytes.len() != expected {
        return Err(invalid());
    }
    String::from_utf8(buffer.bytes).map_err(|_| invalid())
}
struct CanonicalCompare<'a> {
    expected: &'a [u8],
    consumed: usize,
}
impl std::io::Write for CanonicalCompare<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .consumed
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("private definition capacity"))?;
        if self.expected.get(self.consumed..end) != Some(bytes) {
            return Err(std::io::Error::other("private definition encoding"));
        }
        self.consumed = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn require_canonical<T: Serialize>(value: &T, bytes: &str) -> Result<(), RebornBuildError> {
    let mut compare = CanonicalCompare {
        expected: bytes.as_bytes(),
        consumed: 0,
    };
    brassclaw_common::json::write_json(&mut compare, value).map_err(|_| invalid())?;
    if compare.consumed != bytes.len() {
        return Err(invalid());
    }
    Ok(())
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
    root_revision: brassclaw_skills::component_revision::RetainedComponentRevision,
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
        let wire = CaptureWire {
            format: "private-work-definition/2",
            work_id: request.work_id(),
            run_id: request.run_id().as_uuid(),
            request_checksum,
            preparation_bytes: preparation.bytes(),
            preparation_checksum: preparation.checksum(),
            worker_checksum,
            root_revision_bytes: root.draft().exact_bytes(),
            root_revision_version: root.reference().version,
            root_preparation_bytes: root.draft().preparation().map(|p| p.bytes()),
            root_preparation_checksum: root.draft().preparation().map(|p| p.checksum()),
            workflow_checksum,
            input_checksum,
            domain: &domain,
        };
        let bytes = encode_capture(&wire, &preparation)?;
        let digest = checksum(&bytes);
        Self::decode(request, request_checksum, &bytes, digest)
    }
    pub(crate) async fn restore(
        tx: &tokio_postgres::Transaction<'_>,
        request: &PrivateWorkRequest,
        expected_request_checksum: [u8; 32],
        bytes: &str,
        expected: [u8; 32],
    ) -> Result<Self, RebornBuildError> {
        let original = Self::decode(request, expected_request_checksum, bytes, expected)?;
        original.verify_retained_root(tx).await?;
        Ok(original)
    }
    fn decode(
        request: &PrivateWorkRequest,
        expected_request_checksum: [u8; 32],
        bytes: &str,
        expected: [u8; 32],
    ) -> Result<Self, RebornBuildError> {
        if checksum(bytes) != expected {
            return Err(invalid());
        }
        let wire: Wire = serde_json::from_str(bytes).map_err(|_| invalid())?;
        if matches!(&wire, Wire::V2 { .. }) {
            require_canonical(&wire, bytes)?;
        }
        let root_policy = match &wire {
            Wire::V1 { .. } => None,
            Wire::V2 {
                root_preparation_bytes,
                root_preparation_checksum,
                ..
            } => match (root_preparation_bytes, root_preparation_checksum) {
                (None, None) => None,
                (Some(bytes), Some(hash)) => Some(
                    brassclaw_host_api::task_preparation::RetainedPreparation::restore_original(
                        bytes, hash,
                    )
                    .map_err(|_| invalid())?,
                ),
                _ => return Err(invalid()),
            },
        };
        let (Wire::V1 {
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
        }
        | Wire::V2 {
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
        }) = &wire;
        if *work_id != request.work_id()
            || *run_id != request.run_id().as_uuid()
            || *request_checksum != expected_request_checksum
        {
            return Err(invalid());
        }
        let preparation =
            PreparationCapture::restore_original(preparation_bytes, preparation_checksum)
                .map_err(|_| invalid())?;
        if matches!(&wire, Wire::V2 { .. }) {
            checked_definition_size(
                u64::try_from(bytes.len()).map_err(|_| invalid())?,
                &preparation,
            )?;
        }
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
                DomainCapture::OriginalPureCaseV1 {
                    case_id,
                    case_checksum,
                },
                brassclaw_turns::private_work::PrivateOriginalRef::OriginalPureCaseV1 {
                    case_id: original,
                    case_checksum: expected,
                },
            ) if case_id == original
                && case_checksum == expected
                && *case_id == request.work_id()
                && *case_id == request.run_id().as_uuid() => {}
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
        let revision = ComponentRevisionDraft::from_json_with_preparation(
            root_revision_bytes,
            root_policy.as_ref(),
        )
        .map_err(|_| invalid())?
        .at_version(*root_revision_version)
        .map_err(|_| invalid())?;
        let root = Arc::new(GlobalRootDefinition::from_revision(&revision).map_err(|_| invalid())?);
        Ok(Self {
            wire,
            root,
            root_revision: revision,
            preparation,
            bytes: bytes.to_owned(),
            checksum: expected,
        })
    }
    /// Detached bytes are not authority for their own parser policy. Verify the
    /// exact historical row and its immutable policy in the caller's snapshot.
    pub(crate) async fn verify_retained_root(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        brassclaw_skills::revision_store::PgComponentRevisionStore::verify_original_in_transaction(
            tx,
            &self.root_revision,
        )
        .await
        .map_err(|_| invalid())
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
        let (Wire::V1 {
            worker_checksum, ..
        }
        | Wire::V2 {
            worker_checksum, ..
        }) = &self.wire;
        *worker_checksum
    }
    pub(crate) fn workflow_checksum(&self) -> [u8; 32] {
        let (Wire::V1 {
            workflow_checksum, ..
        }
        | Wire::V2 {
            workflow_checksum, ..
        }) = &self.wire;
        *workflow_checksum
    }
    pub(crate) fn input_checksum(&self) -> [u8; 32] {
        let (Wire::V1 { input_checksum, .. } | Wire::V2 { input_checksum, .. }) = &self.wire;
        *input_checksum
    }
    pub(crate) fn domain(&self) -> &DomainCapture {
        let (Wire::V1 { domain, .. } | Wire::V2 { domain, .. }) = &self.wire;
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

#[cfg(test)]
mod encoding_tests {
    use super::*;
    fn preparation(bytes: u64) -> PreparationCapture {
        use brassclaw_host_api::{
            MontyExecutionLimits,
            task_preparation::{PreparationOrigin, RetainedPreparation},
        };
        PreparationCapture::from_original(
            RetainedPreparation::encode(
                19,
                PreparationOrigin::AcknowledgedLive,
                MontyExecutionLimits {
                    max_value_bytes: bytes,
                    ..MontyExecutionLimits::default()
                },
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn fixture<'a>(
        preparation: &'a PreparationCapture,
        root: &'a str,
        domain: &'a DomainCapture,
    ) -> CaptureWire<'a> {
        // Serialization mechanics only: these data are not an admitted Work or root.
        CaptureWire {
            format: "private-work-definition/2",
            work_id: Uuid::from_u128(1),
            run_id: Uuid::from_u128(2),
            request_checksum: [3; 32],
            preparation_bytes: preparation.bytes(),
            preparation_checksum: preparation.checksum(),
            worker_checksum: [4; 32],
            root_revision_bytes: root,
            root_revision_version: 1,
            root_preparation_bytes: None,
            root_preparation_checksum: None,
            workflow_checksum: [5; 32],
            input_checksum: [6; 32],
            domain,
        }
    }
    #[test]
    fn private_v2_complete_encoding_counts_escaped_root_and_compares_canonical_without_second_copy()
    {
        let root = "\"\\\n猫".repeat(1024);
        let domain = DomainCapture::PrefixSourceOnly {
            job_id: Uuid::from_u128(7),
        };
        let mut bound = 64 * 1024;
        // Include the decimal budget's own encoding in the complete byte count.
        for _ in 0..8 {
            let prep = preparation(bound);
            let size =
                brassclaw_common::json::count_json_bytes(&fixture(&prep, &root, &domain), None)
                    .unwrap();
            if size == bound {
                break;
            }
            bound = size;
        }
        let prep = preparation(bound);
        let wire = fixture(&prep, &root, &domain);
        let encoded = encode_capture(&wire, &prep).unwrap();
        assert_eq!(encoded.len() as u64, bound);
        let owned: Wire = serde_json::from_str(&encoded).unwrap();
        require_canonical(&owned, &encoded).unwrap();
        assert!(require_canonical(&owned, &format!("{encoded} ")).is_err());
        let smaller = preparation(bound - 1);
        assert_eq!(
            brassclaw_common::json::count_json_bytes(&fixture(&smaller, &root, &domain), None)
                .unwrap(),
            bound
        );
        assert!(encode_capture(&fixture(&smaller, &root, &domain), &smaller).is_err());
    }
}
