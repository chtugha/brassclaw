//! Descriptor-driven per-route body limit for the WebChat v2 native
//! surface.
//!
//! `brassclaw_webui_v2::webui_v2_routes()` carries a [`BodyLimitPolicy`]
//! per route (16 KiB for `create_thread`, 1 MiB for `send_message`, 4
//! KiB for `cancel_run`/`resolve_gate`, `NoBody` for the read /
//! streaming routes). The v2 crate's CLAUDE.md designates enforcement
//! as host-composition responsibility; this module is that enforcement.
//!
//! Small/static bodies retain early finite enforcement. Named preparation
//! bodies are collected and their acknowledged policy is captured only AFTER
//! the existing protected operator-auth and rate owners. The outer layer adds
//! cheap, lazy framing protection; it never buffers or captures a policy for an
//! unauthenticated preparation request. Explicit host bounds still narrow it.
//! Public routes keep their own existing static/NoBody checks.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use brassclaw_host_api::ingress::{BodyLimitPolicy, IngressRouteDescriptor};

use crate::webui_route_match::{network_method_to_axum, parse_pattern, segments_match};

#[derive(Debug, Clone)]
struct RouteBodyLimit {
    route_id: String,
    method: Method,
    segments: Vec<Option<String>>,
    policy: ResolvedBodyPolicy,
}

#[derive(Debug, Clone, Copy)]
enum ResolvedBodyPolicy {
    NoBody,
    Limited { max_bytes: u64 },
    ComponentPreparation,
}

/// Shared state for [`enforce_body_limit`]. Cheap to clone — the inner
/// route table is `Arc`-shared across every per-request invocation.
#[derive(Clone)]
pub(crate) struct BodyLimitState {
    routes: Arc<Vec<RouteBodyLimit>>,
    services: Option<Arc<dyn brassclaw_product_workflow::RebornServicesApi>>,
    outer_default: usize,
    outer_explicit: Option<usize>,
}

impl std::fmt::Debug for BodyLimitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BodyLimitState")
            .field("routes", &self.routes.len())
            .finish_non_exhaustive()
    }
}

/// Resolve the v2 descriptor set into a fixed lookup table consumed by
/// [`enforce_body_limit`]. Composition is infallible today because all three
/// [`BodyLimitPolicy`] variants are supported; if upstream adds a new
/// variant this `match` fails to compile, which is the fail-closed
/// contract the gateway needs.
pub(crate) fn build_body_limit_state(descriptors: &[IngressRouteDescriptor]) -> BodyLimitState {
    let routes = descriptors
        .iter()
        .map(|descriptor| {
            let policy = match descriptor.policy().body_limit() {
                BodyLimitPolicy::NoBody => ResolvedBodyPolicy::NoBody,
                BodyLimitPolicy::ComponentPreparation => ResolvedBodyPolicy::ComponentPreparation,
                BodyLimitPolicy::Limited { max_bytes } => ResolvedBodyPolicy::Limited {
                    max_bytes: max_bytes.get(),
                },
            };
            RouteBodyLimit {
                route_id: descriptor.route_id().as_str().to_string(),
                method: network_method_to_axum(descriptor.method()),
                segments: parse_pattern(descriptor.route_pattern().as_str()),
                policy,
            }
        })
        .collect();
    BodyLimitState {
        routes: Arc::new(routes),
        services: None,
        outer_default: crate::webui_serve::DEFAULT_WEBUI_MAX_BODY_BYTES,
        outer_explicit: None,
    }
}

// Existing non-global authoring transport contract; never selected on a failed
// acknowledged lookup. Its actual store must explicitly report the legacy mode.
#[derive(Clone, Copy)]
struct LegacyComponentBody;
const LEGACY_COMPONENT_BODY_BYTES: u64 = 14 * 1024 * 1024;

enum PreparationRejection {
    Unavailable,
    InvalidLimit,
}
impl IntoResponse for PreparationRejection {
    fn into_response(self) -> Response {
        match self {
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Acknowledged preparation unavailable.",
            )
                .into_response(),
            Self::InvalidLimit => too_large_for(ResolvedBodyPolicy::ComponentPreparation),
        }
    }
}

impl BodyLimitState {
    pub(crate) fn with_host(
        mut self,
        services: Arc<dyn brassclaw_product_workflow::RebornServicesApi>,
        outer_default: usize,
        outer_explicit: Option<usize>,
    ) -> Self {
        self.services = Some(services);
        self.outer_default = outer_default;
        self.outer_explicit = outer_explicit;
        self
    }
    async fn accept_preparation(&self, request: &mut Request) -> Result<u64, PreparationRejection> {
        use brassclaw_host_api::task_preparation::RetainedPreparation;
        if request.extensions().get::<LegacyComponentBody>().is_some() {
            return Ok(self
                .outer_explicit
                .map_or(LEGACY_COMPONENT_BODY_BYTES, |n| {
                    LEGACY_COMPONENT_BODY_BYTES.min(n as u64)
                }));
        }
        if request.extensions().get::<RetainedPreparation>().is_none() {
            let Some(services) = &self.services else {
                return Err(PreparationRejection::Unavailable);
            };
            let original = services
                .capture_component_preparation()
                .await
                .map_err(|_| PreparationRejection::Unavailable)?;
            match original {
                Some(original) => {
                    request.extensions_mut().insert(original);
                }
                None => {
                    request.extensions_mut().insert(LegacyComponentBody);
                    return Ok(self
                        .outer_explicit
                        .map_or(LEGACY_COMPONENT_BODY_BYTES, |n| {
                            LEGACY_COMPONENT_BODY_BYTES.min(n as u64)
                        }));
                }
            }
        }
        let policy = request
            .extensions()
            .get::<RetainedPreparation>()
            .ok_or(PreparationRejection::Unavailable)?
            .policy();
        let limit = policy.max_value_bytes;
        if limit == 0 || usize::try_from(limit).is_err() {
            return Err(PreparationRejection::InvalidLimit);
        }
        Ok(match self.outer_explicit {
            Some(max) => limit.min(max as u64),
            None => limit,
        })
    }
}
/// Preserve finite unmatched/static protection while named preparation routes
/// use their accepted effective policy. Explicit host overrides still narrow it.
pub(crate) async fn enforce_outer_body_limit(
    State(state): State<BodyLimitState>,
    request: Request,
    next: Next,
) -> Response {
    let prepared = match_route(&state.routes, request.method(), request.uri().path())
        .is_some_and(|route| matches!(route.policy, ResolvedBodyPolicy::ComponentPreparation));
    let max = if prepared {
        // Cheap framing only before authentication. New preparation values are
        // constrained by the existing u32 IPC carrier; no body is collected or
        // policy captured here. Explicit host narrowing remains independent.
        state
            .outer_explicit
            .map_or(u32::MAX as usize, |limit| limit.min(u32::MAX as usize))
    } else {
        state.outer_explicit.unwrap_or(state.outer_default)
    };
    if declared_content_length(&request).is_some_and(|n| n > max as u64) {
        return too_large_for(ResolvedBodyPolicy::Limited {
            max_bytes: max as u64,
        });
    }
    let (parts, body) = request.into_parts();
    let body = Body::new(http_body_util::Limited::new(body, max));
    next.run(Request::from_parts(parts, body)).await
}

fn match_route<'a>(
    routes: &'a [RouteBodyLimit],
    method: &Method,
    path: &str,
) -> Option<&'a RouteBodyLimit> {
    routes
        .iter()
        .find(|route| route.method == *method && segments_match(&route.segments, path))
}

fn declared_content_length(request: &Request) -> Option<u64> {
    request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.parse::<u64>().ok())
}

fn too_large_for(policy: ResolvedBodyPolicy) -> Response {
    let detail = match policy {
        ResolvedBodyPolicy::NoBody => "Request body not allowed for this route.".to_owned(),
        ResolvedBodyPolicy::Limited { max_bytes } => format!(
            "Request body exceeds the effective limit of {max_bytes} bytes. Component preparation also respects an explicit [webui].max_body_bytes_fallback host limit."
        ),
        ResolvedBodyPolicy::ComponentPreparation => {
            "Request body exceeds the preparation body limit.".to_owned()
        }
    };
    (StatusCode::PAYLOAD_TOO_LARGE, detail).into_response()
}

/// Keep existing small/static checks before auth. A dynamic preparation body
/// passes untouched to the actual auth/rate layers; it is collected below them.
pub(crate) async fn enforce_before_auth_body_limit(
    State(state): State<BodyLimitState>,
    request: Request,
    next: Next,
) -> Response {
    if match_route(&state.routes, request.method(), request.uri().path())
        .is_some_and(|route| matches!(route.policy, ResolvedBodyPolicy::ComponentPreparation))
    {
        return next.run(request).await;
    }
    enforce_body_limit(State(state), request, next).await
}
/// Mounted inside the existing authenticated/rate-limited protected stack only.
pub(crate) async fn enforce_prepared_body_limit(
    State(state): State<BodyLimitState>,
    request: Request,
    next: Next,
) -> Response {
    if match_route(&state.routes, request.method(), request.uri().path())
        .is_some_and(|route| matches!(route.policy, ResolvedBodyPolicy::ComponentPreparation))
    {
        return enforce_body_limit(State(state), request, next).await;
    }
    next.run(request).await
}

/// Axum middleware enforcing the descriptor's [`BodyLimitPolicy`] for
/// every matched v2 route. Unmatched paths pass through — the outer
/// finite outer body middleware in `webui_serve` still caps them.
pub(crate) async fn enforce_body_limit(
    State(state): State<BodyLimitState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(route) = match_route(&state.routes, request.method(), request.uri().path()) else {
        return next.run(request).await;
    };

    let max_bytes_u64: u64 = match route.policy {
        ResolvedBodyPolicy::NoBody => 0,
        ResolvedBodyPolicy::Limited { max_bytes } => max_bytes,
        ResolvedBodyPolicy::ComponentPreparation => {
            if request
                .extensions()
                .get::<brassclaw_product_workflow::WebUiAuthenticatedCaller>()
                .is_none()
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    "Operator authentication required.",
                )
                    .into_response();
            }
            match state.accept_preparation(&mut request).await {
                Ok(bytes) => bytes,
                Err(rejection) => return rejection.into_response(),
            }
        }
    };

    // Reject upfront when the client advertised a Content-Length that
    // already exceeds the policy. Saves us from buffering bytes we'd
    // drop anyway.
    if let Some(declared) = declared_content_length(&request)
        && declared > max_bytes_u64
    {
        tracing::debug!(
            target = "brassclaw::reborn::webui_body_limit",
            route_id = %route.route_id,
            declared,
            limit = max_bytes_u64,
            "rejecting oversized request by declared Content-Length",
        );
        return too_large_for(ResolvedBodyPolicy::Limited {
            max_bytes: max_bytes_u64,
        });
    }

    // Acknowledged bounds still require native representation checks.
    let max_bytes_usize = match usize::try_from(max_bytes_u64) {
        Ok(value) => value,
        Err(_) => {
            tracing::debug!(
                target = "brassclaw::reborn::webui_body_limit",
                route_id = %route.route_id,
                limit = max_bytes_u64,
                "body limit exceeds usize; rejecting as if oversized",
            );
            return too_large_for(ResolvedBodyPolicy::Limited {
                max_bytes: max_bytes_u64,
            });
        }
    };

    // Buffer only within the accepted finite whole-body policy. Capture stays
    // in server request extensions; it is never decoded from caller JSON.
    let (parts, body) = request.into_parts();
    let buffered = match axum::body::to_bytes(body, max_bytes_usize).await {
        Ok(bytes) => bytes,
        Err(_) => {
            tracing::debug!(
                target = "brassclaw::reborn::webui_body_limit",
                route_id = %route.route_id,
                limit = max_bytes_u64,
                "rejecting body that exceeded the per-route cap during read",
            );
            return too_large_for(ResolvedBodyPolicy::Limited {
                max_bytes: max_bytes_u64,
            });
        }
    };

    // Defensive: `to_bytes` should already enforce the bound, but if
    // a future axum release loosens that contract the explicit length
    // check still fails closed.
    if buffered.len() as u64 > max_bytes_u64 {
        return too_large_for(ResolvedBodyPolicy::Limited {
            max_bytes: max_bytes_u64,
        });
    }

    let request = Request::from_parts(parts, Body::from(buffered));
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::ingress::{
        AllowedEffectPath, AuditTraceClass, CorsPolicy, IngressAuthPolicy, IngressAuthScheme,
        IngressPolicy, IngressPolicyParts, IngressRouteDescriptor, ListenerClass, RateLimitPolicy,
        RateLimitScope, StreamingMode, WebSocketOriginPolicy,
    };
    use brassclaw_host_api::{IngressScopeSource, NetworkMethod};
    use std::num::{NonZeroU32, NonZeroU64};

    fn limited_policy(max_bytes_kib: u64) -> IngressPolicy {
        let max_bytes = NonZeroU64::new(max_bytes_kib * 1024).expect("nz");
        IngressPolicy::new(IngressPolicyParts {
            listener_class: ListenerClass::LocalGateway,
            auth: IngressAuthPolicy::Required {
                schemes: vec![IngressAuthScheme::BearerToken],
            },
            scope_source: IngressScopeSource::AuthenticatedCaller,
            body_limit: BodyLimitPolicy::Limited { max_bytes },
            rate_limit: RateLimitPolicy::Limited {
                scope: RateLimitScope::PerCaller,
                max_requests: NonZeroU32::new(60).expect("nz"),
                window_seconds: NonZeroU32::new(60).expect("nz"),
            },
            cors: CorsPolicy::SameOriginOnly,
            websocket_origin: WebSocketOriginPolicy::NotApplicable,
            streaming: StreamingMode::None,
            audit: AuditTraceClass::UserAction,
            effect_path: AllowedEffectPath::ProductWorkflow,
        })
        .expect("policy")
    }

    fn nobody_policy() -> IngressPolicy {
        IngressPolicy::new(IngressPolicyParts {
            listener_class: ListenerClass::LocalGateway,
            auth: IngressAuthPolicy::Required {
                schemes: vec![IngressAuthScheme::BearerToken],
            },
            scope_source: IngressScopeSource::AuthenticatedCaller,
            body_limit: BodyLimitPolicy::NoBody,
            rate_limit: RateLimitPolicy::Limited {
                scope: RateLimitScope::PerCaller,
                max_requests: NonZeroU32::new(60).expect("nz"),
                window_seconds: NonZeroU32::new(60).expect("nz"),
            },
            cors: CorsPolicy::SameOriginOnly,
            websocket_origin: WebSocketOriginPolicy::NotApplicable,
            streaming: StreamingMode::None,
            audit: AuditTraceClass::UserAction,
            effect_path: AllowedEffectPath::ProjectionOnly,
        })
        .expect("policy")
    }

    fn descriptor(
        route_id: &str,
        method: NetworkMethod,
        pattern: &str,
        policy: IngressPolicy,
    ) -> IngressRouteDescriptor {
        IngressRouteDescriptor::new(route_id.to_string(), method, pattern.to_string(), policy)
            .expect("descriptor")
    }

    #[test]
    fn build_body_limit_state_accepts_webui_v2_descriptors() {
        let descriptors = brassclaw_webui_v2::webui_v2_routes();
        let state = build_body_limit_state(&descriptors);
        assert_eq!(
            state.routes.len(),
            descriptors.len(),
            "every descriptor produced a RouteBodyLimit entry",
        );
        // Locks in the descriptor contract: send_message must be 1 MiB,
        // get_timeline and stream_events must be NoBody. A regression
        // that flips these would trip here before reaching production.
        let send = state
            .routes
            .iter()
            .find(|r| r.route_id == "webui.v2.send_message")
            .expect("send_message route");
        assert!(matches!(
            send.policy,
            ResolvedBodyPolicy::Limited { max_bytes } if max_bytes == 1024 * 1024,
        ));
        let timeline = state
            .routes
            .iter()
            .find(|r| r.route_id == "webui.v2.get_timeline")
            .expect("get_timeline route");
        assert!(matches!(timeline.policy, ResolvedBodyPolicy::NoBody));
    }

    #[test]
    fn build_body_limit_state_preserves_per_route_caps_from_descriptors() {
        let descriptors = vec![
            descriptor(
                "test.small",
                NetworkMethod::Post,
                "/api/small",
                limited_policy(4),
            ),
            descriptor(
                "test.large",
                NetworkMethod::Post,
                "/api/large",
                limited_policy(1024),
            ),
            descriptor(
                "test.read",
                NetworkMethod::Get,
                "/api/read",
                nobody_policy(),
            ),
        ];
        let state = build_body_limit_state(&descriptors);
        let small = state
            .routes
            .iter()
            .find(|r| r.route_id == "test.small")
            .unwrap();
        let large = state
            .routes
            .iter()
            .find(|r| r.route_id == "test.large")
            .unwrap();
        let read = state
            .routes
            .iter()
            .find(|r| r.route_id == "test.read")
            .unwrap();
        assert!(
            matches!(small.policy, ResolvedBodyPolicy::Limited { max_bytes } if max_bytes == 4 * 1024)
        );
        assert!(
            matches!(large.policy, ResolvedBodyPolicy::Limited { max_bytes } if max_bytes == 1024 * 1024)
        );
        assert!(matches!(read.policy, ResolvedBodyPolicy::NoBody));
    }
}
