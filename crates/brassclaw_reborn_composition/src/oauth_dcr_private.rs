//! Private encrypted DCR payloads. Never carry these through Auth projections,
//! model inputs, generic JSON values or Debug output.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use brassclaw_auth::AuthProductError;
use brassclaw_host_api::{RuntimeHttpEgressResponse, RuntimeHttpSavedBody};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DcrLegacyPayload {
    #[serde(with = "secret_text")]
    pub pkce: SecretString,
    pub client: crate::oauth_dcr_protocol::StoredDcrClientMaterial,
}
impl DcrLegacyPayload {
    pub(super) fn decode(raw: &SecretString) -> Result<Self, AuthProductError> {
        decode(raw)
    }
    pub(super) fn encode(&self) -> Result<SecretString, AuthProductError> {
        encode(self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DcrPreparationRecord {
    pub format: String,
    #[serde(with = "secret_text")]
    pub pkce: SecretString,
    #[serde(with = "secret_text")]
    pub state: SecretString,
    pub redirect_uri: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
    pub client_name: String,
    pub resource: Option<String>,
}
impl DcrPreparationRecord {
    pub(super) fn decode(raw: &SecretString) -> Result<Self, AuthProductError> {
        let record: Self = decode(raw)?;
        if record.format != "dcr-private-preparation/1" {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(record)
    }
    pub(super) fn encode(&self) -> Result<SecretString, AuthProductError> {
        encode(self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DcrResponseRecord {
    pub status: u16,
    #[serde(with = "secret_headers")]
    pub headers: Vec<(String, SecretString)>,
    #[serde(with = "secret_text")]
    pub body_base64: SecretString,
    #[serde(default)]
    pub saved_body: Option<RuntimeHttpSavedBody>,
    #[serde(default)]
    pub request_bytes: u64,
    #[serde(default)]
    pub response_bytes: u64,
    #[serde(default)]
    pub redaction_applied: bool,
}
impl DcrResponseRecord {
    pub(super) fn capture(response: RuntimeHttpEgressResponse) -> Self {
        let body = SecretBox::new(Box::new(response.body));
        Self {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            body_base64: URL_SAFE_NO_PAD.encode(body.expose_secret()).into(),
            saved_body: response.saved_body,
            request_bytes: response.request_bytes,
            response_bytes: response.response_bytes,
            redaction_applied: response.redaction_applied,
        }
    }
    pub(super) fn decode(raw: &SecretString) -> Result<Self, AuthProductError> {
        decode(raw)
    }
    pub(super) fn encode(&self) -> Result<SecretString, AuthProductError> {
        encode(self)
    }
    pub(super) fn body(&self) -> Result<SecretBox<Vec<u8>>, AuthProductError> {
        URL_SAFE_NO_PAD
            .decode(self.body_base64.expose_secret())
            .map(|bytes| SecretBox::new(Box::new(bytes)))
            .map_err(|_| AuthProductError::BackendUnavailable)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DcrManagementRecord {
    #[serde(with = "secret_text")]
    pub uri: SecretString,
    #[serde(with = "secret_text")]
    pub token: SecretString,
}
impl DcrManagementRecord {
    /// Cleanup needs only the management pair. A malformed/missing client_id
    /// must prevent challenge publication, but must not discard an otherwise
    /// recoverable registration cleanup obligation.
    pub(super) fn from_registration(body: &[u8]) -> Result<Option<Self>, AuthProductError> {
        #[derive(Deserialize)]
        struct Fields {
            registration_client_uri: Option<SecretString>,
            registration_access_token: Option<SecretString>,
        }
        let fields: Fields =
            serde_json::from_slice(body).map_err(|_| AuthProductError::BackendUnavailable)?;
        Ok(
            match (
                fields.registration_client_uri,
                fields.registration_access_token,
            ) {
                (Some(uri), Some(token)) => Some(Self { uri, token }),
                _ => None,
            },
        )
    }
    pub(super) fn encode(&self) -> Result<SecretString, AuthProductError> {
        encode(self)
    }
}

fn encode<T: Serialize>(value: &T) -> Result<SecretString, AuthProductError> {
    serde_json::to_string(value)
        .map(SecretString::from)
        .map_err(|_| AuthProductError::BackendUnavailable)
}
fn decode<T: for<'de> Deserialize<'de>>(value: &SecretString) -> Result<T, AuthProductError> {
    serde_json::from_str(value.expose_secret()).map_err(|_| AuthProductError::BackendUnavailable)
}
mod secret_text {
    use super::*;
    pub(super) fn serialize<S: serde::Serializer>(
        value: &SecretString,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.expose_secret())
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SecretString, D::Error> {
        String::deserialize(deserializer).map(SecretString::from)
    }
}
mod secret_headers {
    use super::*;
    pub(super) fn serialize<S: serde::Serializer>(
        value: &[(String, SecretString)],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            value
                .iter()
                .map(|(name, value)| (name.as_str(), value.expose_secret())),
        )
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<(String, SecretString)>, D::Error> {
        Vec::<(String, String)>::deserialize(deserializer)
            .map(|values| values.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}
