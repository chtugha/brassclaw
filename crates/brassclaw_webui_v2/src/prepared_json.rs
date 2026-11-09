//! Finite extraction even when embedding the router without the host's body
//! middleware. The host supplies a typed capture; caller JSON cannot select it.
use crate::error::WebUiV2HttpError;
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{FromRequest, Request},
};
use brassclaw_host_api::task_preparation::RetainedPreparation;
use brassclaw_product_workflow::{RebornServicesError, RebornServicesErrorCode};
use serde::de::DeserializeOwned;

pub(crate) struct PreparedJson<T>(pub(crate) T);
impl<S, T> FromRequest<S> for PreparedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = WebUiV2HttpError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        // No capture means the explicitly supported historical/non-global
        // transport profile. Global mounted ingress resolves its real owner
        // before this point and errors instead of falling back on ACK failure.
        let limit = request
            .extensions()
            .get::<RetainedPreparation>()
            .map_or(14 * 1024 * 1024, |p| p.policy().max_value_bytes);
        let overflow = || {
            WebUiV2HttpError::from(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                413,
                false,
            ))
        };
        let limit = usize::try_from(limit).map_err(|_| overflow())?;
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, limit).await.map_err(|_| overflow())?;
        let Json(value) =
            Json::<T>::from_request(Request::from_parts(parts, Body::from(bytes)), state)
                .await
                .map_err(|rejection| {
                    let status = rejection.status().as_u16();
                    let code = if status >= 500 {
                        RebornServicesErrorCode::Internal
                    } else {
                        RebornServicesErrorCode::InvalidRequest
                    };
                    WebUiV2HttpError::from(RebornServicesError::from_status(code, status, false))
                })?;
        Ok(Self(value))
    }
}
