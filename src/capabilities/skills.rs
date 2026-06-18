use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::sync::Arc;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};

use brassclaw_skills::catalog::{SkillCatalog, catalog_entry_is_installed, resolve_catalog_slug_for_name};
use brassclaw_skills::registry::SkillRegistry;

use crate::tools::builtin::skill_tools::{SkillFetchError, SkillInstallPayload, fetch_skill_payload};

pub const PROVIDER_ID: &str = "builtin";
pub const SKILL_INSTALL_CAPABILITY_ID: &str = "builtin.skill_install";
pub const SKILL_REMOVE_CAPABILITY_ID: &str = "builtin.skill_remove";
pub const SKILL_LIST_CAPABILITY_ID: &str = "builtin.skill_list";
pub const SKILL_SEARCH_CAPABILITY_ID: &str = "builtin.skill_search";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 2_000;
const MAX_WALL_CLOCK_MS: u64 = 30_000;

const MAX_CHAIN_DEPS: usize = 10;
const MAX_CHAIN_QUEUE: usize = MAX_CHAIN_DEPS * 10;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct SkillsCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl SkillsCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

impl From<SkillFetchError> for SkillsCapabilityError {
    fn from(e: SkillFetchError) -> Self {
        Self::operation(e.to_string())
    }
}

pub struct SkillsContext {
    pub registry: Arc<std::sync::RwLock<SkillRegistry>>,
    pub catalog: Arc<SkillCatalog>,
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn skill_install_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SKILL_INSTALL_CAPABILITY_ID,
        "Install a skill from SKILL.md content, a URL, or by name from the ClawHub catalog.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Skill name or slug (from search results)"
                },
                "slug": {
                    "type": "string",
                    "description": "Registry slug from catalog search results; preferred when installing from ClawHub"
                },
                "url": {
                    "type": "string",
                    "description": "Direct URL to a SKILL.md file"
                },
                "content": {
                    "type": "string",
                    "description": "Raw SKILL.md content to install directly"
                },
                "install_dependencies": {
                    "type": "boolean",
                    "description": "When true, also install companion skills declared in requires.skills. Defaults to false so dependency installs stay explicit in the approved tool call.",
                    "default": false
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn skill_remove_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SKILL_REMOVE_CAPABILITY_ID,
        "Permanently remove an installed skill from disk. This action cannot be undone — the skill files will be deleted.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to remove"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn skill_list_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SKILL_LIST_CAPABILITY_ID,
        "List all loaded skills with their trust level, source, and activation keywords.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "verbose": {
                    "type": "boolean",
                    "description": "Include extra detail (tags, content_hash, version)",
                    "default": false
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn skill_search_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SKILL_SEARCH_CAPABILITY_ID,
        "Search for skills in the ClawHub catalog and among locally loaded skills.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (name, keyword, or description fragment)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        skill_install_descriptor(),
        skill_remove_descriptor(),
        skill_list_descriptor(),
        skill_search_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, SkillsCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SkillsCapabilityError::input(format!("missing required parameter: {key}"))
        })
}

fn registry_read(
    registry: &Arc<std::sync::RwLock<SkillRegistry>>,
) -> std::sync::RwLockReadGuard<'_, SkillRegistry> {
    registry.read().unwrap_or_else(|poison| {
        tracing::error!(
            "skill registry RwLock was poisoned (a previous writer panicked); \
             recovering — skill state may be from before the panic"
        );
        poison.into_inner()
    })
}

fn registry_write(
    registry: &Arc<std::sync::RwLock<SkillRegistry>>,
) -> std::sync::RwLockWriteGuard<'_, SkillRegistry> {
    registry.write().unwrap_or_else(|poison| {
        tracing::error!(
            "skill registry RwLock was poisoned (a previous writer panicked); \
             recovering — skill state may be from before the panic"
        );
        poison.into_inner()
    })
}

#[derive(Debug, Default)]
struct ChainInstallReport {
    installed: Vec<String>,
    failed: Vec<String>,
    missing: Vec<String>,
    skipped: Vec<String>,
    pending_explicit_install: Vec<String>,
}

impl ChainInstallReport {
    fn has_warnings(&self) -> bool {
        !self.failed.is_empty()
            || !self.missing.is_empty()
            || !self.skipped.is_empty()
            || !self.pending_explicit_install.is_empty()
    }
}

fn append_chain_install_report_fields(output: &mut Value, report: &ChainInstallReport) {
    if !report.installed.is_empty() {
        output["chain_installed"] = json!(&report.installed);
    }
    if !report.failed.is_empty() {
        output["chain_install_failed"] = json!(&report.failed);
    }
    if !report.missing.is_empty() {
        output["missing_dependencies"] = json!(&report.missing);
        output["missing_dependencies_message"] = json!(format!(
            "These required skills could not be found in the catalog and need manual installation: {}",
            report.missing.join(", ")
        ));
    }
    if !report.skipped.is_empty() {
        output["skipped_dependencies"] = json!(&report.skipped);
        output["skipped_dependencies_message"] = json!(format!(
            "{} dependency chain hit the MAX_CHAIN_DEPS={} *attempt* cap. These deps were not attempted and must be installed manually with a follow-up skill_install call: {}",
            report.skipped.len(),
            MAX_CHAIN_DEPS,
            report.skipped.join(", ")
        ));
    }
    if !report.pending_explicit_install.is_empty() {
        output["pending_dependency_install"] = json!(&report.pending_explicit_install);
        output["pending_dependency_install_message"] = json!(format!(
            "Companion skills were not installed automatically. Re-run skill_install with install_dependencies=true to approve installing: {}",
            report.pending_explicit_install.join(", ")
        ));
    }
}

fn build_skill_install_output(installed_name: &str, report: &ChainInstallReport) -> Value {
    let status = if report.has_warnings() {
        "installed_with_warnings"
    } else {
        "installed"
    };
    let message = if report.has_warnings() {
        format!(
            "Skill '{}' installed with warnings. It will activate when matching keywords are detected.",
            installed_name
        )
    } else {
        format!(
            "Skill '{}' installed successfully. It will activate when matching keywords are detected.",
            installed_name
        )
    };

    let mut output = json!({
        "name": installed_name,
        "status": status,
        "trust": "installed",
        "message": message,
    });

    append_chain_install_report_fields(&mut output, report);
    output
}

fn build_already_installed_output(name: &str, report: &ChainInstallReport) -> Value {
    let status = if report.has_warnings() {
        "already_installed_with_warnings"
    } else {
        "already_installed"
    };
    let message = if report.has_warnings() {
        format!(
            "Skill '{}' is already active; dependency installation finished with warnings.",
            name
        )
    } else if report.installed.is_empty() {
        format!("Skill '{}' is already active — no install needed.", name)
    } else {
        format!(
            "Skill '{}' is already active; companion skills were installed.",
            name
        )
    };

    let mut output = json!({
        "name": name,
        "status": status,
        "trust": "installed",
        "message": message,
    });

    append_chain_install_report_fields(&mut output, report);
    output
}

async fn install_missing_skill_dependencies<F, Fut>(
    registry: &Arc<std::sync::RwLock<SkillRegistry>>,
    registry_url: &str,
    required_skills: Vec<String>,
    fetcher: F,
) -> Result<ChainInstallReport, SkillsCapabilityError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<SkillInstallPayload, SkillFetchError>>,
{
    let (user_dir, initial_missing) = {
        let guard = registry_read(registry);
        let missing = required_skills
            .into_iter()
            .filter(|name| !guard.has(name))
            .collect::<Vec<_>>();
        (guard.install_target_dir().to_path_buf(), missing)
    };

    let mut report = ChainInstallReport::default();
    let mut queue: VecDeque<String> = initial_missing.into_iter().collect();
    let mut queued_or_seen: HashSet<String> = queue.iter().cloned().collect();
    let mut attempted = 0usize;

    while let Some(dep_name) = queue.pop_front() {
        if !brassclaw_skills::validate_skill_name(&dep_name) {
            report
                .failed
                .push(format!("{}: invalid skill dependency name", dep_name));
            continue;
        }

        {
            let guard = registry_read(registry);
            if guard.has(&dep_name) {
                continue;
            }
        }

        if attempted >= MAX_CHAIN_DEPS {
            report.skipped.push(dep_name);
            continue;
        }

        attempted += 1;

        let download_url = brassclaw_skills::catalog::skill_download_url(registry_url, &dep_name);
        match fetcher(download_url).await {
            Ok(dep_bundle) => {
                let normalized = brassclaw_skills::normalize_line_endings(&dep_bundle.skill_md);
                match brassclaw_skills::registry::SkillRegistry::prepare_install_bundle_to_disk(
                    &user_dir,
                    &dep_name,
                    &normalized,
                    &dep_bundle.extra_files,
                    dep_bundle.install_metadata.as_ref(),
                )
                .await
                {
                    Ok((name, skill)) => {
                        if name != dep_name {
                            let orphan_dir = user_dir.join(&name);
                            if let Err(cleanup_err) = tokio::fs::remove_dir_all(&orphan_dir).await {
                                tracing::debug!(
                                    "chain install: failed to clean up mismatched-name dir {}: {}",
                                    orphan_dir.display(),
                                    cleanup_err
                                );
                            }
                            report.failed.push(format!(
                                "{}: manifest declares name '{}' (dependency-confusion guard)",
                                dep_name, name
                            ));
                            continue;
                        }
                        let nested_required = skill.manifest.requires.skills.clone();
                        enum CommitOutcome {
                            Installed,
                            Duplicate,
                            Failed(String),
                        }
                        let outcome: CommitOutcome = {
                            let mut guard = registry_write(registry);
                            if guard.has(&name) {
                                CommitOutcome::Duplicate
                            } else {
                                match guard.commit_install(&name, skill) {
                                    Ok(()) => CommitOutcome::Installed,
                                    Err(e) => CommitOutcome::Failed(e.to_string()),
                                }
                            }
                        };
                        match outcome {
                            CommitOutcome::Installed => {
                                report.installed.push(name);
                                if attempted < MAX_CHAIN_DEPS {
                                    for nested_dep in nested_required {
                                        if queue.len() >= MAX_CHAIN_QUEUE {
                                            tracing::warn!(
                                                "chain install: queue hit MAX_CHAIN_QUEUE={}; dropping further nested deps",
                                                MAX_CHAIN_QUEUE
                                            );
                                            break;
                                        }
                                        if queued_or_seen.insert(nested_dep.clone()) {
                                            queue.push_back(nested_dep);
                                        }
                                    }
                                }
                            }
                            CommitOutcome::Duplicate => {
                                let orphan_dir = user_dir.join(&name);
                                if let Err(cleanup_err) =
                                    tokio::fs::remove_dir_all(&orphan_dir).await
                                {
                                    tracing::debug!(
                                        "chain install: failed to clean up orphan skill dir {}: {}",
                                        orphan_dir.display(),
                                        cleanup_err
                                    );
                                }
                            }
                            CommitOutcome::Failed(e) => {
                                report.failed.push(format!("{}: {}", dep_name, e))
                            }
                        }
                    }
                    Err(e) => report.failed.push(format!("{}: {}", dep_name, e)),
                }
            }
            Err(e) => {
                if e.is_missing_dependency() {
                    report.missing.push(dep_name);
                } else {
                    report.failed.push(format!("{}: {}", dep_name, e));
                }
            }
        }
    }

    Ok(report)
}

async fn resolve_catalog_download_key(
    catalog: &SkillCatalog,
    name: &str,
    slug: Option<&str>,
) -> Result<String, SkillsCapabilityError> {
    if let Some(slug) = slug.filter(|s| !s.is_empty()) {
        return Ok(slug.to_string());
    }

    if name.contains('/') {
        return Ok(name.to_string());
    }

    let outcome = catalog.search(name).await;
    match resolve_catalog_slug_for_name(name, &outcome.results) {
        Ok(Some(resolved)) => Ok(resolved),
        Ok(None) => {
            let reason = outcome
                .error
                .unwrap_or_else(|| "no unique catalog match was found".to_string());
            Err(SkillsCapabilityError::operation(format!(
                "Could not resolve skill name '{}' to a catalog slug: {}",
                name, reason
            )))
        }
        Err(e) => Err(SkillsCapabilityError::operation(e.to_string())),
    }
}

pub async fn execute_skill_install(
    params: &Value,
    ctx: &SkillsContext,
) -> Result<Value, SkillsCapabilityError> {
    let name = require_str(params, "name")?;
    let install_dependencies = params
        .get("install_dependencies")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let loaded_required_skills = {
        let guard = registry_read(&ctx.registry);

        if let Some(loaded_skill) = guard.find_by_name(name) {
            let required_skills = loaded_skill.manifest.requires.skills.clone();
            if install_dependencies && !required_skills.is_empty() {
                Some(required_skills)
            } else {
                let report = ChainInstallReport::default();
                return Ok(build_already_installed_output(name, &report));
            }
        } else {
            None
        }
    };

    if let Some(required_skills) = loaded_required_skills {
        let chain_report = install_missing_skill_dependencies(
            &ctx.registry,
            ctx.catalog.registry_url(),
            required_skills,
            |url| async move { fetch_skill_payload(&url).await },
        )
        .await?;

        return Ok(build_already_installed_output(name, &chain_report));
    }

    let install_payload = if let Some(raw) = params.get("content").and_then(|v| v.as_str()) {
        SkillInstallPayload {
            skill_md: raw.to_string(),
            ..SkillInstallPayload::default()
        }
    } else if let Some(url) = params
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        fetch_skill_payload(url).await?
    } else {
        let download_key = resolve_catalog_download_key(
            ctx.catalog.as_ref(),
            name,
            slug.as_deref(),
        )
        .await?;
        let download_url = brassclaw_skills::catalog::skill_download_url(
            ctx.catalog.registry_url(),
            &download_key,
        );
        fetch_skill_payload(&download_url).await?
    };

    let normalized = brassclaw_skills::normalize_line_endings(&install_payload.skill_md);

    let (user_dir, skill_name_from_parse, install_content) = {
        let guard = registry_read(&ctx.registry);

        let (skill_name, install_content) =
            brassclaw_skills::registry::SkillRegistry::resolve_install_content(
                &normalized,
                slug.as_deref(),
            )
            .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?;

        if guard.has(&skill_name) {
            let report = ChainInstallReport::default();
            return Ok(build_already_installed_output(&skill_name, &report));
        }

        (
            guard.install_target_dir().to_path_buf(),
            skill_name,
            install_content,
        )
    };

    let (skill_name, loaded_skill) =
        brassclaw_skills::registry::SkillRegistry::prepare_install_bundle_to_disk(
            &user_dir,
            &skill_name_from_parse,
            &install_content,
            &install_payload.extra_files,
            install_payload.install_metadata.as_ref(),
        )
        .await
        .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?;

    enum CommitResult {
        Installed(String, Vec<String>),
        AlreadyInstalled,
    }
    let commit_result: CommitResult = {
        let mut guard = registry_write(&ctx.registry);
        if guard.has(&skill_name) {
            CommitResult::AlreadyInstalled
        } else {
            let reqs = loaded_skill.manifest.requires.clone();
            guard
                .commit_install(&skill_name, loaded_skill)
                .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?;
            CommitResult::Installed(skill_name, reqs.skills)
        }
    };

    let (installed_name, required_skills) = match commit_result {
        CommitResult::Installed(name, skills) => (name, skills),
        CommitResult::AlreadyInstalled => {
            let orphan_dir = user_dir.join(&skill_name_from_parse);
            if let Err(cleanup_err) = tokio::fs::remove_dir_all(&orphan_dir).await {
                tracing::debug!(
                    "skill_install: failed to clean up orphan skill dir {}: {}",
                    orphan_dir.display(),
                    cleanup_err
                );
            }
            return Ok(json!({
                "name": skill_name_from_parse,
                "status": "already_installed",
                "trust": "installed",
                "message": format!(
                    "Skill '{}' was already installed by a concurrent call — no install needed.",
                    skill_name_from_parse
                ),
            }));
        }
    };

    let chain_report = if required_skills.is_empty() {
        ChainInstallReport::default()
    } else if !install_dependencies {
        let missing_required_skills = {
            let guard = registry_read(&ctx.registry);
            required_skills
                .into_iter()
                .filter(|skill| !guard.has(skill))
                .collect::<Vec<_>>()
        };

        ChainInstallReport {
            pending_explicit_install: missing_required_skills,
            ..Default::default()
        }
    } else {
        install_missing_skill_dependencies(
            &ctx.registry,
            ctx.catalog.registry_url(),
            required_skills,
            |url| async move { fetch_skill_payload(&url).await },
        )
        .await?
    };

    Ok(build_skill_install_output(&installed_name, &chain_report))
}

pub async fn execute_skill_remove(
    params: &Value,
    ctx: &SkillsContext,
) -> Result<Value, SkillsCapabilityError> {
    let name = require_str(params, "name")?;

    let skill_path = {
        let guard = registry_read(&ctx.registry);
        guard
            .validate_remove(name)
            .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?
    };

    brassclaw_skills::registry::SkillRegistry::delete_skill_files(&skill_path)
        .await
        .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?;

    {
        let mut guard = registry_write(&ctx.registry);
        guard
            .commit_remove(name)
            .map_err(|e| SkillsCapabilityError::operation(e.to_string()))?;
    }

    Ok(json!({
        "name": name,
        "status": "removed",
        "message": format!("Skill '{}' has been removed.", name),
    }))
}

pub async fn execute_skill_list(
    params: &Value,
    ctx: &SkillsContext,
) -> Result<Value, SkillsCapabilityError> {
    let verbose = params
        .get("verbose")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let guard = registry_read(&ctx.registry);

    let skills: Vec<Value> = guard
        .skills()
        .iter()
        .map(|s| {
            let mut entry = json!({
                "name": s.manifest.name,
                "description": s.manifest.description,
                "trust": s.trust.to_string(),
                "source": format!("{:?}", s.source),
                "keywords": s.manifest.activation.keywords,
            });

            if verbose
                && let Some(obj) = entry.as_object_mut()
            {
                obj.insert(
                    "version".to_string(),
                    serde_json::Value::String(s.manifest.version.clone()),
                );
                obj.insert(
                    "tags".to_string(),
                    json!(s.manifest.activation.tags),
                );
                obj.insert(
                    "content_hash".to_string(),
                    serde_json::Value::String(s.content_hash.clone()),
                );
                obj.insert(
                    "max_context_tokens".to_string(),
                    json!(s.manifest.activation.max_context_tokens),
                );
            }

            entry
        })
        .collect();

    let count = skills.len();
    Ok(json!({
        "skills": skills,
        "count": count,
    }))
}

pub async fn execute_skill_search(
    params: &Value,
    ctx: &SkillsContext,
) -> Result<Value, SkillsCapabilityError> {
    let query = require_str(params, "query")?;

    let catalog_outcome = ctx.catalog.search(query).await;
    let catalog_error = catalog_outcome.error.clone();

    let mut catalog_entries = catalog_outcome.results;
    ctx.catalog
        .enrich_search_results(&mut catalog_entries, 5)
        .await;

    let installed_names: Vec<String> = {
        let guard = registry_read(&ctx.registry);
        guard
            .skills()
            .iter()
            .map(|s| s.manifest.name.clone())
            .collect()
    };

    let catalog_json: Vec<Value> = catalog_entries
        .iter()
        .map(|entry| {
            let is_installed =
                catalog_entry_is_installed(&entry.slug, &entry.name, &installed_names);
            json!({
                "slug": entry.slug,
                "name": entry.name,
                "description": entry.description,
                "version": entry.version,
                "score": entry.score,
                "installed": is_installed,
                "stars": entry.stars,
                "downloads": entry.downloads,
                "owner": entry.owner,
            })
        })
        .collect();

    let query_lower = query.to_lowercase();
    let local_matches: Vec<Value> = {
        let guard = registry_read(&ctx.registry);
        guard
            .skills()
            .iter()
            .filter(|s| {
                s.manifest.name.to_lowercase().contains(&query_lower)
                    || s.manifest.description.to_lowercase().contains(&query_lower)
                    || s.manifest
                        .activation
                        .keywords
                        .iter()
                        .any(|k| k.to_lowercase().contains(&query_lower))
            })
            .map(|s| {
                json!({
                    "name": s.manifest.name,
                    "description": s.manifest.description,
                    "trust": s.trust.to_string(),
                })
            })
            .collect()
    };

    let catalog_count = catalog_json.len();
    let installed_count = local_matches.len();
    let mut output = json!({
        "catalog": catalog_json,
        "catalog_count": catalog_count,
        "installed": local_matches,
        "installed_count": installed_count,
        "registry_url": ctx.catalog.registry_url(),
    });
    if let Some(err) = catalog_error {
        output["catalog_error"] = serde_json::Value::String(err);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_install_descriptor_is_valid() {
        let desc = skill_install_descriptor();
        assert_eq!(desc.id.as_str(), SKILL_INSTALL_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn skill_remove_descriptor_is_valid() {
        let desc = skill_remove_descriptor();
        assert_eq!(desc.id.as_str(), SKILL_REMOVE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn skill_list_descriptor_is_valid() {
        let desc = skill_list_descriptor();
        assert_eq!(desc.id.as_str(), SKILL_LIST_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn skill_search_descriptor_is_valid() {
        let desc = skill_search_descriptor();
        assert_eq!(desc.id.as_str(), SKILL_SEARCH_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn descriptors_returns_four() {
        let descs = descriptors();
        assert_eq!(descs.len(), 4);
        assert!(descs.iter().any(|d| d.id.as_str() == SKILL_INSTALL_CAPABILITY_ID));
        assert!(descs.iter().any(|d| d.id.as_str() == SKILL_REMOVE_CAPABILITY_ID));
        assert!(descs.iter().any(|d| d.id.as_str() == SKILL_LIST_CAPABILITY_ID));
        assert!(descs.iter().any(|d| d.id.as_str() == SKILL_SEARCH_CAPABILITY_ID));
    }

    #[test]
    fn skill_install_schema_has_required_name() {
        let desc = skill_install_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "name"));
        assert!(!required.iter().any(|v| v == "url"));
        assert!(!required.iter().any(|v| v == "content"));
    }

    #[test]
    fn skill_remove_schema_has_required_name() {
        let desc = skill_remove_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "name"));
    }

    #[test]
    fn skill_search_schema_has_required_query() {
        let desc = skill_search_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }
}
