use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use brassclaw_safety::LeakDetector;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};

// V1 - deleted: auth module no longer exists
// use crate::auth::resolve_secret_for_runtime;
use crate::db::UserStore;
use crate::secrets::SecretsStore;

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

/// Stub for deleted V1 SharedCredentialRegistry type
pub struct SharedCredentialRegistry {
    // Minimal stub - no fields needed
}

/// Stub for deleted V1 InjectedCredentials type
#[derive(Default)]
pub struct InjectedCredentials {
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
}

impl InjectedCredentials {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Stub for deleted V1 inject_credential function
#[allow(dead_code)]
fn inject_credential(
    _injected: &mut InjectedCredentials,
    _location: &crate::secrets::CredentialLocation,
    _secret: &str,
) {
    // No-op stub - credential injection not supported in V1 stub
}

/// Stub for deleted V1 convert_html_to_markdown function
#[cfg(feature = "html-to-markdown")]
fn convert_html_to_markdown(html: &str, _base_url: &str) -> Result<String, String> {
    // Return HTML as-is - no conversion in V1 stub
    Ok(html.to_string())
}

/// Stub module for deleted V1 path_utils
mod path_utils_stub {
    use std::path::{Path, PathBuf};
    
    pub fn validate_path(raw: &str, base: Option<&Path>) -> Result<PathBuf, String> {
        let path = Path::new(raw);
        
        if raw.is_empty() {
            return Err("empty path".to_string());
        }
        
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(base) = base {
            base.join(path)
        } else {
            path.to_path_buf()
        };
        
        if let Some(base) = base {
            if !resolved.starts_with(base) {
                return Err(format!("path escapes base directory: {}", raw));
            }
        }
        
        Ok(resolved)
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

pub const PROVIDER_ID: &str = "builtin";
pub const HTTP_CAPABILITY_ID: &str = "builtin.http";

const MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const MAX_SAVE_TO_SIZE: usize = 50 * 1024 * 1024;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 300;
const MAX_REDIRECTS: usize = 3;

const DEFAULT_OUTPUT_BYTES: u64 = 16 * 1024;
const MAX_OUTPUT_BYTES: u64 = 5 * 1024 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 5_000;
const MAX_WALL_CLOCK_MS: u64 = 300_000;

const USER_AGENT: &str = concat!(
    "BrassClaw-Agent/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/chtugha/brassclaw)"
);

const REDACTED_RESPONSE_HEADERS: &[&str] = &[
    "authorization",
    "www-authenticate",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "proxy-authenticate",
    "proxy-authorization",
];

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct NetworkCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl NetworkCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }

    fn not_authorized(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

pub struct NetworkContext {
    pub credential_registry: Option<Arc<SharedCredentialRegistry>>,
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    pub role_lookup: Option<Arc<dyn UserStore>>,
    pub user_id: String,
    pub http_interceptor: Option<Arc<dyn brassclaw_llm::recording::HttpInterceptor>>,
}

impl Default for NetworkContext {
    fn default() -> Self {
        Self {
            credential_registry: None,
            secrets_store: None,
            role_lookup: None,
            user_id: String::new(),
            http_interceptor: None,
        }
    }
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn http_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        HTTP_CAPABILITY_ID,
        "Make HTTP requests to external APIs. Supports GET, POST, PUT, DELETE methods. \
         Use save_to to download binary files (images, PDFs, etc.) to a local path, \
         e.g. {\"method\":\"GET\",\"url\":\"https://picsum.photos/800/600\",\"save_to\":\"/tmp/photo.jpg\"}.",
        vec![EffectKind::Network],
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"],
                    "description": "HTTP method (default: GET)"
                },
                "url": {
                    "type": "string",
                    "description": "The URL to request"
                },
                "headers": {
                    "type": "array",
                    "description": "Optional headers as a list of {name, value} objects",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "value": { "type": "string" }
                        },
                        "required": ["name", "value"],
                        "additionalProperties": false
                    }
                },
                "body": {
                    "description": "Request body (for POST/PUT/PATCH). Can be a JSON object, array, string, or other value."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Request timeout in seconds (default: 30)"
                },
                "save_to": {
                    "type": "string",
                    "description": "Save response body as raw bytes to this file path instead of returning it. Use for binary downloads (images, PDFs, etc.). The path must be under /tmp/."
                }
            },
            "required": ["url"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![http_descriptor()]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, NetworkCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| NetworkCapabilityError::input(format!("missing required parameter: {key}")))
}

fn allow_localhost() -> bool {
    static ALLOW: OnceLock<bool> = OnceLock::new();
    *ALLOW.get_or_init(|| {
        std::env::var("HTTP_ALLOW_LOCALHOST")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
    })
}

pub(crate) fn validate_url(url: &str) -> Result<reqwest::Url, NetworkCapabilityError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| NetworkCapabilityError::input(format!("invalid URL: {}", e)))?;

    if allow_localhost() {
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(NetworkCapabilityError::not_authorized(
                "only http(s) URLs are allowed".to_string(),
            ));
        }
        return Ok(parsed);
    }

    if parsed.scheme() != "https" {
        return Err(NetworkCapabilityError::not_authorized(
            "only https URLs are allowed".to_string(),
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| NetworkCapabilityError::input("URL missing host".to_string()))?;

    let host_lower = host.to_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Err(NetworkCapabilityError::not_authorized(
            "localhost is not allowed".to_string(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>()
        && is_disallowed_ip(&ip)
    {
        return Err(NetworkCapabilityError::not_authorized(
            "private or local IPs are not allowed".to_string(),
        ));
    }

    Ok(parsed)
}

pub(crate) async fn validate_and_resolve_url(
    url: &reqwest::Url,
) -> Result<Vec<SocketAddr>, NetworkCapabilityError> {
    let host = url
        .host_str()
        .ok_or_else(|| NetworkCapabilityError::input("URL missing host".to_string()))?;

    let port = url.port_or_known_default().unwrap_or(443);

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:{}", host, port))
        .await
        .map_err(|e| {
            NetworkCapabilityError::operation(format!(
                "DNS resolution failed for '{}': {}",
                host, e
            ))
        })?
        .collect();

    if addrs.is_empty() {
        return Err(NetworkCapabilityError::operation(format!(
            "DNS resolution for '{}' returned no addresses",
            host
        )));
    }

    if !allow_localhost() {
        for addr in &addrs {
            if is_disallowed_ip(&addr.ip()) {
                return Err(NetworkCapabilityError::not_authorized(format!(
                    "hostname '{}' resolves to disallowed IP {}",
                    host,
                    addr.ip()
                )));
            }
        }
    }

    Ok(addrs)
}

pub(crate) fn build_pinned_client(
    host: &str,
    resolved_addrs: &[SocketAddr],
    timeout: Duration,
    redirect_policy: reqwest::redirect::Policy,
) -> Result<Client, NetworkCapabilityError> {
    let builder = Client::builder()
        .timeout(timeout)
        .redirect(redirect_policy)
        .user_agent(USER_AGENT)
        .resolve_to_addrs(host, resolved_addrs);

    builder.build().map_err(|e| {
        NetworkCapabilityError::operation(format!("failed to build HTTP client: {}", e))
    })
}

fn is_disallowed_ipv4(v4: &Ipv4Addr) -> bool {
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_multicast()
        || v4.is_unspecified()
        || *v4 == Ipv4Addr::new(169, 254, 169, 254)
        || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)
}

fn is_disallowed_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_disallowed_ipv4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped()
                && is_disallowed_ipv4(&v4)
            {
                return true;
            }
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6.is_unspecified()
        }
    }
}

#[cfg(feature = "html-to-markdown")]
fn is_html_response(headers: &HashMap<String, String>) -> bool {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_lowercase().contains("text/html"))
        .unwrap_or(false)
}

fn parse_headers_param(
    headers: Option<&Value>,
) -> Result<Vec<(String, String)>, NetworkCapabilityError> {
    fn parse_header_object(
        map: &serde_json::Map<String, Value>,
    ) -> Result<Vec<(String, String)>, NetworkCapabilityError> {
        let mut out = Vec::with_capacity(map.len());
        for (k, v) in map {
            let value = v.as_str().ok_or_else(|| {
                NetworkCapabilityError::input(format!(
                    "header '{}' must have a string value",
                    k
                ))
            })?;
            out.push((k.clone(), value.to_string()));
        }
        Ok(out)
    }

    fn parse_header_array(
        items: &[Value],
    ) -> Result<Vec<(String, String)>, NetworkCapabilityError> {
        let mut out = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                NetworkCapabilityError::input(format!(
                    "headers[{}] must be an object with 'name' and 'value'",
                    idx
                ))
            })?;
            let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                NetworkCapabilityError::input(format!(
                    "headers[{}].name must be a string",
                    idx
                ))
            })?;
            let value = obj.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                NetworkCapabilityError::input(format!(
                    "headers[{}].value must be a string",
                    idx
                ))
            })?;
            out.push((name.to_string(), value.to_string()));
        }
        Ok(out)
    }

    match headers {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(Vec::new());
            }
            let parsed = serde_json::from_str::<Value>(trimmed).map_err(|e| {
                NetworkCapabilityError::input(format!(
                    "headers string must contain valid JSON object/array: {}",
                    e
                ))
            })?;
            match parsed {
                Value::Object(map) => parse_header_object(&map),
                Value::Array(items) => parse_header_array(&items),
                _ => Err(NetworkCapabilityError::input(
                    "headers string must decode to a JSON object or array".to_string(),
                )),
            }
        }
        Some(Value::Object(map)) => parse_header_object(map),
        Some(Value::Array(items)) => parse_header_array(items),
        Some(_) => Err(NetworkCapabilityError::input(
            "'headers' must be an object or an array of {name, value}".to_string(),
        )),
    }
}

fn parse_timeout_secs_param(
    timeout: Option<&Value>,
) -> Result<Option<u64>, NetworkCapabilityError> {
    let parsed = match timeout {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(|| {
            NetworkCapabilityError::input(
                "timeout_secs must be a non-negative integer".to_string(),
            )
        }),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let secs = trimmed.parse::<u64>().map_err(|_| {
                NetworkCapabilityError::input(
                    "timeout_secs string must contain a non-negative integer".to_string(),
                )
            })?;
            Ok(Some(secs))
        }
        Some(_) => Err(NetworkCapabilityError::input(
            "timeout_secs must be an integer".to_string(),
        )),
    }?;

    if let Some(secs) = parsed
        && secs > MAX_TIMEOUT_SECS
    {
        return Err(NetworkCapabilityError::input(format!(
            "timeout_secs must be <= {}",
            MAX_TIMEOUT_SECS
        )));
    }

    Ok(parsed)
}

fn parse_save_to_param(save_to: Option<&Value>) -> Result<Option<String>, NetworkCapabilityError> {
    match save_to {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        Some(_) => Err(NetworkCapabilityError::input(
            "save_to must be a string".to_string(),
        )),
    }
}

fn validate_save_to_path(
    save_to: &str,
) -> Result<std::path::PathBuf, NetworkCapabilityError> {
    if !save_to.starts_with("/tmp/") {
        return Err(NetworkCapabilityError::input(
            "save_to path must be under /tmp/".to_string(),
        ));
    }
    let tmp_base = std::path::Path::new("/tmp");
    let validated =
        path_utils_stub::validate_path(save_to, Some(tmp_base)).map_err(
            |e| NetworkCapabilityError::operation(format!("save_to path validation failed: {}", e)),
        )?;
    if let Some(parent) = validated.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            NetworkCapabilityError::operation(format!("failed to create directory: {}", e))
        })?;
    }
    Ok(validated)
}

pub fn extract_host_from_params(params: &Value) -> Option<String> {
    params
        .get("url")
        .and_then(|u| u.as_str())
        .and_then(|u| reqwest::Url::parse(u).ok())
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

#[allow(dead_code)]
pub(crate) fn dedup_credential_mappings(
    mappings: Vec<crate::secrets::CredentialMapping>,
) -> Vec<crate::secrets::CredentialMapping> {
    let mut seen: std::collections::HashSet<(String, crate::secrets::CredentialLocation)> =
        std::collections::HashSet::new();
    mappings
        .into_iter()
        .filter(|m| seen.insert((m.secret_name.clone(), m.location.clone())))
        .collect()
}

pub async fn execute_http(
    params: &Value,
    ctx: &NetworkContext,
) -> Result<Value, NetworkCapabilityError> {
    let method = params["method"].as_str().unwrap_or("GET");
    let method_upper = method.to_uppercase();

    let url = require_str(params, "url")?;
    let mut parsed_url = validate_url(url)?;

    let resolved_addrs = validate_and_resolve_url(&parsed_url).await?;
    let host = parsed_url
        .host_str()
        .ok_or_else(|| NetworkCapabilityError::input("URL missing host".to_string()))?
        .to_string();
    let client = build_pinned_client(
        &host,
        &resolved_addrs,
        Duration::from_secs(30),
        reqwest::redirect::Policy::none(),
    )?;

    let headers_vec = parse_headers_param(params.get("headers"))?;
    let caller_headers: Vec<(String, String)> = headers_vec.clone();
    let caller_url = parsed_url.clone();

    // V1 - deleted: credential registry validation
    // if let Some(registry) = ctx.credential_registry.as_ref() {
    //     let cred_host = parsed_url.host_str().unwrap_or("");
    //     if registry.has_credentials_for_host(cred_host) {
    //         let forbidden: &[&str] = &["authorization", "x-api-key", "api-key", "x-auth-token"];
    //         for (name, _) in &headers_vec {
    //             if forbidden.iter().any(|f| name.eq_ignore_ascii_case(f)) {
    //                 return Err(NetworkCapabilityError::not_authorized(format!(
    //                     "Manual '{}' header blocked for host '{}': \
    //                      credentials are auto-injected by the credential system",
    //                     name, cred_host
    //                 )));
    //             }
    //         }
    //     }
    // }

    let timeout_secs = parse_timeout_secs_param(params.get("timeout_secs"))?;
    let save_to = parse_save_to_param(params.get("save_to"))?;
    let effective_timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(parsed_url.clone()),
        "POST" => client.post(parsed_url.clone()),
        "PUT" => client.put(parsed_url.clone()),
        "DELETE" => client.delete(parsed_url.clone()),
        "PATCH" => client.patch(parsed_url.clone()),
        _ => {
            return Err(NetworkCapabilityError::input(format!(
                "unsupported method: {}",
                method
            )));
        }
    };

    request = request.timeout(effective_timeout);

    for (key, value) in &headers_vec {
        request = request.header(key.as_str(), value.as_str());
    }

    let body_bytes = if let Some(body) = params.get("body")
        && !body.is_null()
    {
        if let Some(body_str) = body.as_str() {
            if body_str.is_empty() {
                None
            } else if let Ok(json_body) = serde_json::from_str::<Value>(body_str) {
                let bytes = serde_json::to_vec(&json_body).map_err(|e| {
                    NetworkCapabilityError::input(format!("invalid body JSON: {}", e))
                })?;
                request = request.json(&json_body);
                Some(bytes)
            } else {
                let bytes = body_str.as_bytes().to_vec();
                request = request.body(body_str.to_string());
                Some(bytes)
            }
        } else {
            let bytes = serde_json::to_vec(body).map_err(|e| {
                NetworkCapabilityError::input(format!("invalid body JSON: {}", e))
            })?;
            request = request.json(body);
            Some(bytes)
        }
    } else {
        None
    };

    let detector = LeakDetector::new();
    detector
        .scan_http_request(parsed_url.as_str(), &headers_vec, body_bytes.as_deref())
        .map_err(|e| NetworkCapabilityError::not_authorized(format!("{}", e)))?;

    // V1 - deleted: credential injection logic
    // #[derive(Clone, Copy, Debug)]
    // enum MissingReason {
    //     NotConfigured,
    //     RefreshFailed,
    // }
    // let mut missing_credential: Option<(String, MissingReason)> = None;
    // if let (Some(registry), Some(store)) = (
    //     ctx.credential_registry.as_ref(),
    //     ctx.secrets_store.as_ref(),
    // ) {
    //     let cred_host = parsed_url.host_str().unwrap_or("").to_string();
    //     let cred_path = parsed_url.path();
    //     let matched: Vec<crate::secrets::CredentialMapping> =
    //         registry.find_for_url(&cred_host, cred_path);
    //     let dedup_matched = dedup_credential_mappings(matched);
    //     for mapping in &dedup_matched {
    //         let oauth_refresh = registry.oauth_refresh_for_secret(&mapping.secret_name);
    //         match resolve_secret_for_runtime(
    //             store.as_ref(),
    //             &ctx.user_id,
    //             &mapping.secret_name,
    //             ctx.role_lookup.as_deref(),
    //             oauth_refresh.as_ref(),
    //             crate::auth::DefaultFallback::AdminOnly,
    //         )
    //         .await
    //         {
    //             Ok(secret) => {
    //                 let mut injected = InjectedCredentials::empty();
    //                 inject_credential(&mut injected, &mapping.location, &secret);
    //                 for (name, value) in &injected.headers {
    //                     request = request.header(name.as_str(), value.as_str());
    //                     headers_vec.push((name.clone(), value.clone()));
    //                 }
    //                 for (name, value) in &injected.query_params {
    //                     parsed_url.query_pairs_mut().append_pair(name, value);
    //                     request = request.query(&[(name.as_str(), value.as_str())]);
    //                 }
    //             }
    //             Err(error) if error.requires_authentication() => {
    //                 if mapping.optional {
    //                     continue;
    //                 }
    //                 let reason = match error {
    //                     crate::auth::CredentialResolutionError::RefreshFailed => {
    //                         MissingReason::RefreshFailed
    //                     }
    //                     _ => MissingReason::NotConfigured,
    //                 };
    //                 if missing_credential.is_none() {
    //                     missing_credential = Some((mapping.secret_name.clone(), reason));
    //                 }
    //             }
    //             Err(_) => {}
    //         }
    //     }
    // }

    let intercept_req = brassclaw_llm::recording::HttpExchangeRequest {
        method: method_upper,
        url: caller_url.to_string(),
        headers: caller_headers,
        body: body_bytes
            .as_ref()
            .map(|b| brassclaw_llm::recording::redact_body(&String::from_utf8_lossy(b))),
    };

    if let Some(ref interceptor) = ctx.http_interceptor
        && let Some(recorded) = interceptor.before_request(&intercept_req).await
    {
        let headers: HashMap<String, String> = recorded.headers.iter().cloned().collect();
        let body: Value = serde_json::from_str(&recorded.body)
            .unwrap_or_else(|_| Value::String(recorded.body.clone()));
        return Ok(json!({
            "status": recorded.status,
            "headers": headers,
            "body": body
        }));
    }

    let is_simple_get =
        method.eq_ignore_ascii_case("GET") && headers_vec.is_empty() && body_bytes.is_none();

    let response = if is_simple_get {
        let mut redirects_remaining = MAX_REDIRECTS;
        loop {
            let hop_addrs = validate_and_resolve_url(&parsed_url).await?;
            let hop_host = parsed_url
                .host_str()
                .ok_or_else(|| NetworkCapabilityError::input("URL missing host".to_string()))?
                .to_string();
            let hop_client = build_pinned_client(
                &hop_host,
                &hop_addrs,
                effective_timeout,
                reqwest::redirect::Policy::none(),
            )?;

            let resp = hop_client
                .get(parsed_url.clone())
                .header(
                    reqwest::header::ACCEPT,
                    "text/markdown, text/html;q=0.9, application/json;q=0.9, */*;q=0.8",
                )
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        NetworkCapabilityError::operation(format!(
                            "Request timed out after {} seconds",
                            effective_timeout.as_secs()
                        ))
                    } else {
                        NetworkCapabilityError::operation(e.to_string())
                    }
                })?;

            let status = resp.status().as_u16();
            if (300..400).contains(&status) {
                if redirects_remaining == 0 {
                    return Err(NetworkCapabilityError::operation(format!(
                        "too many redirects (max {})",
                        MAX_REDIRECTS
                    )));
                }

                let location = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| {
                        NetworkCapabilityError::operation(format!(
                            "redirect (HTTP {}) has no Location header",
                            status
                        ))
                    })?;

                let next_url_str =
                    if location.starts_with("http://") || location.starts_with("https://") {
                        location.to_string()
                    } else {
                        parsed_url
                            .join(location)
                            .map(|u| u.to_string())
                            .map_err(|e| {
                                NetworkCapabilityError::operation(format!(
                                    "could not resolve relative redirect '{}': {}",
                                    location, e
                                ))
                            })?
                    };

                parsed_url = validate_url(&next_url_str)?;
                let hop_detector = LeakDetector::new();
                hop_detector
                    .scan_http_request(parsed_url.as_str(), &[], None)
                    .map_err(|e| NetworkCapabilityError::not_authorized(e.to_string()))?;

                redirects_remaining -= 1;
                continue;
            }

            break resp;
        }
    } else {
        let resp = request.send().await.map_err(|e| {
            if e.is_timeout() {
                NetworkCapabilityError::operation(format!(
                    "Request timed out after {} seconds",
                    effective_timeout.as_secs()
                ))
            } else {
                NetworkCapabilityError::operation(e.to_string())
            }
        })?;

        let status = resp.status().as_u16();
        if (300..400).contains(&status) {
            return Err(NetworkCapabilityError::not_authorized(format!(
                "request returned redirect (HTTP {}), which is blocked to prevent SSRF",
                status
            )));
        }

        resp
    };

    let status = response.status().as_u16();

    // V1 - deleted: credential error handling
    // if matches!(status, 401 | 403)
    //     && let Some((cred_name, reason)) = missing_credential.as_ref()
    // {
    //     let (error_kind, message) = match reason {
    //         MissingReason::NotConfigured => (
    //             "authentication_required",
    //             format!(
    //                 "Credential '{}' is not configured. \
    //                  The server returned HTTP {}. Set up credentials to access this endpoint.",
    //                 cred_name, status
    //             ),
    //         ),
    //         MissingReason::RefreshFailed => (
    //             "authentication_refresh_failed",
    //             format!(
    //                 "Credential '{}' exists but its OAuth refresh failed. \
    //                  The server returned HTTP {}. Re-authenticate this credential to repair the stored tokens.",
    //                 cred_name, status
    //             ),
    //         ),
    //     };
    //     return Err(NetworkCapabilityError::operation(
    //         json!({
    //             "error": error_kind,
    //             "credential_name": cred_name,
    //             "message": message,
    //         })
    //         .to_string(),
    //     ));
    // }

    let headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            let key = k.to_string();
            if REDACTED_RESPONSE_HEADERS
                .iter()
                .any(|r| key.eq_ignore_ascii_case(r))
            {
                None
            } else {
                v.to_str().ok().map(|v| (key, v.to_string()))
            }
        })
        .collect();

    let saving_to_disk = save_to.is_some();
    let max_size = if saving_to_disk {
        MAX_SAVE_TO_SIZE
    } else {
        MAX_RESPONSE_SIZE
    };

    if let Some(content_length) = response.headers().get(reqwest::header::CONTENT_LENGTH)
        && let Ok(s) = content_length.to_str()
        && let Ok(len) = s.parse::<usize>()
        && len > max_size
    {
        return Err(NetworkCapabilityError::operation(format!(
            "Response Content-Length ({} bytes) exceeds maximum allowed size ({} bytes)",
            len, max_size
        )));
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = StreamExt::next(&mut stream).await {
        let chunk = chunk.map_err(|e| {
            NetworkCapabilityError::operation(format!("failed to read response body: {}", e))
        })?;
        if body.len() + chunk.len() > max_size {
            return Err(NetworkCapabilityError::operation(format!(
                "Response body exceeds maximum allowed size ({} bytes)",
                max_size
            )));
        }
        body.extend_from_slice(&chunk);
    }
    let body_bytes_resp = bytes::Bytes::from(body);

    if let Some(save_to) = save_to {
        let saved_to = save_to.clone();
        let bytes_clone = body_bytes_resp.clone();
        tokio::task::spawn_blocking(move || {
            let canonical = validate_save_to_path(&save_to)?;
            std::fs::write(&canonical, &bytes_clone).map_err(|e| {
                NetworkCapabilityError::operation(format!("failed to write file: {}", e))
            })?;
            Ok::<_, NetworkCapabilityError>(canonical)
        })
        .await
        .map_err(|e| {
            NetworkCapabilityError::operation(format!("spawn_blocking failed: {}", e))
        })?
        .map_err(|e: NetworkCapabilityError| e)?;
        return Ok(json!({
            "status": status,
            "saved_to": saved_to,
            "size_bytes": body_bytes_resp.len(),
            "headers": headers,
        }));
    }

    let body_text = String::from_utf8_lossy(&body_bytes_resp).into_owned();

    let response_detector = LeakDetector::new();
    let scan_result = response_detector.scan(&body_text);
    if scan_result.should_block {
        return Err(NetworkCapabilityError::not_authorized(
            "Response blocked: contains credential patterns that must not reach the LLM"
                .to_string(),
        ));
    }

    if let Some(ref interceptor) = ctx.http_interceptor {
        let resp_headers: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        interceptor
            .after_response(
                &intercept_req,
                &brassclaw_llm::recording::HttpExchangeResponse {
                    status,
                    headers: resp_headers,
                    body: body_text.clone(),
                },
            )
            .await;
    }

    #[cfg(feature = "html-to-markdown")]
    let body_text = if is_html_response(&headers) {
        match convert_html_to_markdown(&body_text, parsed_url.as_str()) {
            Ok(md) => md,
            Err(_) => body_text,
        }
    } else {
        body_text
    };

    let body: Value = serde_json::from_str(&body_text)
        .unwrap_or_else(|_| Value::String(body_text.clone()));

    Ok(json!({
        "status": status,
        "headers": headers,
        "body": body
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_descriptor_is_valid() {
        let desc = http_descriptor();
        assert_eq!(desc.id.as_str(), HTTP_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::Network));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_http() {
        let descs = descriptors();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].id.as_str(), HTTP_CAPABILITY_ID);
    }

    #[test]
    fn validate_url_rejects_http() {
        let result = validate_url("http://example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("https"));
    }

    #[test]
    fn validate_url_accepts_https() {
        let result = validate_url("https://example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_url_rejects_localhost() {
        let result = validate_url("https://localhost/api");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("localhost"));
    }

    #[test]
    fn validate_url_rejects_private_ip() {
        let result = validate_url("https://192.168.1.1/api");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_rejects_invalid() {
        let result = validate_url("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn parse_headers_none_returns_empty() {
        let result = parse_headers_param(None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_headers_object() {
        let headers = json!({"Authorization": "Bearer token"});
        let result = parse_headers_param(Some(&headers)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Authorization");
        assert_eq!(result[0].1, "Bearer token");
    }

    #[test]
    fn parse_headers_array() {
        let headers = json!([{"name": "X-Custom", "value": "foo"}]);
        let result = parse_headers_param(Some(&headers)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "X-Custom");
        assert_eq!(result[0].1, "foo");
    }

    #[test]
    fn parse_timeout_valid() {
        let result = parse_timeout_secs_param(Some(&json!(30))).unwrap();
        assert_eq!(result, Some(30));
    }

    #[test]
    fn parse_timeout_exceeds_max() {
        let result = parse_timeout_secs_param(Some(&json!(999)));
        assert!(result.is_err());
    }

    #[test]
    fn parse_timeout_none() {
        let result = parse_timeout_secs_param(None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn save_to_rejects_non_tmp() {
        let result = validate_save_to_path("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn extract_host_works() {
        let host = extract_host_from_params(&json!({"url": "https://api.example.com/v1"}));
        assert_eq!(host, Some("api.example.com".to_string()));
    }

    #[test]
    fn extract_host_invalid_url() {
        let host = extract_host_from_params(&json!({"url": "not-a-url"}));
        assert_eq!(host, None);
    }
}
