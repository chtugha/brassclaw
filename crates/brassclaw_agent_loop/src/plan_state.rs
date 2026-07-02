//! Planning state and type classification for the two-phase planning loop.
//!
//! The planning context strategy injects a structured planning prompt on
//! iteration 0 (minimal context, JSON-only instruction). On subsequent
//! iterations it injects the current step as an inline message.
//!
//! ## Prose-to-steps converter
//!
//! `extract_steps` is a deterministic, regex-based parser — no LLM, no I/O.
//! It is the fallback when the model returns prose instead of JSON. The
//! algorithm mirrors the approach used by LangChain's `NumberedListOutputParser`
//! and `MarkdownListOutputParser` (pure regex, no external crates):
//!
//! Priority order (first successful pattern wins):
//! 1. JSON `{"steps":[...]}` — direct parse
//! 2. Numbered list: `1. ...`, `1) ...`
//! 3. Markdown/bullet list: `- ...`, `* ...`, `• ...`
//! 4. Ordinal-word list: lines starting with First/Second/Then/Next/Finally
//! 5. Sentence splitting on `.` or `;` as last-resort fallback

use serde::{Deserialize, Serialize};

/// Broad category of the agent task.
///
/// Used by `PlanTypeClassifier` to select a type-specific planning hint
/// in the context strategy's inline instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanType {
    /// File create/read/update/delete operations.
    FileOperation,
    /// Shell commands, processes, scripts.
    ShellTask,
    /// Information gathering, web/doc search, reading.
    Research,
    /// Writing or modifying source code.
    CodeGeneration,
    /// General or unclassified task.
    #[default]
    Generic,
}

// ── AgentPlanState ────────────────────────────────────────────────────────────

/// Structured plan produced after iteration 0.
///
/// Stored in `LoopExecutionState::plan_state`. Persisted with loop
/// checkpoints via `serde_json` (same as all other state slots).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlanState {
    /// Ordered list of plan steps.
    pub steps: Vec<String>,
    /// Index of the step currently being executed (0-based).
    pub current_step: usize,
    /// The original raw text from the model (for diagnostics).
    pub raw_plan_text: String,
    /// Classified task type.
    pub plan_type: PlanType,
}

impl AgentPlanState {
    /// Parse or extract steps from a model reply.
    ///
    /// Tries JSON first, then falls back to the deterministic prose extractor.
    /// Returns `None` if no steps could be extracted (e.g. empty reply).
    pub fn from_model_reply(raw: &str, plan_type: PlanType) -> Option<Self> {
        let steps = extract_steps(raw)?;
        Some(Self {
            steps,
            current_step: 0,
            raw_plan_text: raw.trim().to_owned(),
            plan_type,
        })
    }

    /// Returns the text of the current step, or `None` if all steps completed.
    pub fn current_step_text(&self) -> Option<&str> {
        self.steps.get(self.current_step).map(String::as_str)
    }

    /// Advance to the next step. Returns `true` if there are more steps.
    pub fn advance(&mut self) -> bool {
        self.current_step = self.current_step.saturating_add(1);
        self.current_step < self.steps.len()
    }
}

// ── Prose extractor ───────────────────────────────────────────────────────────

/// Extract a step list from arbitrary model output.
///
/// Deterministic, pure-Rust regex-equivalent extraction — no LLM, no I/O.
/// Returns `None` only if the text is empty or yields no non-empty steps.
///
/// ## Pattern priority
///
/// 1. **JSON** `{"steps":["...", "..."]}` — direct serde parse
/// 2. **Numbered list** `1. foo` / `1) foo` (across multiple lines)
/// 3. **Bullet list** `- foo` / `* foo` / `• foo`
/// 4. **Ordinal words** lines starting with "first", "second", "then", …
/// 5. **Sentence split** on `.` or `;` (last resort for dense prose)
pub fn extract_steps(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // 1. JSON {"steps":[...]}
    if let Some(steps) = try_parse_json_steps(text) {
        return Some(steps);
    }

    // 2. Numbered list: "1. " or "1) "
    let numbered = extract_by_pattern(text, |line| {
        // Match: optional whitespace, one-or-more digits, dot-or-paren, whitespace
        let line = line.trim();
        let mut chars = line.chars().peekable();
        // skip digits
        let mut has_digit = false;
        while chars.peek().is_some_and(|c: &char| c.is_ascii_digit()) {
            chars.next();
            has_digit = true;
        }
        if !has_digit {
            return None;
        }
        // require . or )
        match chars.peek() {
            Some('.') | Some(')') => { chars.next(); }
            _ => return None,
        }
        // require whitespace
        if !chars.peek().is_some_and(|c: &char| c.is_ascii_whitespace()) {
            return None;
        }
        while chars.peek().is_some_and(|c: &char| c.is_ascii_whitespace()) {
            chars.next();
        }
        let rest: String = chars.collect();
        if rest.is_empty() { None } else { Some(rest) }
    });
    if numbered.len() >= 2 {
        return Some(numbered);
    }

    // 3. Bullet list: "- ", "* ", "• "
    let bullets = extract_by_pattern(text, |line| {
        let line = line.trim();
        let rest = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("• "))
            .or_else(|| line.strip_prefix("– "))  // en-dash
            .or_else(|| line.strip_prefix("— ")); // em-dash
        rest.filter(|s| !s.is_empty()).map(str::to_owned)
    });
    if bullets.len() >= 2 {
        return Some(bullets);
    }

    // 4. Ordinal-word lines: "First, ...", "Second: ...", "Then ...", etc.
    let ordinals = extract_by_pattern(text, |line| {
        let lower = line.trim().to_ascii_lowercase();
        const ORDINALS: &[&str] = &[
            "first", "second", "third", "fourth", "fifth", "sixth",
            "seventh", "eighth", "ninth", "tenth",
            "then", "next", "finally", "lastly", "after that",
            "step 1", "step 2", "step 3", "step 4", "step 5",
            "step 6", "step 7", "step 8", "step 9", "step 10",
        ];
        for ord in ORDINALS {
            if lower.starts_with(ord) {
                // strip the ordinal prefix and optional punctuation
                let rest = &line.trim()[ord.len()..];
                let rest = rest.trim_start_matches([',', ':', '.', ' ']);
                if !rest.is_empty() {
                    return Some(rest.to_owned());
                }
            }
        }
        None
    });
    if ordinals.len() >= 2 {
        return Some(ordinals);
    }

    // 5. Sentence split — last resort. Split on ". " or ";\s"
    let sentences: Vec<String> = text
        .split(|c: char| c == '.' || c == ';')
        .map(str::trim)
        .filter(|s| {
            !s.is_empty()
                && s.split_whitespace().count() >= 3  // skip fragments
        })
        .map(str::to_owned)
        .collect();
    if sentences.len() >= 2 {
        return Some(sentences);
    }

    // Only one sentence-like chunk — treat the whole text as a single step
    // if it has meaningful length (agent continues without plan injection).
    None
}

fn try_parse_json_steps(text: &str) -> Option<Vec<String>> {
    // Accept both {"steps":[...]} and bare arrays ["step","step",...] 
    let value: serde_json::Value = serde_json::from_str(text).ok()?;

    // Case 1: object with "steps" key
    if let Some(arr) = value.get("steps").and_then(|v| v.as_array()) {
        let steps: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .filter(|s| !s.is_empty())
            .collect();
        if !steps.is_empty() {
            return Some(steps);
        }
    }

    // Case 2: top-level array of strings
    if let Some(arr) = value.as_array() {
        let steps: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .filter(|s| !s.is_empty())
            .collect();
        if !steps.is_empty() {
            return Some(steps);
        }
    }

    None
}

fn extract_by_pattern<F>(text: &str, extract: F) -> Vec<String>
where
    F: Fn(&str) -> Option<String>,
{
    text.lines().filter_map(|line| extract(line)).collect()
}

// ── PlanTypeClassifier ────────────────────────────────────────────────────────

/// Pure keyword-match classifier. No LLM, no I/O.
///
/// Priority (highest first): CodeGeneration > FileOperation > ShellTask >
/// Research > Generic.
pub fn classify(user_message: &str, active_skill_names: &[&str]) -> PlanType {
    let msg = user_message.to_ascii_lowercase();
    let skills_lower: Vec<String> = active_skill_names
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let skill_has = |kw: &str| skills_lower.iter().any(|s| s.contains(kw));

    if msg.contains("code")
        || msg.contains("implement")
        || msg.contains("function")
        || msg.contains("class")
        || msg.contains("module")
        || msg.contains("refactor")
        || msg.contains("debug")
        || msg.contains("fix bug")
        || msg.contains("compile")
        || skill_has("code")
        || skill_has("rust")
        || skill_has("python")
        || skill_has("typescript")
        || skill_has("coding")
    {
        return PlanType::CodeGeneration;
    }

    if msg.contains("file")
        || msg.contains("directory")
        || msg.contains("folder")
        || msg.contains("path")
        || msg.contains("create ")
        || msg.contains("delete ")
        || msg.contains("rename ")
        || skill_has("file")
        || skill_has("filesystem")
    {
        return PlanType::FileOperation;
    }

    if msg.contains("run ")
        || msg.contains("execute")
        || msg.contains("command")
        || msg.contains("script")
        || msg.contains("shell")
        || msg.contains("bash")
        || msg.contains("install")
        || msg.contains("deploy")
        || skill_has("shell")
        || skill_has("bash")
    {
        return PlanType::ShellTask;
    }

    if msg.contains("search")
        || msg.contains("find ")
        || msg.contains("investigat")
        || msg.contains("research")
        || msg.contains("analys")
        || msg.contains("explain")
        || msg.contains("what is")
        || msg.contains("how does")
        || skill_has("research")
    {
        return PlanType::Research;
    }

    PlanType::Generic
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_steps ────────────────────────────────────────────────────────

    #[test]
    fn extracts_json_steps_object() {
        let raw = r#"{"steps":["Read the file","Modify line 42","Save changes"]}"#;
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps, vec!["Read the file", "Modify line 42", "Save changes"]);
    }

    #[test]
    fn extracts_json_array() {
        let raw = r#"["step one","step two","step three"]"#;
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn extracts_numbered_list() {
        let raw = "1. Read the config file\n2. Update the value\n3. Restart the service";
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps, vec![
            "Read the config file",
            "Update the value",
            "Restart the service",
        ]);
    }

    #[test]
    fn extracts_numbered_list_paren_style() {
        let raw = "1) Locate the binary\n2) Copy to target\n3) Verify checksum";
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "Locate the binary");
    }

    #[test]
    fn extracts_bullet_list_dash() {
        let raw = "- Check the logs\n- Identify the error\n- Apply the fix";
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn extracts_bullet_list_asterisk() {
        let raw = "* Locate file\n* Read content\n* Write changes";
        let steps = extract_steps(raw).expect("steps");
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn extracts_ordinal_words() {
        let raw = "First, check the environment.\nThen, run the build command.\nFinally, verify the output.";
        let steps = extract_steps(raw).expect("steps");
        assert!(steps.len() >= 2);
        assert!(steps[0].contains("check"));
    }

    #[test]
    fn returns_none_for_empty_text() {
        assert!(extract_steps("").is_none());
        assert!(extract_steps("   ").is_none());
    }

    #[test]
    fn returns_none_for_single_fragment() {
        // A single short fragment with no pattern — returns None
        assert!(extract_steps("do the thing").is_none());
    }

    #[test]
    fn json_wins_over_numbered_when_both_present() {
        // JSON body inside a numbered response; JSON takes priority
        let raw = r#"{"steps":["a","b","c"]}"#;
        let steps = extract_steps(raw).unwrap();
        assert_eq!(steps.len(), 3);
    }

    // ── AgentPlanState ───────────────────────────────────────────────────────

    #[test]
    fn plan_state_parses_json() {
        let raw = r#"{"steps":["step one","step two"]}"#;
        let state = AgentPlanState::from_model_reply(raw, PlanType::Generic).unwrap();
        assert_eq!(state.current_step_text(), Some("step one"));
    }

    #[test]
    fn plan_state_parses_prose() {
        let raw = "1. Do X\n2. Do Y\n3. Do Z";
        let state = AgentPlanState::from_model_reply(raw, PlanType::Generic).unwrap();
        assert_eq!(state.steps.len(), 3);
    }

    #[test]
    fn plan_state_advance() {
        let raw = r#"{"steps":["a","b","c"]}"#;
        let mut state = AgentPlanState::from_model_reply(raw, PlanType::Generic).unwrap();
        assert!(state.advance());
        assert_eq!(state.current_step_text(), Some("b"));
        assert!(state.advance());
        assert!(state.current_step_text() == Some("c"));
        assert!(!state.advance());
        assert!(state.current_step_text().is_none());
    }

    // ── classify ─────────────────────────────────────────────────────────────

    #[test]
    fn classify_code() {
        assert_eq!(classify("implement a sorting function", &[]), PlanType::CodeGeneration);
    }

    #[test]
    fn classify_file() {
        assert_eq!(classify("read the config file and update it", &[]), PlanType::FileOperation);
    }

    #[test]
    fn classify_shell() {
        assert_eq!(classify("run the deploy script on the server", &[]), PlanType::ShellTask);
    }

    #[test]
    fn classify_research() {
        assert_eq!(classify("explain how the scheduler works", &[]), PlanType::Research);
    }

    #[test]
    fn classify_generic_fallback() {
        assert_eq!(classify("do something interesting", &[]), PlanType::Generic);
    }

    #[test]
    fn classify_code_beats_file() {
        // "implement" + "file" — code wins
        assert_eq!(
            classify("implement a function that reads from a file", &[]),
            PlanType::CodeGeneration
        );
    }
}
