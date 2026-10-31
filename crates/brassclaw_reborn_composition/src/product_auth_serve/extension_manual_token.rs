//! Secret ingress delegates through the existing product facade. Host scope,
//! canonical invocation, requirement and retained target are resolved there.
use super::*;
use axum::extract::rejection::JsonRejection;
use brassclaw_product_workflow::{
    ExtensionManualTokenCompletion, ExtensionManualTokenSetupReceipt, LifecyclePackageKind,
    LifecyclePackageRef, RebornServicesError, RebornServicesErrorCode,
    WebUiExtensionManualTokenReferenceRequest, WebUiExtensionManualTokenSetupRequest,
    WebUiExtensionManualTokenSubmitRequest,
};
use brassclaw_webui_v2::WebUiV2HttpError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSubmit {
    requirement: String,
    identity: ManualTokenIdentityInput,
    invocation_id: String,
    token: UnvalidatedRawSecretValue,
}

fn invalid() -> WebUiV2HttpError {
    RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 400, false).into()
}
fn package(id: String) -> Result<LifecyclePackageRef, WebUiV2HttpError> {
    LifecyclePackageRef::new(LifecyclePackageKind::Extension, id).map_err(|_| invalid())
}
async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, RebornServicesError>>,
) -> Result<T, WebUiV2HttpError> {
    match tokio::time::timeout(PRODUCT_AUTH_BACKEND_TIMEOUT, future).await {
        Ok(result) => result.map_err(Into::into),
        Err(_) => {
            Err(
                RebornServicesError::from_status(RebornServicesErrorCode::Unavailable, 504, true)
                    .into(),
            )
        }
    }
}

pub(super) async fn setup(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(id): Path<String>,
    payload: Result<Json<WebUiExtensionManualTokenSetupRequest>, JsonRejection>,
) -> Result<Json<ExtensionManualTokenSetupReceipt>, WebUiV2HttpError> {
    let Json(request) = payload?;
    let services = state.services()?;
    bounded(services.prepare_extension_manual_token(caller, package(id)?, request))
        .await
        .map(Json)
}

pub(super) async fn submit(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(id): Path<String>,
    payload: Result<Json<RawSubmit>, JsonRejection>,
) -> Result<Json<ExtensionManualTokenCompletion>, WebUiV2HttpError> {
    let Json(request) = payload?;
    let identity = request.identity.parse().map_err(|_| invalid())?;
    let invocation_id = InvocationId::parse(&request.invocation_id).map_err(|_| invalid())?;
    let secret = request
        .token
        .into_validated()
        .map_err(|_| invalid())?
        .into_secret();
    let services = state.services()?;
    bounded(services.submit_extension_manual_token(
        caller,
        package(id)?,
        WebUiExtensionManualTokenSubmitRequest {
            requirement: request.requirement,
            identity,
            invocation_id,
            secret,
        },
    ))
    .await
    .map(Json)
}

pub(super) async fn continue_saved(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(id): Path<String>,
    payload: Result<Json<WebUiExtensionManualTokenReferenceRequest>, JsonRejection>,
) -> Result<Json<ExtensionManualTokenCompletion>, WebUiV2HttpError> {
    let Json(request) = payload?;
    let services = state.services()?;
    bounded(services.continue_extension_manual_token(caller, package(id)?, request))
        .await
        .map(Json)
}

pub(super) async fn cancel(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(id): Path<String>,
    payload: Result<Json<WebUiExtensionManualTokenReferenceRequest>, JsonRejection>,
) -> Result<Json<ExtensionManualTokenSetupReceipt>, WebUiV2HttpError> {
    let Json(request) = payload?;
    let services = state.services()?;
    bounded(services.cancel_extension_manual_token(caller, package(id)?, request))
        .await
        .map(Json)
}
