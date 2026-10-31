//! Exact declaration identity carried from the trusted catalogue to Auth.
//! This is selection metadata, never permission to use a Tool or credential.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    ExtensionId, HostApiError, RuntimeCredentialAccountProviderId, RuntimeCredentialAccountSetup,
    RuntimeCredentialRequirement, RuntimeCredentialRequirementSource, SecretHandle,
    canonical_json_v1, sha256_digest_token,
};

const FORMAT: &str = "runtime-credential-account-requirement/1";

/// An exact product-auth usage, independent of presentation name/order and of
/// invocation/flow identifiers. Private fields plus validated deserialization
/// prevent a SecretHandle declaration from becoming a product-account usage.
/// Retain this original value with setup and task selections; do not reconstruct
/// it from current labels or a later catalogue after a wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RequirementRecord", into = "RequirementRecord")]
pub struct RuntimeCredentialAccountRequirement {
    requester_extension: ExtensionId,
    requirement: RuntimeCredentialRequirement,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementRecord {
    format: String,
    requester_extension: ExtensionId,
    #[serde(deserialize_with = "deserialize_stored_declaration")]
    requirement: RuntimeCredentialRequirement,
}

/// This versioned stored envelope is written in the declaration's own
/// serialization shape. Legacy bare descriptors keep their existing decoder;
/// an unknown nested constraint must not disappear from an exact stored usage.
fn deserialize_stored_declaration<'de, D>(
    deserializer: D,
) -> Result<RuntimeCredentialRequirement, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let stored = serde_json::Value::deserialize(deserializer)?;
    let declaration: RuntimeCredentialRequirement = serde_json::from_value(stored.clone())
        .map_err(|_| {
            <D::Error as serde::de::Error>::custom("invalid stored credential declaration")
        })?;
    let encoded = serde_json::to_value(&declaration).map_err(|_| {
        <D::Error as serde::de::Error>::custom("stored credential declaration cannot be encoded")
    })?;
    if stored != encoded {
        return Err(<D::Error as serde::de::Error>::custom(
            "stored credential declaration differs from its supported exact format",
        ));
    }
    Ok(declaration)
}

fn invalid(reason: &str) -> HostApiError {
    HostApiError::InvariantViolation {
        reason: reason.to_owned(),
    }
}

impl RuntimeCredentialAccountRequirement {
    pub fn new(
        requester_extension: ExtensionId,
        requirement: RuntimeCredentialRequirement,
    ) -> Result<Self, HostApiError> {
        let RuntimeCredentialRequirementSource::ProductAuthAccount { setup, .. } =
            &requirement.source
        else {
            return Err(invalid("product-account requirement source is required"));
        };
        requirement.audience.validate_declaration()?;
        requirement.target.validate_declaration()?;
        fn check_scopes(scopes: &[String]) -> Result<(), HostApiError> {
            let mut seen = BTreeSet::new();
            for scope in scopes {
                if scope.is_empty()
                    || scope.trim() != scope
                    || scope.chars().any(char::is_control)
                    || !seen.insert(scope)
                {
                    return Err(invalid("credential requirement scopes are invalid"));
                }
            }
            Ok(())
        }
        check_scopes(&requirement.provider_scopes)?;
        if let RuntimeCredentialAccountSetup::OAuth { scopes } = setup {
            check_scopes(scopes)?;
        }
        Ok(Self {
            requester_extension,
            requirement,
        })
    }

    pub fn requester_extension(&self) -> &ExtensionId {
        &self.requester_extension
    }

    pub fn requirement(&self) -> &RuntimeCredentialRequirement {
        &self.requirement
    }

    /// Content identity only. A digest cannot establish provenance, account
    /// ownership, a selected credential generation or a dispatch grant.
    pub fn reference(&self) -> Result<String, HostApiError> {
        let value = serde_json::to_value(self)
            .map_err(|_| invalid("credential requirement cannot be encoded"))?;
        let canonical = canonical_json_v1(&value)?;
        let bytes = serde_json::to_vec(&canonical)
            .map_err(|_| invalid("credential requirement cannot be encoded"))?;
        Ok(sha256_digest_token(&bytes))
    }

    /// Transitional fields must agree with the retained exact declaration.
    /// They never provide evidence for a missing declaration or account binding.
    pub fn matches_dispatch(
        &self,
        handle: &SecretHandle,
        provider: &RuntimeCredentialAccountProviderId,
        provider_scopes: &[String],
        requester_extension: &ExtensionId,
    ) -> bool {
        self.requester_extension == *requester_extension
            && self.requirement.handle == *handle
            && self.requirement.provider_scopes.as_slice() == provider_scopes
            && matches!(
                &self.requirement.source,
                RuntimeCredentialRequirementSource::ProductAuthAccount { provider: actual, .. }
                    if actual == provider
            )
    }
}

impl TryFrom<RequirementRecord> for RuntimeCredentialAccountRequirement {
    type Error = HostApiError;

    fn try_from(value: RequirementRecord) -> Result<Self, Self::Error> {
        if value.format != FORMAT {
            return Err(invalid("unsupported credential requirement format"));
        }
        Self::new(value.requester_extension, value.requirement)
    }
}

impl From<RuntimeCredentialAccountRequirement> for RequirementRecord {
    fn from(value: RuntimeCredentialAccountRequirement) -> Self {
        Self {
            format: FORMAT.to_owned(),
            requester_extension: value.requester_extension,
            requirement: value.requirement,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkScheme, NetworkTargetPattern, RuntimeCredentialTarget};

    fn declaration() -> RuntimeCredentialRequirement {
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("access").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
                setup: RuntimeCredentialAccountSetup::ManualToken,
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: Some(NetworkScheme::Https),
                host_pattern: "api.example.com".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        }
    }

    fn capture(requirement: RuntimeCredentialRequirement) -> RuntimeCredentialAccountRequirement {
        RuntimeCredentialAccountRequirement::new(
            ExtensionId::new("example-extension").unwrap(),
            requirement,
        )
        .unwrap()
    }

    #[test]
    fn selection_identity_retains_every_part_of_the_original_usage() {
        let original = declaration();
        let captured = capture(original.clone());
        let reference = captured.reference().unwrap();
        let mut variants = Vec::new();
        let mut changed = original.clone();
        changed.handle = SecretHandle::new("other-access").unwrap();
        variants.push(changed);
        let mut changed = original.clone();
        changed.provider_scopes = vec!["write".into()];
        variants.push(changed);
        let mut changed = original.clone();
        changed.audience.host_pattern = "other.example.com".into();
        variants.push(changed);
        let mut changed = original.clone();
        changed.target = RuntimeCredentialTarget::QueryParam {
            name: "token".into(),
        };
        variants.push(changed);
        let mut changed = original.clone();
        changed.source = RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
            setup: RuntimeCredentialAccountSetup::OAuth {
                scopes: vec!["read".into()],
            },
        };
        variants.push(changed);
        let mut changed = original.clone();
        changed.required = false;
        variants.push(changed);
        for changed in variants {
            assert_ne!(capture(changed).reference().unwrap(), reference);
        }
        let other_requester = RuntimeCredentialAccountRequirement::new(
            ExtensionId::new("other-extension").unwrap(),
            original.clone(),
        )
        .unwrap();
        assert_ne!(other_requester.reference().unwrap(), reference);
        let roundtrip: RuntimeCredentialAccountRequirement =
            serde_json::from_value(serde_json::to_value(&captured).unwrap()).unwrap();
        assert_eq!(roundtrip.requirement(), &original);
        assert_eq!(roundtrip.reference().unwrap(), reference);
    }

    #[test]
    fn malformed_stored_requirement_does_not_become_a_selected_usage() {
        let original = serde_json::to_value(capture(declaration())).unwrap();
        for pointer in [
            "/requirement",
            "/requirement/source",
            "/requirement/source/setup",
            "/requirement/audience",
            "/requirement/target",
        ] {
            let mut changed = original.clone();
            changed
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unrecognized_constraint".into(), serde_json::json!(true));
            assert!(
                serde_json::from_value::<RuntimeCredentialAccountRequirement>(changed).is_err(),
                "nested constraints must not be silently discarded at {pointer}"
            );
        }
        let mut changed = original.clone();
        changed["format"] = serde_json::json!("unrecognized");
        assert!(serde_json::from_value::<RuntimeCredentialAccountRequirement>(changed).is_err());
        let mut changed = original.clone();
        changed["requirement"]["source"] = serde_json::json!({"type":"secret_handle"});
        assert!(serde_json::from_value::<RuntimeCredentialAccountRequirement>(changed).is_err());
        let mut changed = original.clone();
        changed["requirement"]["provider_scopes"] = serde_json::json!(["read", "read"]);
        assert!(serde_json::from_value::<RuntimeCredentialAccountRequirement>(changed).is_err());
        let mut changed = original;
        changed["requirement"]["audience"]["host_pattern"] = serde_json::json!("invalid host");
        assert!(serde_json::from_value::<RuntimeCredentialAccountRequirement>(changed).is_err());
    }

    #[test]
    fn transitional_dispatch_fields_cannot_substitute_a_different_usage() {
        let original = declaration();
        let captured = capture(original.clone());
        let provider = RuntimeCredentialAccountProviderId::new("example").unwrap();
        assert!(captured.matches_dispatch(
            &original.handle,
            &provider,
            &original.provider_scopes,
            captured.requester_extension(),
        ));
        assert!(!captured.matches_dispatch(
            &SecretHandle::new("other-slot").unwrap(),
            &provider,
            &original.provider_scopes,
            captured.requester_extension(),
        ));
        assert!(!captured.matches_dispatch(
            &original.handle,
            &provider,
            &original.provider_scopes,
            &ExtensionId::new("other-extension").unwrap(),
        ));
    }

    #[test]
    fn conflicting_captured_obligation_is_rejected_before_allow_normalization() {
        let original = declaration();
        let captured = capture(original.clone());
        let obligation = crate::Obligation::InjectCredentialAccountOnce {
            handle: SecretHandle::new("other-slot").unwrap(),
            provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
            provider_scopes: original.provider_scopes,
            requester_extension: captured.requester_extension().clone(),
            requirement: Some(Box::new(captured)),
        };
        assert!(crate::Obligations::new(vec![obligation]).is_err());
    }
}
