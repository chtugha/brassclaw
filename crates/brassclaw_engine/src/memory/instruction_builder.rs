//! Instruction-Building-System (IBS) — the pure-Rust compiler that turns
//! human-editable StepDescriptions into a two-channel [`BuildInstruction`] at
//! intent-match time (§0.4, §0.7).
//!
//! Pure Rust, no async, no DB calls. Called synchronously inside
//! `PostgresSource::fetch_for_turn` after an intent match resolves to a Recipe.
//! `BuildInstruction`s are never stored — they are compiled on demand (§0.4).
//!
//! Data-model types ([`ToolBinding`], [`ErrorPolicy`], [`VariablePattern`])
//! live in [`crate::types::ibs`]; this module owns the IBS **builder / output**
//! types only (FIND-NEW-01 / Decision 1).

use regex::{Captures, Match, Regex};
use serde::{Deserialize, Serialize};

use crate::types::ibs::{ToolBinding, VariablePattern};

// ---------------------------------------------------------------------------
// Authoring model — StepDescription (maps to the `step_descriptions` JSONB)
// ---------------------------------------------------------------------------

/// One element of the `step_descriptions` JSONB array (§0.5). Holds both the
/// verbatim YAML source (WebUI renderer) and the pre-parsed structured array
/// (the IBS reads `steps` only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepDescriptionEntry {
    /// 0-based index into the `step_descriptions` JSONB array.
    pub desc_idx: usize,
    /// Human-readable label for this StepDescription (WebUI only).
    pub label: String,
    /// Raw YAML string as typed by the author. Preserved verbatim; never read
    /// by the IBS.
    pub yaml_source: String,
    /// Pre-parsed step array. Used exclusively by the IBS.
    #[serde(default)]
    pub steps: Vec<StepEntry>,
}

/// One step inside a [`StepDescriptionEntry`] (§0.5 mandatory/optional fields).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepEntry {
    /// 1-based ordinal position within this StepDescription's step sequence.
    pub stepnumber: u32,
    /// Which runtime channel reads this step.
    pub knowledge: StepOwner,
    /// What this step accomplishes (human-readable).
    pub goal: String,
    /// Short description of step content.
    pub content: String,
    /// IBS treatment (§0.5 step types). Serialized as `type` in the JSONB.
    #[serde(rename = "type")]
    pub step_type: RecipeStepType,
    /// Human-readable documentation (WebUI only). NOT emitted to the orchestrator.
    #[serde(default)]
    pub info: Option<String>,
    /// Component UUIDs needed at this step. IBS emits a fetch for each UUID.
    #[serde(default)]
    pub include: Vec<uuid::Uuid>,
    /// Rust-channel tool invocations (rust/both steps only). Authored per step
    /// and passed through to the compiled `IbsRecipeStep` (FIND-IBS-05 / §0.4.1).
    /// Empty for orchestrator-only steps.
    #[serde(default)]
    pub tool_bindings: Vec<ToolBinding>,
    /// Traversal expression string into this step's component's
    /// `dependency_registry` (§0.19). `None` or empty = no dependencies.
    #[serde(default)]
    pub dependencies: Option<String>,
}

/// Which runtime channel reads a step (§0.5 `knowledge` field).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StepOwner {
    Orchestrator,
    Rust,
    Both,
}

/// IBS treatment of a step (§0.5 `type` field).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecipeStepType {
    Text,
    Component,
    Snippet,
}

// ---------------------------------------------------------------------------
// step_link parse output
// ---------------------------------------------------------------------------

/// One segment of a parsed `step_link` formula (§0.6).
///
/// `{desc_idx}:{start}-{desc_idx}:{end}` → one [`StepRange`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRange {
    /// 0-based StepDescription index.
    pub desc_idx: usize,
    /// 1-based stepnumber, or `0` (sentinel = first step in the sequence).
    pub start: u32,
    /// End bound — either a 1-based stepnumber or `E` (last step).
    pub end: StepBound,
}

/// End bound of a [`StepRange`] (§0.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepBound {
    /// 1-based stepnumber.
    Num(u32),
    /// `E` sentinel = last step in the sequence.
    End,
}

// ---------------------------------------------------------------------------
// Compiled output — BuildInstruction + IbsRecipeStep
// ---------------------------------------------------------------------------

/// One compiled step inside a `BuildInstruction` channel (FIND-NEW-02: renamed
/// from `RecipeStep` to avoid collision with the v2 `RecipeStep` in
/// `types/recipe.rs`).
///
/// Rust-channel steps carry `tool_bindings`; orchestrator-channel steps carry
/// `info` (WebUI annotation, not emitted). Both share `include` (component
/// UUIDs) and the parsed `dependencies` tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IbsRecipeStep {
    /// Synthesized stable id `"{desc_idx}:{stepnumber}"` (FIND-IBS-04). Used
    /// for `IbsError` attribution.
    pub step_id: String,
    /// Which runtime channel reads this step.
    pub knowledge: StepOwner,
    /// IBS treatment of the step.
    pub step_type: RecipeStepType,
    /// Component UUIDs to fetch for this step.
    #[serde(default)]
    pub include: Vec<uuid::Uuid>,
    /// Rust-channel tool invocations (rust/both steps only).
    #[serde(default)]
    pub tool_bindings: Vec<ToolBinding>,
    /// WebUI annotation (orchestrator/both steps only; not emitted at runtime).
    #[serde(default)]
    pub info: Option<String>,
    /// Parsed dependency traversal tree (§0.19), attached by `build_instruction`
    /// when the step declared a `dependencies` string. `None` = no dependencies.
    #[serde(default)]
    pub dependencies: Option<DependencyExpr>,
}

/// The compiled, two-channel instruction emitted by the IBS (§0.4).
///
/// Never stored; compiled on demand at intent-match time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildInstruction {
    /// `false` for Tier-0/Actions (skip LLM, execute directly); `true` for
    /// Tier-1+. Passed in by the caller (`fetch_for_turn`), which knows the
    /// recipe's tier (FIND-IBS-02).
    pub llm_call_required: bool,
    /// Slot-variable refinement rules, applied before any channel is read.
    #[serde(default)]
    pub variable_patterns: Vec<VariablePattern>,
    /// Navigation hints into the cached basic-prompt (no re-fetch). Filled by
    /// the KV-cache interaction in `fetch_for_turn` (Phase E/H); empty from the
    /// IBS.
    #[serde(default)]
    pub basic_prompt_section_refs: Vec<String>,
    /// CHANNEL R — Rust execution layer reads this only.
    #[serde(default)]
    pub rust_steps: Vec<IbsRecipeStep>,
    /// CHANNEL O — serialized into `orchestrator_content`.
    #[serde(default)]
    pub orchestrator_steps: Vec<IbsRecipeStep>,
}

// ---------------------------------------------------------------------------
// Dependency traversal expression (§0.19) — parsed tree
// ---------------------------------------------------------------------------

/// A sub-expression inside a dependency traversal node (§0.19).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencySubExpr {
    /// `[all]` — full transitive closure from this node.
    All,
    /// `[n, m[...], ...]` — selective sub-indices.
    Selective(Vec<DependencyNode>),
}

/// One node in a dependency traversal expression (§0.19).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DependencyNode {
    /// Positional index into the component's `dependency_registry`.
    pub idx: usize,
    /// `None` = load component only, no sub-deps. `Some` = recurse per the
    /// sub-expression.
    #[serde(default)]
    pub sub: Option<DependencySubExpr>,
}

/// A parsed dependency traversal expression (§0.19): a comma-separated list of
/// [`DependencyNode`]s.
pub type DependencyExpr = Vec<DependencyNode>;

// ---------------------------------------------------------------------------
// Derived content type for orchestrator formatting (§0.5)
// ---------------------------------------------------------------------------

/// Derived content type for orchestrator-channel steps (§0.5). Inferred from
/// the component's `class_code` in `assemble_prior_knowledge_with_hint` at
/// fetch time — never stored, never set by authors.
///
/// v3 Phase F.5 (Q-F7-1 / Q-F7-2): extended beyond the plan's 6
/// orchestrator-channel variants to cover **every** component class, because
/// the `Components` broad-scan arm of `assemble_pkr_from_fetch` returns all
/// retrieved classes and the prose formatter (`format_orchestrator_content`)
/// must label each. `Annotation` is plan-faithful (a `type:"text"` step — never
/// produced from a `ComponentItem`). Class 13 (ToolSkill) and class 11
/// (reserved) map to `None` via [`StepContextSpec::from_class_code`] — they are
/// never emitted into `orchestrator_content` (ToolSkill is Rust-channel-only,
/// §0.9 invariant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepContextSpec {
    Skill,
    Spec,
    Recipe,
    PythonCode,
    Catalogue,
    Annotation,
    Extension,
    Orchestrator,
    Plan,
    Summary,
    Action,
    Docu,
    Lesson,
    Issue,
    Note,
    Scaffold,
    Tool,
    Component,
}

impl StepContextSpec {
    /// Derive the context spec from a component `class_code`.
    ///
    /// Returns `None` for class 13 (ToolSkill — never in the orchestrator
    /// channel, §0.9 invariant) and class 11 (reserved), so the formatter
    /// skips those items. Every other class maps to its category variant;
    /// unknown codes fall back to [`StepContextSpec::Component`].
    pub fn from_class_code(class_code: i32) -> Option<StepContextSpec> {
        let spec = match class_code {
            0 => StepContextSpec::Tool,
            1..=3 => StepContextSpec::Skill,
            4..=9 => StepContextSpec::Extension,
            10 => StepContextSpec::Orchestrator,
            11 => return None,
            12 => StepContextSpec::Spec,
            13 => return None,
            14 => StepContextSpec::Plan,
            15 => StepContextSpec::Summary,
            16 => StepContextSpec::Action,
            17 => StepContextSpec::Docu,
            18 => StepContextSpec::Lesson,
            19 => StepContextSpec::Issue,
            20 => StepContextSpec::Note,
            21 => StepContextSpec::Recipe,
            22 => StepContextSpec::PythonCode,
            23 => StepContextSpec::Catalogue,
            50 => StepContextSpec::Scaffold,
            _ => StepContextSpec::Component,
        };
        Some(spec)
    }

    /// The Capitalized category label used as the prose block heading
    /// (`## [{heading}: {name}]`). Per Q-F7-2 / Q-F7-case this is a category
    /// label, NOT the lowercase specific subtype from `class_label()`.
    pub fn heading(&self) -> &'static str {
        match self {
            StepContextSpec::Skill => "Skill",
            StepContextSpec::Spec => "Spec",
            StepContextSpec::Recipe => "Recipe",
            StepContextSpec::PythonCode => "PythonCode",
            StepContextSpec::Catalogue => "Catalogue",
            StepContextSpec::Annotation => "Annotation",
            StepContextSpec::Extension => "Extension",
            StepContextSpec::Orchestrator => "Orchestrator",
            StepContextSpec::Plan => "Plan",
            StepContextSpec::Summary => "Summary",
            StepContextSpec::Action => "Action",
            StepContextSpec::Docu => "Docu",
            StepContextSpec::Lesson => "Lesson",
            StepContextSpec::Issue => "Issue",
            StepContextSpec::Note => "Note",
            StepContextSpec::Scaffold => "Scaffold",
            StepContextSpec::Tool => "Tool",
            StepContextSpec::Component => "Component",
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// IBS compile errors (§0.7 Errors). Canonical enum — unchanged by FIND-IBS-01
/// (the empty-include rule is a Q1 invalidation, not an IBS error).
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum IbsError {
    #[error("unpromoted snippet step {step_id}: promote to type:component after Q1+Q2")]
    UnpromotedSnippet { step_id: String },
    #[error("invalid UUID at step {step_id}: {value}")]
    InvalidUuid { step_id: String, value: String },
    #[error(
        "step order violation in SD{desc_idx} at stepnumber {stepnumber}: steps must be monotonically increasing"
    )]
    StepOrderViolation { desc_idx: usize, stepnumber: u32 },
    #[error("unknown StepDescription index {desc_idx}")]
    UnknownDescIdx { desc_idx: usize },
    #[error("step_link parse error in {formula}: {reason}")]
    ParseError { formula: String, reason: String },
    #[error("S7 violation: rust tool_bindings present but no orchestrator skill_ids")]
    S7Violation,
    #[error("invalid dependency expression at step {step_id}: {reason}")]
    InvalidDependencyExpr { step_id: String, reason: String },
}

// ---------------------------------------------------------------------------
// parse_step_link
// ---------------------------------------------------------------------------

/// Parse a `step_link` formula (§0.6) into an ordered list of [`StepRange`]s.
///
/// Notation: `"{desc_idx}:{start}-{desc_idx}:{end}[+...]"`, where `start` is a
/// 1-based stepnumber or `0` (first-step sentinel), and `end` is a 1-based
/// stepnumber or `E` (last-step sentinel).
pub fn parse_step_link(step_link: &str) -> Result<Vec<StepRange>, IbsError> {
    let formula = step_link.to_string();
    let trimmed = step_link.trim();
    if trimmed.is_empty() {
        return Err(IbsError::ParseError {
            formula,
            reason: "empty step_link".into(),
        });
    }

    let mut ranges = Vec::new();
    for raw_segment in trimmed.split('+') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            return Err(IbsError::ParseError {
                formula,
                reason: "empty segment in step_link".into(),
            });
        }
        let (left, right) = segment
            .split_once('-')
            .ok_or_else(|| IbsError::ParseError {
                formula: formula.clone(),
                reason: format!("segment {segment:?} missing '-' separator"),
            })?;
        let (left_idx, start_str) =
            parse_bound_part(left).map_err(|reason| IbsError::ParseError {
                formula: formula.clone(),
                reason,
            })?;
        let (right_idx, end_str) =
            parse_bound_part(right).map_err(|reason| IbsError::ParseError {
                formula: formula.clone(),
                reason,
            })?;
        if left_idx != right_idx {
            return Err(IbsError::ParseError {
                formula,
                reason: format!("segment {segment:?} desc_idx mismatch: {left_idx} vs {right_idx}"),
            });
        }
        let start: u32 = start_str.parse().map_err(|_| IbsError::ParseError {
            formula: formula.clone(),
            reason: format!("invalid start bound {start_str:?}"),
        })?;
        let end = parse_end_bound(end_str).map_err(|reason| IbsError::ParseError {
            formula: formula.clone(),
            reason,
        })?;
        ranges.push(StepRange {
            desc_idx: left_idx,
            start,
            end,
        });
    }

    Ok(ranges)
}

fn parse_bound_part(s: &str) -> Result<(usize, &str), String> {
    let s = s.trim();
    let (idx_str, bound_str) = s
        .split_once(':')
        .ok_or_else(|| format!("bound {s:?} missing ':' separator"))?;
    let idx: usize = idx_str
        .trim()
        .parse()
        .map_err(|_| format!("invalid desc_idx {idx_str:?}"))?;
    Ok((idx, bound_str.trim()))
}

fn parse_end_bound(s: &str) -> Result<StepBound, String> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("E") {
        return Ok(StepBound::End);
    }
    let n: u32 = s
        .parse()
        .map_err(|_| format!("invalid end bound {s:?} (expected number or 'E')"))?;
    Ok(StepBound::Num(n))
}

// ---------------------------------------------------------------------------
// parse_dependency_expr
// ---------------------------------------------------------------------------

/// Parse a dependency traversal expression (§0.19) into a [`DependencyExpr`]
/// tree.
///
/// Grammar: `node ("," node)*` where `node := idx ("[" inner "]")?` and
/// `inner := "all" | expr`. An empty string yields an empty expression.
///
/// The returned error carries an empty `step_id` when called standalone;
/// `build_instruction` re-attributes the real `step_id` when parsing on behalf
/// of a step.
pub fn parse_dependency_expr(expr: &str) -> Result<DependencyExpr, IbsError> {
    parse_dep_expr_inner(expr).map_err(|reason| IbsError::InvalidDependencyExpr {
        step_id: String::new(),
        reason,
    })
}

fn parse_dep_expr_inner(expr: &str) -> Result<DependencyExpr, String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut parser = DepParser {
        chars: trimmed.chars().peekable(),
    };
    let nodes = parser.parse_expr()?;
    parser.skip_ws();
    if parser.chars.peek().is_some() {
        return Err("trailing characters after expression".into());
    }
    Ok(nodes)
}

struct DepParser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> DepParser<'a> {
    fn skip_ws(&mut self) {
        while let Some(c) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Vec<DependencyNode>, String> {
        let mut nodes = Vec::new();
        self.skip_ws();
        nodes.push(self.parse_node()?);
        loop {
            self.skip_ws();
            match self.chars.peek() {
                Some(',') => {
                    self.chars.next();
                    self.skip_ws();
                    nodes.push(self.parse_node()?);
                }
                _ => break,
            }
        }
        Ok(nodes)
    }

    fn parse_node(&mut self) -> Result<DependencyNode, String> {
        self.skip_ws();
        let mut idx_str = String::new();
        while let Some(c) = self.chars.peek() {
            if c.is_ascii_digit() {
                idx_str.push(*c);
                self.chars.next();
            } else {
                break;
            }
        }
        if idx_str.is_empty() {
            return Err("expected index at node start".into());
        }
        let idx: usize = idx_str
            .parse()
            .map_err(|_| format!("invalid index {idx_str:?}"))?;
        self.skip_ws();
        let sub = match self.chars.peek() {
            Some('[') => {
                self.chars.next();
                let inner = self.parse_inner()?;
                self.skip_ws();
                match self.chars.next() {
                    Some(']') => Some(inner),
                    other => {
                        return Err(format!("expected ']' after sub-expression, got {other:?}"));
                    }
                }
            }
            _ => None,
        };
        Ok(DependencyNode { idx, sub })
    }

    fn parse_inner(&mut self) -> Result<DependencySubExpr, String> {
        self.skip_ws();
        match self.chars.peek() {
            Some('a') | Some('A') => {
                for expected in ['a', 'l', 'l'] {
                    match self.chars.next() {
                        Some(c) if c.eq_ignore_ascii_case(&expected) => {}
                        other => {
                            return Err(format!("expected 'all', got {other:?}"));
                        }
                    }
                }
                Ok(DependencySubExpr::All)
            }
            _ => {
                let nodes = self.parse_expr()?;
                Ok(DependencySubExpr::Selective(nodes))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// build_instruction
// ---------------------------------------------------------------------------

/// Compile human-editable StepDescriptions into a two-channel
/// [`BuildInstruction`] (§0.4, §0.7 assembly algorithm).
///
/// `llm_call_required` is passed by the caller (`fetch_for_turn`), which knows
/// the recipe's tier (FIND-IBS-02). The IBS stores it directly into the
/// `BuildInstruction`.
pub fn build_instruction(
    step_link: &str,
    step_descriptions: &[StepDescriptionEntry],
    variable_patterns: &[VariablePattern],
    llm_call_required: bool,
) -> Result<BuildInstruction, IbsError> {
    // 1. Parse step_link -> Vec<StepRange>.
    let ranges = parse_step_link(step_link)?;

    // 4a. Monotonic check over ALL StepDescriptionEntry provided (FIND-IBS-03).
    for entry in step_descriptions {
        let mut prev: Option<u32> = None;
        for step in &entry.steps {
            if let Some(p) = prev
                && step.stepnumber <= p
            {
                return Err(IbsError::StepOrderViolation {
                    desc_idx: entry.desc_idx,
                    stepnumber: step.stepnumber,
                });
            }
            prev = Some(step.stepnumber);
        }
    }

    // 2. Select steps per range into an ordered list with desc_idx provenance.
    let mut ordered: Vec<(&StepDescriptionEntry, &StepEntry)> = Vec::new();
    for range in &ranges {
        let entry = step_descriptions
            .iter()
            .find(|e| e.desc_idx == range.desc_idx)
            .ok_or(IbsError::UnknownDescIdx {
                desc_idx: range.desc_idx,
            })?;
        if entry.steps.is_empty() {
            continue;
        }
        let start_idx = resolve_start_index(&entry.steps, range.start).map_err(|reason| {
            IbsError::ParseError {
                formula: step_link.to_string(),
                reason: format!("SD{}: {reason}", range.desc_idx),
            }
        })?;
        let end_idx =
            resolve_end_index(&entry.steps, &range.end).map_err(|reason| IbsError::ParseError {
                formula: step_link.to_string(),
                reason: format!("SD{}: {reason}", range.desc_idx),
            })?;
        if start_idx > end_idx {
            return Err(IbsError::ParseError {
                formula: step_link.to_string(),
                reason: format!("SD{}: start bound after end bound", range.desc_idx),
            });
        }
        for s in &entry.steps[start_idx..=end_idx] {
            ordered.push((entry, s));
        }
    }

    // 3 + 5 + 6: emit / partition / attach dependencies.
    let mut rust_steps: Vec<IbsRecipeStep> = Vec::new();
    let mut orchestrator_steps: Vec<IbsRecipeStep> = Vec::new();
    for (entry, step) in &ordered {
        let step_id = format!("{}:{}", entry.desc_idx, step.stepnumber);

        // 3. step type handling.
        match step.step_type {
            RecipeStepType::Text => continue, // WebUI annotation only; no runtime emission.
            RecipeStepType::Snippet => {
                return Err(IbsError::UnpromotedSnippet { step_id });
            }
            RecipeStepType::Component => {} // emit; route by knowledge below.
        }

        // 6. parse dependencies if present.
        let dependencies =
            match step.dependencies.as_deref().map(str::trim) {
                Some(s) if !s.is_empty() => Some(parse_dep_expr_inner(s).map_err(|reason| {
                    IbsError::InvalidDependencyExpr {
                        step_id: step_id.clone(),
                        reason,
                    }
                })?),
                _ => None,
            };

        let base = IbsRecipeStep {
            step_id,
            knowledge: step.knowledge,
            step_type: step.step_type,
            include: step.include.clone(),
            tool_bindings: step.tool_bindings.clone(),
            info: step.info.clone(),
            dependencies,
        };

        // 5. partition by knowledge. Keep channels clean: rust copies drop
        // `info`, orchestrator copies drop `tool_bindings`.
        match step.knowledge {
            StepOwner::Rust => {
                let mut r = base.clone();
                r.info = None;
                rust_steps.push(r);
            }
            StepOwner::Orchestrator => {
                let mut o = base.clone();
                o.tool_bindings = Vec::new();
                orchestrator_steps.push(o);
            }
            StepOwner::Both => {
                let mut r = base.clone();
                r.info = None;
                rust_steps.push(r);
                let mut o = base;
                o.tool_bindings = Vec::new();
                orchestrator_steps.push(o);
            }
        }
    }

    // 4b. S7 guard: if any rust step emits tool_bindings, orchestrator_steps
    // must contain >=1 step with non-empty include. The Tier-0 class-22
    // refinement is a Q1 check (UUIDs are opaque to the IBS — FIND-IBS-02).
    let rust_has_bindings = rust_steps.iter().any(|s| !s.tool_bindings.is_empty());
    if rust_has_bindings {
        let orchestrator_has_include = orchestrator_steps.iter().any(|s| !s.include.is_empty());
        if !orchestrator_has_include {
            return Err(IbsError::S7Violation);
        }
    }

    Ok(BuildInstruction {
        llm_call_required,
        variable_patterns: variable_patterns.to_vec(),
        basic_prompt_section_refs: Vec::new(),
        rust_steps,
        orchestrator_steps,
    })
}

fn resolve_start_index(steps: &[StepEntry], start: u32) -> Result<usize, String> {
    if start == 0 {
        return Ok(0);
    }
    steps
        .iter()
        .position(|s| s.stepnumber == start)
        .ok_or_else(|| format!("start bound {start} not found in StepDescription"))
}

fn resolve_end_index(steps: &[StepEntry], end: &StepBound) -> Result<usize, String> {
    match end {
        StepBound::End => {
            if steps.is_empty() {
                Err("end bound E on empty StepDescription".into())
            } else {
                Ok(steps.len() - 1)
            }
        }
        StepBound::Num(n) => steps
            .iter()
            .position(|s| s.stepnumber == *n)
            .ok_or_else(|| format!("end bound {n} not found in StepDescription")),
    }
}

// ---------------------------------------------------------------------------
// Template slot extraction + variable capture + {{vars.name}} substitution
// (§0.17.3 / §0.20.3 / Phase M.4 + M.5). Front-loaded into the IBS at Phase E
// because `PostgresSource::fetch_for_turn` (E.4) must substitute {{vars.name}}
// into orchestrator_content + ToolBinding `params` at assembly time, BEFORE
// Phase M runs. Phase M later re-references these helpers (the M.4 `parse_
// template` prefix/suffix split for DB indexing stays in `intent_system.rs`
// as Phase M's own concern — it is a different operation over the template
// string). This front-load is an upgrade accepted per the task rule "do not
// blindly remove upgrades; document, repair, complete or leave them" — see
// `docs/agents-v3/subplan_problem_stepE_of_saved_plan_to_v3.md` §6.
// ---------------------------------------------------------------------------

/// Extract positional slot values from a matched template expression
/// (§0.17.3 / Phase M.4).
///
/// Splits `template` on `%` into literal segments, then walks `user_text`:
/// the prefix segment (`segments[0]`) anchors the start (found from the
/// left), the suffix segment (`segments[n_slots]`) anchors the end (found
/// from the right), and each gap between consecutive segments is one slot
/// value. Middle separators are searched left-to-right within
/// `[cursor, suffix_start]`. Slots are named positionally — `slot0`,
/// `slot1`, … in left-to-right order.
///
/// Returns an empty vec when the template has no `%` (a literal match with
/// no slots), and a partial vec when a middle separator cannot be found
/// (the remaining slots are unextractable — the caller treats them as
/// missing). Adjacent `%` yield empty slot values; Q1 rejects adjacent slots
/// at authoring time (Phase I), so a validated recipe never reaches that
/// path.
///
/// Pure, synchronous, no DB. Canonical signature + algorithm per Phase M.4.
pub fn extract_template_slots(template: &str, user_text: &str) -> Vec<(String, String)> {
    let segments: Vec<&str> = template.split('%').collect();
    let n_slots = segments.len().saturating_sub(1);
    if n_slots == 0 {
        return Vec::new();
    }

    // Prefix anchor (segments[0]): found from the left. For a prefix-anchored
    // template this sits at position 0; for a leading-`%` template segments[0]
    // is "" and `find("")` yields Some(0).
    let mut cursor = match user_text.find(segments[0]) {
        Some(i) => i + segments[0].len(),
        None => return Vec::new(),
    };

    // Suffix anchor (segments[n_slots]): found from the right. For a
    // trailing-`%` template this is "" and the suffix sits at the end.
    let suffix = segments[n_slots];
    let suffix_start = if suffix.is_empty() {
        user_text.len()
    } else {
        user_text.rfind(suffix).unwrap_or(user_text.len())
    };

    let mut result: Vec<(String, String)> = Vec::with_capacity(n_slots);
    for slot_idx in 0..n_slots {
        if slot_idx == n_slots - 1 {
            // Last slot: the gap between the current cursor and the suffix anchor.
            let value = user_text
                .get(cursor..suffix_start)
                .unwrap_or("")
                .to_string();
            result.push((format!("slot{slot_idx}"), value));
        } else {
            // Middle slot: the gap between the cursor and the next separator
            // (segments[slot_idx + 1]), searched within [cursor, suffix_start].
            let sep = segments[slot_idx + 1];
            let search_end = suffix_start.max(cursor);
            let region = user_text.get(cursor..search_end).unwrap_or("");
            match region.find(sep) {
                Some(rel) => {
                    let abs = cursor + rel;
                    result.push((
                        format!("slot{slot_idx}"),
                        user_text.get(cursor..abs).unwrap_or("").to_string(),
                    ));
                    cursor = abs + sep.len();
                }
                None => return result, // separator missing — remaining slots unextractable
            }
        }
    }
    result
}

/// Capture template variables from the user text, refining the positional
/// slots with the matched variant's `variable_patterns` (§0.17.3 / Phase M.5).
///
/// Step 1: auto-extract positional slots via [`extract_template_slots`]
/// (`slot0`, `slot1`, …). Step 2: for each `variable_patterns` entry (paired
/// with the slot BY ORDER), apply its regex to the auto-extracted value —
/// NOT to the full `user_text`. When the regex matches, the slot is RENAMED
/// to the entry's semantic `name`; if the regex captures a named group, that
/// group's value replaces the raw value (transformation), otherwise the raw
/// value is kept (validation only). When the regex does NOT match, or fails
/// to compile, the slot is DEMOTED — the raw value + positional name are kept
/// (§0.17.3: "extraction just gets the raw value"; a bad regex is a Q1
/// authoring error caught at Phase I, not a runtime turn failure). Entries
/// beyond the slot count are ignored (dangling — Q1 warns at Phase I).
///
/// Pure, synchronous, no DB. The E.4 `fetch_for_turn` caller feeds the
/// returned `(name, value)` pairs to [`substitute_vars`] /
/// [`substitute_vars_in_value`].
pub fn capture_variables(
    template: &str,
    user_text: &str,
    variable_patterns: &[VariablePattern],
) -> Vec<(String, String)> {
    let mut slots = extract_template_slots(template, user_text);
    for (idx, vp) in variable_patterns.iter().enumerate() {
        let Some(slot) = slots.get_mut(idx) else {
            break; // more patterns than slots — dangling (Q1 warns at Phase I)
        };
        let Some(pattern) = &vp.pattern else {
            // No regex: rename to the semantic name only (no validation/transform).
            slot.0 = vp.name.clone();
            continue;
        };
        let Ok(re) = Regex::new(pattern) else {
            // Bad regex: demote — keep raw value + positional name (Q1 catches at Phase I).
            continue;
        };
        let Some(caps) = re.captures(&slot.1) else {
            // Validation failed: demote — keep raw value + positional name.
            continue;
        };
        // Matched: rename to the semantic name. Prefer a named capture group
        // (matching `vp.name`, else the first named group) for the value;
        // fall back to the full match (validation-only pattern, no named group).
        let refined = caps
            .name(&vp.name)
            .or_else(|| first_named_capture(&re, &caps))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| slot.1.clone());
        *slot = (vp.name.clone(), refined);
    }
    slots
}

/// Return the first named capture group's match in `caps`, or `None` when the
/// regex has no named groups (validation-only pattern).
fn first_named_capture<'c>(re: &Regex, caps: &Captures<'c>) -> Option<Match<'c>> {
    for name in re.capture_names().flatten() {
        if let Some(m) = caps.name(name) {
            return Some(m);
        }
    }
    None
}

/// Substitute `{{vars.NAME}}` placeholders in `content` with the captured
/// values (§0.20.3 — the IBS bakes variables into `orchestrator_content` /
/// ToolBinding `params` before execution, so runtime bodies see literal
/// values, not placeholders).
///
/// Each `(name, value)` pair replaces the literal `{{vars.name}}` token.
/// Distinct names never overlap (`{{vars.dir}}` and `{{vars.directory}}` are
/// different literals). Placeholders with no matching capture are left intact
/// (a Q1 "missing template" authoring error — Phase I — not a runtime
/// fabrication).
///
/// Pure, synchronous, no DB.
pub fn substitute_vars(content: &str, vars: &[(String, String)]) -> String {
    let mut out = content.to_string();
    for (name, value) in vars {
        let placeholder = format!("{{{{vars.{name}}}}}");
        if out.contains(&placeholder) {
            out = out.replace(&placeholder, value);
        }
    }
    out
}

/// Recursively substitute `{{vars.NAME}}` placeholders in every string leaf of
/// a JSON value (§0.4.1 — ToolBinding `params` carry `{{vars.name}}`
/// placeholders substituted before `__execute_action__` dispatch).
///
/// Object keys are preserved; only string values are substituted. Non-string
/// leaves (numbers, bools, null) pass through unchanged. Placeholders with no
/// matching capture are left intact (same rule as [`substitute_vars`]).
///
/// Pure, synchronous, no DB.
pub fn substitute_vars_in_value(
    value: &serde_json::Value,
    vars: &[(String, String)],
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(substitute_vars(s, vars)),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| substitute_vars_in_value(v, vars))
                .collect(),
        ),
        serde_json::Value::Object(obj) => {
            let mut map = serde_json::Map::with_capacity(obj.len());
            for (k, v) in obj {
                map.insert(k.clone(), substitute_vars_in_value(v, vars));
            }
            serde_json::Value::Object(map)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ibs::{ErrorPolicy, ToolBinding, VariablePattern};

    fn uuid(n: u8) -> uuid::Uuid {
        uuid::Uuid::from_bytes([n; 16])
    }

    fn binding(name: &str) -> ToolBinding {
        ToolBinding {
            tool_id: uuid(1),
            tool_name: name.into(),
            params: serde_json::json!({}),
            error_policy: ErrorPolicy::Fail,
        }
    }

    fn entry(
        stepnumber: u32,
        knowledge: StepOwner,
        step_type: RecipeStepType,
        include: Vec<uuid::Uuid>,
        tool_bindings: Vec<ToolBinding>,
    ) -> StepEntry {
        StepEntry {
            stepnumber,
            knowledge,
            goal: format!("step {stepnumber}"),
            content: "c".into(),
            step_type,
            info: None,
            include,
            tool_bindings,
            dependencies: None,
        }
    }

    fn sd(desc_idx: usize, steps: Vec<StepEntry>) -> StepDescriptionEntry {
        StepDescriptionEntry {
            desc_idx,
            label: format!("sd{desc_idx}"),
            yaml_source: "steps: []".into(),
            steps,
        }
    }

    #[test]
    fn step_description_entry_roundtrips_with_yaml_and_steps() {
        let e = StepDescriptionEntry {
            desc_idx: 0,
            label: "base".into(),
            yaml_source: "steps:\n  - stepnumber: 1".into(),
            steps: vec![entry(
                1,
                StepOwner::Orchestrator,
                RecipeStepType::Text,
                vec![],
                vec![],
            )],
        };
        let v = serde_json::to_value(&e).unwrap();
        let back: StepDescriptionEntry = serde_json::from_value(v).unwrap();
        assert_eq!(e, back);
        // yaml_source preserved verbatim (not re-derived from steps).
        assert_eq!(back.yaml_source, "steps:\n  - stepnumber: 1");
    }

    #[test]
    fn step_entry_renames_type_field() {
        let e = entry(
            1,
            StepOwner::Rust,
            RecipeStepType::Component,
            vec![uuid(2)],
            vec![],
        );
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["stepnumber"], 1);
        assert_eq!(v["knowledge"], "rust");
        assert_eq!(v["type"], "component");
        let back: StepEntry = serde_json::from_value(v).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn parse_step_link_single_range_all_steps() {
        let ranges = parse_step_link("0:0-0:E").unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].desc_idx, 0);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, StepBound::End);
    }

    #[test]
    fn parse_step_link_two_ranges() {
        let ranges = parse_step_link("0:0-0:30+1:0-1:E").unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].desc_idx, 0);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, StepBound::Num(30));
        assert_eq!(ranges[1].desc_idx, 1);
        assert_eq!(ranges[1].start, 0);
        assert_eq!(ranges[1].end, StepBound::End);
    }

    #[test]
    fn parse_step_link_rejects_desc_idx_mismatch() {
        assert!(matches!(
            parse_step_link("0:0-1:E"),
            Err(IbsError::ParseError { .. })
        ));
    }

    #[test]
    fn parse_step_link_rejects_empty() {
        assert!(matches!(
            parse_step_link("   "),
            Err(IbsError::ParseError { .. })
        ));
    }

    #[test]
    fn build_instruction_routes_rust_step_only_to_rust_channel() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Rust,
                RecipeStepType::Component,
                vec![uuid(3)],
                vec![],
            )],
        )];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        assert_eq!(bi.rust_steps.len(), 1);
        assert!(bi.orchestrator_steps.is_empty());
        assert_eq!(bi.rust_steps[0].step_id, "0:1");
    }

    #[test]
    fn build_instruction_routes_both_step_to_both_channels() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Both,
                RecipeStepType::Component,
                vec![uuid(3)],
                vec![],
            )],
        )];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        assert_eq!(bi.rust_steps.len(), 1);
        assert_eq!(bi.orchestrator_steps.len(), 1);
        // rust copy drops info; orch copy drops tool_bindings.
        assert!(bi.rust_steps[0].info.is_none());
        assert!(bi.orchestrator_steps[0].tool_bindings.is_empty());
    }

    #[test]
    fn build_instruction_text_step_emits_nothing() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Orchestrator,
                RecipeStepType::Text,
                vec![],
                vec![],
            )],
        )];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        assert!(bi.rust_steps.is_empty());
        assert!(bi.orchestrator_steps.is_empty());
    }

    #[test]
    fn build_instruction_snippet_step_is_unpromoted() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Orchestrator,
                RecipeStepType::Snippet,
                vec![],
                vec![],
            )],
        )];
        assert!(matches!(
            build_instruction("0:0-0:E", &sds, &[], true),
            Err(IbsError::UnpromotedSnippet { .. })
        ));
    }

    #[test]
    fn recipe_machine_form_round_trips_through_ibs() {
        // C.4.5.1 conformance: a v3 recipe variant's machine form (step_link +
        // variable_patterns + step include UUIDs) compiles via the IBS into the
        // universal BuildInstruction and serde-round-trips — the reference shape
        // every other component class will conform to (F1=A).
        let includes = vec![uuid(7), uuid(8)];
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Both,
                RecipeStepType::Component,
                includes.clone(),
                vec![],
            )],
        )];
        let vps = vec![crate::types::ibs::VariablePattern {
            name: "dir".into(),
            pattern: Some("^[a-z]+$".into()),
            description: None,
        }];
        let bi = build_instruction("0:0-0:E", &sds, &vps, true).unwrap();
        assert_eq!(bi.variable_patterns, vps);
        assert_eq!(bi.rust_steps[0].include, includes);
        assert_eq!(bi.orchestrator_steps[0].include, includes);
        let v = serde_json::to_value(&bi).unwrap();
        let back: BuildInstruction = serde_json::from_value(v).unwrap();
        assert_eq!(bi, back);
    }

    #[test]
    fn build_instruction_rejects_non_monotonic_stepnumbers() {
        let sds = vec![sd(
            0,
            vec![
                entry(
                    1,
                    StepOwner::Orchestrator,
                    RecipeStepType::Text,
                    vec![],
                    vec![],
                ),
                entry(
                    1,
                    StepOwner::Orchestrator,
                    RecipeStepType::Text,
                    vec![],
                    vec![],
                ),
            ],
        )];
        assert!(matches!(
            build_instruction("0:0-0:E", &sds, &[], true),
            Err(IbsError::StepOrderViolation {
                desc_idx: 0,
                stepnumber: 1
            })
        ));
    }

    #[test]
    fn s7_guard_violation_when_rust_bindings_but_no_orchestrator_include() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Rust,
                RecipeStepType::Component,
                vec![uuid(3)],
                vec![binding("ls")],
            )],
        )];
        // No orchestrator steps at all -> no orchestrator include -> S7Violation.
        assert!(matches!(
            build_instruction("0:0-0:E", &sds, &[], true),
            Err(IbsError::S7Violation)
        ));
    }

    #[test]
    fn s7_guard_passes_when_orchestrator_has_include() {
        let sds = vec![sd(
            0,
            vec![
                entry(
                    1,
                    StepOwner::Rust,
                    RecipeStepType::Component,
                    vec![uuid(3)],
                    vec![binding("ls")],
                ),
                entry(
                    2,
                    StepOwner::Orchestrator,
                    RecipeStepType::Component,
                    vec![uuid(4)],
                    vec![],
                ),
            ],
        )];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        assert_eq!(bi.rust_steps.len(), 1);
        assert_eq!(bi.orchestrator_steps.len(), 1);
    }

    #[test]
    fn parse_dependency_expr_full_tree() {
        let expr = parse_dependency_expr("1[all], 5[2,6], 17[3, 7[1,4]]").unwrap();
        assert_eq!(expr.len(), 3);
        assert_eq!(expr[0].idx, 1);
        assert_eq!(expr[0].sub, Some(DependencySubExpr::All));
        match &expr[1].sub {
            Some(DependencySubExpr::Selective(nodes)) => {
                assert_eq!(nodes.len(), 2);
                assert_eq!(nodes[0].idx, 2);
                assert!(nodes[0].sub.is_none());
                assert_eq!(nodes[1].idx, 6);
                assert!(nodes[1].sub.is_none());
            }
            other => panic!("expected selective, got {other:?}"),
        }
        match &expr[2].sub {
            Some(DependencySubExpr::Selective(nodes)) => {
                assert_eq!(nodes.len(), 2);
                assert_eq!(nodes[0].idx, 3);
                match &nodes[1].sub {
                    Some(DependencySubExpr::Selective(inner)) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner[0].idx, 1);
                        assert_eq!(inner[1].idx, 4);
                    }
                    other => panic!("expected inner selective, got {other:?}"),
                }
            }
            other => panic!("expected selective, got {other:?}"),
        }
    }

    #[test]
    fn parse_dependency_expr_single_node_no_sub() {
        let expr = parse_dependency_expr("0").unwrap();
        assert_eq!(expr.len(), 1);
        assert_eq!(expr[0].idx, 0);
        assert!(expr[0].sub.is_none());
    }

    #[test]
    fn parse_dependency_expr_node_with_all() {
        let expr = parse_dependency_expr("1[all]").unwrap();
        assert_eq!(expr.len(), 1);
        assert_eq!(expr[0].idx, 1);
        assert_eq!(expr[0].sub, Some(DependencySubExpr::All));
    }

    #[test]
    fn parse_dependency_expr_empty_yields_empty_vec() {
        let expr = parse_dependency_expr("").unwrap();
        assert!(expr.is_empty());
    }

    #[test]
    fn parse_dependency_expr_malformed_is_error() {
        assert!(matches!(
            parse_dependency_expr("1[all"),
            Err(IbsError::InvalidDependencyExpr { .. })
        ));
    }

    #[test]
    fn build_instruction_attaches_parsed_dependencies() {
        let mut s = entry(
            1,
            StepOwner::Orchestrator,
            RecipeStepType::Component,
            vec![uuid(4)],
            vec![],
        );
        s.dependencies = Some("1[all]".into());
        let sds = vec![sd(0, vec![s])];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        let dep = bi.orchestrator_steps[0].dependencies.as_ref().unwrap();
        assert_eq!(dep.len(), 1);
        assert_eq!(dep[0].sub, Some(DependencySubExpr::All));
    }

    #[test]
    fn build_instruction_unknown_desc_idx_is_error() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Orchestrator,
                RecipeStepType::Component,
                vec![uuid(4)],
                vec![],
            )],
        )];
        assert!(matches!(
            build_instruction("5:0-5:E", &sds, &[], true),
            Err(IbsError::UnknownDescIdx { desc_idx: 5 })
        ));
    }

    #[test]
    fn build_instruction_passes_llm_call_required_through() {
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Orchestrator,
                RecipeStepType::Text,
                vec![],
                vec![],
            )],
        )];
        let bi_tier0 = build_instruction("0:0-0:E", &sds, &[], false).unwrap();
        assert!(!bi_tier0.llm_call_required);
        let bi_tier1 = build_instruction("0:0-0:E", &sds, &[], true).unwrap();
        assert!(bi_tier1.llm_call_required);
    }

    #[test]
    fn build_instruction_and_dependency_node_serde_roundtrips() {
        let bi = BuildInstruction {
            llm_call_required: false,
            variable_patterns: vec![VariablePattern {
                name: "dir".into(),
                pattern: None,
                description: None,
            }],
            basic_prompt_section_refs: vec!["§skill-ls".into()],
            rust_steps: vec![IbsRecipeStep {
                step_id: "0:2".into(),
                knowledge: StepOwner::Rust,
                step_type: RecipeStepType::Component,
                include: vec![uuid(9)],
                tool_bindings: vec![binding("ls")],
                info: None,
                dependencies: Some(vec![DependencyNode {
                    idx: 0,
                    sub: Some(DependencySubExpr::All),
                }]),
            }],
            orchestrator_steps: vec![],
        };
        let v = serde_json::to_value(&bi).unwrap();
        let back: BuildInstruction = serde_json::from_value(v).unwrap();
        assert_eq!(bi, back);
    }

    // -------------------------------------------------------------------------
    // E.3 — extract_template_slots (Phase M.4 canonical tests)
    // -------------------------------------------------------------------------

    #[test]
    fn extract_template_slots_single_slot_prefix_suffix_anchored() {
        let slots = extract_template_slots(
            "show me files in the % directory",
            "show me files in the /tmp directory",
        );
        assert_eq!(slots, vec![("slot0".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn extract_template_slots_two_slots_middle_separator() {
        let slots = extract_template_slots("search for % in %", "search for TODO in /src");
        assert_eq!(
            slots,
            vec![
                ("slot0".to_string(), "TODO".to_string()),
                ("slot1".to_string(), "/src".to_string()),
            ]
        );
    }

    #[test]
    fn extract_template_slots_no_slots_returns_empty() {
        // A literal template (no `%`) has no slots to extract.
        let slots = extract_template_slots("no slots here", "no slots here");
        assert!(slots.is_empty());
    }

    #[test]
    fn extract_template_slots_trailing_percent() {
        // "search for %" → prefix "search for ", suffix "" → slot0 is the tail.
        let slots = extract_template_slots("search for %", "search for TODO");
        assert_eq!(slots, vec![("slot0".to_string(), "TODO".to_string())]);
    }

    #[test]
    fn extract_template_slots_leading_percent() {
        // "% directory" → prefix "", suffix " directory" → slot0 is the head.
        let slots = extract_template_slots("% directory", "/tmp directory");
        assert_eq!(slots, vec![("slot0".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn extract_template_slots_adjacent_slots_degenerate() {
        // Truly adjacent slots ("%%", no literal between) are a Q1 hard error at
        // authoring time (Phase I). The pure helper still produces a defined
        // (degenerate) result: slot0 collapses to empty, slot1 takes the
        // remainder. This test documents that degenerate path; a validated
        // recipe never reaches it.
        let slots = extract_template_slots("%%", "ab");
        assert_eq!(slots[0].0, "slot0");
        assert!(slots[0].1.is_empty(), "adjacent slot0 collapses to empty");
        assert_eq!(slots[1], ("slot1".to_string(), "ab".to_string()));
    }

    #[test]
    fn extract_template_slots_missing_middle_separator_returns_partial() {
        // When a middle separator is absent the helper stops: slots already
        // extracted are returned, the rest are dropped (unextractable). With a
        // 3-slot template "a % b % c % d" whose " c " separator is missing in
        // the user text, only slot0 is recoverable.
        let slots = extract_template_slots("a % b % c % d", "a 1 b 2 x 3 d");
        assert_eq!(slots, vec![("slot0".to_string(), "1".to_string())]);
    }

    // -------------------------------------------------------------------------
    // E.3 — capture_variables (Phase M.5 refinement)
    // -------------------------------------------------------------------------

    fn vp(name: &str, pattern: Option<&str>) -> VariablePattern {
        VariablePattern {
            name: name.to_string(),
            pattern: pattern.map(str::to_string),
            description: None,
        }
    }

    #[test]
    fn capture_variables_empty_patterns_keeps_positional() {
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[],
        );
        assert_eq!(vars, vec![("slot0".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_named_group_transforms_and_renames() {
        // The pattern carries a named group `dir`; its captured value replaces
        // the raw auto-extracted value AND the slot is renamed to `dir`.
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", Some(r"(?P<dir>/[\w./-]+)"))],
        );
        assert_eq!(vars, vec![("dir".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_validation_only_pattern_renames_keeps_raw() {
        // No named group → the pattern only validates; the raw value is kept
        // and the slot is renamed to the semantic name.
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", Some(r"^/\w+$"))],
        );
        assert_eq!(vars, vec![("dir".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_validation_failure_demotes_keeps_positional() {
        // The regex does not match "/tmp" → demote: raw value + positional name.
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", Some(r"^[0-9]+$"))],
        );
        assert_eq!(vars, vec![("slot0".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_no_pattern_renames_only() {
        // pattern: None → rename to the semantic name, value unchanged.
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", None)],
        );
        assert_eq!(vars, vec![("dir".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_bad_regex_demotes_keeps_positional() {
        // An uncompiled regex is a Q1 authoring error (Phase I); at runtime the
        // slot is demoted — raw value + positional name (no turn failure).
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", Some("["))],
        );
        assert_eq!(vars, vec![("slot0".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_more_patterns_than_slots_ignores_extras() {
        // Two patterns but one slot → only the first applies; the second is
        // dangling (Q1 warns at Phase I) and ignored.
        let vars = capture_variables(
            "show me files in the % directory",
            "show me files in the /tmp directory",
            &[vp("dir", None), vp("extra", None)],
        );
        assert_eq!(vars, vec![("dir".to_string(), "/tmp".to_string())]);
    }

    #[test]
    fn capture_variables_two_slots_two_patterns_paired_by_order() {
        let vars = capture_variables(
            "search for % in %",
            "search for TODO in /src",
            &[vp("term", None), vp("path", Some(r"^/.*"))],
        );
        assert_eq!(
            vars,
            vec![
                ("term".to_string(), "TODO".to_string()),
                ("path".to_string(), "/src".to_string()),
            ]
        );
    }

    // -------------------------------------------------------------------------
    // E.3 — substitute_vars (§0.20.3 string substitution)
    // -------------------------------------------------------------------------

    #[test]
    fn substitute_vars_replaces_all_placeholders() {
        let vars = vec![
            ("dir".to_string(), "/tmp".to_string()),
            ("file".to_string(), "a.txt".to_string()),
        ];
        assert_eq!(
            substitute_vars("run {{vars.dir}}/{{vars.file}}", &vars),
            "run /tmp/a.txt"
        );
    }

    #[test]
    fn substitute_vars_distinct_names_do_not_overlap() {
        // `{{vars.dir}}` and `{{vars.directory}}` are different literals.
        let vars = vec![
            ("dir".to_string(), "x".to_string()),
            ("directory".to_string(), "y".to_string()),
        ];
        assert_eq!(
            substitute_vars("{{vars.dir}} {{vars.directory}}", &vars),
            "x y"
        );
    }

    #[test]
    fn substitute_vars_unresolved_placeholder_left_intact() {
        // No matching capture → placeholder stays (Q1 "missing template" is an
        // authoring error caught at Phase I, not a runtime fabrication).
        let vars = vec![("dir".to_string(), "/tmp".to_string())];
        assert_eq!(
            substitute_vars("{{vars.missing}}", &vars),
            "{{vars.missing}}"
        );
    }

    #[test]
    fn substitute_vars_no_placeholders_unchanged() {
        let vars = vec![("dir".to_string(), "/tmp".to_string())];
        assert_eq!(substitute_vars("plain text", &vars), "plain text");
    }

    // -------------------------------------------------------------------------
    // E.3 — substitute_vars_in_value (§0.4.1 ToolBinding params substitution)
    // -------------------------------------------------------------------------

    #[test]
    fn substitute_vars_in_value_substitutes_string_leaves_only() {
        let vars = vec![("dir".to_string(), "/tmp".to_string())];
        let input = serde_json::json!({ "path": "{{vars.dir}}/a.txt", "n": 3, "flag": true });
        let out = substitute_vars_in_value(&input, &vars);
        assert_eq!(out["path"], serde_json::json!("/tmp/a.txt"));
        assert_eq!(
            out["n"],
            serde_json::json!(3),
            "non-string leaves pass through"
        );
        assert_eq!(out["flag"], serde_json::json!(true));
    }

    #[test]
    fn substitute_vars_in_value_walks_nested_arrays_and_objects() {
        let vars = vec![("dir".to_string(), "/tmp".to_string())];
        let input = serde_json::json!([
            { "inner": "{{vars.dir}}" },
            ["{{vars.dir}}", "literal"],
        ]);
        let out = substitute_vars_in_value(&input, &vars);
        assert_eq!(out[0]["inner"], serde_json::json!("/tmp"));
        assert_eq!(out[1][0], serde_json::json!("/tmp"));
        assert_eq!(out[1][1], serde_json::json!("literal"));
    }

    #[test]
    fn substitute_vars_in_value_preserves_object_keys() {
        let vars = vec![("dir".to_string(), "/tmp".to_string())];
        let input = serde_json::json!({ "{{vars.dir}}": "value" });
        // Only string VALUES are substituted; keys are preserved verbatim.
        let out = substitute_vars_in_value(&input, &vars);
        let keys: Vec<&String> = out.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec![&"{{vars.dir}}".to_string()]);
        assert_eq!(out["{{vars.dir}}"], serde_json::json!("value"));
    }

    // -----------------------------------------------------------------------
    // Phase E.6 pure-unit tests (the DB-dependent half lives in the
    // composition `tests/` tier — see fetch_for_turn.rs).
    // -----------------------------------------------------------------------

    #[test]
    fn build_instruction_both_step_include_uuid_appears_in_both_channels() {
        // Plan E.6 unit #2: a `knowledge: both` step's `include` UUID must
        // appear in BOTH the rust channel and the orchestrator channel of the
        // compiled BuildInstruction (§0.5/§0.7 partition — Both emits to both;
        // rust copy drops `info`, orchestrator copy drops `tool_bindings`). The
        // live DB path (fetch_for_turn) then fetches that UUID into both
        // `rust_items` and `orchestrator_items` — see the integration test
        // `full_intent_match_correct_channel_split_by_class_code`.
        let id = uuid(9);
        let sds = vec![sd(
            0,
            vec![entry(
                1,
                StepOwner::Both,
                RecipeStepType::Component,
                vec![id],
                vec![],
            )],
        )];
        let bi = build_instruction("0:0-0:E", &sds, &[], true).expect("both step compiles");
        assert!(
            bi.rust_steps.iter().any(|s| s.include.contains(&id)),
            "both-step include UUID must be in rust_steps"
        );
        assert!(
            bi.orchestrator_steps
                .iter()
                .any(|s| s.include.contains(&id)),
            "both-step include UUID must be in orchestrator_steps"
        );
    }

    #[test]
    fn capture_variables_then_substitute_replaces_vars_dir_in_body_and_params() {
        // Plan E.6 unit #5 (pure-mechanism half): prove the E.4-wired
        // substitution CHAIN — `capture_variables` (auto-extract `%`-template
        // slots + refine via the matched variant's `variable_patterns`) feeds
        // `substitute_vars` / `substitute_vars_in_value`, which replace
        // `{{vars.NAME}}` placeholders with the captured values. At Phase E
        // `fetch_for_turn` uses exact-match intent (`ii.input_text = $5`, no
        // `%` template) so `capture_variables(query, query, …)` yields `vars=[]`
        // and substitution is a wired NO-OP through the live path — see the DB
        // test `substitution_noop_at_phase_e_preserves_placeholder`. Phase M
        // switches `resolve_intent` to `%`-template matching, feeding a
        // non-empty `vars` through THIS chain; exercising it directly covers
        // the mechanism today, not only at Phase M.
        let vps = vec![VariablePattern {
            name: "dir".into(),
            pattern: None,
            description: None,
        }];
        let vars = capture_variables("list files of %", "list files of /tmp", &vps);
        assert_eq!(vars, vec![("dir".to_string(), "/tmp".to_string())]);

        // §0.20.3 — body substitution (orchestrator_items[].effective_content).
        assert_eq!(
            substitute_vars("Run ls inside {{vars.dir}} now", &vars),
            "Run ls inside /tmp now"
        );
        // §0.4.1 — ToolBinding params substitution (recursive JSON leaves).
        let params = serde_json::json!({
            "flags": "-la",
            "dir": "{{vars.dir}}",
            "nested": { "path": "{{vars.dir}}/sub" },
            "list": ["{{vars.dir}}", "plain"],
            "count": 3
        });
        let sub = substitute_vars_in_value(&params, &vars);
        assert_eq!(sub["dir"], "/tmp");
        assert_eq!(sub["nested"]["path"], "/tmp/sub");
        assert_eq!(sub["list"][0], "/tmp");
        assert_eq!(sub["list"][1], "plain");
        assert_eq!(sub["count"], 3);
    }

    #[test]
    fn step_context_spec_from_class_code_maps_all_classes_and_skips_toolskill() {
        // v3 Phase F.5 (Q-F7-1 / Q-F7-2): the prose formatter derives a
        // `StepContextSpec` per item; class 13 (ToolSkill) and 11 (reserved)
        // MUST map to `None` so they are never emitted into
        // `orchestrator_content` (§0.9 invariant — ToolSkill is Rust-channel
        // only). Every other class maps to its Capitalized category variant.
        assert_eq!(StepContextSpec::from_class_code(13), None);
        assert_eq!(StepContextSpec::from_class_code(11), None);

        // Plan's 5 orchestrator-channel classes + the skill sub-range.
        assert_eq!(
            StepContextSpec::from_class_code(1),
            Some(StepContextSpec::Skill)
        );
        assert_eq!(
            StepContextSpec::from_class_code(3),
            Some(StepContextSpec::Skill)
        );
        assert_eq!(
            StepContextSpec::from_class_code(12),
            Some(StepContextSpec::Spec)
        );
        assert_eq!(
            StepContextSpec::from_class_code(21),
            Some(StepContextSpec::Recipe)
        );
        assert_eq!(
            StepContextSpec::from_class_code(22),
            Some(StepContextSpec::PythonCode)
        );
        assert_eq!(
            StepContextSpec::from_class_code(23),
            Some(StepContextSpec::Catalogue)
        );

        // Extended categories (Q-F7-1 broad-scan coverage).
        assert_eq!(
            StepContextSpec::from_class_code(4),
            Some(StepContextSpec::Extension)
        );
        assert_eq!(
            StepContextSpec::from_class_code(9),
            Some(StepContextSpec::Extension)
        );
        assert_eq!(
            StepContextSpec::from_class_code(10),
            Some(StepContextSpec::Orchestrator)
        );
        assert_eq!(
            StepContextSpec::from_class_code(0),
            Some(StepContextSpec::Tool)
        );
        assert_eq!(
            StepContextSpec::from_class_code(14),
            Some(StepContextSpec::Plan)
        );
        assert_eq!(
            StepContextSpec::from_class_code(15),
            Some(StepContextSpec::Summary)
        );
        assert_eq!(
            StepContextSpec::from_class_code(16),
            Some(StepContextSpec::Action)
        );
        assert_eq!(
            StepContextSpec::from_class_code(17),
            Some(StepContextSpec::Docu)
        );
        assert_eq!(
            StepContextSpec::from_class_code(18),
            Some(StepContextSpec::Lesson)
        );
        assert_eq!(
            StepContextSpec::from_class_code(19),
            Some(StepContextSpec::Issue)
        );
        assert_eq!(
            StepContextSpec::from_class_code(20),
            Some(StepContextSpec::Note)
        );
        assert_eq!(
            StepContextSpec::from_class_code(50),
            Some(StepContextSpec::Scaffold)
        );
        // Unknown code → Component fallback.
        assert_eq!(
            StepContextSpec::from_class_code(99),
            Some(StepContextSpec::Component)
        );

        // Headings are Capitalized category labels (Q-F7-2 / Q-F7-case), NOT
        // the lowercase `class_label()` subtypes.
        assert_eq!(StepContextSpec::Skill.heading(), "Skill");
        assert_eq!(StepContextSpec::PythonCode.heading(), "PythonCode");
        assert_eq!(StepContextSpec::Extension.heading(), "Extension");
        assert_eq!(StepContextSpec::Action.heading(), "Action");
        assert_eq!(StepContextSpec::Component.heading(), "Component");
    }
}
