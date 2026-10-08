//! Native builtin registration binds real handlers to their loaded image and
//! exact input/adapter declaration. It supplies no component approval or grant.
use std::sync::Arc;

use brassclaw_host_api::{CapabilityDescriptor, RuntimeKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::NativeExecutableImage;

/// Expected selection data. Deserializing this reference does not create a
/// registration; retain_native compares it with the actual private registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeImplementationRef {
    artifact_checksum: [u8; 32],
    contract_checksum: [u8; 32],
}
impl NativeImplementationRef {
    pub fn artifact_checksum(&self) -> [u8; 32] {
        self.artifact_checksum
    }
    pub fn contract_checksum(&self) -> [u8; 32] {
        self.contract_checksum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NativeRegistrationError {
    #[error(transparent)]
    Image(#[from] crate::NativeImageError),
    #[error("native builtin registration or declaration is incomplete")]
    Declaration,
}

/// Only the concrete builtin constructor issues these registrations. Arbitrary
/// with_handler/insert_handler calls cannot inherit their artifact provenance.
pub(crate) struct NativeBuiltinRegistration {
    image: Arc<NativeExecutableImage>,
    descriptor: CapabilityDescriptor,
    input_schema: Value,
    reference: NativeImplementationRef,
}
impl NativeBuiltinRegistration {
    pub(crate) fn new(
        image: Arc<NativeExecutableImage>,
        descriptor: CapabilityDescriptor,
        input_schema: Value,
    ) -> Result<Self, NativeRegistrationError> {
        if descriptor.runtime != RuntimeKind::FirstParty
            || descriptor.provider.as_str() != "builtin"
            || !descriptor.id.as_str().starts_with("builtin.")
            || !input_schema.is_object()
            || jsonschema::validator_for(&input_schema).is_err()
        {
            return Err(NativeRegistrationError::Declaration);
        }
        // This Rust trait is an in-process interface in the verified linked
        // executable. No vtable may be restored/loaded across another binary.
        // The checksum also pins the actual registered declaration and resolved
        // schema rather than trusting a mutable reference filename.
        let bytes = serde_json::to_vec(&json!({
            "format":"native-first-party-contract/1",
            "adapter":"FirstPartyCapabilityHandler/1",
            "target_os":std::env::consts::OS,"target_arch":std::env::consts::ARCH,
            "declaration":descriptor,"input_schema":input_schema,
        }))
        .map_err(|_| NativeRegistrationError::Declaration)?;
        let reference = NativeImplementationRef {
            artifact_checksum: image.checksum(),
            contract_checksum: Sha256::digest(bytes).into(),
        };
        Ok(Self {
            image,
            descriptor,
            input_schema,
            reference,
        })
    }
    pub(crate) fn reference(&self) -> NativeImplementationRef {
        self.reference
    }
    pub(crate) fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    pub(crate) fn input_schema(&self) -> &Value {
        &self.input_schema
    }
    pub(crate) fn image(&self) -> &Arc<NativeExecutableImage> {
        &self.image
    }
}
