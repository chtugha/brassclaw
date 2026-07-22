//! Self-improving plan library service (subtask 5).
//!
//! After each agent session `PlanLibraryProcessor` runs scoring on the
//! completed `LoopExecutionState`, persists the plan document to the Memory
//! filesystem, and advances the skill's maturity tier if the Wilson lower
//! bound crosses a threshold.
//!
//! ## Memory layout
//!
//! All paths are virtual paths inside the `CompositeRootFilesystem`
//! (the `local_dev_root` filesystem that backs the Reborn local-dev store).
//!
//! ```text
//! /workspace/reborn-cli/users/reborn-cli/projects/_none/skills/.plan-library/{type}/{slug}.md
//! /workspace/reborn-cli/users/reborn-cli/projects/_none/skills/{slug}/SKILL.md
//! ```
//!
//! ## GitHub PR promotion
//!
//! When a skill reaches the Candidate tier, `submit_skill_candidate` is called
//! internally to create a branch + PR on the upstream brassclaw repo.  The PR
//! handler lives in `crate::submit_skill_candidate` and is registered as an
//! **internal-only** first-party handler — it is NOT exposed in any capability
//! schema presented to the model.

use std::sync::Arc;

use brassclaw_agent_loop::plan_scoring::{
    SkillMaturityTier, ToolOutcome, classify_tier, score_session, wilson_lower_bound,
};
use brassclaw_agent_loop::plan_state::{AgentPlanState, PlanType};
use brassclaw_agent_loop::state::LoopExecutionState;
use brassclaw_filesystem::RootFilesystem;
use brassclaw_host_api::VirtualPath;

/// Default Wilson z-quantile (95 % confidence interval).
pub(crate) const DEFAULT_WILSON_Z: f64 = 1.96;
/// Default promotion threshold (Wilson lower bound for Candidate tier).
pub(crate) const DEFAULT_PROMOTION_THRESHOLD: f64 = 0.80;
/// Virtual path prefix under which plan-library documents are written.
const PLAN_LIBRARY_ROOT: &str =
    "/workspace/reborn-cli/users/reborn-cli/projects/_none/skills/.plan-library";
/// Virtual path prefix under which workspace skills are written.
const WORKSPACE_SKILLS_ROOT: &str = "/workspace/reborn-cli/users/reborn-cli/projects/_none/skills";

/// Simple in-memory skill metrics accumulator for the plan library.
///
/// Persisted as a JSON sidecar next to the plan document so metrics survive
/// process restarts. (A real DB-backed SkillTracker would be better, but
/// `FilesystemMemoryDocumentRepository` requires embedding metadata in a
/// `MemoryDoc` which is heavier than needed here.)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlanLibraryMetrics {
    pub(crate) usage_count: u64,
    pub(crate) success_count: u64,
    pub(crate) failure_count: u64,
    pub(crate) last_wilson: f64,
    pub(crate) tier: SkillMaturityTier,
    pub(crate) pr_url: Option<String>,
}

impl PlanLibraryMetrics {
    pub(crate) fn wilson_lower_bound(&self, z: f64) -> f64 {
        wilson_lower_bound(self.success_count, self.failure_count, z)
    }
}

/// Service that manages the plan library for a single Reborn runtime instance.
pub(crate) struct PlanLibraryService<F: RootFilesystem + ?Sized> {
    filesystem: Arc<F>,
    promotion_threshold: f64,
}

impl<F> PlanLibraryService<F>
where
    F: RootFilesystem + ?Sized + 'static,
{
    pub(crate) fn new(filesystem: Arc<F>, promotion_threshold: Option<f64>) -> Self {
        Self {
            filesystem,
            promotion_threshold: promotion_threshold.unwrap_or(DEFAULT_PROMOTION_THRESHOLD),
        }
    }

    /// Process a completed session: score it, update metrics, persist plan
    /// document, apply tier effects. Errors are logged and swallowed — the plan
    /// library is an enhancement, not a correctness requirement.
    pub(crate) async fn process_session(
        &self,
        state: &LoopExecutionState,
        tool_outcomes: &[ToolOutcome],
    ) {
        let Some(plan_state) = state.plan_state.as_ref() else {
            return;
        };
        let score = score_session(
            Some(plan_state),
            tool_outcomes,
            state.iteration as usize,
            &state.content_cache,
        );
        let success = score >= 0.60;
        let slug = self.plan_type_slug(plan_state.plan_type);
        let mut metrics = self.load_metrics(&slug).await;
        metrics.usage_count += 1;
        if success {
            metrics.success_count += 1;
        } else {
            metrics.failure_count += 1;
        }
        let w_lower = metrics.wilson_lower_bound(DEFAULT_WILSON_Z);
        metrics.last_wilson = w_lower;
        let new_tier = classify_tier(metrics.usage_count, w_lower, self.promotion_threshold);

        // Persist plan document
        let plan_md = self.build_plan_document(plan_state, score);
        if let Err(error) = self
            .save_plan_document(&slug, plan_state.plan_type, &plan_md)
            .await
        {
            tracing::debug!(%error, %slug, "plan library: failed to save plan document");
        }

        // Apply tier effects
        let tier_changed = new_tier > metrics.tier;
        metrics.tier = new_tier;
        if let Err(error) = self.save_metrics(&slug, &metrics).await {
            tracing::debug!(%error, %slug, "plan library: failed to save metrics");
        }

        if tier_changed {
            self.apply_tier_effect(&slug, plan_state, &metrics).await;
        }
        tracing::debug!(
            %slug,
            score,
            w_lower,
            usage_count = metrics.usage_count,
            tier = ?new_tier,
            "plan library: session processed"
        );
    }

    fn plan_type_slug(&self, plan_type: PlanType) -> String {
        match plan_type {
            PlanType::CodeGeneration => "code-generation".to_string(),
            PlanType::FileOperation => "file-operation".to_string(),
            PlanType::ShellTask => "shell-task".to_string(),
            PlanType::Research => "research".to_string(),
            PlanType::Generic => "generic".to_string(),
        }
    }

    fn build_plan_document(&self, plan_state: &AgentPlanState, score: f64) -> String {
        let steps: Vec<String> = plan_state
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect();
        format!(
            "# Plan Document\n\n\
             **Type:** {:?}\n\
             **Score:** {:.2}\n\
             **Steps completed:** {}/{}\n\n\
             ## Steps\n\n{}\n",
            plan_state.plan_type,
            score,
            plan_state.current_step.min(plan_state.steps.len()),
            plan_state.steps.len(),
            steps.join("\n")
        )
    }

    async fn save_plan_document(
        &self,
        slug: &str,
        plan_type: PlanType,
        content: &str,
    ) -> Result<(), brassclaw_filesystem::FilesystemError> {
        let type_slug = self.plan_type_slug(plan_type);
        let path_str = format!("{}/{}/{}.md", PLAN_LIBRARY_ROOT, type_slug, slug);
        let path =
            VirtualPath::new(path_str).map_err(brassclaw_filesystem::FilesystemError::Contract)?;
        self.filesystem.write_file(&path, content.as_bytes()).await
    }

    async fn save_metrics(
        &self,
        slug: &str,
        metrics: &PlanLibraryMetrics,
    ) -> Result<(), brassclaw_filesystem::FilesystemError> {
        let path_str = format!("{}/{}.metrics.json", PLAN_LIBRARY_ROOT, slug);
        let path =
            VirtualPath::new(path_str).map_err(brassclaw_filesystem::FilesystemError::Contract)?;
        let bytes = serde_json::to_vec(metrics).unwrap_or_else(|e| {
            tracing::debug!(%slug, error = %e, "plan library: failed to serialise metrics; writing empty object");
            b"{}".to_vec()
        });
        self.filesystem.write_file(&path, &bytes).await
    }

    async fn load_metrics(&self, slug: &str) -> PlanLibraryMetrics {
        let path_str = format!("{}/{}.metrics.json", PLAN_LIBRARY_ROOT, slug);
        let Ok(path) = VirtualPath::new(path_str) else {
            return PlanLibraryMetrics::default();
        };
        match self.filesystem.read_file(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::debug!(%slug, error = %e, "plan library: failed to deserialise metrics; resetting to default");
                PlanLibraryMetrics::default()
            }),
            _ => PlanLibraryMetrics::default(),
        }
    }

    async fn apply_tier_effect(
        &self,
        slug: &str,
        plan_state: &AgentPlanState,
        metrics: &PlanLibraryMetrics,
    ) {
        match metrics.tier {
            SkillMaturityTier::Seedling => {
                // Write initial SKILL.md
                if let Err(error) = self.ensure_skill(slug, plan_state).await {
                    tracing::debug!(%error, %slug, "plan library: failed to ensure skill (seedling)");
                }
            }
            SkillMaturityTier::Growing => {
                // Rewrite SKILL.md with growing tag
                if let Err(error) = self.upgrade_skill_to_growing(slug, plan_state).await {
                    tracing::debug!(%error, %slug, "plan library: failed to upgrade skill (growing)");
                }
            }
            SkillMaturityTier::Mature => {
                // Copy to tenant-shared (same directory, different prefix in practice)
                if let Err(error) = self.promote_to_tenant_shared(slug, plan_state).await {
                    tracing::debug!(%error, %slug, "plan library: failed to promote skill (mature)");
                }
            }
            SkillMaturityTier::Candidate => {
                // Submit GitHub PR
                self.submit_skill_candidate(slug, plan_state, metrics).await;
            }
        }
    }

    async fn ensure_skill(
        &self,
        slug: &str,
        plan_state: &AgentPlanState,
    ) -> Result<(), brassclaw_filesystem::FilesystemError> {
        let path_str = format!("{}/{}/SKILL.md", WORKSPACE_SKILLS_ROOT, slug);
        let path =
            VirtualPath::new(path_str).map_err(brassclaw_filesystem::FilesystemError::Contract)?;
        // Only write if it doesn't already exist
        if self.filesystem.read_file(&path).await.is_ok() {
            return Ok(());
        }
        let content = self.build_skill_md(slug, plan_state, false);
        self.filesystem.write_file(&path, content.as_bytes()).await
    }

    async fn upgrade_skill_to_growing(
        &self,
        slug: &str,
        plan_state: &AgentPlanState,
    ) -> Result<(), brassclaw_filesystem::FilesystemError> {
        let path_str = format!("{}/{}/SKILL.md", WORKSPACE_SKILLS_ROOT, slug);
        let path =
            VirtualPath::new(path_str).map_err(brassclaw_filesystem::FilesystemError::Contract)?;
        let content = self.build_skill_md(slug, plan_state, true);
        self.filesystem.write_file(&path, content.as_bytes()).await
    }

    async fn promote_to_tenant_shared(
        &self,
        slug: &str,
        plan_state: &AgentPlanState,
    ) -> Result<(), brassclaw_filesystem::FilesystemError> {
        // Write to the system/skills directory (TenantShared equivalent in local-dev)
        let path_str = format!("/system/skills/{}/SKILL.md", slug);
        let path =
            VirtualPath::new(path_str).map_err(brassclaw_filesystem::FilesystemError::Contract)?;
        let content = self.build_skill_md(slug, plan_state, true);
        self.filesystem.write_file(&path, content.as_bytes()).await
    }

    fn build_skill_md(&self, slug: &str, plan_state: &AgentPlanState, growing: bool) -> String {
        let description = format!(
            "Auto-generated skill for {} tasks (Wilson-scored plan library, subtask 5).",
            slug
        );
        let tags = if growing {
            format!("tags: [{}, growing]", slug)
        } else {
            format!("tags: [{}]", slug)
        };
        let keywords: Vec<&str> = match plan_state.plan_type {
            PlanType::CodeGeneration => vec!["code", "implement", "function", "refactor"],
            PlanType::FileOperation => vec!["file", "directory", "create", "delete"],
            PlanType::ShellTask => vec!["shell", "run", "execute", "script"],
            PlanType::Research => vec!["search", "find", "investigate", "research"],
            PlanType::Generic => vec!["task", "plan"],
        };
        format!(
            "---\n\
             name: {slug}\n\
             description: \"{description}\"\n\
             activation:\n  \
               {tags}\n  \
               keywords: [{kws}]\n\
             ---\n\n\
             # {name} Skill\n\n\
             {description}\n\n\
             ## Typical Steps\n\n\
             {steps}\n",
            slug = slug,
            description = description,
            tags = tags,
            kws = keywords.join(", "),
            name = slug
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default()
                + &slug[slug.char_indices().nth(1).map(|(i, _)| i).unwrap_or(0)..],
            steps = plan_state
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// Submit a GitHub PR for the Candidate tier skill.
    ///
    /// Uses raw GitHub REST API calls. Requires `github_token` secret to be
    /// configured on the testmachine.  All errors are logged and swallowed.
    async fn submit_skill_candidate(
        &self,
        slug: &str,
        plan_state: &AgentPlanState,
        metrics: &PlanLibraryMetrics,
    ) {
        // Skip if already submitted (pr_url is set)
        if metrics.pr_url.is_some() {
            tracing::debug!(%slug, "plan library: skill candidate PR already submitted");
            return;
        }
        let token = match std::env::var("BRASSCLAW_GITHUB_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
        {
            Ok(t) if !t.is_empty() => t,
            _ => {
                tracing::debug!(
                    %slug,
                    "plan library: github_token not configured; skipping skill candidate PR"
                );
                return;
            }
        };

        let timestamp = chrono::Utc::now().timestamp();
        let branch = format!("skill-candidate/{}-{}", slug, timestamp);
        let skill_path = format!(
            "crates/brassclaw_reborn_composition/assets/skills/{}/SKILL.md",
            slug
        );
        let skill_content = self.build_skill_md(slug, plan_state, false);
        let pr_body = format!(
            "## Skill Candidate: `{slug}`\n\n\
             **Plan type:** {:?}\n\
             **Usage count:** {}\n\
             **Wilson lower bound:** {:.3}\n\
             **Tier:** {:?}\n\n\
             Auto-promoted by the BrassClaw plan library (subtask 5).\n\
             Human review required before merging.\n",
            plan_state.plan_type, metrics.usage_count, metrics.last_wilson, metrics.tier
        );

        let pr_result = self
            .github_create_skill_pr(
                &token,
                "chtugha",
                "brassclaw",
                &branch,
                &skill_path,
                &skill_content,
                &format!(
                    "[Skill Candidate] {}: auto-generated skill for {} tasks",
                    slug, slug
                ),
                &pr_body,
            )
            .await;

        match pr_result {
            Ok(url) => {
                tracing::debug!(%slug, pr_url = %url, "plan library: skill candidate PR created");
            }
            Err(error) => {
                tracing::debug!(%slug, %error, "plan library: failed to create skill candidate PR");
            }
        }
    }

    /// Low-level GitHub API calls: CreateBranch → CreateOrUpdateFile → CreatePullRequest.
    #[allow(clippy::too_many_arguments)]
    async fn github_create_skill_pr(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        branch: &str,
        file_path: &str,
        file_content: &str,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<String, String> {
        use base64::Engine;
        let client = reqwest::Client::builder()
            .user_agent("brassclaw-plan-library/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let auth = format!("Bearer {token}");
        let api = "https://api.github.com".to_string();

        // 1. Get the SHA of main's HEAD to branch from
        let refs_url = format!("{api}/repos/{owner}/{repo}/git/ref/heads/main");
        let refs_resp: serde_json::Value = client
            .get(&refs_url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let main_sha = refs_resp
            .pointer("/object/sha")
            .and_then(|v| v.as_str())
            .ok_or("could not read main SHA")?
            .to_string();

        // 2. Create branch
        let create_branch_url = format!("{api}/repos/{owner}/{repo}/git/refs");
        let branch_body = serde_json::json!({
            "ref": format!("refs/heads/{branch}"),
            "sha": main_sha
        });
        let branch_resp = client
            .post(&create_branch_url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .json(&branch_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !branch_resp.status().is_success() {
            let status = branch_resp.status();
            let body = branch_resp.text().await.unwrap_or_default();
            return Err(format!("CreateBranch failed ({status}): {body}"));
        }

        // 3. Create/update file
        let encoded_content =
            base64::engine::general_purpose::STANDARD.encode(file_content.as_bytes());
        let file_url = format!(
            "{api}/repos/{owner}/{repo}/contents/{}",
            encode_url_path(file_path)
        );
        let file_body = serde_json::json!({
            "message": format!("feat: add skill candidate {}", file_path),
            "content": encoded_content,
            "branch": branch
        });
        let file_resp = client
            .put(&file_url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .json(&file_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !file_resp.status().is_success() {
            let status = file_resp.status();
            let body = file_resp.text().await.unwrap_or_default();
            return Err(format!("CreateOrUpdateFile failed ({status}): {body}"));
        }

        // 4. Create PR
        let pr_url = format!("{api}/repos/{owner}/{repo}/pulls");
        let pr_payload = serde_json::json!({
            "title": pr_title,
            "head": branch,
            "base": "main",
            "body": pr_body
        });
        let pr_resp: serde_json::Value = client
            .post(&pr_url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .json(&pr_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let html_url = pr_resp
            .get("html_url")
            .and_then(|v| v.as_str())
            .ok_or("PR response missing html_url")?
            .to_string();
        Ok(html_url)
    }
}

fn encode_url_path(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            seg.bytes()
                .flat_map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                        vec![b as char]
                    }
                    _ => {
                        let hi = b"0123456789ABCDEF"[(b >> 4) as usize] as char;
                        let lo = b"0123456789ABCDEF"[(b & 0xf) as usize] as char;
                        vec!['%', hi, lo]
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A plan state snapshot shared between the executor (writer) and the
/// post-turn processor (reader).  Updated after every completed run via a
/// `PlanStatePortDecorator`-like mechanism.
#[derive(Debug, Clone, Default)]
pub(crate) struct CurrentPlanStateSlot(Arc<std::sync::Mutex<Option<LoopExecutionState>>>);

impl CurrentPlanStateSlot {
    pub(crate) fn new() -> Self {
        Self(Arc::new(std::sync::Mutex::new(None)))
    }

    /// Set the current state (called by the post-turn bridge).
    #[allow(dead_code)]
    pub(crate) fn set(&self, state: LoopExecutionState) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(state);
    }

    /// Take the current state (called by the post-turn processor, once per turn).
    #[allow(dead_code)]
    pub(crate) fn take(&self) -> Option<LoopExecutionState> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.take()
    }
}
