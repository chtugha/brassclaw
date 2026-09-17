//! Postgres-backed [`SkillsProductFacade`] for the Settings → Skills tab.
//!
//! Replaces the filesystem-only [`RebornLocalSkillsProductFacade`] as the source
//! for `GET /api/webchat/v2/skills`.  The filesystem facade only saw skills
//! stored under `~/.brassclaw/reborn/skills/` — it was invisible to the builtin
//! skills seeded into `reborn_skills` by `builtin_bootstrap.rs`.
//!
//! ## Install and remove
//! Install and remove operations still delegate to the filesystem
//! [`RebornLocalSkillManagementPort`] — SKILL.md-based install/remove is the
//! existing flow.  After the operation completes the user can click Regenerate
//! in the Prefix tab to rebuild the system bundle.
//!
//! Feature-gated: `postgres`.

#[cfg(feature = "postgres")]
pub(crate) mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{
        RebornListSkillsResponse, RebornServicesError, RebornServicesErrorCode, RebornSkillInfo,
        RebornSkillInstallResult, RebornSkillRemoveResult, SkillsProductFacade,
        WebUiAuthenticatedCaller,
    };

    use crate::lifecycle::RebornLocalSkillManagementPort;

    /// Postgres-backed [`SkillsProductFacade`] implementation.
    ///
    /// `list_skills` queries `reborn_skills WHERE validation_status = 'validated'`
    /// so the Settings Skills tab shows all bootstrapped and user-installed skills.
    ///
    /// Install / remove still go through the filesystem port so the existing
    /// install flow (SKILL.md → parse → write to filesystem → Q1 → validated)
    /// is preserved.
    pub(crate) struct PgSkillsProductFacade {
        pool: Arc<PgPool>,
        tenant_id: String,
        skill_management: Arc<RebornLocalSkillManagementPort>,
    }

    impl PgSkillsProductFacade {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            skill_management: Arc<RebornLocalSkillManagementPort>,
        ) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
                skill_management,
            }
        }
    }

    #[async_trait]
    impl SkillsProductFacade for PgSkillsProductFacade {
        async fn list_skills(
            &self,
            _caller: &WebUiAuthenticatedCaller,
        ) -> Result<RebornListSkillsResponse, RebornServicesError> {
            let client = self.pool.get().await.map_err(|e| {
                tracing::debug!(error = %e, "pg_skills_facade: pool unavailable");
                RebornServicesError::from_status(RebornServicesErrorCode::Unavailable, 503, false)
            })?;

            let rows = client
                .query(
                    "SELECT name, \
                            COALESCE(version, '0.0.0') AS version, \
                            COALESCE(description, '') AS description, \
                            COALESCE(source, 'system') AS source, \
                            COALESCE(keywords, ARRAY[]::text[]) AS keywords, \
                            COALESCE(tags, ARRAY[]::text[]) AS tags \
                     FROM reborn_skills \
                     WHERE tenant_id = $1 \
                       AND validation_status = 'validated' \
                       AND NOT ('05:validator' = ANY(COALESCE(consumer_tags, ARRAY[]::text[]))) \
                     ORDER BY class_code ASC, prompt_uid ASC \
                     LIMIT 500",
                    &[&self.tenant_id.as_str()],
                )
                .await
                .map_err(|e| {
                    tracing::debug!(error = %e, "pg_skills_facade: list query failed");
                    RebornServicesError::from_status(
                        RebornServicesErrorCode::Unavailable,
                        503,
                        false,
                    )
                })?;

            let skills = rows
                .into_iter()
                .filter_map(|row| {
                    let name: String = row.try_get("name").ok()?;
                    let version: String = row.try_get("version").ok()?;
                    let description: String = row.try_get("description").ok()?;
                    let source: String = row.try_get("source").ok()?;
                    let keywords: Vec<String> = row.try_get("keywords").unwrap_or_default();
                    let tags: Vec<String> = row.try_get("tags").unwrap_or_default();
                    Some(RebornSkillInfo {
                        name,
                        version,
                        description,
                        source,
                        keywords,
                        tags,
                        requires_skills: Vec::new(),
                    })
                })
                .collect();

            Ok(RebornListSkillsResponse { skills })
        }

        async fn install_skill(
            &self,
            _caller: &WebUiAuthenticatedCaller,
            content: String,
            source_url: Option<String>,
        ) -> Result<RebornSkillInstallResult, RebornServicesError> {
            // Install still writes to the filesystem skill management port.
            let result = self
                .skill_management
                .install_with_source_url(None, &content, source_url.as_deref())
                .await
                .map_err(map_skill_mgmt_error)?;
            Ok(RebornSkillInstallResult {
                name: result.name,
                source: result.source.as_str().to_string(),
                success: true,
                message: "installed".to_string(),
            })
        }

        async fn remove_skill(
            &self,
            _caller: &WebUiAuthenticatedCaller,
            name: &str,
        ) -> Result<RebornSkillRemoveResult, RebornServicesError> {
            // Remove still targets the filesystem skill management port.
            let result = self
                .skill_management
                .remove(name)
                .await
                .map_err(map_skill_mgmt_error)?;
            Ok(RebornSkillRemoveResult {
                name: result.name,
                success: true,
                message: "removed".to_string(),
            })
        }
    }

    fn map_skill_mgmt_error(
        error: crate::lifecycle::RebornLocalSkillManagementError,
    ) -> RebornServicesError {
        use brassclaw_skills::SkillManagementErrorKind;
        match error {
            crate::lifecycle::RebornLocalSkillManagementError::InvalidContext { .. } => {
                RebornServicesError::from_status(
                    RebornServicesErrorCode::InvalidRequest,
                    400,
                    false,
                )
            }
            crate::lifecycle::RebornLocalSkillManagementError::Skill(e) => match e.kind() {
                SkillManagementErrorKind::InvalidInput
                | SkillManagementErrorKind::NotFound
                | SkillManagementErrorKind::Conflict
                | SkillManagementErrorKind::InvalidSkill => RebornServicesError::from_status(
                    RebornServicesErrorCode::InvalidRequest,
                    400,
                    false,
                ),
                SkillManagementErrorKind::FilesystemDenied => {
                    RebornServicesError::from_status(RebornServicesErrorCode::Forbidden, 403, false)
                }
                SkillManagementErrorKind::Resource => RebornServicesError::from_status(
                    RebornServicesErrorCode::Unavailable,
                    503,
                    false,
                ),
            },
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgSkillsProductFacade;
