use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{InvocationId, ResourceScope, RuntimeHttpEgress};
use brassclaw_product_workflow::{
    LifecyclePackageKind, LifecyclePackageRef, LifecyclePhase, LifecycleProductAction,
    LifecycleProductContext, LifecycleProductFacade, LifecycleProductPayload,
    LifecycleProductResponse, LifecycleReadinessBlocker, ProductWorkflowError,
};

use crate::extension_lifecycle::RebornLocalExtensionManagementPort;

#[derive(Clone, Default)]
pub(crate) struct RebornLocalLifecycleFacade {
    extension_management: Option<Arc<RebornLocalExtensionManagementPort>>,
    runtime_http_egress: Option<Arc<dyn RuntimeHttpEgress>>,
}

impl RebornLocalLifecycleFacade {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_extension_management(
        mut self,
        extension_management: Arc<RebornLocalExtensionManagementPort>,
    ) -> Self {
        self.extension_management = Some(extension_management);
        self
    }

    pub(crate) fn with_runtime_http_egress(
        mut self,
        runtime_http_egress: Arc<dyn RuntimeHttpEgress>,
    ) -> Self {
        self.runtime_http_egress = Some(runtime_http_egress);
        self
    }

    async fn execute_action(
        &self,
        context: LifecycleProductContext,
        action: LifecycleProductAction,
    ) -> Result<LifecycleProductResponse, ProductWorkflowError> {
        match action {
            LifecycleProductAction::ExtensionSearch { query } => {
                let Some(extension_management) = &self.extension_management else {
                    return unsupported_projection(None);
                };
                extension_management.search(&query).await
            }
            LifecycleProductAction::ExtensionList => {
                let Some(extension_management) = &self.extension_management else {
                    return unsupported_projection(None);
                };
                extension_management.list_installed().await
            }
            LifecycleProductAction::ExtensionInstall { package_ref } => {
                let Some(extension_management) = &self.extension_management else {
                    return unsupported_projection(Some(package_ref));
                };
                extension_management.install(package_ref).await
            }
            LifecycleProductAction::ExtensionActivate { package_ref } => {
                let Some(extension_management) = &self.extension_management else {
                    return unsupported_projection(Some(package_ref));
                };
                if extension_management
                    .package_requires_hosted_mcp_discovery(&package_ref)
                    .await?
                {
                    let Some(runtime_http_egress) = self.runtime_http_egress.clone() else {
                        return Err(ProductWorkflowError::InvalidBindingRequest {
                            reason: format!(
                                "extension {} requires hosted MCP schema discovery and cannot be activated through the static lifecycle facade",
                                package_ref.id
                            ),
                        });
                    };
                    let scope = lifecycle_resource_scope(&context)?;
                    return extension_management
                        .activate(
                            package_ref,
                            crate::extension_lifecycle::ExtensionActivationMode::HostedMcpDiscovery {
                                scope,
                                runtime_http_egress,
                            },
                        )
                        .await;
                }
                // This projection facade has no runtime egress services, so it
                // intentionally only supports static extension activation.
                extension_management
                    .activate(
                        package_ref,
                        crate::extension_lifecycle::ExtensionActivationMode::Static,
                    )
                    .await
            }
            LifecycleProductAction::ExtensionRemove { package_ref } => {
                let Some(extension_management) = &self.extension_management else {
                    return unsupported_projection(Some(package_ref));
                };
                extension_management.remove(package_ref).await
            }
            LifecycleProductAction::ExtensionAuth { package_ref }
            | LifecycleProductAction::ExtensionConfigure { package_ref, .. } => {
                unsupported_extension_auth_configure_projection(Some(package_ref))
            }
        }
    }
}

#[async_trait]
impl LifecycleProductFacade for RebornLocalLifecycleFacade {
    async fn execute(
        &self,
        context: LifecycleProductContext,
        action: LifecycleProductAction,
    ) -> Result<LifecycleProductResponse, ProductWorkflowError> {
        self.execute_action(context, action).await
    }

    async fn project_package(
        &self,
        _context: LifecycleProductContext,
        package_ref: LifecyclePackageRef,
    ) -> Result<LifecycleProductResponse, ProductWorkflowError> {
        if package_ref.kind == LifecyclePackageKind::Extension {
            let Some(extension_management) = &self.extension_management else {
                return unsupported_projection(Some(package_ref));
            };
            return extension_management.project(package_ref).await;
        }
        unsupported_projection(Some(package_ref))
    }
}

fn lifecycle_resource_scope(
    context: &LifecycleProductContext,
) -> Result<ResourceScope, ProductWorkflowError> {
    let LifecycleProductContext::Surface(context) = context else {
        return Err(ProductWorkflowError::InvalidBindingRequest {
            reason: "hosted MCP lifecycle activation requires a surface caller".to_string(),
        });
    };
    Ok(ResourceScope {
        tenant_id: context.tenant_id.clone(),
        user_id: context.user_id.clone(),
        agent_id: context.agent_id.clone(),
        project_id: context.project_id.clone(),
        thread_id: None,
        invocation_id: InvocationId::new(),
    })
}

pub(crate) fn response_with_payload(
    package_ref: Option<LifecyclePackageRef>,
    phase: LifecyclePhase,
    payload: LifecycleProductPayload,
) -> LifecycleProductResponse {
    LifecycleProductResponse {
        package_ref,
        phase,
        blockers: Vec::new(),
        message: None,
        payload: Some(payload),
    }
}

fn unsupported_projection(
    package_ref: Option<LifecyclePackageRef>,
) -> Result<LifecycleProductResponse, ProductWorkflowError> {
    Ok(LifecycleProductResponse::projection(
        package_ref,
        LifecyclePhase::UnsupportedOrLegacy,
        vec![LifecycleReadinessBlocker::runtime(Some(
            "extension_lifecycle_local_runtime_unwired".to_string(),
        ))?],
    ))
}

fn unsupported_extension_auth_configure_projection(
    package_ref: Option<LifecyclePackageRef>,
) -> Result<LifecycleProductResponse, ProductWorkflowError> {
    Ok(LifecycleProductResponse::projection(
        package_ref,
        LifecyclePhase::UnsupportedOrLegacy,
        vec![LifecycleReadinessBlocker::runtime(Some(
            "extension_auth_and_configure_not_yet_wired".to_string(),
        ))?],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, UserId};
    use brassclaw_product_workflow::LifecycleProductSurfaceContext;

    #[test]
    fn lifecycle_resource_scope_uses_surface_caller_identity() {
        let context = LifecycleProductContext::Surface(LifecycleProductSurfaceContext {
            tenant_id: TenantId::new("tenant-alpha").expect("tenant"),
            user_id: UserId::new("user-alpha").expect("user"),
            agent_id: Some(AgentId::new("agent-alpha").expect("agent")),
            project_id: Some(ProjectId::new("project-alpha").expect("project")),
        });

        let scope = lifecycle_resource_scope(&context).expect("surface scope");

        assert_eq!(scope.tenant_id.as_str(), "tenant-alpha");
        assert_eq!(scope.user_id.as_str(), "user-alpha");
        assert_eq!(
            scope.agent_id.as_ref().map(|id| id.as_str()),
            Some("agent-alpha")
        );
        assert_eq!(
            scope.project_id.as_ref().map(|id| id.as_str()),
            Some("project-alpha")
        );
        assert!(scope.thread_id.is_none());
    }
}
