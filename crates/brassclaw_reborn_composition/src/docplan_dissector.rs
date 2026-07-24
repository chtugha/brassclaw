//! DocPlan dissector — Phase 4 Step 4.3.
//!
//! Reads plan-library documents from the filesystem and decomposes each into:
//!
//! 1. A thin `monty`-class extension row in `reborn_extensions_unified` (the
//!    orchestration recipe body — preserves the plan type slug + steps as the
//!    extension payload for downstream Monty execution).
//!
//! 2. Zero or more `reborn_recipes` rows (class 21) for any trigger-annotated
//!    step patterns extracted from the plan document.
//!
//! The dissector is **idempotent**: it uses `upsert` on both tables keyed by
//! `(scope, name)` + `content_hash`, so repeated runs skip unchanged rows.
//!
//! # Plan document format
//!
//! ```text
//! # Plan Document
//!
//! **Type:** CodeGeneration
//! **Score:** 0.85
//! **Steps completed:** 3/3
//!
//! ## Steps
//!
//! 1. Read the file
//! 2. Apply the patch
//! 3. Write back
//! ```
//!
//! The dissector emits one `reborn_extensions_unified` row per plan document
//! (class = `monty`, payload = `{plan_type, score, steps: [...]}`).
//!
//! It also emits one `reborn_recipes` row per plan document as a
//! "keyword"-trigger Recipe whose keywords are drawn from the step text,
//! allowing the RecipeLookup to surface this plan to the agent loop via
//! keyword matching until the intent system is fully wired.
//!
//! # Scope
//!
//! All rows are inserted under the default local-dev scope
//! `(local, default, default, default)` which matches the scope used by the
//! existing plan library and RecipeLibrary adapters.

// Phase-5 postgres wiring — not yet called from factory; items unused until wiring lands.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_filesystem::RootFilesystem;
use brassclaw_host_api::VirtualPath;
use sha2::{Digest, Sha256};
use tracing::debug;

use crate::pg_recipe_store::{NewPgRecipe, PgRecipeStore};
use brassclaw_extensions::unified_store::{
    ExtensionClass, NewUnifiedExtension, PgUnifiedExtensionStore, UnifiedExtensionStore as _,
};

/// Default local-dev scope used for dissected plan rows.
const DEFAULT_TENANT: &str = "local";
const DEFAULT_USER: &str = "default";
const DEFAULT_AGENT: &str = "default";
const DEFAULT_PROJECT: &str = "default";

/// Virtual path prefix under which plan-library documents are stored.
const PLAN_LIBRARY_ROOT: &str =
    "/workspace/reborn-cli/users/reborn-cli/projects/_none/skills/.plan-library";

/// DocPlan dissector service.
///
/// Reads plan documents from the virtual filesystem and writes the dissected
/// rows to `reborn_extensions_unified` + `reborn_recipes`.
pub(crate) struct DocPlanDissector<F: RootFilesystem + ?Sized> {
    filesystem: Arc<F>,
    unified_store: PgUnifiedExtensionStore,
    recipe_store: PgRecipeStore,
}

impl<F: RootFilesystem + ?Sized + 'static> DocPlanDissector<F> {
    pub(crate) fn new(
        filesystem: Arc<F>,
        unified_store: PgUnifiedExtensionStore,
        recipe_store: PgRecipeStore,
    ) -> Self {
        Self {
            filesystem,
            unified_store,
            recipe_store,
        }
    }

    /// Walk all plan-type directories under `PLAN_LIBRARY_ROOT`, parse each
    /// `.md` file, and upsert the dissected rows.  Errors are logged and
    /// swallowed per-document so a single malformed file does not abort the
    /// whole dissection pass.
    pub(crate) async fn dissect_all(&self) {
        let root = match VirtualPath::new(PLAN_LIBRARY_ROOT.to_string()) {
            Ok(p) => p,
            Err(e) => {
                debug!(%e, "docplan_dissector: invalid plan library root path");
                return;
            }
        };
        let entries = match self.filesystem.list_dir(&root).await {
            Ok(e) => e,
            Err(_) => {
                // Plan library directory doesn't exist yet — nothing to dissect.
                return;
            }
        };
        for plan_type_entry in entries {
            if plan_type_entry.file_type != brassclaw_filesystem::FileType::Directory {
                continue;
            }
            let type_path = match VirtualPath::new(format!(
                "{}/{}",
                PLAN_LIBRARY_ROOT, plan_type_entry.name
            )) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let plan_files = match self.filesystem.list_dir(&type_path).await {
                Ok(f) => f,
                Err(_) => continue,
            };
            for file_entry in plan_files {
                if !file_entry.name.ends_with(".md") {
                    continue;
                }
                let file_path = match VirtualPath::new(format!(
                    "{}/{}/{}",
                    PLAN_LIBRARY_ROOT, plan_type_entry.name, file_entry.name
                )) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let content = match self.filesystem.read_file(&file_path).await {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                };
                let slug = file_entry.name.trim_end_matches(".md").to_string();
                let plan_type = plan_type_entry.name.clone();
                if let Err(e) = self.dissect_document(&plan_type, &slug, &content).await {
                    debug!(
                        plan_type = %plan_type,
                        slug = %slug,
                        error = %e,
                        "docplan_dissector: failed to dissect plan document"
                    );
                }
            }
        }
    }

    async fn dissect_document(
        &self,
        plan_type: &str,
        slug: &str,
        content: &str,
    ) -> Result<(), String> {
        let parsed = parse_plan_document(content);
        let hash = content_hash(content);

        // 1. Emit a monty-class extension row (thin orchestration body).
        let ext_name = format!("plan-{}-{}", plan_type, slug)
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>();
        let ext_name = sanitize_name(&ext_name);
        let ext_payload = serde_json::json!({
            "plan_type": plan_type,
            "slug": slug,
            "score": parsed.score,
            "steps": parsed.steps,
            "body": content,
        });
        // Default consumer tags for Monty + 05:validator (pending validation).
        let monty_tags = vec![
            "01:monty".to_string(),
            "02:orchestrator".to_string(),
            "05:validator".to_string(),
        ];
        let new_ext = NewUnifiedExtension {
            tenant_id: DEFAULT_TENANT.to_string(),
            user_id: DEFAULT_USER.to_string(),
            agent_id: DEFAULT_AGENT.to_string(),
            project_id: DEFAULT_PROJECT.to_string(),
            name: ext_name.clone(),
            description: format!(
                "{} plan ({} steps)",
                plan_type_label(plan_type),
                parsed.steps.len()
            ),
            class: ExtensionClass::Monty,
            payload: ext_payload,
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: monty_tags,
            intent_examples: None,
            source: "plan_library".to_string(),
        };
        self.unified_store
            .upsert(new_ext, &hash)
            .await
            .map_err(|e| format!("unified_store upsert extension: {e}"))?;
        debug!(
            plan_type,
            slug,
            name = %ext_name,
            "docplan_dissector: emitted monty-class extension row"
        );

        // 2. Emit a recipe row (class 21) with keyword trigger drawn from steps.
        let recipe_name = format!("plan-recipe-{}-{}", plan_type, slug);
        let recipe_name = sanitize_name(&recipe_name);
        let keywords: Vec<String> = parsed
            .steps
            .iter()
            .flat_map(|s| {
                s.split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(|w| w.to_lowercase())
                    .collect::<Vec<_>>()
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .take(20) // cap keywords to 20 per agentskills.io cardinality rule
            .collect();
        let trigger = if keywords.is_empty() {
            None
        } else {
            Some(serde_json::json!({
                "type": "keyword",
                "payload": keywords
            }))
        };
        let steps_json: Vec<serde_json::Value> = parsed
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                serde_json::json!({
                    "step": i + 1,
                    "skill": ext_name,
                    "tool": "monty_call",
                    "params": { "instruction": step },
                    "description": step
                })
            })
            .collect();
        // Default consumer tags for Recipe (class 21) + 05:validator.
        let recipe_tags = vec![
            "02:orchestrator".to_string(),
            "03:llm".to_string(),
            "05:validator".to_string(),
        ];
        let new_recipe = NewPgRecipe {
            tenant_id: DEFAULT_TENANT.to_string(),
            user_id: DEFAULT_USER.to_string(),
            agent_id: DEFAULT_AGENT.to_string(),
            project_id: DEFAULT_PROJECT.to_string(),
            name: recipe_name.clone(),
            description: format!(
                "Auto-dissected plan: {} ({} steps, score {:.2})",
                plan_type_label(plan_type),
                parsed.steps.len(),
                parsed.score
            ),
            trigger,
            steps: serde_json::Value::Array(steps_json),
            prior_knowledge_content: Some(content.to_string()),
            override_prompt_creation: false,
            consumer_tags: recipe_tags,
            intent_examples: None,
            source: "plan_library".to_string(),
        };
        self.recipe_store
            .upsert(new_recipe, &hash)
            .await
            .map_err(|e| format!("recipe_store upsert: {e}"))?;
        debug!(
            plan_type,
            slug,
            name = %recipe_name,
            "docplan_dissector: emitted class-21 recipe row"
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Plan document parser
// ---------------------------------------------------------------------------

struct ParsedPlan {
    score: f64,
    steps: Vec<String>,
}

fn parse_plan_document(content: &str) -> ParsedPlan {
    let mut score = 0.0_f64;
    let mut steps: Vec<String> = Vec::new();
    let mut in_steps = false;

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("**Score:**") {
            if let Ok(s) = rest.trim().parse::<f64>() {
                score = s;
            }
        } else if line == "## Steps" {
            in_steps = true;
        } else if line.starts_with("## ") {
            // Another section heading — stop collecting steps.
            in_steps = false;
        } else if in_steps && !line.is_empty() {
            // Strip leading numbering "N. " if present.
            let step_text = if let Some(dot_pos) = line.find(". ") {
                let prefix = &line[..dot_pos];
                if prefix.chars().all(|c| c.is_ascii_digit()) {
                    &line[dot_pos + 2..]
                } else {
                    line
                }
            } else {
                line
            };
            if !step_text.is_empty() {
                steps.push(step_text.to_string());
            }
        }
    }

    ParsedPlan { score, steps }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute a SHA-256 content hash (hex-encoded).
fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Convert a plan_type directory name to a human-readable label.
fn plan_type_label(plan_type: &str) -> &str {
    match plan_type {
        "code-generation" => "Code Generation",
        "file-operation" => "File Operation",
        "shell-task" => "Shell Task",
        "research" => "Research",
        "generic" => "Generic",
        other => other,
    }
}

/// Sanitize a name string to match the `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$` pattern.
fn sanitize_name(name: &str) -> String {
    let mut result: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Trim leading/trailing dashes.
    result = result.trim_matches('-').to_string();
    // Collapse consecutive dashes.
    while result.contains("--") {
        result = result.replace("--", "-");
    }
    // Truncate to 64 chars.
    result.truncate(64);
    result.trim_end_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plan_document_extracts_score_and_steps() {
        let content = "# Plan Document\n\n\
            **Type:** CodeGeneration\n\
            **Score:** 0.85\n\
            **Steps completed:** 3/3\n\n\
            ## Steps\n\n\
            1. Read the file\n\
            2. Apply the patch\n\
            3. Write back\n";
        let plan = parse_plan_document(content);
        assert!((plan.score - 0.85).abs() < 1e-9);
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0], "Read the file");
        assert_eq!(plan.steps[2], "Write back");
    }

    #[test]
    fn parse_plan_document_empty_steps_section() {
        let content = "# Plan Document\n\n**Score:** 0.5\n\n## Steps\n\n## Other\n\nfoo\n";
        let plan = parse_plan_document(content);
        assert!((plan.score - 0.5).abs() < 1e-9);
        assert!(plan.steps.is_empty());
    }

    #[test]
    fn content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        let h3 = content_hash("hello world!");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn sanitize_name_removes_special_chars() {
        assert_eq!(sanitize_name("Plan Type A!"), "plan-type-a");
        assert_eq!(sanitize_name("--foo--bar--"), "foo-bar");
        assert_eq!(sanitize_name("code_generation"), "code-generation");
    }
}
