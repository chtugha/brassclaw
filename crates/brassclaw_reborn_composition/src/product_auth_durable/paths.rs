use brassclaw_filesystem::FilesystemError;
use brassclaw_host_api::{ResourceScope, ScopedPath, SecretHandle};

use brassclaw_auth::{
    AuthFlowId, AuthInteractionId, AuthProductError, AuthSurface, CredentialAccountId,
};

pub(super) fn flow_path(
    scope: &brassclaw_auth::AuthProductScope,
    flow_id: AuthFlowId,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/flows/{flow_id}.json",
        product_auth_root(scope)
    ))
}

pub(super) fn flow_root(
    scope: &brassclaw_auth::AuthProductScope,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!("{}/flows", product_auth_root(scope)))
}

pub(super) fn surface_sessions_root(
    resource: &ResourceScope,
    surface: AuthSurface,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/{}/sessions",
        product_auth_base_root(resource),
        surface_path_segment(surface)
    ))
}

pub(super) fn interaction_path(
    scope: &brassclaw_auth::AuthProductScope,
    interaction_id: AuthInteractionId,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/interactions/{interaction_id}.json",
        product_auth_root(scope)
    ))
}

pub(super) fn account_path(
    scope: &brassclaw_auth::AuthProductScope,
    account_id: CredentialAccountId,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/accounts/{account_id}.json",
        product_auth_root(scope)
    ))
}

pub(super) fn account_root(
    scope: &brassclaw_auth::AuthProductScope,
) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!("{}/accounts", product_auth_root(scope)))
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
    let mut base = String::from("/secrets");
    if let Some(agent_id) = &resource.agent_id {
        base.push_str("/agents/");
        base.push_str(agent_id.as_str());
    }
    if let Some(project_id) = &resource.project_id {
        base.push_str("/projects/");
        base.push_str(project_id.as_str());
    }
    base.push_str("/product-auth");
    base
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

pub(super) fn join_scoped(prefix: &ScopedPath, leaf: &str) -> Result<ScopedPath, AuthProductError> {
    scoped_path(&format!(
        "{}/{}",
        prefix.as_str().trim_end_matches('/'),
        leaf
    ))
}

pub(super) fn manual_token_secret_handle(
    account_id: CredentialAccountId,
    interaction_id: AuthInteractionId,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("product-auth-manual-{account_id}-{interaction_id}"))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

pub(super) fn fs_error(error: FilesystemError) -> AuthProductError {
    match error {
        // CAS precondition failure — callers can detect and retry on BackendConflict.
        FilesystemError::VersionMismatch { .. } => AuthProductError::BackendConflict,
        _ => AuthProductError::BackendUnavailable,
    }
}
