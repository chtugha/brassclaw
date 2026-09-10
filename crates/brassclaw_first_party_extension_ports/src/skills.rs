use std::sync::Arc;

use brassclaw_filesystem::{RootFilesystem, ScopedFilesystem};
use brassclaw_host_api::{ScopedPath, TenantId};
use brassclaw_loop_support::{FilesystemSkillBundleRoot, FilesystemSkillBundleSource};

use crate::{
    SelectableSkillContextSource, SkillActivationSelectorConfig, SkillExecutionAdapter,
    error::FirstPartySkillsExtensionError, setup_markers::FilesystemSetupMarkerSource,
};

const SYSTEM_SKILLS_ROOT: &str = "/system/skills";
const USER_SKILLS_ROOT: &str = "/skills";
const TENANT_SHARED_SKILLS_ROOT: &str = "/tenant-shared/skills";

/// Explicit scoped read handles granted to the first-party skills extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstPartySkillsExtensionHandles {
    system_skills: Option<ScopedPath>,
    user_skills: Option<ScopedPath>,
    tenant_shared_skills: Option<ScopedPath>,
}

impl FirstPartySkillsExtensionHandles {
    /// Handles for the standard first-slice Reborn skill roots.
    pub fn reborn_default() -> Result<Self, FirstPartySkillsExtensionError> {
        Self::without_tenant_shared()
    }

    /// Handles for deployments that do not expose tenant-shared skills.
    pub fn without_tenant_shared() -> Result<Self, FirstPartySkillsExtensionError> {
        Ok(Self {
            system_skills: Some(scoped_root(SYSTEM_SKILLS_ROOT)?),
            user_skills: Some(scoped_root(USER_SKILLS_ROOT)?),
            tenant_shared_skills: None,
        })
    }

    /// Builds handles from explicit roots and validates that each handle points
    /// at its Reborn-owned skill namespace.
    pub fn new(
        system_skills: Option<ScopedPath>,
        user_skills: Option<ScopedPath>,
        tenant_shared_skills: Option<ScopedPath>,
    ) -> Result<Self, FirstPartySkillsExtensionError> {
        if let Some(root) = system_skills.as_ref() {
            validate_handle_root("read_system_skills", root, SYSTEM_SKILLS_ROOT)?;
        }
        if let Some(root) = user_skills.as_ref() {
            validate_handle_root("read_user_skills", root, USER_SKILLS_ROOT)?;
        }
        if let Some(root) = tenant_shared_skills.as_ref() {
            validate_handle_root("read_tenant_shared_skills", root, TENANT_SHARED_SKILLS_ROOT)?;
        }
        Ok(Self {
            system_skills,
            user_skills,
            tenant_shared_skills,
        })
    }

    pub fn system_skills(&self) -> Option<&ScopedPath> {
        self.system_skills.as_ref()
    }

    pub fn user_skills(&self) -> Option<&ScopedPath> {
        self.user_skills.as_ref()
    }

    pub fn tenant_shared_skills(&self) -> Option<&ScopedPath> {
        self.tenant_shared_skills.as_ref()
    }

    fn bundle_roots(&self, tenant_id: &TenantId) -> Vec<FilesystemSkillBundleRoot> {
        let mut roots = Vec::new();
        if let Some(root) = &self.system_skills {
            roots.push(FilesystemSkillBundleRoot::system(root.clone()));
        }
        if let Some(root) = &self.tenant_shared_skills {
            roots.push(FilesystemSkillBundleRoot::tenant_shared(
                root.clone(),
                tenant_id.clone(),
            ));
        }
        if let Some(root) = &self.user_skills {
            roots.push(FilesystemSkillBundleRoot::user(root.clone()));
        }
        roots
    }
}

/// First-party in-process skills extension.
///
/// It is userland composition: it receives explicit scoped skill read handles
/// and exports loop-facing skill context sources. It does not expose raw
/// filesystem, database, secrets, network, dispatcher, or tool authority.
#[derive(Clone)]
pub struct FirstPartySkillsExtension<F>
where
    F: RootFilesystem + 'static,
{
    bundle_source: Arc<FilesystemSkillBundleSource<F>>,
    default_selectable_runtime: FirstPartySelectableSkillsRuntime<F>,
}

pub struct FirstPartySelectableSkillsRuntime<F>
where
    F: RootFilesystem + 'static,
{
    activation_source: Arc<SelectableSkillContextSource<FilesystemSkillBundleSource<F>>>,
    execution_adapter: Arc<SkillExecutionAdapter<FilesystemSkillBundleSource<F>>>,
}

impl<F> Clone for FirstPartySelectableSkillsRuntime<F>
where
    F: RootFilesystem + 'static,
{
    fn clone(&self) -> Self {
        Self {
            activation_source: Arc::clone(&self.activation_source),
            execution_adapter: Arc::clone(&self.execution_adapter),
        }
    }
}

impl<F> std::fmt::Debug for FirstPartySelectableSkillsRuntime<F>
where
    F: RootFilesystem + 'static,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FirstPartySelectableSkillsRuntime")
            .field("activation_source", &self.activation_source)
            .field("execution_adapter", &self.execution_adapter)
            .finish()
    }
}

impl<F> FirstPartySelectableSkillsRuntime<F>
where
    F: RootFilesystem + 'static,
{
    fn new(
        activation_source: Arc<SelectableSkillContextSource<FilesystemSkillBundleSource<F>>>,
        execution_adapter: Arc<SkillExecutionAdapter<FilesystemSkillBundleSource<F>>>,
    ) -> Self {
        Self {
            activation_source,
            execution_adapter,
        }
    }

    pub fn activation_source(
        &self,
    ) -> Arc<SelectableSkillContextSource<FilesystemSkillBundleSource<F>>> {
        Arc::clone(&self.activation_source)
    }

    pub fn execution_adapter(&self) -> Arc<SkillExecutionAdapter<FilesystemSkillBundleSource<F>>> {
        Arc::clone(&self.execution_adapter)
    }
}

impl<F> std::fmt::Debug for FirstPartySkillsExtension<F>
where
    F: RootFilesystem + 'static,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FirstPartySkillsExtension")
            .finish_non_exhaustive()
    }
}

impl<F> FirstPartySkillsExtension<F>
where
    F: RootFilesystem + 'static,
{
    pub fn new(
        filesystem: Arc<ScopedFilesystem<F>>,
        handles: FirstPartySkillsExtensionHandles,
        tenant_id: TenantId,
    ) -> Result<Self, FirstPartySkillsExtensionError> {
        let bundle_source = Arc::new(
            FilesystemSkillBundleSource::new(filesystem, handles.bundle_roots(&tenant_id))
                .map_err(|error| {
                    FirstPartySkillsExtensionError::InvalidBundleSource(error.to_string())
                })?,
        );
        let default_selectable_context_source = Arc::new(SelectableSkillContextSource::new(
            Arc::clone(&bundle_source),
            SkillActivationSelectorConfig::default(),
        ));
        let execution_adapter = Arc::new(SkillExecutionAdapter::new(Arc::clone(
            &default_selectable_context_source,
        )));
        let default_selectable_runtime = FirstPartySelectableSkillsRuntime::new(
            default_selectable_context_source,
            execution_adapter,
        );
        Ok(Self {
            bundle_source,
            default_selectable_runtime,
        })
    }

    pub fn selectable_skill_context_source(
        &self,
        config: SkillActivationSelectorConfig,
    ) -> Arc<SelectableSkillContextSource<FilesystemSkillBundleSource<F>>> {
        if config == SkillActivationSelectorConfig::default() {
            return self.default_selectable_runtime.activation_source();
        }
        Arc::new(SelectableSkillContextSource::new(
            Arc::clone(&self.bundle_source),
            config,
        ))
    }

    pub fn selectable_skill_runtime(
        &self,
        config: SkillActivationSelectorConfig,
    ) -> FirstPartySelectableSkillsRuntime<F> {
        if config == SkillActivationSelectorConfig::default() {
            return self.default_selectable_runtime.clone();
        }
        let activation_source = self.selectable_skill_context_source(config);
        let execution_adapter =
            Arc::new(SkillExecutionAdapter::new(Arc::clone(&activation_source)));
        FirstPartySelectableSkillsRuntime::new(activation_source, execution_adapter)
    }

    pub fn selectable_skill_runtime_with_setup_markers<W>(
        &self,
        config: SkillActivationSelectorConfig,
        workspace_filesystem: Arc<ScopedFilesystem<W>>,
    ) -> FirstPartySelectableSkillsRuntime<F>
    where
        W: RootFilesystem + 'static,
    {
        let setup_marker_source = Arc::new(FilesystemSetupMarkerSource::new(workspace_filesystem));
        let activation_source = Arc::new(
            SelectableSkillContextSource::new(Arc::clone(&self.bundle_source), config)
                .with_setup_marker_source(setup_marker_source),
        );
        let execution_adapter =
            Arc::new(SkillExecutionAdapter::new(Arc::clone(&activation_source)));
        FirstPartySelectableSkillsRuntime::new(activation_source, execution_adapter)
    }

    pub fn skill_execution_adapter(
        &self,
    ) -> Arc<SkillExecutionAdapter<FilesystemSkillBundleSource<F>>> {
        self.default_selectable_runtime.execution_adapter()
    }
}

fn scoped_root(path: &'static str) -> Result<ScopedPath, FirstPartySkillsExtensionError> {
    ScopedPath::new(path)
        .map_err(|reason| FirstPartySkillsExtensionError::InvalidRootPath(reason.to_string()))
}

fn validate_handle_root(
    handle: &'static str,
    root: &ScopedPath,
    expected: &'static str,
) -> Result<(), FirstPartySkillsExtensionError> {
    if root.as_str() == expected {
        return Ok(());
    }
    Err(FirstPartySkillsExtensionError::InvalidHandle {
        handle,
        expected,
        actual: root.as_str().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_handles_are_exact_reborn_skill_roots() {
        let handles = FirstPartySkillsExtensionHandles::reborn_default().unwrap();

        assert_eq!(
            handles.system_skills().map(ScopedPath::as_str),
            Some("/system/skills")
        );
        assert_eq!(
            handles.user_skills().map(ScopedPath::as_str),
            Some("/skills")
        );
        assert_eq!(handles.tenant_shared_skills().map(ScopedPath::as_str), None);
    }

    #[test]
    fn handles_reject_non_skill_roots() {
        let error = FirstPartySkillsExtensionHandles::new(
            None,
            Some(ScopedPath::new("/workspace").unwrap()),
            None,
        )
        .unwrap_err();

        assert_eq!(
            error,
            FirstPartySkillsExtensionError::InvalidHandle {
                handle: "read_user_skills",
                expected: "/skills",
                actual: "/workspace".to_string()
            }
        );
    }
}
