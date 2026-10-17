use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{
    EffectKind, MountAlias, MountGrant, MountPermissions, MountView, NetworkMethod, NetworkPolicy,
    PermissionMode, ResourceEstimate, ResourceProfile, ResourceUsage, RuntimeDispatchErrorKind,
    RuntimeHttpEgressError, RuntimeHttpEgressReasonCode, RuntimeHttpEgressRequest,
    RuntimeHttpSaveTarget, RuntimeKind, ScopedPath, VirtualPath, valid_http_field_name,
};
use serde_json::Value;

use crate::{FirstPartyCapabilityError, FirstPartyCapabilityRequest};

use super::{
    first_party_capability_manifest,
    http_output::{HttpDispatchOutput, shape_response},
    input_error,
};

pub const HTTP_CAPABILITY_ID: &str = "builtin.http";
pub const HTTP_SAVE_CAPABILITY_ID: &str = "builtin.http.save";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpSaveMode {
    Disabled,
    Required,
}

impl HttpSaveMode {
    fn for_capability(capability_id: &str) -> Self {
        if capability_id == HTTP_SAVE_CAPABILITY_ID {
            Self::Required
        } else {
            Self::Disabled
        }
    }
}

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    http_manifest(
        HTTP_CAPABILITY_ID,
        "Perform an outbound HTTP request through host egress. Redirect responses are returned; the host transport does not follow them.",
        vec![EffectKind::DispatchCapability, EffectKind::Network],
    )
}

pub(super) fn save_manifest() -> Result<CapabilityManifest, ExtensionError> {
    http_manifest(
        HTTP_SAVE_CAPABILITY_ID,
        "Perform an outbound HTTP request through host egress and save the sanitized response body through scoped filesystem authority.",
        vec![
            EffectKind::DispatchCapability,
            EffectKind::Network,
            EffectKind::WriteFilesystem,
        ],
    )
}

fn http_manifest(
    capability_id: &str,
    description: &str,
    effects: Vec<EffectKind>,
) -> Result<CapabilityManifest, ExtensionError> {
    first_party_capability_manifest(
        capability_id,
        description,
        effects,
        PermissionMode::Ask,
        Some(http_resource_profile()),
    )
}

fn http_resource_profile() -> ResourceProfile {
    // Actual transport bounds and live kernel policy apply to every dispatch.
    // Static estimates/ceilings must not silently override operator settings.
    ResourceProfile {
        default_estimate: ResourceEstimate::default(),
        hard_ceiling: None,
    }
}

pub(super) async fn dispatch(
    request: &FirstPartyCapabilityRequest,
) -> Result<HttpDispatchOutput, FirstPartyCapabilityError> {
    let egress = request
        .services
        .runtime_http_egress
        .as_ref()
        .ok_or_else(|| FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::NetworkDenied))?
        .clone();
    // Keep this handler as a translator only: URL parsing, DNS/private-IP
    // enforcement, allowlists, transport, and credential injection remain in
    // HostHttpEgressService / brassclaw_network.
    let unsafe_raw_diagnostics_allowed = request.services.unsafe_raw_diagnostics_allowed;
    let mut headers = headers(&request.input).map_err(|error| {
        log_raw_http_input_error_for_local_diagnostics(
            unsafe_raw_diagnostics_allowed,
            &request.input,
            "headers",
            error,
        )
    })?;
    if json_body_needs_default_content_type(&request.input) && !has_header(&headers, "content-type")
    {
        headers.push(("content-type".to_string(), "application/json".to_string()));
    }
    let method = method(&request.input).map_err(|error| {
        log_raw_http_input_error_for_local_diagnostics(
            unsafe_raw_diagnostics_allowed,
            &request.input,
            "method",
            error,
        )
    })?;
    let url = required_string(&request.input, "url")
        .map_err(|error| {
            log_raw_http_input_error_for_local_diagnostics(
                unsafe_raw_diagnostics_allowed,
                &request.input,
                "url",
                error,
            )
        })?
        .to_string();
    let body = body(&request.input).map_err(|error| {
        log_raw_http_input_error_for_local_diagnostics(
            unsafe_raw_diagnostics_allowed,
            &request.input,
            "body",
            error,
        )
    })?;
    let save_mode = HttpSaveMode::for_capability(request.capability_id.as_str());
    let response_body_limit = response_body_limit(&request.input).map_err(|error| {
        log_raw_http_input_error_for_local_diagnostics(
            unsafe_raw_diagnostics_allowed,
            &request.input,
            "response_body_limit",
            error,
        )
    })?;
    let timeout_ms = timeout_ms(&request.input).map_err(|error| {
        log_raw_http_input_error_for_local_diagnostics(
            unsafe_raw_diagnostics_allowed,
            &request.input,
            "timeout_ms",
            error,
        )
    })?;
    let save_body_to =
        save_body_to(&request.input, request.mounts.as_ref(), save_mode).map_err(|error| {
            log_raw_http_input_error_for_local_diagnostics(
                unsafe_raw_diagnostics_allowed,
                &request.input,
                "save_to",
                error,
            )
        })?;
    let expected_saved_path = save_body_to
        .as_ref()
        .map(|target| target.path.as_str().to_owned());
    let http_request = RuntimeHttpEgressRequest {
        runtime: RuntimeKind::FirstParty,
        scope: request.scope.clone(),
        capability_id: request.capability_id.clone(),
        method,
        url,
        headers,
        body,
        network_policy: staged_policy_placeholder(),
        credential_injections: Vec::new(),
        // Omission inherits the finite acknowledged transport default. Explicit
        // bounds are exact, and overflow is a classified strict-port failure.
        response_body_limit,
        save_body_to,
        // Omission reaches the acknowledged transport default; explicit values
        // retain their exact typed value rather than being silently clamped.
        timeout_ms,
    };
    // Use the existing strict, sanitized host boundary. Do not convert a
    // response-limit failure into a successful partial model-visible answer.
    let egress_future = egress.execute(http_request);
    let response = super::run_egress_catching_panic(
        egress_future,
        "first-party HTTP egress future panicked",
        || FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::Backend),
    )
    .await?
    .map_err(http_error)?;
    if response_body_limit.is_some_and(|limit| response.response_bytes > limit) {
        return Err(
            FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::OutputTooLarge).with_usage(
                ResourceUsage {
                    network_egress_bytes: response.request_bytes,
                    ..ResourceUsage::default()
                },
            ),
        );
    }
    if expected_saved_path.as_deref()
        != response
            .saved_body
            .as_ref()
            .map(|saved| saved.path.as_str())
    {
        return Err(
            FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::OutputDecode).with_usage(
                ResourceUsage {
                    network_egress_bytes: response.request_bytes,
                    ..ResourceUsage::default()
                },
            ),
        );
    }
    Ok(shape_response(response))
}

fn method(input: &Value) -> Result<NetworkMethod, FirstPartyCapabilityError> {
    let method = match input.get("method") {
        Some(value) => value.as_str().ok_or_else(input_error)?,
        None => "get",
    };
    match method.to_ascii_lowercase().as_str() {
        "get" => Ok(NetworkMethod::Get),
        "post" => Ok(NetworkMethod::Post),
        "put" => Ok(NetworkMethod::Put),
        "patch" => Ok(NetworkMethod::Patch),
        "delete" => Ok(NetworkMethod::Delete),
        "head" => Ok(NetworkMethod::Head),
        _ => Err(input_error()),
    }
}

fn required_string<'a>(
    input: &'a Value,
    field: &str,
) -> Result<&'a str, FirstPartyCapabilityError> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(input_error)
}

fn headers(input: &Value) -> Result<Vec<(String, String)>, FirstPartyCapabilityError> {
    let Some(headers) = input.get("headers") else {
        return Ok(Vec::new());
    };
    let parsed: Vec<(String, String)> = match headers {
        Value::Object(object) => object
            .iter()
            .map(|(name, value)| {
                let value = value.as_str().ok_or_else(input_error)?;
                header_pair(name, value)
            })
            .collect::<Result<_, _>>()?,
        Value::Array(items) => items
            .iter()
            .map(|item| {
                let name = required_string(item, "name")?;
                let value = required_string(item, "value")?;
                header_pair(name, value)
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(input_error()),
    };
    Ok(parsed)
}

fn header_pair(name: &str, value: &str) -> Result<(String, String), FirstPartyCapabilityError> {
    if !valid_http_field_name(name)
        || value
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(input_error());
    }
    Ok((name.to_string(), value.to_string()))
}

fn body(input: &Value) -> Result<Vec<u8>, FirstPartyCapabilityError> {
    if input.get("body").is_some() && input.get("body_base64").is_some() {
        return Err(input_error());
    }
    let body = if let Some(encoded) = input.get("body_base64") {
        let encoded = encoded.as_str().ok_or_else(input_error)?;
        BASE64_STANDARD.decode(encoded).map_err(|_| input_error())?
    } else {
        match input.get("body") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::String(value)) => value.as_bytes().to_vec(),
            Some(value) => serde_json::to_vec(value).map_err(|_| input_error())?,
        }
    };
    Ok(body)
}

fn json_body_needs_default_content_type(input: &Value) -> bool {
    matches!(
        input.get("body"),
        Some(Value::Array(_))
            | Some(Value::Bool(_))
            | Some(Value::Number(_))
            | Some(Value::Object(_))
    )
}

fn has_header(headers: &[(String, String)], expected: &str) -> bool {
    headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(expected))
}

fn staged_policy_placeholder() -> NetworkPolicy {
    // First-party HTTP policy is staged in HostHttpEgressService from the grant
    // obligation for this scope/capability. This fallback request field is
    // ignored on that path and only exists for request-policy test services.
    NetworkPolicy::default()
}

fn response_body_limit(input: &Value) -> Result<Option<u64>, FirstPartyCapabilityError> {
    let Some(value) = input.get("response_body_limit") else {
        return Ok(None);
    };
    let limit = value
        .as_u64()
        .filter(|value| *value > 0 && *value <= isize::MAX as u64)
        .ok_or_else(input_error)?;
    Ok(Some(limit))
}

fn timeout_ms(input: &Value) -> Result<Option<u32>, FirstPartyCapabilityError> {
    let Some(value) = input.get("timeout_ms") else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(input_error)?;
    Ok(Some(value))
}

fn save_body_to(
    input: &Value,
    mounts: Option<&MountView>,
    mode: HttpSaveMode,
) -> Result<Option<RuntimeHttpSaveTarget>, FirstPartyCapabilityError> {
    let Some(value) = input.get("save_to") else {
        if mode == HttpSaveMode::Required {
            return Err(input_error());
        }
        return Ok(None);
    };
    if mode == HttpSaveMode::Disabled {
        return Err(input_error());
    }
    let path = value.as_str().ok_or_else(input_error)?;
    if path.trim().is_empty() {
        return Err(input_error());
    }
    let mounts = mounts.ok_or_else(input_error)?;
    let path = mounts
        .scoped_path(path.to_string())
        .map_err(|_| input_error())?;
    let (virtual_path, grant) = mounts
        .resolve_with_grant(&path)
        .map_err(|_| input_error())?;
    if !grant.permissions.write {
        return Err(input_error());
    }
    Ok(Some(RuntimeHttpSaveTarget {
        mount_grant: Some(write_only_save_grant(&path, virtual_path)?),
        path,
    }))
}

fn write_only_save_grant(
    path: &ScopedPath,
    virtual_path: VirtualPath,
) -> Result<MountGrant, FirstPartyCapabilityError> {
    Ok(MountGrant::new(
        MountAlias::new(path.as_str()).map_err(|_| input_error())?,
        virtual_path,
        MountPermissions {
            read: false,
            write: true,
            delete: false,
            list: false,
            execute: false,
        },
    ))
}

fn http_error(error: RuntimeHttpEgressError) -> FirstPartyCapabilityError {
    let kind = match error.reason_code() {
        // Host credential injection failures are backend/client integration faults;
        // production maps RuntimeDispatchErrorKind::Client to RuntimeFailureKind::Backend.
        RuntimeHttpEgressReasonCode::CredentialUnavailable => RuntimeDispatchErrorKind::Client,
        RuntimeHttpEgressReasonCode::RequestDenied => RuntimeDispatchErrorKind::InputEncode,
        RuntimeHttpEgressReasonCode::PolicyDenied => RuntimeDispatchErrorKind::PolicyDenied,
        RuntimeHttpEgressReasonCode::NetworkError => RuntimeDispatchErrorKind::NetworkDenied,
        RuntimeHttpEgressReasonCode::ResponseError => RuntimeDispatchErrorKind::OperationFailed,
        RuntimeHttpEgressReasonCode::ResponseBodyLimitExceeded => {
            RuntimeDispatchErrorKind::OutputTooLarge
        }
    };
    tracing::debug!(
        runtime_http_reason = error.stable_runtime_reason(),
        dispatch_error_kind = kind.as_str(),
        "first-party HTTP egress failed"
    );
    let mut usage = ResourceUsage::default();
    if !matches!(error, RuntimeHttpEgressError::Credential { .. }) {
        usage.network_egress_bytes = error.request_bytes();
    }
    let cause = match &error {
        RuntimeHttpEgressError::Request { reason, .. } => match reason.as_str() {
            "sensitive_header_denied" => "sensitive_header_denied",
            "manual_credentials_denied" => "manual_credentials_denied",
            "credential_leak_blocked" => "credential_leak_blocked",
            "response_body_store_unavailable" => "response_body_store_unavailable",
            "response_body_store_unauthorized" => "response_body_store_unauthorized",
            _ => error.stable_runtime_reason(),
        },
        _ => error.stable_runtime_reason(),
    };
    FirstPartyCapabilityError::with_safe_summary(kind, format!("HTTP egress failed: {cause}"))
        .with_usage(usage)
}

fn log_raw_http_input_error_for_local_diagnostics(
    unsafe_raw_diagnostics_allowed: bool,
    input: &Value,
    validation_stage: &'static str,
    mut error: FirstPartyCapabilityError,
) -> FirstPartyCapabilityError {
    if let FirstPartyCapabilityError::Dispatch { safe_summary, .. } = &mut error {
        safe_summary.get_or_insert_with(|| format!("HTTP {validation_stage} input invalid"));
    }
    tracing::debug!(
        validation_stage,
        "first-party HTTP tool input validation failed"
    );
    if crate::unsafe_raw_http_diagnostics_enabled(unsafe_raw_diagnostics_allowed) {
        tracing::warn!(
            validation_stage,
            raw_http_tool_input = %input,
            unsafe_raw_diagnostics = true,
            "unsafe raw HTTP tool input diagnostic enabled"
        );
    }
    error
}
