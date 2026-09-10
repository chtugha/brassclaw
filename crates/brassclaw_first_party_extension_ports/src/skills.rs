use std::sync::Arc;

use brassclaw_host_api::ScopedPath;

use crate::{SelectableSkillContextSource, error::FirstPartySkillsExtensionError};

const SYSTEM_SKILLS_ROOT: &str = "/system/skills";
const USER_SKILLS_ROOT: &str = "/skills";
const TENANT_SHARED_SKILLS_ROOT: &str = "/tenant-shared/skills";

/// Explicit scoped read handles granted to the first-party skills extension.
///
/// Retained for callers that need to validate or restrict skill root paths;
/// the VFS-based SKILL.md loading was removed in Phase P.1 Step C but these
/// handles remain as a lightweight path-validation utility.
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
}

/// First-party in-process skills extension.
///
/// Exports a `SelectableSkillContextSource` for message-text recording. The
/// VFS-based SKILL.md loading path was removed in Phase P.1 Step C
/// (subplan_step8_of_plan_skill_context_removal.md); skills are now DB
/// components injected via `PgBasicPromptStore`.
#[derive(Clone, Debug)]
pub struct FirstPartySkillsExtension {
    activation_source: Arc<SelectableSkillContextSource>,
}

impl FirstPartySkillsExtension {
    pub fn new() -> Self {
        Self {
            activation_source: Arc::new(SelectableSkillContextSource::new()),
        }
    }

    pub fn activation_source(&self) -> Arc<SelectableSkillContextSource> {
        Arc::clone(&self.activation_source)
    }
}

impl Default for FirstPartySkillsExtension {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn extension_new_provides_activation_source() {
        let ext = FirstPartySkillsExtension::new();
        // activation_source must be a fresh, non-null Arc
        let _ = ext.activation_source();
    }
}
