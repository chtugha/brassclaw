//! OAuth 2.0 session manager for OpenAI Codex (ChatGPT subscription).
//!
//! Supports two auth flows:
//! - **Device Code** (primary): Works on headless servers, no browser needed.
//! - **Browser PKCE** (fallback): Standard OAuth for local machines.
//!
//! Tokens are persisted to `~/.brassclaw/openai_codex_session.json` and
//! auto-refreshed before expiry.

/// Default session TTL (seconds) when the server returns expires_in=0 — 1 hour.
const SECS_PER_HOUR: u64 = 3600;
/// Default session keepalive TTL for the token manager — 15 minutes.
const SECS_15_MINUTES: u64 = 900;

use chrono::{DateTime, Utc};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::config::OpenAiCodexConfig;
use crate::error::LlmError;

/// Persisted OAuth session data.
///
/// Note: `Debug` is manually implemented to redact tokens.
#[derive(Serialize, Deserialize)]
pub struct OpenAiCodexSession {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
}

impl std::fmt::Debug for OpenAiCodexSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCodexSession")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// Request body for the device code usercode endpoint.
#[derive(Debug, Serialize)]
struct UserCodeRequest {
    client_id: String,
}

/// Response from the device code usercode endpoint.
#[derive(Debug, Deserialize)]
struct UserCodeResponse {
    /// Unique ID for this device auth session.
    device_auth_id: String,
    /// Code the user enters in their browser.
    user_code: String,
    /// URL where the user enters the code (may not be present).
    #[serde(default = "default_verification_uri")]
    verification_uri: String,
    /// Polling interval in seconds (OpenAI sends this as a string).
    #[serde(
        default = "default_interval",
        deserialize_with = "deserialize_string_or_u64"
    )]
    interval: u64,
    /// Expiry timestamp (OpenAI sends `expires_at` as ISO-8601).
    #[serde(default)]
    expires_at: Option<String>,
    /// Seconds until the device code expires (standard field, may not be present).
    #[serde(default)]
    expires_in: Option<u64>,
}

fn default_verification_uri() -> String {
    "https://auth.openai.com/codex/device".to_string()
}

fn default_interval() -> u64 {
    5
}

/// Deserialize a value that may be either a string or a number as u64.
fn deserialize_string_or_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct StringOrU64;
    impl<'de> de::Visitor<'de> for StringOrU64 {
        type Value = u64;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer")
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse().map_err(de::Error::custom)
        }
    }
    deserializer.deserialize_any(StringOrU64)
}

impl UserCodeResponse {
    /// Get the expiry duration in seconds, from either `expires_in` or `expires_at`.
    fn expires_in_secs_at(&self, now: DateTime<Utc>) -> Result<u64, LlmError> {
        if let Some(secs) = self.expires_in {
            return Ok(secs);
        }
        if let Some(ref ts) = self.expires_at {
            let dt =
                chrono::DateTime::parse_from_rfc3339(ts).map_err(|_| invalid_oauth_response())?;
            let remaining = dt.signed_duration_since(now).num_seconds();
            return Ok(remaining.max(0) as u64);
        }
        Ok(SECS_15_MINUTES) // default only when the issuer supplies no expiry
    }
}

/// Request body for polling the device auth token endpoint.
#[derive(Debug, Serialize)]
struct DeviceTokenPollRequest {
    device_auth_id: String,
    user_code: String,
}

/// Successful response from the device auth token endpoint.
/// Returns an authorization code + PKCE pair for the final token exchange.
#[derive(Debug, Deserialize)]
struct DeviceAuthCodeResponse {
    authorization_code: String,
    code_challenge: String,
    code_verifier: String,
}

/// Response from the final OAuth token exchange.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    token_type: String,
}

fn invalid_oauth_response() -> LlmError {
    LlmError::SessionRenewalFailed {
        provider: "openai_codex".into(),
        reason: "OAuth response has invalid credentials or expiry".into(),
    }
}

impl TokenResponse {
    fn into_session(
        self,
        previous: Option<(&str, DateTime<Utc>)>,
    ) -> Result<OpenAiCodexSession, LlmError> {
        self.into_session_at(previous, Utc::now())
    }
    fn into_session_at(
        self,
        previous: Option<(&str, DateTime<Utc>)>,
        now: DateTime<Utc>,
    ) -> Result<OpenAiCodexSession, LlmError> {
        if self.access_token.is_empty()
            || HeaderValue::from_str(&format!("Bearer {}", self.access_token)).is_err()
            || (!self.token_type.is_empty() && !self.token_type.eq_ignore_ascii_case("bearer"))
        {
            return Err(invalid_oauth_response());
        }
        let seconds = if self.expires_in == 0 {
            tracing::debug!("Token response has expires_in=0, defaulting to 3600s");
            SECS_PER_HOUR
        } else {
            self.expires_in
        };
        let duration = chrono::Duration::try_seconds(
            i64::try_from(seconds).map_err(|_| invalid_oauth_response())?,
        )
        .ok_or_else(invalid_oauth_response)?;
        let expires_at = now
            .checked_add_signed(duration)
            .ok_or_else(invalid_oauth_response)?;
        let created_at = previous.map_or(now, |(_, created_at)| created_at);
        let refresh_token = if self.refresh_token.is_empty() {
            previous.map_or_else(String::new, |(token, _)| token.to_owned())
        } else {
            self.refresh_token
        };
        Ok(OpenAiCodexSession {
            access_token: self.access_token,
            refresh_token,
            expires_at,
            created_at,
        })
    }
}

/// A started device-code flow. Returned by
/// [`OpenAiCodexSessionManager::initiate_device_code`]; the caller shows the
/// `user_code` + `verification_uri` to the user, then passes this back to
/// [`OpenAiCodexSessionManager::complete_device_code`] to poll for completion.
/// Internal polling state (device id, interval, expiry) is opaque to callers.
#[derive(Clone)]
pub struct DeviceCodeStart {
    device_auth_id: String,
    /// Code the user enters at `verification_uri`.
    pub user_code: String,
    /// URL where the user enters the code.
    pub verification_uri: String,
    interval_secs: u64,
    expires_secs: u64,
}

impl std::fmt::Debug for DeviceCodeStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceCodeStart([REDACTED])")
    }
}
impl DeviceCodeStart {
    /// One-shot native consumption after the host authenticates encrypted raw
    /// result custody. This accessor grants no transport or credential authority.
    pub fn into_native_parts(self) -> (SecretString, SecretString, String, u64, u64) {
        (
            self.device_auth_id.into(),
            self.user_code.into(),
            self.verification_uri,
            self.interval_secs,
            self.expires_secs,
        )
    }
}
/// Issuer-returned PKCE material, not a client-generated/fake callback state.
/// Neither this nor the token result implements serde or secret-bearing Debug.
pub struct CodexDeviceAuthorization {
    pub authorization_code: SecretString,
    pub code_challenge: String,
    pub code_verifier: SecretString,
}
impl std::fmt::Debug for CodexDeviceAuthorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodexDeviceAuthorization([REDACTED])")
    }
}
pub struct CodexDeviceTokens {
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub expires_at: DateTime<Utc>,
}
impl std::fmt::Debug for CodexDeviceTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodexDeviceTokens([REDACTED])")
    }
}
/// Actual OpenAI wire parser shared by the legacy File manager and native host.
/// The provided first observation time must be retained; re-reading originals
/// cannot create a fresh expiry. HTTP/body/egress admission belongs to the host.
pub fn parse_codex_device_start(
    body: &[u8],
    observed_at: DateTime<Utc>,
) -> Result<DeviceCodeStart, LlmError> {
    Ok(parse_codex_device_start_with_issuer_expiry(body, observed_at)?.0)
}
/// Preserve whether the issuer actually supplied an expiry. Native setup uses
/// its captured configurable host deadline when neither wire expiry exists;
/// only the legacy File manager retains the historical 15-minute fallback.
pub fn parse_codex_device_start_with_issuer_expiry(
    body: &[u8],
    observed_at: DateTime<Utc>,
) -> Result<(DeviceCodeStart, Option<DateTime<Utc>>), LlmError> {
    let device: UserCodeResponse =
        serde_json::from_slice(body).map_err(|_| invalid_oauth_response())?;
    if device.device_auth_id.is_empty() || device.user_code.is_empty() {
        return Err(invalid_oauth_response());
    }
    let expires_secs = device.expires_in_secs_at(observed_at)?;
    let issuer_expiry = if let Some(seconds) = device.expires_in {
        let duration = chrono::Duration::try_seconds(
            i64::try_from(seconds).map_err(|_| invalid_oauth_response())?,
        )
        .ok_or_else(invalid_oauth_response)?;
        Some(
            observed_at
                .checked_add_signed(duration)
                .ok_or_else(invalid_oauth_response)?,
        )
    } else if let Some(value) = &device.expires_at {
        Some(
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| invalid_oauth_response())?
                .with_timezone(&Utc),
        )
    } else {
        None
    };
    Ok((
        DeviceCodeStart {
            device_auth_id: device.device_auth_id,
            user_code: device.user_code,
            verification_uri: device.verification_uri,
            interval_secs: device.interval.max(default_interval()),
            expires_secs,
        },
        issuer_expiry,
    ))
}
pub fn parse_codex_device_authorization(body: &[u8]) -> Result<CodexDeviceAuthorization, LlmError> {
    let value: DeviceAuthCodeResponse =
        serde_json::from_slice(body).map_err(|_| invalid_oauth_response())?;
    if value.authorization_code.is_empty()
        || value.code_challenge.is_empty()
        || value.code_verifier.is_empty()
    {
        return Err(invalid_oauth_response());
    }
    Ok(CodexDeviceAuthorization {
        authorization_code: value.authorization_code.into(),
        code_challenge: value.code_challenge,
        code_verifier: value.code_verifier.into(),
    })
}
pub fn parse_codex_device_tokens(
    body: &[u8],
    request_started_at: DateTime<Utc>,
) -> Result<CodexDeviceTokens, LlmError> {
    let value: TokenResponse =
        serde_json::from_slice(body).map_err(|_| invalid_oauth_response())?;
    let session = value.into_session_at(None, request_started_at)?;
    Ok(CodexDeviceTokens {
        access_token: session.access_token.into(),
        refresh_token: session.refresh_token.into(),
        expires_at: session.expires_at,
    })
}
pub fn codex_device_minimum_interval_secs() -> u64 {
    default_interval()
}

/// Manages OpenAI Codex OAuth sessions with persistence and auto-refresh.
pub struct OpenAiCodexSessionManager {
    config: OpenAiCodexConfig,
    client: Client,
    session: RwLock<Option<OpenAiCodexSession>>,
    renewal_lock: Mutex<()>,
}

impl OpenAiCodexSessionManager {
    /// Create a new session manager. Tries to load existing session from disk.
    ///
    /// # Errors
    ///
    /// Returns `LlmError` if the HTTP client cannot be constructed or existing
    /// credentials cannot be read or decoded. A missing file is permitted.
    pub fn new(config: OpenAiCodexConfig) -> Result<Self, LlmError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("brassclaw/", env!("CARGO_PKG_VERSION"))),
        );
        let client = Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| LlmError::RequestFailed {
                provider: "openai_codex".into(),
                reason: format!("HTTP client build failed: {e}"),
            })?;

        // Only a missing file means that authentication has not been configured.
        // Corrupt or unreadable credentials must not start a new login implicitly.
        let session =
            match std::fs::read_to_string(&config.session_path) {
                Ok(data) => Some(serde_json::from_str::<OpenAiCodexSession>(&data).map_err(
                    |_| LlmError::SessionRenewalFailed {
                        provider: "openai_codex".into(),
                        reason: "Failed to parse saved OAuth session".into(),
                    },
                )?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(LlmError::Io(error)),
            };
        Ok(Self {
            config,
            client,
            session: RwLock::new(session),
            renewal_lock: Mutex::new(()),
        })
    }

    /// Check if we have a session (may be expired).
    pub async fn has_session(&self) -> bool {
        self.session.read().await.is_some()
    }

    /// Check if the current access token needs refreshing.
    pub async fn needs_refresh(&self) -> bool {
        let guard = self.session.read().await;
        match guard.as_ref() {
            None => true,
            Some(s) => match s.expires_at.signed_duration_since(Utc::now()).to_std() {
                Ok(remaining) => {
                    remaining
                        <= std::time::Duration::from_secs(self.config.token_refresh_margin_secs)
                }
                Err(_) => true,
            },
        }
    }

    /// Get the current access token, refreshing if needed.
    ///
    /// If the token is within the refresh margin, silently refreshes first.
    /// If no session exists, returns an AuthFailed error.
    pub async fn get_access_token(&self) -> Result<SecretString, LlmError> {
        if self.needs_refresh().await {
            let has_refresh = self
                .session
                .read()
                .await
                .as_ref()
                .map(|s| !s.refresh_token.is_empty())
                .unwrap_or(false);
            if has_refresh {
                self.refresh_tokens().await?;
            } else {
                let guard = self.session.read().await;
                if let Some(session) = guard
                    .as_ref()
                    .filter(|session| session.expires_at > Utc::now())
                {
                    return Ok(SecretString::from(session.access_token.clone()));
                }
                return Err(LlmError::AuthFailed {
                    provider: "openai_codex".to_string(),
                });
            }
        }

        let guard = self.session.read().await;
        guard
            .as_ref()
            .map(|s| SecretString::from(s.access_token.clone()))
            .ok_or_else(|| LlmError::AuthFailed {
                provider: "openai_codex".to_string(),
            })
    }

    /// Ensure we have a valid session. Loads from disk, refreshes, or prompts login.
    pub async fn ensure_authenticated(&self) -> Result<(), LlmError> {
        // Try loading from disk if we don't have a session
        if !self.has_session().await {
            match self.load_session().await {
                Ok(()) => {}
                Err(LlmError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        if !self.has_session().await {
            // No session at all -- need to authenticate
            return self.device_code_login().await;
        }

        // An access-only token inside the prefetch margin is still valid;
        // get_access_token distinguishes it from a genuinely expired session.
        match self.get_access_token().await {
            Ok(_) => Ok(()),
            Err(error) => {
                tracing::debug!(%error, "Stored authentication unavailable; explicit legacy login required");
                self.device_code_login().await
            }
        }
    }

    /// Step 1 of OpenAI's device code auth flow: request a user code.
    ///
    /// Uses OpenAI's custom `/api/accounts/deviceauth/*` endpoints (not the
    /// standard Auth0 `/oauth/device/code` which is behind Cloudflare managed
    /// challenge). Returns the [`DeviceCodeStart`] the caller displays and then
    /// hands to [`Self::complete_device_code`]. This does NOT block on
    /// authorization, so a web flow can show the code immediately.
    pub async fn initiate_device_code(&self) -> Result<DeviceCodeStart, LlmError> {
        let auth_base = format!("{}/api/accounts", self.config.auth_endpoint);
        let usercode_url = format!("{}/deviceauth/usercode", auth_base);
        let resp = self
            .client
            .post(&usercode_url)
            .json(&UserCodeRequest {
                client_id: self.config.client_id.clone(),
            })
            .send()
            .await
            .map_err(|e| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Device code request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Device code request failed: HTTP {status}"),
            });
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Failed to read device code response: {}", e),
            })?;
        tracing::debug!("Device code response received ({} bytes)", body_text.len());
        parse_codex_device_start(body_text.as_bytes(), Utc::now())
    }

    /// Step 2-3 of the device code flow: poll for authorization, then exchange
    /// the code for tokens and persist them. Blocks until the user authorizes
    /// (or the code expires). Pass the [`DeviceCodeStart`] from
    /// [`Self::initiate_device_code`].
    pub async fn complete_device_code(&self, start: &DeviceCodeStart) -> Result<(), LlmError> {
        let _guard = self.renewal_lock.lock().await;
        let session = self.exchange_device_code(start).await?;
        self.save_session(&session).await?;
        self.set_session(session).await;
        Ok(())
    }

    /// Obtain credentials for an explicit host login without publishing them.
    /// The host must retain the original provider/role request and commit its
    /// credential and selection together. No session file or live token changes.
    pub async fn authorize_device_code(
        &self,
        start: &DeviceCodeStart,
    ) -> Result<OpenAiCodexSession, LlmError> {
        let _guard = self.renewal_lock.lock().await;
        self.exchange_device_code(start).await
    }

    async fn exchange_device_code(
        &self,
        start: &DeviceCodeStart,
    ) -> Result<OpenAiCodexSession, LlmError> {
        let auth_base = format!("{}/api/accounts", self.config.auth_endpoint);

        // Step 3: Poll for authorization code
        let poll_url = format!("{}/deviceauth/token", auth_base);
        let mut interval = std::time::Duration::from_secs(start.interval_secs.max(5));
        let deadline = tokio::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(start.expires_secs))
            .ok_or_else(invalid_oauth_response)?;

        let auth_code = loop {
            let now = tokio::time::Instant::now();
            if now < deadline {
                tokio::time::sleep_until(
                    now.checked_add(interval).unwrap_or(deadline).min(deadline),
                )
                .await;
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(LlmError::SessionRenewalFailed {
                    provider: "openai_codex".to_string(),
                    reason: "Device code authorization timed out".to_string(),
                });
            }

            let resp = self
                .client
                .post(&poll_url)
                .json(&DeviceTokenPollRequest {
                    device_auth_id: start.device_auth_id.clone(),
                    user_code: start.user_code.clone(),
                })
                .send()
                .await
                .map_err(|e| LlmError::SessionRenewalFailed {
                    provider: "openai_codex".to_string(),
                    reason: format!("Token poll request failed: {}", e),
                })?;

            let status = resp.status();
            if status.is_success() {
                let bytes = resp.bytes().await.map_err(|_| invalid_oauth_response())?;
                let parsed = parse_codex_device_authorization(&bytes)?;
                use secrecy::ExposeSecret;
                break DeviceAuthCodeResponse {
                    authorization_code: parsed.authorization_code.expose_secret().to_owned(),
                    code_challenge: parsed.code_challenge,
                    code_verifier: parsed.code_verifier.expose_secret().to_owned(),
                };
            }

            // 403 = authorization_pending, keep polling
            // 404 = device code not found / not enabled
            if status == reqwest::StatusCode::FORBIDDEN {
                continue;
            }

            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(LlmError::SessionRenewalFailed {
                    provider: "openai_codex".to_string(),
                    reason: "Device code login is not enabled. Please check your OpenAI account settings.".to_string(),
                });
            }

            // Slow down on 429, cap at 60s to avoid unbounded growth
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                interval = interval
                    .saturating_add(std::time::Duration::from_secs(5))
                    .min(std::time::Duration::from_secs(60));
                continue;
            }

            return Err(LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Device auth poll failed: HTTP {status}"),
            });
        };

        // Step 4: Exchange authorization code for tokens (form-encoded, per Auth0 spec)
        let token_url = format!("{}/oauth/token", self.config.auth_endpoint);
        let resp = self
            .client
            .post(&token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", &auth_code.authorization_code),
                ("code_verifier", &auth_code.code_verifier),
                ("client_id", &self.config.client_id),
                (
                    "redirect_uri",
                    &format!("{}/deviceauth/callback", self.config.auth_endpoint),
                ),
            ])
            .send()
            .await
            .map_err(|e| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Token exchange failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Token exchange failed: HTTP {status}"),
            });
        }

        let token_resp: TokenResponse =
            resp.json()
                .await
                .map_err(|_| LlmError::SessionRenewalFailed {
                    provider: "openai_codex".to_string(),
                    reason: "Failed to parse token response".into(),
                })?;

        token_resp.into_session(None)
    }

    /// Run OpenAI's device code auth flow interactively (CLI).
    ///
    /// Convenience wrapper over [`Self::initiate_device_code`] +
    /// [`Self::complete_device_code`] that prints the code to stdout and blocks
    /// until the user authorizes. Web flows call the two halves directly so the
    /// code can be rendered in the browser instead of the terminal.
    pub async fn device_code_login(&self) -> Result<(), LlmError> {
        let start = self.initiate_device_code().await?;

        println!();
        println!("===========================================================");
        println!("               OpenAI Codex Authentication                  ");
        println!("===========================================================");
        println!();
        println!("  1. Open this URL in any browser:");
        println!("     {}", start.verification_uri);
        println!();
        println!("  2. Enter this code:");
        println!();
        println!("              [  {}  ]", start.user_code);
        println!();
        println!(
            "  Waiting for authorization... (expires in {} min)",
            start.expires_secs / 60
        );
        println!("===========================================================");
        println!();

        self.complete_device_code(&start).await?;

        println!();
        println!("Authentication successful!");
        println!();
        Ok(())
    }

    /// Refresh the access token using the refresh token.
    pub async fn refresh_tokens(&self) -> Result<(), LlmError> {
        let _guard = self.renewal_lock.lock().await;

        // Double-check: another task may have refreshed while we waited on the lock
        if !self.needs_refresh().await {
            return Ok(());
        }

        let (refresh_token, created_at) = {
            let guard = self.session.read().await;
            guard
                .as_ref()
                .map(|s| (s.refresh_token.clone(), s.created_at))
                .filter(|(token, _)| !token.is_empty())
                .ok_or_else(|| LlmError::AuthFailed {
                    provider: "openai_codex".into(),
                })?
        };

        let token_url = format!("{}/oauth/token", self.config.auth_endpoint);
        let resp = self
            .client
            .post(&token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", self.config.client_id.as_str()),
            ])
            .send()
            .await
            .map_err(|e| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Token refresh request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Token refresh failed: HTTP {status}"),
            });
        }

        let token_resp: TokenResponse =
            resp.json()
                .await
                .map_err(|_| LlmError::SessionRenewalFailed {
                    provider: "openai_codex".to_string(),
                    reason: "Failed to parse refresh response".into(),
                })?;

        let session = token_resp.into_session(Some((&refresh_token, created_at)))?;

        self.save_session(&session).await?;
        self.set_session(session).await;

        tracing::debug!("OpenAI Codex token refreshed successfully");
        Ok(())
    }

    /// Save session data to disk with restrictive permissions.
    pub async fn save_session(&self, session: &OpenAiCodexSession) -> Result<(), LlmError> {
        if let Some(parent) = self.config.session_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                LlmError::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to create session directory: {}", e),
                ))
            })?;
        }

        let json =
            serde_json::to_string_pretty(session).map_err(|e| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: format!("Failed to serialize session: {}", e),
            })?;

        tokio::fs::write(&self.config.session_path, &json)
            .await
            .map_err(|e| {
                LlmError::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to write session file: {}", e),
                ))
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(&self.config.session_path, perms)
                .await
                .map_err(|e| {
                    LlmError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to set permissions: {}", e),
                    ))
                })?;
        }

        Ok(())
    }

    /// Load session from disk.
    pub async fn load_session(&self) -> Result<(), LlmError> {
        let data = tokio::fs::read_to_string(&self.config.session_path)
            .await
            .map_err(|e| {
                LlmError::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to read session file: {}", e),
                ))
            })?;

        let session: OpenAiCodexSession =
            serde_json::from_str(&data).map_err(|_| LlmError::SessionRenewalFailed {
                provider: "openai_codex".to_string(),
                reason: "Failed to parse saved OAuth session".into(),
            })?;

        let mut guard = self.session.write().await;
        *guard = Some(session);
        tracing::debug!(
            "Loaded OpenAI Codex session from {}",
            self.config.session_path.display()
        );
        Ok(())
    }

    /// Set session directly (for testing or after auth).
    pub async fn set_session(&self, session: OpenAiCodexSession) {
        let mut guard = self.session.write().await;
        *guard = Some(session);
    }

    /// Handle a 401 response by refreshing, or re-authenticating.
    pub async fn handle_auth_failure(&self) -> Result<(), LlmError> {
        match self.refresh_tokens().await {
            Ok(()) => Ok(()),
            Err(_) => self.device_code_login().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex_test_helpers::test_codex_config as test_config;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_and_load_session() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let config = test_config(path.clone());

        let mgr = OpenAiCodexSessionManager::new(config).unwrap();

        // No session initially
        assert!(!mgr.has_session().await);

        // Save a session
        let session = OpenAiCodexSession {
            access_token: "access_abc".to_string(),
            refresh_token: "refresh_xyz".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
        };
        mgr.save_session(&session).await.unwrap();
        mgr.set_session(session).await;

        assert!(mgr.has_session().await);

        // Load from disk in a new manager
        let config2 = test_config(path);
        let mgr2 = OpenAiCodexSessionManager::new(config2).unwrap();
        mgr2.load_session().await.unwrap();
        assert!(mgr2.has_session().await);
    }

    #[tokio::test]
    async fn test_needs_refresh_when_near_expiry() {
        let dir = tempdir().unwrap();
        let config = test_config(dir.path().join("session.json"));
        let mgr = OpenAiCodexSessionManager::new(config).unwrap();

        // Token expiring in 2 minutes (margin is 300s = 5 min)
        let session = OpenAiCodexSession {
            access_token: "access_abc".to_string(),
            refresh_token: "refresh_xyz".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(2),
            created_at: chrono::Utc::now(),
        };
        mgr.set_session(session).await;

        assert!(mgr.needs_refresh().await);
    }

    #[tokio::test]
    async fn test_no_refresh_when_fresh() {
        let dir = tempdir().unwrap();
        let config = test_config(dir.path().join("session.json"));
        let mgr = OpenAiCodexSessionManager::new(config).unwrap();

        // Token expiring in 30 minutes (margin is 300s = 5 min)
        let session = OpenAiCodexSession {
            access_token: "access_abc".to_string(),
            refresh_token: "refresh_xyz".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
            created_at: chrono::Utc::now(),
        };
        mgr.set_session(session).await;

        assert!(!mgr.needs_refresh().await);
    }
}

#[cfg(test)]
#[path = "openai_codex_session_native_tests.rs"]
mod native_tests;

#[cfg(test)]
mod shared_native_protocol_tests {
    use super::*;
    #[test]
    fn shared_device_expiry_distinguishes_issuer_origin_and_keeps_legacy_default_only_in_legacy_result()
     {
        let first = Utc::now();
        let absent = br#"{"device_auth_id":"device","user_code":"user"}"#;
        let (legacy, issuer) = parse_codex_device_start_with_issuer_expiry(absent, first).unwrap();
        assert!(issuer.is_none());
        assert_eq!(legacy.expires_secs, SECS_15_MINUTES);
        let body =
            br#"{"device_auth_id":"device","user_code":"user","expires_in":120,"interval":"8"}"#;
        let (parsed, issuer) = parse_codex_device_start_with_issuer_expiry(body, first).unwrap();
        assert_eq!(issuer, Some(first + chrono::Duration::seconds(120)));
        assert_eq!(parsed.interval_secs, 8);
        assert!(!format!("{parsed:?}").contains("user"));
        let tokens=br#"{"access_token":"actual-access","refresh_token":"actual-refresh","expires_in":60,"token_type":"Bearer"}"#;
        let request_started = first - chrono::Duration::hours(2);
        let parsed = parse_codex_device_tokens(tokens, request_started).unwrap();
        assert_eq!(
            parsed.expires_at,
            request_started + chrono::Duration::seconds(60)
        );
        assert!(!format!("{parsed:?}").contains("actual-access"));
        assert!(
            parse_codex_device_tokens(br#"{"access_token":"bad\nheader","expires_in":60}"#, first)
                .is_err()
        );
    }
}
