//! One explicit model-listing exchange. No provider registry, credential
//! discovery, model selection, guessed models or workflow decisions.
use crate::ProviderProtocol;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDiscoveryPolicy {
    pub timeout_secs: u64,
    pub max_response_bytes: u64,
}

impl Default for ModelDiscoveryPolicy {
    fn default() -> Self {
        Self {
            timeout_secs: 5,
            max_response_bytes: 10 * 1024 * 1024,
        }
    }
}

impl ModelDiscoveryPolicy {
    pub fn validate(self) -> Result<(), ModelDiscoveryError> {
        if self.timeout_secs == 0
            || self.max_response_bytes == 0
            || self.max_response_bytes == u64::MAX
            || usize::try_from(self.max_response_bytes).is_err()
            || Instant::now()
                .checked_add(Duration::from_secs(self.timeout_secs))
                .is_none()
        {
            return Err(ModelDiscoveryError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelDiscoveryError {
    #[error("model discovery limits are invalid or cannot be represented on this host")]
    InvalidPolicy,
    #[error("configure an HTTP(S) model endpoint without credentials, query or fragment")]
    InvalidEndpoint,
    #[error("this adapter does not implement model discovery")]
    UnsupportedAdapter,
    #[error("model discovery exceeded its configured deadline")]
    Timeout,
    #[error("model endpoint could not be connected to")]
    Connection,
    #[error("model discovery transport failed")]
    Transport,
    #[error("model endpoint returned HTTP {0}; no fallback models were supplied")]
    Http(u16),
    #[error("model endpoint response exceeded the configured byte limit")]
    Capacity,
    #[error("memory for the model response could not be allocated")]
    Allocation,
    #[error("model endpoint returned an invalid or conflicting model list")]
    InvalidResponse,
}

fn transport(error: reqwest::Error) -> ModelDiscoveryError {
    // Never format a raw URL/header-bearing request error.
    if error.is_timeout() {
        ModelDiscoveryError::Timeout
    } else if error.is_connect() {
        ModelDiscoveryError::Connection
    } else {
        ModelDiscoveryError::Transport
    }
}

/// Implemented transport capability, independent of catalogue setup labels
/// and of whether this specific endpoint/credential will accept a request.
pub fn supports_protocol(protocol: ProviderProtocol) -> bool {
    !matches!(
        protocol,
        ProviderProtocol::GeminiOauth | ProviderProtocol::Bedrock | ProviderProtocol::OpenAiCodex
    )
}

/// Adapter/endpoint/key come from the caller's captured definition or explicit
/// operator draft. An absent key remains absent; no environment is inspected.
pub async fn fetch_models(
    protocol: ProviderProtocol,
    base_url: &str,
    api_key: Option<&str>,
    policy: ModelDiscoveryPolicy,
) -> Result<Vec<(String, String)>, ModelDiscoveryError> {
    policy.validate()?;
    let mut endpoint =
        reqwest::Url::parse(base_url).map_err(|_| ModelDiscoveryError::InvalidEndpoint)?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(ModelDiscoveryError::InvalidEndpoint);
    }
    let (path, ollama) = match protocol {
        ProviderProtocol::Anthropic | ProviderProtocol::NearAi => {
            let path = endpoint.path().trim_end_matches('/');
            (
                if path.ends_with("/v1") {
                    format!("{path}/models")
                } else {
                    format!("{path}/v1/models")
                },
                false,
            )
        }
        ProviderProtocol::Ollama => (
            format!("{}/api/tags", endpoint.path().trim_end_matches('/')),
            true,
        ),
        ProviderProtocol::Gemini => {
            let path = endpoint.path().trim_end_matches('/');
            (
                if path.ends_with("/v1beta") || path.ends_with("/v1") {
                    format!("{path}/models")
                } else {
                    format!("{path}/v1beta/models")
                },
                false,
            )
        }
        ProviderProtocol::OpenAiCompletions
        | ProviderProtocol::DeepSeek
        | ProviderProtocol::OpenRouter
        | ProviderProtocol::GithubCopilot => (
            format!("{}/models", endpoint.path().trim_end_matches('/')),
            false,
        ),
        ProviderProtocol::GeminiOauth
        | ProviderProtocol::Bedrock
        | ProviderProtocol::OpenAiCodex => return Err(ModelDiscoveryError::UnsupportedAdapter),
    };
    endpoint.set_path(&path);
    static CLIENT: OnceLock<Result<reqwest::Client, reqwest::Error>> = OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
        })
        .as_ref()
        .map_err(|_| ModelDiscoveryError::Transport)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(policy.timeout_secs))
        .ok_or(ModelDiscoveryError::InvalidPolicy)?;
    let mut total_bytes = 0u64;
    let mut models = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cursors = BTreeSet::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ModelDiscoveryError::Timeout);
        }
        let mut request = client.get(endpoint.clone()).timeout(remaining);
        if protocol == ProviderProtocol::Anthropic {
            request = request.header("anthropic-version", "2023-06-01");
            if let Some(key) = api_key {
                request = request.header("x-api-key", key);
            }
        } else if protocol == ProviderProtocol::Gemini {
            if let Some(key) = api_key {
                request = request.header("x-goog-api-key", key);
            }
        } else if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }
        let mut response = request.send().await.map_err(transport)?;
        if !response.status().is_success() {
            return Err(ModelDiscoveryError::Http(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|bytes| bytes > policy.max_response_bytes.saturating_sub(total_bytes))
        {
            return Err(ModelDiscoveryError::Capacity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport)? {
            total_bytes = total_bytes
                .checked_add(chunk.len() as u64)
                .ok_or(ModelDiscoveryError::Capacity)?;
            if total_bytes > policy.max_response_bytes {
                return Err(ModelDiscoveryError::Capacity);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| ModelDiscoveryError::Allocation)?;
            bytes.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        struct Entry {
            id: String,
        }
        #[derive(Deserialize)]
        struct ModelList {
            data: Vec<Entry>,
            #[serde(default)]
            has_more: bool,
            #[serde(default)]
            last_id: Option<String>,
        }
        #[derive(Deserialize)]
        struct Tag {
            name: String,
        }
        #[derive(Deserialize)]
        struct OllamaList {
            models: Vec<Tag>,
        }
        #[derive(Deserialize)]
        struct GeminiList {
            #[serde(default)]
            models: Vec<Tag>,
            #[serde(default, rename = "nextPageToken")]
            next_page_token: Option<String>,
        }
        let (identifiers, next) = if protocol == ProviderProtocol::Gemini {
            let page = serde_json::from_slice::<GeminiList>(&bytes)
                .map_err(|_| ModelDiscoveryError::InvalidResponse)?;
            let identifiers = page
                .models
                .into_iter()
                .map(|model| {
                    let id = model
                        .name
                        .strip_prefix("models/")
                        .filter(|id| !id.is_empty() && !id.contains('/'))
                        .ok_or(ModelDiscoveryError::InvalidResponse)?;
                    Ok(id.to_owned())
                })
                .collect::<Result<Vec<_>, ModelDiscoveryError>>()?;
            (
                identifiers,
                page.next_page_token.filter(|token| !token.is_empty()),
            )
        } else if ollama {
            (
                serde_json::from_slice::<OllamaList>(&bytes)
                    .map_err(|_| ModelDiscoveryError::InvalidResponse)?
                    .models
                    .into_iter()
                    .map(|model| model.name)
                    .collect::<Vec<_>>(),
                None,
            )
        } else {
            let page = serde_json::from_slice::<ModelList>(&bytes)
                .map_err(|_| ModelDiscoveryError::InvalidResponse)?;
            let next = if page.has_more {
                // Documented Anthropic last_id/after_id protocol. Other
                // adapters cannot silently return a partial first page.
                if protocol != ProviderProtocol::Anthropic
                    || page.data.last().map(|v| &v.id) != page.last_id.as_ref()
                {
                    return Err(ModelDiscoveryError::InvalidResponse);
                }
                Some(page.last_id.ok_or(ModelDiscoveryError::InvalidResponse)?)
            } else {
                None
            };
            (
                page.data
                    .into_iter()
                    .map(|model| model.id)
                    .collect::<Vec<_>>(),
                next,
            )
        };
        for id in identifiers {
            if id.trim().is_empty()
                || id.trim() != id
                || id.chars().any(char::is_control)
                || !seen.insert(id.clone())
            {
                return Err(ModelDiscoveryError::InvalidResponse);
            }
            models.push((id.clone(), id));
        }
        if let Some(cursor) = next {
            if !cursors.insert(cursor.clone()) {
                return Err(ModelDiscoveryError::InvalidResponse);
            }
            endpoint.query_pairs_mut().clear().append_pair(
                if protocol == ProviderProtocol::Gemini {
                    "pageToken"
                } else {
                    "after_id"
                },
                &cursor,
            );
        } else {
            if Instant::now() >= deadline {
                return Err(ModelDiscoveryError::Timeout);
            }
            return Ok(models);
        }
    }
}
