//! Complete server-owned usage handoff for Extension OAuth originals.
//! Resolution is new-setup metadata work; recovery reads immutable Auth custody.
use super::*;
use brassclaw_auth::{AuthFlowRecord, CredentialRequirementSetup};
use brassclaw_product_workflow::{
    ExtensionCredentialTarget, LifecyclePackageKind, LifecyclePackageRef,
    extension_manual_token_label,
};

pub(super) type PreparedOAuthRequirement = (
    CredentialRequirementSetup,
    Option<CredentialAccountUpdateBinding>,
);

pub(super) async fn prepare(
    state: &ProductAuthRouteState,
    caller: &WebUiAuthenticatedCaller,
    scope: &AuthProductScope,
    requester: &ExtensionId,
    request: &ExtensionOAuthStartRequest,
) -> Result<PreparedOAuthRequirement, ProductAuthRouteFailure> {
    let name = request
        .requirement
        .as_deref()
        .ok_or_else(ProductAuthRouteFailure::invalid_request)?;
    let target = request
        .target
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::invalid_request)?;
    let services = state
        .services
        .as_deref()
        .ok_or_else(ProductAuthRouteFailure::backend_unavailable)?;
    let package = LifecyclePackageRef::new(LifecyclePackageKind::Extension, requester.as_str())
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let candidate = tokio::time::timeout(
        PRODUCT_AUTH_BACKEND_TIMEOUT,
        services.resolve_extension_oauth_requirement(
            caller.clone(),
            package,
            name.to_owned(),
            scope.resource.invocation_id,
        ),
    )
    .await
    .map_err(|_| ProductAuthRouteFailure::backend_timeout())?
    .map_err(|error| match error.status_code {
        400 | 422 => ProductAuthRouteFailure::invalid_request(),
        403 => AuthProductError::CrossScopeDenied.into(),
        404 => ProductAuthRouteFailure::unknown_or_expired_flow(),
        409 => AuthProductError::BackendConflict.into(),
        _ => ProductAuthRouteFailure::backend_unavailable(),
    })?;
    let brassclaw_host_api::RuntimeCredentialRequirementSource::ProductAuthAccount {
        provider,
        setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth { scopes },
    } = &candidate.requirement().source
    else {
        return Err(ProductAuthRouteFailure::invalid_request());
    };
    if candidate.requester_extension() != requester
        || candidate
            .reference()
            .map_err(|_| ProductAuthRouteFailure::backend_unavailable())?
            != name
        || provider.as_str() != request.provider
        || scopes != &request.scopes
        || request.account_label
            != extension_manual_token_label(requester.as_str(), provider.as_str(), name)
    {
        return Err(ProductAuthRouteFailure::invalid_request());
    }
    let requested_account = match target {
        ExtensionCredentialTarget::New {} => None,
        ExtensionCredentialTarget::Existing { account_id } => Some(*account_id),
    };
    run_with_backend_timeout(state.product_auth.prepare_extension_oauth_requirement(
        scope,
        candidate,
        requested_account,
    ))
    .await
}

pub(super) async fn verify(
    state: &ProductAuthRouteState,
    scope: &AuthProductScope,
    flow: &AuthFlowRecord,
    requester: &ExtensionId,
    request: &ExtensionOAuthStartRequest,
) -> Result<(), ProductAuthRouteFailure> {
    run_with_backend_timeout(state.product_auth.verify_extension_oauth_original(
        scope,
        flow,
        requester,
        request.requirement.as_deref(),
        request.target.as_ref(),
    ))
    .await
}

#[cfg(test)]
pub(super) fn test_requirement(
    package: &str,
) -> brassclaw_host_api::RuntimeCredentialAccountRequirement {
    use brassclaw_host_api::*;
    let (provider, scopes, host) = if package == "google-calendar" {
        (
            "google",
            vec![brassclaw_auth::GOOGLE_CALENDAR_READONLY_SCOPE.to_owned()],
            "www.googleapis.com",
        )
    } else if package == "google-drive" {
        (
            "google",
            vec!["https://www.googleapis.com/auth/drive".to_owned()],
            "www.googleapis.com",
        )
    } else {
        assert_eq!(package, "notion");
        ("notion", vec![], "api.notion.com")
    };
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new(package).unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("oauth-account").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new(provider).unwrap(),
                setup: RuntimeCredentialAccountSetup::OAuth {
                    scopes: scopes.clone(),
                },
            },
            provider_scopes: scopes,
            audience: NetworkTargetPattern {
                scheme: Some(NetworkScheme::Https),
                host_pattern: host.into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap()
}

#[cfg(test)]
struct TestLifecycle(brassclaw_host_api::RuntimeCredentialAccountRequirement);
#[cfg(test)]
#[async_trait::async_trait]
impl brassclaw_product_workflow::LifecycleProductFacade for TestLifecycle {
    async fn execute(
        &self,
        _: brassclaw_product_workflow::LifecycleProductContext,
        _: brassclaw_product_workflow::LifecycleProductAction,
    ) -> Result<
        brassclaw_product_workflow::LifecycleProductResponse,
        brassclaw_product_workflow::ProductWorkflowError,
    > {
        panic!("credential metadata must project the package, not execute an action")
    }
    async fn project_package(
        &self,
        _: brassclaw_product_workflow::LifecycleProductContext,
        package: LifecyclePackageRef,
    ) -> Result<
        brassclaw_product_workflow::LifecycleProductResponse,
        brassclaw_product_workflow::ProductWorkflowError,
    > {
        use brassclaw_product_workflow::*;
        assert_eq!(package.id.as_str(), self.0.requester_extension().as_str());
        let brassclaw_host_api::RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider,
            setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth { scopes },
        } = &self.0.requirement().source
        else {
            panic!("OAuth fixture")
        };
        let mut response = LifecycleProductResponse::projection(
            Some(package.clone()),
            LifecyclePhase::Configured,
            vec![],
        );
        response.payload = Some(LifecycleProductPayload::ExtensionList {
            extensions: vec![LifecycleInstalledExtensionSummary {
                summary: LifecycleExtensionSummary {
                    package_ref: package.clone(),
                    name: package.id.as_str().into(),
                    version: "1.0.0".into(),
                    description: "OAuth metadata fixture".into(),
                    source: LifecycleExtensionSource::HostBundled,
                    runtime_kind: LifecycleExtensionRuntimeKind::FirstParty,
                    visible_capability_ids: vec![],
                    visible_read_only_capability_ids: vec![],
                    onboarding: None,
                    credential_requirements: vec![LifecycleExtensionCredentialRequirement {
                        name: self.0.reference().unwrap(),
                        provider: provider.as_str().into(),
                        required: true,
                        setup: LifecycleExtensionCredentialSetup::OAuth {
                            scopes: scopes.clone(),
                        },
                        runtime_requirement: Some(self.0.clone()),
                    }],
                },
                phase: LifecyclePhase::Configured,
            }],
            count: 1,
        });
        Ok(response)
    }
}

/// Actual Product facade and ordinary metadata projection. Turn execution is
/// not used by these setup requests; no alternate Auth/Secret owner is installed.
#[cfg(test)]
pub(super) fn test_services(
    package: &str,
) -> Arc<dyn brassclaw_product_workflow::RebornServicesApi> {
    Arc::new(
        brassclaw_product_workflow::RebornServices::new(
            Arc::new(brassclaw_threads::InMemorySessionThreadService::default()),
            Arc::new(brassclaw_turns::DefaultTurnCoordinator::new(Arc::new(
                brassclaw_turns::InMemoryTurnStateStore::default(),
            ))),
        )
        .with_lifecycle_product_facade(Arc::new(TestLifecycle(test_requirement(package)))),
    )
}

#[cfg(test)]
pub(super) async fn test_request(package: &str) -> serde_json::Value {
    let services = test_services(package);
    let projected = services
        .setup_extension(
            super::tests::test_caller(),
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, package).unwrap(),
            serde_json::from_value(serde_json::json!({})).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(projected.secrets.len(), 1);
    let secret = &projected.secrets[0];
    let brassclaw_product_workflow::RebornExtensionCredentialSetup::OAuth {
        account_label,
        scopes,
        invocation_id,
    } = &secret.setup
    else {
        panic!("OAuth projection")
    };
    serde_json::json!({
        "action":"start", "flow_id":AuthFlowId::new().to_string(), "requirement":secret.name, "target":{"kind":"new"},
        "provider":secret.provider, "account_label":account_label, "scopes":scopes, "invocation_id":invocation_id,
        "expires_at":(Utc::now()+ChronoDuration::minutes(5)).to_rfc3339(),
    })
}
