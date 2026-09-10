use brassclaw_skills::{SkillManagementError, SkillManagementErrorKind};

use crate::{
    RebornBuildError,
    lifecycle::{RebornLocalSkillManagementError, build_existing_local_dev_skill_management_port},
};

pub async fn list_reborn_local_skills(
    owner_id: impl Into<String>,
    local_dev_storage_root: impl Into<std::path::PathBuf>,
) -> Result<Vec<brassclaw_skills::SkillSummary>, RebornSkillListError> {
    // System skills are now seeded into `reborn_skills` DB rows via Phase P.1
    // (`builtin_bootstrap.rs` Passes 8–14). The bundled compile-time embedded
    // catalog (`bundled_skills.rs`) has been removed; DB rows are the sole
    // authoritative source (Phase P.1 Step A).
    let mut skills =
        match build_existing_local_dev_skill_management_port(owner_id, local_dev_storage_root)? {
            Some(skill_management) => skill_management
                .list()
                .await
                .map_err(map_local_skill_management_error)?,
            None => Vec::new(),
        };

    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.source.as_str().cmp(right.source.as_str()))
    });
    Ok(skills)
}

#[derive(Debug, thiserror::Error)]
pub enum RebornSkillListError {
    #[error(transparent)]
    Build(#[from] RebornBuildError),
    #[error("skill list request rejected: {reason}")]
    InvalidRequest { reason: String },
    #[error("skill list access denied")]
    AccessDenied,
    #[error("skill list unavailable: {reason}")]
    Unavailable { reason: String },
}

fn map_local_skill_management_error(
    error: RebornLocalSkillManagementError,
) -> RebornSkillListError {
    match error {
        RebornLocalSkillManagementError::InvalidContext { reason } => {
            RebornSkillListError::InvalidRequest { reason }
        }
        RebornLocalSkillManagementError::Skill(error) => map_skill_management_error(error),
    }
}

fn map_skill_management_error(error: SkillManagementError) -> RebornSkillListError {
    match error.kind() {
        SkillManagementErrorKind::InvalidInput
        | SkillManagementErrorKind::NotFound
        | SkillManagementErrorKind::Conflict
        | SkillManagementErrorKind::InvalidSkill => RebornSkillListError::InvalidRequest {
            reason: error
                .reason()
                .unwrap_or("skill management request rejected")
                .to_string(),
        },
        SkillManagementErrorKind::FilesystemDenied => RebornSkillListError::AccessDenied,
        SkillManagementErrorKind::Resource => RebornSkillListError::Unavailable {
            reason: "skill management resource unavailable".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_skill_list_rejects_non_directory_storage_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");
        std::fs::write(&storage_root, "not a directory").expect("storage root file");

        let error = match list_reborn_local_skills("list-owner", &storage_root).await {
            Ok(_) => panic!("file storage root must fail"),
            Err(error) => error,
        };

        assert!(
            matches!(
                error,
                RebornSkillListError::Build(RebornBuildError::InvalidConfig { .. })
            ),
            "unexpected error: {error}"
        );
        assert!(
            error.to_string().contains("not a directory"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn local_skill_list_rejects_invalid_owner_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");
        std::fs::create_dir_all(&storage_root).expect("storage root");

        let error = match list_reborn_local_skills("list/owner", &storage_root).await {
            Ok(_) => panic!("invalid owner id must fail"),
            Err(error) => error,
        };

        assert!(
            matches!(
                error,
                RebornSkillListError::Build(RebornBuildError::InvalidConfig { .. })
            ),
            "unexpected error: {error}"
        );
        assert!(
            error.to_string().contains("slash") || error.to_string().contains("path"),
            "unexpected error: {error}"
        );
    }

}
