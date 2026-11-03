use brassclaw_filesystem::FilesystemError;
use brassclaw_host_api::{ResourceScope, ScopedPath, SecretHandle};

use brassclaw_auth::{
    AuthFlowId, AuthInteractionId, AuthProductError, AuthSurface, CredentialAccountId,
};

pub(crate) fn flow_path(
    scope: &brassclaw_auth::AuthProductScope,
    flow_id: AuthFlowId,
) -> ScopedPath {
    ScopedPath::from_trusted(format!("{}/flows/{flow_id}.json", product_auth_root(scope)))
}

pub(crate) fn flow_root(scope: &brassclaw_auth::AuthProductScope) -> ScopedPath {
    ScopedPath::from_trusted(format!("{}/flows", product_auth_root(scope)))
}

pub(crate) fn surface_sessions_root(resource: &ResourceScope, surface: AuthSurface) -> ScopedPath {
    ScopedPath::from_trusted(format!(
        "{}/{}/sessions",
        product_auth_base_root(resource),
        surface_path_segment(surface)
    ))
}

pub(crate) fn interaction_path(
    scope: &brassclaw_auth::AuthProductScope,
    interaction_id: AuthInteractionId,
) -> ScopedPath {
    ScopedPath::from_trusted(format!(
        "{}/interactions/{interaction_id}.json",
        product_auth_root(scope)
    ))
}

pub(crate) fn requirement_selection_path(
    scope: &brassclaw_auth::AuthProductScope,
    key: &brassclaw_auth::CredentialRequirementKey,
) -> ScopedPath {
    ScopedPath::from_trusted(format!(
        "{}/requirement-selections/{}.json",
        product_auth_base_root(&scope.resource),
        key.as_str()
    ))
}

pub(crate) fn account_path(
    scope: &brassclaw_auth::AuthProductScope,
    account_id: CredentialAccountId,
) -> ScopedPath {
    ScopedPath::from_trusted(format!(
        "{}/accounts/{account_id}.json",
        product_auth_root(scope)
    ))
}

pub(crate) fn account_root(scope: &brassclaw_auth::AuthProductScope) -> ScopedPath {
    ScopedPath::from_trusted(format!("{}/accounts", product_auth_root(scope)))
}

fn product_auth_root(scope: &brassclaw_auth::AuthProductScope) -> String {
    let mut base = product_auth_base_root(&scope.resource);
    base.push('/');
    base.push_str(surface_path_segment(scope.surface));
    if let Some(session_id) = &scope.session_id {
        base.push_str("/sessions/");
        base.push_str(session_id.as_str());
    }
    base
}

fn product_auth_base_root(resource: &ResourceScope) -> String {
    // Uses `within_agent_project_segment` (not the full within-tenant segment)
    // so product-auth accounts are shared across threads within the same
    // (agent, project) owner scope — a `UserReusable` account created in a
    // setup thread must be visible to any runtime thread under the same owner.
    format!(
        "/secrets/{}/product-auth",
        resource.within_agent_project_segment()
    )
}

fn surface_path_segment(surface: AuthSurface) -> &'static str {
    match surface {
        brassclaw_auth::AuthSurface::Chat => "chat",
        brassclaw_auth::AuthSurface::Web => "web",
        brassclaw_auth::AuthSurface::Cli => "cli",
        brassclaw_auth::AuthSurface::Tui => "tui",
        brassclaw_auth::AuthSurface::Api => "api",
        brassclaw_auth::AuthSurface::SetupAdmin => "setup-admin",
        brassclaw_auth::AuthSurface::Callback => "callback",
    }
}

fn scoped_path(raw: &str) -> Result<ScopedPath, AuthProductError> {
    ScopedPath::new(raw).map_err(|_| AuthProductError::BackendUnavailable)
}

pub(crate) fn join_scoped(prefix: &ScopedPath, leaf: &str) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/{}",
        prefix.as_str().trim_end_matches('/'),
        leaf
    ))
}

pub(crate) fn manual_token_secret_handle(
    account_id: CredentialAccountId,
    interaction_id: AuthInteractionId,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("product-auth-manual-{account_id}-{interaction_id}"))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

pub(crate) fn fs_error(error: FilesystemError) -> AuthProductError {
    match error {
        // CAS precondition failure — callers can detect and retry on BackendConflict.
        FilesystemError::VersionMismatch { .. } | FilesystemError::SnapshotMismatch { .. } => {
            AuthProductError::BackendConflict
        }
        _ => AuthProductError::BackendUnavailable,
    }
}

/// Stable original intent/result location. Only the account envelope's matching
/// final receipt certifies publication; original request facts never change.
pub(crate) fn refresh_action_path(
    original: &brassclaw_auth::CredentialRefreshOriginal,
) -> Result<ScopedPath, AuthProductError> {
    Ok(ScopedPath::from_trusted(format!(
        "{}/refresh-actions/{}/{}.json",
        product_auth_root(&original.scope),
        original.scope_ref()?,
        original.id
    )))
}
