use brassclaw_host_api::{
    NetworkPolicy, RuntimeHttpEgressError, RuntimeHttpEgressRequest, RuntimeHttpEgressResponse,
    RuntimeHttpSaveTarget,
};
use brassclaw_network::{NetworkHttpEgress, NetworkHttpRequest};
use brassclaw_secrets::SecretStore;

use super::{HostHttpEgressService, PipelineError, runtime_network_error, runtime_response};
use crate::http_body::{
    self, RESPONSE_BODY_STORE_UNAUTHORIZED_REASON, RESPONSE_BODY_STORE_UNAVAILABLE_REASON,
    RuntimeHttpBodyStoreError,
};

pub(super) async fn execute<N, S>(
    service: &HostHttpEgressService<N, S>,
    request: RuntimeHttpEgressRequest,
) -> Result<RuntimeHttpEgressResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
    S: SecretStore + Send + Sync,
{
    execute_inner(service, request, false, None).await
}

pub(super) async fn execute_for_model_visible_output<N, S>(
    service: &HostHttpEgressService<N, S>,
    request: RuntimeHttpEgressRequest,
) -> Result<RuntimeHttpEgressResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
    S: SecretStore + Send + Sync,
{
    execute_inner(service, request, true, None).await
}

pub(super) async fn execute_with_original_custody<'a, N, S>(
    service: &'a HostHttpEgressService<N, S>,
    request: RuntimeHttpEgressRequest,
    custody: Box<dyn super::HostHttpOriginalCustody + 'a>,
) -> Result<RuntimeHttpEgressResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
    S: SecretStore + Send + Sync,
{
    execute_inner(service, request, false, Some(custody)).await
}

async fn execute_inner<'a, N, S>(
    service: &'a HostHttpEgressService<N, S>,
    mut request: RuntimeHttpEgressRequest,
    allow_partial_response_body: bool,
    mut custody: Option<Box<dyn super::HostHttpOriginalCustody + 'a>>,
) -> Result<RuntimeHttpEgressResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
    S: SecretStore + Send + Sync,
{
    let network_policy = service.network_policy_for_request(&mut request)?;
    service.validate_credential_sources_for_request(&request)?;
    let save_body_to = authorize_body_store(service, &mut request)?;
    super::sanitize::validate_runtime_request(&request, service.leak_detector())
        .map_err(PipelineError::pre_transport)?;
    let scope = request.scope.clone();
    let capability_id = request.capability_id.clone();
    let redaction_values = super::credential::apply_credential_injections(
        service.secrets(),
        service.secret_injections(),
        &mut request,
    )
    .map_err(PipelineError::pre_transport_keep_staged_secrets)?;

    if let Some(custodian) = custody.as_mut() {
        request.network_policy = network_policy.clone();
        custodian
            .authenticate_request(&request)
            .await
            .map_err(|_| {
                PipelineError::pre_transport(RuntimeHttpEgressError::Request {
                    reason: "original_custody_preflight_failed".into(),
                    request_bytes: 0,
                    response_bytes: 0,
                })
            })?;
    }
    let original_body_limit = request.response_body_limit;
    let response = if allow_partial_response_body {
        dispatch_network_for_model_visible_output(service, request, network_policy).await?
    } else {
        dispatch_network(service, request, network_policy).await?
    };
    if let Some(custodian) = custody {
        if original_body_limit.is_some_and(|limit| response.body.len() as u64 > limit) {
            return Err(PipelineError::post_transport(
                RuntimeHttpEgressError::Response {
                    reason: brassclaw_host_api::RUNTIME_HTTP_REASON_RESPONSE_BODY_LIMIT_EXCEEDED
                        .into(),
                    request_bytes: response.usage.request_bytes,
                    response_bytes: response.usage.response_bytes,
                },
            ));
        }
        let request_bytes = response.usage.request_bytes;
        let response_bytes = response.usage.response_bytes;
        let material = super::original_custody::original_material(&response).map_err(|_| {
            PipelineError::post_transport(RuntimeHttpEgressError::Response {
                reason: "original_custody_failed".into(),
                request_bytes,
                response_bytes,
            })
        })?;
        custodian.retain(material).await.map_err(|_| {
            PipelineError::post_transport(RuntimeHttpEgressError::Response {
                reason: "original_custody_failed".into(),
                request_bytes,
                response_bytes,
            })
        })?;
    }
    let credentials_injected = !redaction_values.is_empty();
    let (response, response_redacted) = super::sanitize::sanitize_runtime_response(
        response,
        &redaction_values,
        service.leak_detector(),
    )
    .map_err(PipelineError::post_transport)?;
    let (response, saved_body) = http_body::apply_body_disposition(
        response,
        save_body_to,
        service.body_store(),
        &scope,
        &capability_id,
    )
    .map_err(PipelineError::post_transport)?;
    Ok(runtime_response(
        response,
        credentials_injected || response_redacted,
        saved_body,
    ))
}

fn authorize_body_store<N, S>(
    service: &HostHttpEgressService<N, S>,
    request: &mut RuntimeHttpEgressRequest,
) -> Result<Option<RuntimeHttpSaveTarget>, PipelineError> {
    let save_body_to = std::mem::take(&mut request.save_body_to);
    if let Some(target) = &save_body_to
        && let Err(error) =
            service
                .body_store()
                .authorize_write(&request.scope, &request.capability_id, target)
    {
        tracing::debug!(
            error = %error,
            capability_id = %request.capability_id,
            "runtime HTTP response body store authorization failed"
        );
        let reason = match error {
            RuntimeHttpBodyStoreError::Unavailable => {
                RESPONSE_BODY_STORE_UNAVAILABLE_REASON.to_string()
            }
            RuntimeHttpBodyStoreError::Unauthorized { .. }
            | RuntimeHttpBodyStoreError::Failed { .. } => {
                RESPONSE_BODY_STORE_UNAUTHORIZED_REASON.to_string()
            }
        };
        return Err(PipelineError::pre_transport(
            RuntimeHttpEgressError::Request {
                reason,
                request_bytes: 0,
                response_bytes: 0,
            },
        ));
    }
    Ok(save_body_to)
}

async fn dispatch_network<N, S>(
    service: &HostHttpEgressService<N, S>,
    mut request: RuntimeHttpEgressRequest,
    network_policy: NetworkPolicy,
) -> Result<brassclaw_network::NetworkHttpResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
{
    service
        .network()
        .execute(network_request(&mut request, network_policy))
        .await
        .map_err(|error| {
            PipelineError::post_transport(runtime_network_error(
                service.unsafe_raw_diagnostics_allowed(),
                error,
            ))
        })
}

async fn dispatch_network_for_model_visible_output<N, S>(
    service: &HostHttpEgressService<N, S>,
    mut request: RuntimeHttpEgressRequest,
    network_policy: NetworkPolicy,
) -> Result<brassclaw_network::NetworkHttpResponse, PipelineError>
where
    N: NetworkHttpEgress + Send + Sync,
{
    match service
        .network()
        .execute(network_request(&mut request, network_policy))
        .await
    {
        Ok(response) => Ok(response),
        Err(brassclaw_network::NetworkHttpError::ResponseBodyLimit {
            partial_response: Some(partial_response),
            ..
        }) => Ok(partial_response),
        Err(error) => Err(PipelineError::post_transport(runtime_network_error(
            service.unsafe_raw_diagnostics_allowed(),
            error,
        ))),
    }
}

fn network_request(
    request: &mut RuntimeHttpEgressRequest,
    network_policy: NetworkPolicy,
) -> NetworkHttpRequest {
    NetworkHttpRequest {
        scope: request.scope.clone(),
        method: request.method,
        url: std::mem::take(&mut request.url),
        headers: std::mem::take(&mut request.headers),
        body: std::mem::take(&mut request.body),
        policy: network_policy,
        response_body_limit: request.response_body_limit,
        timeout_ms: request.timeout_ms,
    }
}
