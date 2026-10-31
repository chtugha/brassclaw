use brassclaw_auth::AuthProductScope;
use brassclaw_host_api::ExtensionId;

use crate::{
    LifecycleExtensionCredentialRequirement, LifecycleExtensionCredentialSetup,
    LifecycleProductPayload, LifecycleProductResponse, RebornExtensionCredentialSetup,
    RebornExtensionSetupSecret, RebornServicesError,
};

use super::{
    ExtensionCredentialSetupService, ExtensionCredentialStatusRequest,
    extension_manual_tokens::manual_label,
};

pub(super) fn requirements(
    lifecycle: &LifecycleProductResponse,
) -> Vec<LifecycleExtensionCredentialRequirement> {
    let Some(LifecycleProductPayload::ExtensionList { extensions, .. }) = &lifecycle.payload else {
        return Vec::new();
    };
    let mut requirements = Vec::new();
    for extension in extensions {
        for requirement in &extension.summary.credential_requirements {
            if requirements
                .iter()
                .any(|seen: &LifecycleExtensionCredentialRequirement| seen.name == requirement.name)
            {
                continue;
            }
            requirements.push(requirement.clone());
        }
    }
    requirements
}

pub(super) async fn project(
    extension_credentials: Option<&dyn ExtensionCredentialSetupService>,
    scope: AuthProductScope,
    extension_id: &ExtensionId,
    requirements: &[LifecycleExtensionCredentialRequirement],
) -> Result<Vec<RebornExtensionSetupSecret>, RebornServicesError> {
    let mut secrets = Vec::with_capacity(requirements.len());
    for requirement in requirements {
        let account = match (
            extension_credentials,
            exact_requirement(requirement, extension_id)?,
        ) {
            (Some(service), Some(requirement)) => {
                service
                    .credential_status(ExtensionCredentialStatusRequest {
                        scope: scope.clone(),
                        requirement,
                    })
                    .await?
            }
            _ => None,
        };
        secrets.push(RebornExtensionSetupSecret {
            name: requirement.name.clone(),
            provider: requirement.provider.clone(),
            prompt: credential_prompt(requirement),
            optional: !requirement.required,
            provided: account.is_some(),
            setup: setup_projection(&scope, extension_id, requirement),
            credential_ref: account.map(|account| account.id.to_string()),
        });
    }
    secrets.sort_by_key(|secret| !secret.provided);
    Ok(secrets)
}

fn setup_projection(
    scope: &AuthProductScope,
    extension_id: &ExtensionId,
    requirement: &LifecycleExtensionCredentialRequirement,
) -> RebornExtensionCredentialSetup {
    match &requirement.setup {
        LifecycleExtensionCredentialSetup::ManualToken => {
            RebornExtensionCredentialSetup::ManualToken {
                account_label: manual_label(
                    extension_id.as_str(),
                    &requirement.provider,
                    &requirement.name,
                ),
                invocation_id: scope.resource.invocation_id.to_string(),
            }
        }
        LifecycleExtensionCredentialSetup::OAuth { scopes } => {
            RebornExtensionCredentialSetup::OAuth {
                account_label: credential_label(extension_id, requirement),
                scopes: scopes.clone(),
                invocation_id: scope.resource.invocation_id.to_string(),
            }
        }
    }
}

fn credential_prompt(requirement: &LifecycleExtensionCredentialRequirement) -> String {
    let Some(exact) = &requirement.runtime_requirement else {
        return format!("{} credential", requirement.provider);
    };
    use brassclaw_host_api::RuntimeCredentialTarget;
    let declaration = exact.requirement();
    let target = match &declaration.target {
        RuntimeCredentialTarget::Header { name, .. } => format!("header {name}"),
        RuntimeCredentialTarget::QueryParam { name } => format!("query parameter {name}"),
        RuntimeCredentialTarget::PathPlaceholder { placeholder } => format!("path {placeholder}"),
    };
    format!(
        "{} credential for {} ({target})",
        requirement.provider, declaration.audience.host_pattern
    )
}

pub(super) fn exact_requirement(
    summary: &LifecycleExtensionCredentialRequirement,
    extension: &ExtensionId,
) -> Result<Option<brassclaw_host_api::RuntimeCredentialAccountRequirement>, RebornServicesError> {
    use brassclaw_host_api::{RuntimeCredentialAccountSetup, RuntimeCredentialRequirementSource};
    let Some(requirement) = &summary.runtime_requirement else {
        return Ok(None);
    };
    if requirement.requester_extension() != extension {
        return Err(RebornServicesError::internal_invariant());
    }
    let declaration = requirement.requirement();
    let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, setup } =
        &declaration.source
    else {
        return Err(RebornServicesError::internal_invariant());
    };
    let compatible = match (&summary.setup, setup) {
        (
            LifecycleExtensionCredentialSetup::ManualToken,
            RuntimeCredentialAccountSetup::ManualToken,
        ) => true,
        (
            LifecycleExtensionCredentialSetup::OAuth { scopes: a },
            RuntimeCredentialAccountSetup::OAuth { scopes: b },
        ) => a == b,
        _ => false,
    };
    if provider.as_str() != summary.provider
        || declaration.required != summary.required
        || !compatible
    {
        return Err(RebornServicesError::internal_invariant());
    }
    Ok(Some(requirement.clone()))
}

fn credential_label(
    extension_id: &ExtensionId,
    requirement: &LifecycleExtensionCredentialRequirement,
) -> String {
    let base = format!("{} {}", extension_id.as_str(), requirement.provider);
    if requirement.runtime_requirement.is_some() {
        manual_label(
            extension_id.as_str(),
            &requirement.provider,
            &requirement.name,
        )
    } else if requirement.name.contains("__") {
        format!("{base} {}", requirement.name)
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use crate::{RebornServicesErrorCode, RebornServicesErrorKind, WebUiInboundValidationCode};
    use async_trait::async_trait;
    use brassclaw_auth::{AuthSurface, CredentialAccountProjection};
    use brassclaw_host_api::{InvocationId, ResourceScope, UserId};

    use super::*;

    #[tokio::test]
    async fn project_preserves_retryable_unavailable_credential_status() {
        let service = FailingCredentialSetupService {
            error: RebornServicesError {
                code: RebornServicesErrorCode::Unavailable,
                kind: RebornServicesErrorKind::ServiceUnavailable,
                status_code: 503,
                retryable: true,
                field: None,
                validation_code: None,
            },
        };
        let extension_id = ExtensionId::new("google-docs").expect("extension id");

        let error = project(
            Some(&service),
            test_scope(),
            &extension_id,
            &[oauth_requirement()],
        )
        .await
        .expect_err("unavailable status is not an unconfigured credential");
        assert_eq!(error.code, RebornServicesErrorCode::Unavailable);
        assert_eq!(error.status_code, 503);
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn project_preserves_non_status_credential_errors() {
        let service = FailingCredentialSetupService {
            error: RebornServicesError {
                code: RebornServicesErrorCode::InvalidRequest,
                kind: RebornServicesErrorKind::Validation,
                status_code: 400,
                retryable: false,
                field: Some("provider".to_string()),
                validation_code: Some(WebUiInboundValidationCode::InvalidValue),
            },
        };
        let extension_id = ExtensionId::new("google-docs").expect("extension id");

        let error = project(
            Some(&service),
            test_scope(),
            &extension_id,
            &[oauth_requirement()],
        )
        .await
        .expect_err("validation errors should not be hidden by setup projection");

        assert_eq!(error.code, RebornServicesErrorCode::InvalidRequest);
        assert_eq!(error.kind, RebornServicesErrorKind::Validation);
        assert_eq!(error.field.as_deref(), Some("provider"));
    }

    #[tokio::test]
    async fn project_preserves_non_retryable_unavailable_credential_errors() {
        let service = FailingCredentialSetupService {
            error: RebornServicesError {
                code: RebornServicesErrorCode::Unavailable,
                kind: RebornServicesErrorKind::ServiceUnavailable,
                status_code: 503,
                retryable: false,
                field: None,
                validation_code: None,
            },
        };
        let extension_id = ExtensionId::new("google-docs").expect("extension id");

        let error = project(
            Some(&service),
            test_scope(),
            &extension_id,
            &[oauth_requirement()],
        )
        .await
        .expect_err("non-retryable unavailable errors should stay visible");

        assert_eq!(error.code, RebornServicesErrorCode::Unavailable);
        assert_eq!(error.kind, RebornServicesErrorKind::ServiceUnavailable);
        assert!(!error.retryable);
    }

    struct FailingCredentialSetupService {
        error: RebornServicesError,
    }

    #[async_trait]
    impl ExtensionCredentialSetupService for FailingCredentialSetupService {
        async fn credential_status(
            &self,
            _request: ExtensionCredentialStatusRequest,
        ) -> Result<Option<CredentialAccountProjection>, RebornServicesError> {
            Err(self.error.clone())
        }

        async fn submit_manual_token(
            &self,
            _request: super::super::ExtensionCredentialSubmitRequest,
        ) -> Result<super::super::ExtensionManualTokenCompletion, RebornServicesError> {
            Err(RebornServicesError::service_unavailable(true))
        }
    }

    fn declared_test_requirement(
        name: &str,
        provider: &str,
        extension: &str,
        setup: &LifecycleExtensionCredentialSetup,
        required: bool,
    ) -> brassclaw_host_api::RuntimeCredentialAccountRequirement {
        use brassclaw_host_api::*;
        let (setup, scopes) = match setup {
            LifecycleExtensionCredentialSetup::ManualToken => {
                (RuntimeCredentialAccountSetup::ManualToken, vec![])
            }
            LifecycleExtensionCredentialSetup::OAuth { scopes } => (
                RuntimeCredentialAccountSetup::OAuth {
                    scopes: scopes.clone(),
                },
                scopes.clone(),
            ),
        };
        RuntimeCredentialAccountRequirement::new(
            ExtensionId::new(extension).unwrap(),
            RuntimeCredentialRequirement {
                handle: SecretHandle::new(name).unwrap(),
                source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                    provider: RuntimeCredentialAccountProviderId::new(provider).unwrap(),
                    setup,
                },
                provider_scopes: scopes,
                audience: NetworkTargetPattern {
                    scheme: None,
                    host_pattern: "api.example.test".into(),
                    port: None,
                },
                target: RuntimeCredentialTarget::Header {
                    name: "Authorization".into(),
                    prefix: Some("Bearer ".into()),
                },
                required,
            },
        )
        .unwrap()
    }

    #[tokio::test]
    async fn legacy_summary_without_exact_usage_never_claims_a_configured_account() {
        let mut legacy = oauth_requirement();
        legacy.runtime_requirement = None;
        let service = FailingCredentialSetupService {
            error: RebornServicesError::internal_invariant(),
        };
        let projection = project(
            Some(&service),
            test_scope(),
            &ExtensionId::new("google-docs").unwrap(),
            std::slice::from_ref(&legacy),
        )
        .await
        .unwrap();
        assert!(!projection[0].provided);
        assert!(projection[0].credential_ref.is_none());
        let bytes = serde_json::to_value(&legacy).unwrap();
        assert!(bytes.get("runtime_requirement").is_none());
        let restored: LifecycleExtensionCredentialRequirement =
            serde_json::from_value(bytes).unwrap();
        assert!(restored.runtime_requirement.is_none());
    }

    fn oauth_requirement() -> LifecycleExtensionCredentialRequirement {
        LifecycleExtensionCredentialRequirement {
            runtime_requirement: Some(declared_test_requirement(
                "google_oauth",
                "google",
                "google-docs",
                &LifecycleExtensionCredentialSetup::OAuth {
                    scopes: vec!["https://www.googleapis.com/auth/documents".into()],
                },
                true,
            )),
            name: "google_oauth".to_string(),
            provider: "google".to_string(),
            required: true,
            setup: LifecycleExtensionCredentialSetup::OAuth {
                scopes: vec!["https://www.googleapis.com/auth/documents".to_string()],
            },
        }
    }

    fn test_scope() -> AuthProductScope {
        AuthProductScope::new(
            ResourceScope::local_default(
                UserId::new("user-alpha").expect("user id"),
                InvocationId::new(),
            )
            .expect("resource scope"),
            AuthSurface::Web,
        )
    }
}
