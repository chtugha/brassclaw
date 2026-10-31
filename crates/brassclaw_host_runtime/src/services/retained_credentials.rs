//! Explicit credential views on the existing retained service resolver.
//! No task-local context, source lookup, auth workflow or permission fallback.
use std::sync::Arc;

use crate::{
    InvocationServices, InvocationServicesError, InvocationServicesResolutionRequest,
    InvocationServicesResolver,
};
use brassclaw_secrets::SecretStore;

pub(super) struct CredentialBoundServices {
    inner: Arc<dyn InvocationServicesResolver>,
    secrets: Arc<dyn SecretStore>,
}
impl CredentialBoundServices {
    pub(super) fn new(
        inner: Arc<dyn InvocationServicesResolver>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self { inner, secrets }
    }
}
impl InvocationServicesResolver for CredentialBoundServices {
    fn resolve(
        &self,
        request: InvocationServicesResolutionRequest<'_>,
    ) -> Result<InvocationServices, InvocationServicesError> {
        let mut services = self.inner.resolve(request)?;
        // None is the original approved placement/access result. Never add a
        // secret service to a plan that did not receive one, or repair errors.
        if services.secret_store.is_some() {
            services.secret_store = Some(self.secrets.clone());
        }
        Ok(services)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExecutionPlan, LocalHostProcessPort, LocalInvocationServicesResolver, RuntimeProcessPort,
    };
    use brassclaw_filesystem::{InMemoryBackend, RootFilesystem};
    use brassclaw_host_api::{
        CapabilityId, ResourceScope,
        runtime_policy::{
            DeploymentMode, FilesystemBackendKind, NetworkMode, ProcessBackendKind, RuntimeProfile,
            SecretMode,
        },
    };
    use brassclaw_secrets::InMemorySecretStore;

    fn plan() -> ExecutionPlan {
        ExecutionPlan {
            capability: CapabilityId::new(crate::ECHO_CAPABILITY_ID).unwrap(),
            deployment: DeploymentMode::LocalSingleUser,
            resolved_profile: RuntimeProfile::LocalDev,
            filesystem_backend: FilesystemBackendKind::HostWorkspace,
            process_backend: ProcessBackendKind::None,
            network_mode: NetworkMode::Deny,
            secret_mode: SecretMode::ScrubbedEnv,
            requires_filesystem: false,
            requires_process: false,
            requires_network: false,
            requires_secret: false,
        }
    }
    fn request<'a>(
        plan: &'a ExecutionPlan,
        scope: &'a ResourceScope,
    ) -> InvocationServicesResolutionRequest<'a> {
        InvocationServicesResolutionRequest {
            plan,
            scope,
            mounts: None,
        }
    }
    fn local_resolver(store: Option<Arc<dyn SecretStore>>) -> LocalInvocationServicesResolver {
        LocalInvocationServicesResolver::new(
            Arc::new(InMemoryBackend::new()),
            None,
            Arc::new(LocalHostProcessPort::new()),
            store,
        )
    }

    #[test]
    fn bound_credentials_preserve_no_secret_access_from_original_plan() {
        let global: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let bound: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let resolver = CredentialBoundServices::new(Arc::new(local_resolver(Some(global))), bound);
        let mut plan = plan();
        let scope = ResourceScope::system(); // Pure resolution; no secret operation.
        assert!(
            resolver
                .resolve(request(&plan, &scope))
                .unwrap()
                .secret_store
                .is_none()
        );
        plan.secret_mode = SecretMode::Deny;
        assert!(
            resolver
                .resolve(request(&plan, &scope))
                .unwrap()
                .secret_store
                .is_none()
        );
    }

    #[test]
    fn bound_credentials_replace_only_the_originally_available_secret_port() {
        let filesystem: Arc<dyn RootFilesystem> = Arc::new(InMemoryBackend::new());
        let process: Arc<dyn RuntimeProcessPort> = Arc::new(LocalHostProcessPort::new());
        let global: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let bound: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let inner = Arc::new(LocalInvocationServicesResolver::new(
            filesystem.clone(),
            None,
            process.clone(),
            Some(global.clone()),
        ));
        let resolver = CredentialBoundServices::new(inner.clone(), bound.clone());
        let mut plan = plan();
        plan.requires_secret = true;
        let scope = ResourceScope::system();
        assert!(Arc::ptr_eq(
            inner
                .resolve(request(&plan, &scope))
                .unwrap()
                .secret_store
                .as_ref()
                .unwrap(),
            &global
        ));
        let services = resolver.resolve(request(&plan, &scope)).unwrap();
        assert!(Arc::ptr_eq(services.secret_store.as_ref().unwrap(), &bound));
        assert!(Arc::ptr_eq(&services.filesystem, &filesystem));
        assert!(Arc::ptr_eq(&services.process, &process));
        assert!(services.child_tasks.is_none());
        assert!(services.runtime_http_egress.is_none());
        assert!(services.tool_call_http_egress.is_none());
    }

    #[test]
    fn bound_credentials_do_not_repair_denied_modes_or_missing_backends() {
        let bound: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let global: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let resolver =
            CredentialBoundServices::new(Arc::new(local_resolver(Some(global))), bound.clone());
        let mut plan = plan();
        plan.requires_secret = true;
        plan.secret_mode = SecretMode::Deny;
        let scope = ResourceScope::system();
        assert!(matches!(
            resolver.resolve(request(&plan, &scope)),
            Err(InvocationServicesError::UnsupportedSecretMode {
                mode: SecretMode::Deny
            })
        ));
        let missing = CredentialBoundServices::new(Arc::new(local_resolver(None)), bound);
        plan.secret_mode = SecretMode::ScrubbedEnv;
        assert!(matches!(
            missing.resolve(request(&plan, &scope)),
            Err(InvocationServicesError::SecretAccessRequired)
        ));
    }
}
