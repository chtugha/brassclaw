//! Immutable original/retained value codec inside the existing secret owner.
//! No runtime raw-decrypt port, authority, workflow or independent registry.
use std::fmt;

use brassclaw_host_api::{ResourceScope, SecretHandle, Timestamp};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{SecretError, SecretMaterial, SecretsCrypto};

/// An exact source selection. A copied reference never grants access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretCustodySource {
    pub scope: ResourceScope,
    pub handle: SecretHandle,
    pub reference: String,
}

/// Nonmaterial state. Context comes from the typed trusted setup producer,
/// never from a source/status label or an authorization claim.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SecretCustodyState {
    RetiredOrdinary {
        original_reference: String,
        context: Value,
    },
    ClosedIdentity {
        context: Value,
    },
    Original {
        context: Value,
    },
    Retained {
        source: SecretCustodySource,
    },
    RetiredOriginal {
        original_reference: String,
        context: Value,
    },
    RetiredRetained {
        source: SecretCustodySource,
        retained_reference: String,
    },
}
impl fmt::Debug for SecretCustodyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Defense in depth if a future trusted producer supplies a bad context.
        f.write_str(match self {
            Self::ClosedIdentity { .. } => "ClosedIdentity([REDACTED])",
            Self::RetiredOrdinary { .. } => "RetiredOrdinary([REDACTED])",
            Self::Original { .. } => "Original([REDACTED])",
            Self::Retained { .. } => "Retained([REDACTED])",
            Self::RetiredOriginal { .. } => "RetiredOriginal([REDACTED])",
            Self::RetiredRetained { .. } => "RetiredRetained([REDACTED])",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretCustodyMetadata {
    pub scope: ResourceScope,
    pub handle: SecretHandle,
    pub reference: String,
    pub state: SecretCustodyState,
}

// This private enum owns every raw value, never derives Debug, and binds ALL
// context/source/retirement fields inside the existing authenticated encryption.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CustodyBody {
    RetiredOrdinary {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        original_reference: String,
        context: Value,
    },
    ClosedIdentity {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        context: Value,
    },
    Original {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        context: Value,
        #[serde(with = "material_serde")]
        material: SecretMaterial,
    },
    Retained {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        #[serde(deserialize_with = "deserialize_stored_source")]
        source: SecretCustodySource,
        #[serde(with = "material_serde")]
        material: SecretMaterial,
    },
    RetiredOriginal {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        original_reference: String,
        context: Value,
    },
    RetiredRetained {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        #[serde(deserialize_with = "deserialize_stored_source")]
        source: SecretCustodySource,
        retained_reference: String,
    },
}
impl CustodyBody {
    pub(crate) fn scope(&self) -> &ResourceScope {
        match self {
            Self::RetiredOrdinary { scope, .. }
            | Self::ClosedIdentity { scope, .. }
            | Self::Original { scope, .. }
            | Self::Retained { scope, .. }
            | Self::RetiredOriginal { scope, .. }
            | Self::RetiredRetained { scope, .. } => scope,
        }
    }
    pub(crate) fn material_for_compare(&self) -> Option<&SecretMaterial> {
        match self {
            Self::Original { material, .. } | Self::Retained { material, .. } => Some(material),
            Self::RetiredOrdinary { .. }
            | Self::ClosedIdentity { .. }
            | Self::RetiredOriginal { .. }
            | Self::RetiredRetained { .. } => None,
        }
    }
    pub(crate) fn material_for_lease(&self) -> Option<&SecretMaterial> {
        match self {
            Self::Retained { material, .. } => Some(material),
            Self::RetiredOrdinary { .. }
            | Self::ClosedIdentity { .. }
            | Self::Original { .. }
            | Self::RetiredOriginal { .. }
            | Self::RetiredRetained { .. } => None,
        }
    }
    pub(crate) fn metadata(
        &self,
        handle: SecretHandle,
        reference: String,
    ) -> SecretCustodyMetadata {
        let state = match self {
            Self::RetiredOrdinary {
                original_reference,
                context,
                ..
            } => SecretCustodyState::RetiredOrdinary {
                original_reference: original_reference.clone(),
                context: context.clone(),
            },
            Self::ClosedIdentity { context, .. } => SecretCustodyState::ClosedIdentity {
                context: context.clone(),
            },
            Self::Original { context, .. } => SecretCustodyState::Original {
                context: context.clone(),
            },
            Self::Retained { source, .. } => SecretCustodyState::Retained {
                source: source.clone(),
            },
            Self::RetiredOriginal {
                original_reference,
                context,
                ..
            } => SecretCustodyState::RetiredOriginal {
                original_reference: original_reference.clone(),
                context: context.clone(),
            },
            Self::RetiredRetained {
                source,
                retained_reference,
                ..
            } => SecretCustodyState::RetiredRetained {
                source: source.clone(),
                retained_reference: retained_reference.clone(),
            },
        };
        SecretCustodyMetadata {
            scope: self.scope().clone(),
            handle,
            reference,
            state,
        }
    }
    /// Consume the old value so its material is dropped/zeroized. Retiring again
    /// is not a new generation or permission to reuse the same identity.
    pub(crate) fn retire(self, reference: String) -> Self {
        match self {
            Self::Original { scope, context, .. } => Self::RetiredOriginal {
                scope,
                original_reference: reference,
                context,
            },
            Self::Retained { scope, source, .. } => Self::RetiredRetained {
                scope,
                source,
                retained_reference: reference,
            },
            retired @ (Self::RetiredOrdinary { .. }
            | Self::ClosedIdentity { .. }
            | Self::RetiredOriginal { .. }
            | Self::RetiredRetained { .. }) => retired,
        }
    }
    pub(crate) fn is_retired(&self) -> bool {
        matches!(
            self,
            Self::RetiredOrdinary { .. }
                | Self::ClosedIdentity { .. }
                | Self::RetiredOriginal { .. }
                | Self::RetiredRetained { .. }
        )
    }
}

// This decoder is called only inside an authenticated private custody body.
// Public ResourceScope and Source DTO parsing keep their ordinary reserved-ID
// rejection; copied metadata is never a grant or authority for this scope.
fn deserialize_stored_source<'de, D>(deserializer: D) -> Result<SecretCustodySource, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StoredSource {
        #[serde(deserialize_with = "crate::stored_scope::deserialize_stored_scope")]
        scope: ResourceScope,
        handle: SecretHandle,
        reference: String,
    }
    let source = StoredSource::deserialize(deserializer)?;
    Ok(SecretCustodySource {
        scope: source.scope,
        handle: source.handle,
        reference: source.reference,
    })
}

fn custody_aad(native_record_aad: &[u8]) -> Vec<u8> {
    // Reuse the actual length-prefixed AAD construction and AES-GCM/HKDF owner;
    // this is a new stored purpose, not a new cipher or plaintext heuristic.
    crate::crypto::build_aad(b"reborn/v1/secret_custody", &[native_record_aad])
}

pub(crate) fn encrypt_body(
    crypto: &SecretsCrypto,
    native_record_aad: &[u8],
    body: &CustodyBody,
) -> Result<(Vec<u8>, Vec<u8>), SecretError> {
    // Both the structured raw field and complete serialized plaintext buffer
    // use SecretString ownership. No raw JSON String survives this owner call.
    let serialized = serde_json::to_string(body)
        .map(SecretMaterial::from)
        .map_err(|_| SecretError::EncryptionFailed("invalid custody encoding".into()))?;
    // Check the actual stored decoder before a backend can persist an envelope
    // it cannot later read (including reserved internal scope shape). Both
    // temporary raw-field copies have zeroizing ownership; errors stay static.
    drop(
        serde_json::from_str::<CustodyBody>(serialized.expose_secret())
            .map_err(|_| SecretError::EncryptionFailed("unsupported custody encoding".into()))?,
    );
    crypto.encrypt(
        serialized.expose_secret().as_bytes(),
        &custody_aad(native_record_aad),
    )
}
pub(crate) fn decrypt_body(
    crypto: &SecretsCrypto,
    native_record_aad: &[u8],
    ciphertext: &[u8],
    salt: &[u8],
) -> Result<CustodyBody, SecretError> {
    let decrypted = crypto.decrypt(ciphertext, salt, &custody_aad(native_record_aad))?;
    serde_json::from_str(decrypted.expose())
        .map_err(|_| SecretError::DecryptionFailed("invalid authenticated custody payload".into()))
}
mod material_serde {
    use super::*;
    pub(super) fn serialize<S: serde::Serializer>(
        material: &SecretMaterial,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(material.expose_secret())
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SecretMaterial, D::Error> {
        String::deserialize(deserializer).map(SecretMaterial::from)
    }
}

pub(crate) const CUSTODY_FORMAT_V1: u8 = 1;
pub(crate) const TIMED_ORDINARY_FORMAT_V1: u8 = 2;

// This structured format is selected only by an explicit authenticated purpose,
// never by inspecting ordinary token text. In particular, a legacy token that
// happens to be JSON must keep its exact original interpretation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrdinarySecretValue {
    #[serde(with = "material_serde")]
    pub(crate) material: SecretMaterial,
    pub(crate) expires_at: Option<Timestamp>,
}

pub(crate) struct EncryptedOrdinarySecret {
    pub(crate) ciphertext: Vec<u8>,
    pub(crate) salt: Vec<u8>,
    pub(crate) format: Option<u8>,
}

impl OrdinarySecretValue {
    pub(crate) fn encrypt(
        &self,
        crypto: &SecretsCrypto,
        native_record_aad: &[u8],
    ) -> Result<EncryptedOrdinarySecret, SecretError> {
        if self.expires_at.is_none() {
            let (ciphertext, salt) =
                crypto.encrypt(self.material.expose_secret().as_bytes(), native_record_aad)?;
            return Ok(EncryptedOrdinarySecret {
                ciphertext,
                salt,
                format: None,
            });
        }
        let serialized = serde_json::to_string(self)
            .map(SecretMaterial::from)
            .map_err(|_| {
                SecretError::EncryptionFailed("invalid expiring secret encoding".into())
            })?;
        let (ciphertext, salt) = crypto.encrypt(
            serialized.expose_secret().as_bytes(),
            &timed_ordinary_aad(native_record_aad),
        )?;
        Ok(EncryptedOrdinarySecret {
            ciphertext,
            salt,
            format: Some(TIMED_ORDINARY_FORMAT_V1),
        })
    }
}

fn timed_ordinary_aad(native_record_aad: &[u8]) -> Vec<u8> {
    crate::crypto::build_aad(b"reborn/v1/expiring_ordinary_secret", &[native_record_aad])
}

/// Owner-internal decoded data. No raw representation crosses a product port.
pub(crate) enum DecodedSecretValue {
    Ordinary(OrdinarySecretValue),
    // Do not inflate every ordinary secret read to the full custody context.
    Custody(Box<CustodyBody>),
}
impl DecodedSecretValue {
    pub(crate) fn material_for_compare(&self) -> Option<&SecretMaterial> {
        match self {
            Self::Ordinary(value) => Some(&value.material),
            Self::Custody(body) => body.material_for_compare(),
        }
    }
    pub(crate) fn into_runtime_material(self) -> Option<SecretMaterial> {
        match self {
            Self::Ordinary(value) => Some(value.material),
            Self::Custody(body) => match *body {
                CustodyBody::Retained { material, .. } => Some(material),
                _ => None,
            },
        }
    }
    pub(crate) fn runtime_eligible(&self) -> bool {
        match self {
            Self::Ordinary(_) => true,
            Self::Custody(body) => body.material_for_lease().is_some(),
        }
    }
    pub(crate) fn custody(&self) -> Option<&CustodyBody> {
        match self {
            Self::Ordinary(_) => None,
            Self::Custody(body) => Some(body.as_ref()),
        }
    }
    pub(crate) fn secret_expires_at(&self) -> Option<Timestamp> {
        match self {
            Self::Ordinary(value) => value.expires_at,
            Self::Custody(_) => None,
        }
    }
}

pub(crate) fn decrypt_stored_value(
    crypto: &SecretsCrypto,
    native_record_aad: &[u8],
    ciphertext: &[u8],
    salt: &[u8],
    format: Option<u8>,
) -> Result<DecodedSecretValue, SecretError> {
    match format {
        None => crypto
            .decrypt(ciphertext, salt, native_record_aad)
            .map(|value| {
                DecodedSecretValue::Ordinary(OrdinarySecretValue {
                    material: value.into_material(),
                    expires_at: None,
                })
            }),
        Some(CUSTODY_FORMAT_V1) => decrypt_body(crypto, native_record_aad, ciphertext, salt)
            .map(|body| DecodedSecretValue::Custody(Box::new(body))),
        Some(TIMED_ORDINARY_FORMAT_V1) => {
            let decrypted =
                crypto.decrypt(ciphertext, salt, &timed_ordinary_aad(native_record_aad))?;
            let value: OrdinarySecretValue =
                serde_json::from_str(decrypted.expose()).map_err(|_| {
                    SecretError::DecryptionFailed("invalid expiring secret payload".into())
                })?;
            if value.expires_at.is_none() {
                return Err(SecretError::DecryptionFailed(
                    "missing authenticated secret expiry".into(),
                ));
            }
            Ok(DecodedSecretValue::Ordinary(value))
        }
        Some(_) => Err(SecretError::DecryptionFailed(
            "unsupported secret custody format".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{InvocationId, UserId};

    #[test]
    fn authenticated_internal_system_custody_preserves_scope_without_public_sentinel_parsing() {
        let scope = ResourceScope::system();
        let public_json = serde_json::to_string(&scope).unwrap();
        assert!(serde_json::from_str::<ResourceScope>(&public_json).is_err());
        let source = SecretCustodySource {
            scope: scope.clone(),
            handle: SecretHandle::new("internal-original").unwrap(),
            reference: "exact-original".into(),
        };
        let body = CustodyBody::Retained {
            scope: scope.clone(),
            source: source.clone(),
            material: "system-retained-value".into(),
        };
        let handle = SecretHandle::new("internal-retained").unwrap();
        let native = crate::filesystem_secret_aad(&scope, &handle);
        let crypto = SecretsCrypto::new("secret-custody-native-master-key-2026".into()).unwrap();
        let (ciphertext, salt) = encrypt_body(&crypto, &native, &body).unwrap();
        let decoded = decrypt_body(&crypto, &native, &ciphertext, &salt).unwrap();
        assert_eq!(decoded.scope(), &scope);
        assert!(
            matches!(decoded, CustodyBody::Retained { source: actual, .. } if actual == source)
        );
        assert!(
            serde_json::from_str::<SecretCustodySource>(&serde_json::to_string(&source).unwrap())
                .is_err()
        );
    }

    #[test]
    fn custody_encoding_rejects_an_unrehydratable_internal_scope_before_encryption() {
        let scope = ResourceScope {
            project_id: Some(
                brassclaw_host_api::ProjectId::new("invalid-system-combination").unwrap(),
            ),
            ..ResourceScope::system()
        };
        let handle = SecretHandle::new("invalid-scope-custody").unwrap();
        let body = CustodyBody::Original {
            scope: scope.clone(),
            context: serde_json::json!({"original":"must not be written"}),
            material: "raw input must never enter an error".into(),
        };
        let crypto = SecretsCrypto::new("secret-custody-native-master-key-2026".into()).unwrap();
        let error = encrypt_body(
            &crypto,
            &crate::filesystem_secret_aad(&scope, &handle),
            &body,
        )
        .unwrap_err();
        assert!(
            !error
                .to_string()
                .contains("raw input must never enter an error")
        );
    }

    #[test]
    fn custody_purpose_is_authenticated_and_retirement_drops_comparison_and_runtime_material() {
        let scope = ResourceScope::local_default(
            UserId::new("custody-owner").unwrap(),
            InvocationId::new(),
        )
        .unwrap();
        let handle = SecretHandle::new("custody-codec-original").unwrap();
        let native = crate::filesystem_secret_aad(&scope, &handle);
        let crypto = SecretsCrypto::new("secret-custody-native-master-key-2026".into()).unwrap();
        let raw = "raw-custody-input-that-must-never-be-visible";
        let body = CustodyBody::Original {
            scope: scope.clone(),
            context: serde_json::json!({"interaction":"original-identity"}),
            material: raw.into(),
        };
        let (ciphertext, salt) = encrypt_body(&crypto, &native, &body).unwrap();
        assert!(
            crypto.decrypt(&ciphertext, &salt, &native).is_err(),
            "ordinary reader cannot reinterpret the custody purpose"
        );
        let decoded = decrypt_body(&crypto, &native, &ciphertext, &salt).unwrap();
        assert_eq!(decoded.material_for_compare().unwrap().expose_secret(), raw);
        assert!(decoded.material_for_lease().is_none());
        let metadata = decoded.metadata(handle.clone(), "opaque-original-reference".into());
        assert_eq!(metadata.scope, scope);
        assert!(!format!("{metadata:?}").contains(raw));
        assert!(!serde_json::to_string(&metadata).unwrap().contains(raw));
        let retired = decoded.retire("opaque-original-reference".into());
        assert!(retired.is_retired());
        assert!(retired.material_for_compare().is_none());
        assert!(retired.material_for_lease().is_none());
        let (encrypted_retired, retired_salt) = encrypt_body(&crypto, &native, &retired).unwrap();
        let retired = decrypt_body(&crypto, &native, &encrypted_retired, &retired_salt).unwrap();
        assert!(
            matches!(retired, CustodyBody::RetiredOriginal { ref original_reference, .. } if original_reference == "opaque-original-reference")
        );
        let (ordinary, ordinary_salt) = crypto.encrypt(raw.as_bytes(), &native).unwrap();
        assert!(decrypt_body(&crypto, &native, &ordinary, &ordinary_salt).is_err());
        let mut foreign = scope;
        foreign.user_id = UserId::new("different-custody-owner").unwrap();
        assert!(
            decrypt_body(
                &crypto,
                &crate::filesystem_secret_aad(&foreign, &handle),
                &ciphertext,
                &salt
            )
            .is_err()
        );
    }
}
