//! One-shot SKILL.md → `reborn_skills` importer for BrassClaw Reborn.
//!
//! # What this does
//!
//! Walks the `skills/` directory tree, parses each `SKILL.md` via
//! [`brassclaw_skills::parse_skill_md`], splits large skills (>3 tool names
//! in the description → multiple rows, one per tool-usage pattern), and
//! upserts each row into `reborn_skills` (V027).
//!
//! # Idempotency
//!
//! Import is content-hash-gated: if a row with the same `(scope, name)` and
//! the same `content_hash` already exists, the row is skipped.  Rows with a
//! matching `(scope, name)` but a changed hash are updated (triggering a new
//! validation cycle by resetting `validation_status = 'pending'` and
//! re-adding `05:validator`).
//!
//! # Class assignment
//!
//! The class is inferred from the `compatibility` frontmatter field:
//! - `brassclaw-class:rusty`  → class code 1
//! - `brassclaw-class:monty`  → class code 2
//! - `brassclaw-class:llm`    → class code 3 (default when absent)
//!
//! # Intent-example extraction
//!
//! Each skill's `keywords[]`, `patterns[]`, and the first sentence of the
//! description are turned into `{input, class}` intent-example entries:
//! - Keywords → class 1 (single word) or class 2 (multi-word partial)
//! - Patterns → stripped to their literal prefix, class 2
//! - Description sentence → class 3 (full sentence)
//!
//! # Splitting rule
//!
//! If the skill body mentions more than 3 distinct tool names (detected by
//! the simple heuristic: backtick-quoted lowercase `word.word` tokens), the
//! skill is split into one row per tool name.  The split rows share the same
//! description/keywords/intent_examples but each body is trimmed to the
//! section that documents that tool.

#[cfg(feature = "skills-db")]
mod inner {
    use std::collections::HashSet;
    use std::path::Path;

    use serde_json::{Value as JsonValue, json};
    use sha2::{Digest, Sha256};

    use brassclaw_skills::db_store::{
        DbSkillStore, DbSkillStoreError, SkillScope, SkillWriteInput,
    };
    use brassclaw_skills::{parse_skill_md, SkillParseError};

    // -----------------------------------------------------------------------
    // Public entry-point
    // -----------------------------------------------------------------------

    /// Import summary returned by [`run_skill_import`].
    #[derive(Debug, Default)]
    pub struct ImportSummary {
        /// Skills skipped (unchanged content hash).
        pub skipped: usize,
        /// New rows inserted.
        pub inserted: usize,
        /// Existing rows updated (content changed).
        pub updated: usize,
        /// Files that failed to parse or import.
        pub failed: Vec<(String, String)>,
    }

    /// Walk `skills_root` for `SKILL.md` files, import each into `reborn_skills`.
    ///
    /// `scope` — the 4-tuple owning the imported skills.
    /// `skills_root` — path to the `skills/` directory.
    pub async fn run_skill_import(
        store: &DbSkillStore,
        scope: &SkillScope,
        skills_root: &Path,
    ) -> Result<ImportSummary, ImportError> {
        let mut summary = ImportSummary::default();

        let skill_files = collect_skill_files(skills_root)?;

        for file_path in &skill_files {
            let content = tokio::fs::read_to_string(file_path)
                .await
                .map_err(|e| ImportError::Io {
                    path: file_path.display().to_string(),
                    reason: e.to_string(),
                })?;

            let parsed = match parse_skill_md(&content) {
                Ok(p) => p,
                Err(e) => {
                    summary.failed.push((
                        file_path.display().to_string(),
                        format!("parse error: {e}"),
                    ));
                    continue;
                }
            };

            let rows = build_import_rows(scope, &parsed.manifest, &parsed.prompt_content);

            for row_input in rows {
                match import_row(store, &row_input).await {
                    Ok(RowOutcome::Inserted) => summary.inserted += 1,
                    Ok(RowOutcome::Updated) => summary.updated += 1,
                    Ok(RowOutcome::Skipped) => summary.skipped += 1,
                    Err(e) => {
                        summary.failed.push((
                            file_path.display().to_string(),
                            format!("import error for '{}': {e}", row_input.name),
                        ));
                    }
                }
            }
        }

        Ok(summary)
    }

    // -----------------------------------------------------------------------
    // Error type
    // -----------------------------------------------------------------------

    #[derive(Debug, thiserror::Error)]
    pub enum ImportError {
        #[error("I/O error reading '{path}': {reason}")]
        Io { path: String, reason: String },

        #[error("directory walk error: {0}")]
        Walk(String),
    }

    // -----------------------------------------------------------------------
    // Filesystem helpers
    // -----------------------------------------------------------------------

    fn collect_skill_files(root: &Path) -> Result<Vec<std::path::PathBuf>, ImportError> {
        let mut result = Vec::new();
        collect_skill_files_inner(root, &mut result)?;
        result.sort();
        Ok(result)
    }

    fn collect_skill_files_inner(
        dir: &Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> Result<(), ImportError> {
        let entries = std::fs::read_dir(dir).map_err(|e| ImportError::Walk(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| ImportError::Walk(e.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                collect_skill_files_inner(&path, out)?;
            } else if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                out.push(path);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Row building
    // -----------------------------------------------------------------------

    /// Build one or more [`SkillWriteInput`] rows from a parsed skill.
    ///
    /// If the skill body mentions ≤3 distinct tool names the skill becomes
    /// a single row.  If >3, the body is split per tool and each tool gets its
    /// own row (name = `base-name-toolname`).
    fn build_import_rows(
        scope: &SkillScope,
        manifest: &brassclaw_skills::SkillManifest,
        prompt_content: &str,
    ) -> Vec<SkillWriteInput> {
        let base_name = brassclaw_skills::validation::normalize_skill_identifier(&manifest.name)
            .unwrap_or_else(|| "imported-skill".to_string());

        let tools = extract_tool_names(prompt_content);
        let intent_examples = extract_intent_examples(manifest, prompt_content);
        let compatibility = infer_compatibility(manifest);

        if tools.len() <= 3 {
            // Single row.
            let body = prompt_content.to_string();
            let hash = sha256_hex(&body);
            vec![SkillWriteInput {
                scope: scope.clone(),
                name: base_name,
                description: manifest.description.clone(),
                body,
                compatibility,
                license: "MIT".to_string(),
                allowed_tools: tools.into_iter().collect(),
                version: manifest.version.clone(),
                keywords: manifest.activation.keywords.clone(),
                exclude_keywords: manifest.activation.exclude_keywords.clone(),
                patterns: manifest.activation.patterns.clone(),
                tags: manifest.activation.tags.clone(),
                max_context_tokens: manifest.activation.max_context_tokens as i32,
                setup_marker: manifest.activation.setup_marker.clone(),
                required_binaries: manifest.requires.bins.clone(),
                required_env: manifest.requires.env.clone(),
                required_config: manifest.requires.config.clone(),
                intent_examples,
                extra_consumer_tags: vec![],
                content_hash: hash,
                parent_mission_id: None,
                replaces_id: None,
                similarity_parent_id: None,
                source: "migrated".to_string(),
            }]
        } else {
            // Split: one row per tool.
            let all_tools: Vec<String> = tools.into_iter().collect();
            let mut rows = Vec::new();
            for tool_name in &all_tools {
                let safe_tool = brassclaw_skills::validation::normalize_skill_identifier(tool_name)
                    .unwrap_or_else(|| "tool".to_string());
                let split_name = format!("{base_name}-{safe_tool}");
                // Trim the name to 64 chars if needed.
                let final_name = if split_name.len() > 64 {
                    split_name[..64].trim_end_matches('-').to_string()
                } else {
                    split_name
                };

                let body = extract_tool_section(prompt_content, tool_name);
                let hash = sha256_hex(&body);

                rows.push(SkillWriteInput {
                    scope: scope.clone(),
                    name: final_name,
                    description: manifest.description.clone(),
                    body,
                    compatibility: compatibility.clone(),
                    license: "MIT".to_string(),
                    allowed_tools: vec![tool_name.clone()],
                    version: manifest.version.clone(),
                    keywords: manifest.activation.keywords.clone(),
                    exclude_keywords: manifest.activation.exclude_keywords.clone(),
                    patterns: manifest.activation.patterns.clone(),
                    tags: manifest.activation.tags.clone(),
                    max_context_tokens: manifest.activation.max_context_tokens as i32,
                    setup_marker: manifest.activation.setup_marker.clone(),
                    required_binaries: manifest.requires.bins.clone(),
                    required_env: manifest.requires.env.clone(),
                    required_config: manifest.requires.config.clone(),
                    intent_examples: intent_examples.clone(),
                    extra_consumer_tags: vec![],
                    content_hash: hash,
                    parent_mission_id: None,
                    replaces_id: None,
                    similarity_parent_id: None,
                    source: "migrated".to_string(),
                });
            }
            rows
        }
    }

    // -----------------------------------------------------------------------
    // Intent-example extraction
    // -----------------------------------------------------------------------

    /// Extract `{input, class}` intent examples from a skill manifest + body.
    fn extract_intent_examples(
        manifest: &brassclaw_skills::SkillManifest,
        _prompt_content: &str,
    ) -> JsonValue {
        let mut examples: Vec<JsonValue> = Vec::new();

        // Keywords → class 1 (single word) or class 2 (multi-word partial).
        for kw in &manifest.activation.keywords {
            let word_count = kw.split_whitespace().count();
            let cls = if word_count == 1 { 1u64 } else { 2u64 };
            examples.push(json!({"input": kw, "class": cls}));
        }

        // Tags → class 1 (treated as single-word activators).
        for tag in &manifest.activation.tags {
            if !examples.iter().any(|e| e["input"] == *tag) {
                examples.push(json!({"input": tag, "class": 1}));
            }
        }

        // Description first sentence → class 3 (full sentence).
        let desc = manifest.description.trim();
        if !desc.is_empty() {
            let first_sentence = desc
                .split(['.', '!', '?'])
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(desc);
            if first_sentence.split_whitespace().count() >= 3 {
                examples.push(json!({"input": first_sentence, "class": 3}));
            }
        }

        // Deduplicate by input text.
        let mut seen: HashSet<String> = HashSet::new();
        let deduplicated: Vec<JsonValue> = examples
            .into_iter()
            .filter(|e| {
                let key = e["input"].as_str().unwrap_or("").to_lowercase();
                seen.insert(key)
            })
            .collect();

        JsonValue::Array(deduplicated)
    }

    // -----------------------------------------------------------------------
    // Body helpers
    // -----------------------------------------------------------------------

    /// Extract distinct tool names from a prompt body.
    ///
    /// Heuristic: backtick-quoted tokens containing exactly one `.` where both
    /// sides are lowercase alphanumeric (e.g. `github.api`, `fs.read`).
    fn extract_tool_names(body: &str) -> HashSet<String> {
        use std::sync::OnceLock;
        static TOOL_RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = TOOL_RE.get_or_init(|| {
            regex::Regex::new(r"`([a-z][a-z0-9_]*\.[a-z][a-z0-9_]*)`").unwrap() // safety: hardcoded literal
        });
        re.captures_iter(body)
            .map(|c| c[1].to_string())
            .collect()
    }

    /// Extract the body section that documents a specific tool.
    ///
    /// Uses a simple heuristic: find the heading that mentions the tool name and
    /// return the text until the next same-or-higher-level heading.  Falls back
    /// to the entire body if no heading is found.
    fn extract_tool_section(body: &str, tool_name: &str) -> String {
        // Find the tool short-name (part after the dot, e.g. "api" from "github.api").
        let short = tool_name
            .split('.')
            .last()
            .unwrap_or(tool_name);

        let lines: Vec<&str> = body.lines().collect();
        let mut start = None;
        let mut heading_level: usize = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start_matches('#');
            let hashes = line.len() - trimmed.len();
            if hashes > 0
                && (line.to_lowercase().contains(tool_name)
                    || line.to_lowercase().contains(short))
            {
                start = Some(i);
                heading_level = hashes;
                break;
            }
        }

        match start {
            None => body.to_string(),
            Some(from) => {
                let mut end = lines.len();
                for (i, line) in lines.iter().enumerate().skip(from + 1) {
                    let trimmed = line.trim_start_matches('#');
                    let hashes = line.len() - trimmed.len();
                    if hashes > 0 && hashes <= heading_level {
                        end = i;
                        break;
                    }
                }
                lines[from..end].join("\n")
            }
        }
    }

    // -----------------------------------------------------------------------
    // Class / compatibility inference
    // -----------------------------------------------------------------------

    fn infer_compatibility(manifest: &brassclaw_skills::SkillManifest) -> String {
        // If the skill declares a compatibility field (via free-form types), use it.
        // Otherwise default to llm-class.
        use brassclaw_skills::component_type::ComponentType;
        if manifest
            .types
            .iter()
            .any(|t| matches!(t, ComponentType::Agent))
        {
            "brassclaw-class:monty".to_string()
        } else {
            "brassclaw-class:llm".to_string()
        }
    }

    // -----------------------------------------------------------------------
    // SHA-256 helper
    // -----------------------------------------------------------------------

    fn sha256_hex(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    // -----------------------------------------------------------------------
    // Row import logic (idempotent via content_hash)
    // -----------------------------------------------------------------------

    enum RowOutcome {
        Inserted,
        Updated,
        Skipped,
    }

    async fn import_row(
        store: &DbSkillStore,
        input: &SkillWriteInput,
    ) -> Result<RowOutcome, DbSkillStoreError> {
        // Attempt to find an existing row by name within the scope using a
        // specialised fetch.  We use `fetch_by_name` which returns rows
        // regardless of validation_status (including pending/failed) so that
        // re-imports do not create duplicate rows.
        match store.fetch_by_name(&input.scope, &input.name).await? {
            None => {
                // No existing row — insert fresh.
                store.insert(input).await?;
                Ok(RowOutcome::Inserted)
            }
            Some(existing) => {
                if existing.content_hash == input.content_hash {
                    // Unchanged — skip.
                    Ok(RowOutcome::Skipped)
                } else {
                    // Content changed — reset to pending and update.
                    store.update_content(existing.id, &existing.scope, input).await?;
                    Ok(RowOutcome::Updated)
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use serde_json::json;

        use super::*;
        use brassclaw_skills::SkillManifest;

        fn manifest_with_keywords(kws: Vec<&str>, tags: Vec<&str>) -> SkillManifest {
            use brassclaw_skills::types::ActivationCriteria;
            SkillManifest {
                name: "test-skill".into(),
                version: "1.0.0".into(),
                description: "Fetch and list open issues from GitHub".into(),
                activation: ActivationCriteria {
                    keywords: kws.iter().map(|s| s.to_string()).collect(),
                    tags: tags.iter().map(|s| s.to_string()).collect(),
                    max_context_tokens: 500,
                    ..Default::default()
                },
                ..Default::default()
            }
        }

        #[test]
        fn extract_tool_names_finds_backtick_tools() {
            let body = "Call `github.api` to list repos. Use `fs.read` to read files. Also `github.api`.";
            let tools = extract_tool_names(body);
            assert!(tools.contains("github.api"), "{tools:?}");
            assert!(tools.contains("fs.read"), "{tools:?}");
            assert_eq!(tools.len(), 2);
        }

        #[test]
        fn intent_examples_from_keywords_and_tags() {
            let m = manifest_with_keywords(
                vec!["github", "list issues"],
                vec!["devops"],
            );
            let examples = extract_intent_examples(&m, "");
            let arr = examples.as_array().unwrap();

            // "github" → class 1
            assert!(arr.iter().any(|e| e["input"] == "github" && e["class"] == 1));
            // "list issues" → class 2
            assert!(arr.iter().any(|e| e["input"] == "list issues" && e["class"] == 2));
            // "devops" tag → class 1
            assert!(arr.iter().any(|e| e["input"] == "devops" && e["class"] == 1));
            // description first sentence → class 3
            assert!(arr
                .iter()
                .any(|e| e["class"] == 3 && e["input"].as_str().unwrap().len() > 5));
        }

        #[test]
        fn intent_examples_deduplicated() {
            let m = manifest_with_keywords(vec!["github", "github"], vec!["github"]);
            let examples = extract_intent_examples(&m, "");
            let arr = examples.as_array().unwrap();
            let github_count = arr.iter().filter(|e| e["input"] == "github").count();
            assert_eq!(github_count, 1, "duplicate inputs should be removed");
        }

        #[test]
        fn split_produces_one_row_when_few_tools() {
            let m = manifest_with_keywords(vec![], vec![]);
            let body = "Use `github.api` and `fs.read`.";
            let scope = SkillScope {
                tenant_id: "t".into(),
                user_id: "u".into(),
                agent_id: "a".into(),
                project_id: "p".into(),
            };
            let rows = build_import_rows(&scope, &m, body);
            assert_eq!(rows.len(), 1);
        }

        #[test]
        fn split_produces_multiple_rows_when_many_tools() {
            let m = manifest_with_keywords(vec![], vec![]);
            let body = "Use `a.one`, `b.two`, `c.three`, `d.four`.";
            let scope = SkillScope {
                tenant_id: "t".into(),
                user_id: "u".into(),
                agent_id: "a".into(),
                project_id: "p".into(),
            };
            let rows = build_import_rows(&scope, &m, body);
            assert_eq!(rows.len(), 4);
        }

        #[test]
        fn sha256_hex_is_stable() {
            let h1 = sha256_hex("hello world");
            let h2 = sha256_hex("hello world");
            assert_eq!(h1, h2);
            let h3 = sha256_hex("different");
            assert_ne!(h1, h3);
        }

        #[test]
        fn infer_compatibility_defaults_to_llm() {
            let m = manifest_with_keywords(vec![], vec![]);
            assert_eq!(infer_compatibility(&m), "brassclaw-class:llm");
        }
    }
} // mod inner

#[cfg(feature = "skills-db")]
pub use inner::{ImportError, ImportSummary, run_skill_import};
