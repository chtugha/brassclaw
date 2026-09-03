//! Composition system — the IBS IS the composition system (C.4.5.17, F4).
//!
//! Turns a compiled [`BuildInstruction`] + a component resolver + bound slot
//! variables into the predefined Monty-facing [`ComposedProgram`]. This is the
//! deterministic, machine-readable form handed to Monty by
//! `host.compose_orchestrator`; Monty then iterates `steplist`, consults the
//! `skills` array for exact tool usage, and runs each step's `executable_code`
//! via `host.run_program` (per-step nested `execute_code`, fresh isolation per
//! step — mirrors `execute_tier_zero_channel`).
//!
//! Pure + DB-free: the DB-bound resolution (fetching components by UUID,
//! resolving cdylib artifact paths) is behind the [`ComponentResolver`] trait,
//! supplied by the `host.compose_orchestrator` handler. Unit tests use a
//! fixture resolver.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::memory::instruction_builder::BuildInstruction;
use crate::types::ibs::ToolBinding;

/// Class code for PythonCode components — the executable body of an
/// orchestrator step.
const CLASS_PYTHON_CODE: i16 = 22;
/// Class codes for Skill components — collected into the `skills` array.
const CLASS_SKILLS: &[i16] = &[1, 2, 3, 13];

/// A skill handed to Monty in the `skills` array (C.4.5.17 Enhanced-C). Skills
/// carry the exact usage of Tools; the steplist steps stay high-level and
/// consult this array on demand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillRef {
    /// UUID of the Skill (class 1-3) or ToolSkill (class 13) component.
    pub id: Uuid,
    /// Component class code (1, 2, 3, or 13).
    pub class_code: i16,
    /// Skill name (e.g. "skill-read-file").
    pub name: String,
    /// Skill body / code snippet — the exact tool-usage instructions Monty
    /// consults while working through the steplist.
    pub body: String,
}

/// One Rust-side dynamic-tool load directive, derived from a rust-channel
/// `ToolBinding`. Built into a `CdylibLoadDirective` by the
/// `host.compose_orchestrator` handler and applied to the `DynamicToolLoader`
/// before the program is handed to Monty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RustDirective {
    /// UUID of the Tool (class 0) row.
    pub tool_id: Uuid,
    /// Registered tool/capability name (e.g. "read_file"). Matches the
    /// `host.<tool_name>` call site.
    pub tool_name: String,
    /// Filesystem path to the compiled cdylib artifact, resolved by the
    /// handler's resolver.
    pub artifact_path: String,
}

/// One step in the composed orchestrator steplist. Monty iterates these in
/// order, consulting `composed.skills` for exact tool usage, and runs
/// `executable_code` via `host.run_program`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposedStep {
    /// Stable IBS step id `"{desc_idx}:{stepnumber}"`.
    pub step_id: String,
    /// High-level human-readable instruction for this step (the resolved
    /// executable component's description, else the step's WebUI `info`).
    /// Granular tool-call syntax lives in the `skills` array, not here.
    pub instructions: String,
    /// Concrete Python code for this step — the resolved PythonCode (class 22)
    /// component's `content`, with `{{vars.NAME}}` placeholders already bound
    /// to concrete values by the composer. Run via `host.run_program`.
    pub executable_code: String,
    /// Tool bindings this step exercises (populated for `Both`-channel steps
    /// from the matching rust step's `tool_bindings`; empty for
    /// orchestrator-only steps). Lets Monty declare which tools a step uses so
    /// it can consult the relevant skills.
    #[serde(default)]
    pub tool_bindings: Vec<ToolBinding>,
}

/// The predefined Monty-facing composed program (C.4.5.17). Returned by
/// `host.compose_orchestrator` as a Monty dict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComposedProgram {
    /// Skills array handed to Monty — consulted while working through the
    /// steplist for exact tool usage. Collected from Skill (1-3) / ToolSkill
    /// (13) components included by any orchestrator step (deduped by id).
    #[serde(default)]
    pub skills: Vec<SkillRef>,
    /// Ordered orchestrator step list. Monty iterates this and runs each
    /// step's `executable_code` via `host.run_program`.
    #[serde(default)]
    pub steplist: Vec<ComposedStep>,
    /// Rust-side dynamic-tool load directives. Applied to the
    /// `DynamicToolLoader` by the handler before the program is returned.
    #[serde(default)]
    pub rust_directives: Vec<RustDirective>,
    /// Bound slot variables (slot name → concrete value), baked into the
    /// `executable_code` via `{{vars.NAME}}` substitution by the composer.
    #[serde(default)]
    pub variables: Vec<(String, String)>,
    /// All steplist `executable_code` concatenated in step order — the
    /// pre-assembled runnable form. Available for single-shot recipes or
    /// fallback; the per-step run path (A) is the primary execution mode.
    #[serde(default)]
    pub assembled_program: String,
    /// Recipe tier hint carried from `BuildInstruction.llm_call_required`
    /// for the orchestrator driver ("tier0" / "tier1").
    pub tier: String,
}

/// A component resolved by the DB-bound [`ComponentResolver`]. The composer
/// classifies by `class_code` to route into `executable_code` (22), `skills`
/// (1-3, 13), or `rust_directives` (0).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedComponent {
    pub class_code: i16,
    pub name: String,
    /// Primary text field — for PythonCode (22) the executable body; for Skill
    /// (1-3) the skill body; for ToolSkill (13) the code snippet.
    pub content: String,
    /// Description — becomes a step's `instructions` when this is the step's
    /// executable component.
    pub description: String,
    /// cdylib artifact path — only set for Tool (class 0) components.
    pub cdylib_artifact_path: Option<String>,
}

/// DB-bound resolution of component UUIDs. Implemented by the
/// `host.compose_orchestrator` handler (pg_pool-backed); fixture-resolved in
/// unit tests.
pub trait ComponentResolver {
    fn resolve(&self, id: Uuid) -> Option<ResolvedComponent>;
}

/// Compose a [`BuildInstruction`] + resolver + bound variables into the
/// predefined [`ComposedProgram`] (C.4.5.17). Pure + deterministic — the sole
/// DB-bound input is the [`ComponentResolver`].
pub fn compose_program(
    instruction: &BuildInstruction,
    resolver: &dyn ComponentResolver,
    variables: &[(String, String)],
) -> ComposedProgram {
    // step_id -> rust-channel tool_bindings. `Both` steps keep their bindings
    // on the rust copy (the orchestrator copy is emptied by `build_instruction`);
    // orchestrator-only steps have no rust counterpart.
    let mut rust_bindings: HashMap<&str, &Vec<ToolBinding>> = HashMap::new();
    for r in &instruction.rust_steps {
        rust_bindings.insert(r.step_id.as_str(), &r.tool_bindings);
    }

    let mut skills: Vec<SkillRef> = Vec::new();
    let mut seen_skills: HashSet<Uuid> = HashSet::new();
    let mut steplist: Vec<ComposedStep> = Vec::new();
    let mut assembled_parts: Vec<String> = Vec::new();

    for step in &instruction.orchestrator_steps {
        let mut executable_code = String::new();
        let mut instructions = String::new();
        for id in &step.include {
            let Some(c) = resolver.resolve(*id) else {
                continue;
            };
            if c.class_code == CLASS_PYTHON_CODE && executable_code.is_empty() {
                executable_code = bind_variables(&c.content, variables);
                instructions = c.description.clone();
            } else if CLASS_SKILLS.contains(&c.class_code) && seen_skills.insert(*id) {
                skills.push(SkillRef {
                    id: *id,
                    class_code: c.class_code,
                    name: c.name.clone(),
                    body: c.content.clone(),
                });
            }
        }
        if instructions.is_empty()
            && let Some(info) = step.info.as_deref()
        {
            instructions = info.to_string();
        }

        let tool_bindings = rust_bindings
            .get(step.step_id.as_str())
            .map(|v| (*v).clone())
            .unwrap_or_default();

        if !executable_code.is_empty() {
            assembled_parts.push(executable_code.clone());
        }

        steplist.push(ComposedStep {
            step_id: step.step_id.clone(),
            instructions,
            executable_code,
            tool_bindings,
        });
    }

    // rust_directives from rust-channel tool_bindings (deduped by tool_id);
    // artifact_path resolved via the resolver.
    let mut rust_directives: Vec<RustDirective> = Vec::new();
    let mut seen_tools: HashSet<Uuid> = HashSet::new();
    for r in &instruction.rust_steps {
        for b in &r.tool_bindings {
            if !seen_tools.insert(b.tool_id) {
                continue;
            }
            let artifact_path = resolver
                .resolve(b.tool_id)
                .and_then(|c| c.cdylib_artifact_path.clone())
                .unwrap_or_default();
            rust_directives.push(RustDirective {
                tool_id: b.tool_id,
                tool_name: b.tool_name.clone(),
                artifact_path,
            });
        }
    }

    let tier = if instruction.llm_call_required {
        "tier1"
    } else {
        "tier0"
    };

    ComposedProgram {
        skills,
        steplist,
        rust_directives,
        variables: variables.to_vec(),
        assembled_program: assembled_parts.join("\n\n"),
        tier: tier.to_string(),
    }
}

/// Substitute `{{vars.NAME}}` placeholders in `text` with bound values.
/// `{{user_input}}` / `{{component_name}}` binding is a handler concern
/// (those have no `vars.` prefix).
fn bind_variables(text: &str, variables: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (name, value) in variables {
        let pattern = format!("{{{{vars.{}}}}}", name);
        out = out.replace(&pattern, value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::instruction_builder::{
        BuildInstruction, IbsRecipeStep, RecipeStepType, StepOwner,
    };
    use crate::types::ibs::{ErrorPolicy, ToolBinding};
    use std::collections::HashMap;
    use uuid::Uuid;

    struct FixtureResolver(HashMap<Uuid, ResolvedComponent>);
    impl ComponentResolver for FixtureResolver {
        fn resolve(&self, id: Uuid) -> Option<ResolvedComponent> {
            self.0.get(&id).cloned()
        }
    }

    fn uuid(seed: &str) -> Uuid {
        // Deterministic distinct UUIDs from a seed.
        let mut bytes = [0u8; 16];
        let s = seed.as_bytes();
        for (i, b) in s.iter().take(16).enumerate() {
            bytes[i] = *b;
        }
        Uuid::from_bytes(bytes)
    }

    fn py(id: Uuid, name: &str, desc: &str, content: &str) -> (Uuid, ResolvedComponent) {
        (
            id,
            ResolvedComponent {
                class_code: 22,
                name: name.to_string(),
                content: content.to_string(),
                description: desc.to_string(),
                cdylib_artifact_path: None,
            },
        )
    }

    fn skill(id: Uuid, name: &str, body: &str) -> (Uuid, ResolvedComponent) {
        (
            id,
            ResolvedComponent {
                class_code: 1,
                name: name.to_string(),
                content: body.to_string(),
                description: name.to_string(),
                cdylib_artifact_path: None,
            },
        )
    }

    fn tool(id: Uuid, name: &str, path: &str) -> (Uuid, ResolvedComponent) {
        (
            id,
            ResolvedComponent {
                class_code: 0,
                name: name.to_string(),
                content: String::new(),
                description: name.to_string(),
                cdylib_artifact_path: Some(path.to_string()),
            },
        )
    }

    fn orch_step(step_id: &str, include: Vec<Uuid>, info: Option<&str>) -> IbsRecipeStep {
        IbsRecipeStep {
            step_id: step_id.to_string(),
            knowledge: StepOwner::Orchestrator,
            step_type: RecipeStepType::Component,
            include,
            tool_bindings: Vec::new(),
            info: info.map(str::to_string),
            dependencies: None,
        }
    }

    fn both_step(step_id: &str, include: Vec<Uuid>) -> IbsRecipeStep {
        IbsRecipeStep {
            step_id: step_id.to_string(),
            knowledge: StepOwner::Both,
            step_type: RecipeStepType::Component,
            include,
            tool_bindings: Vec::new(),
            info: None,
            dependencies: None,
        }
    }

    fn rust_step(step_id: &str, bindings: Vec<ToolBinding>) -> IbsRecipeStep {
        IbsRecipeStep {
            step_id: step_id.to_string(),
            knowledge: StepOwner::Rust,
            step_type: RecipeStepType::Component,
            include: Vec::new(),
            tool_bindings: bindings,
            info: None,
            dependencies: None,
        }
    }

    #[test]
    fn composes_deterministic_program_from_fixture_components() {
        let pc1 = uuid("pycode-step1-----");
        let pc2 = uuid("pycode-step2-----");
        let skl = uuid("skill-read--------");
        let tol = uuid("tool-readfile-----");

        let resolver = FixtureResolver(
            [
                py(
                    pc1,
                    "pc-step1",
                    "Read the file given by slot0",
                    "data = host.read_file(path=\"{{vars.slot0}}\")\nprint(data)",
                ),
                py(
                    pc2,
                    "pc-step2",
                    "Print the file data",
                    "print(data)",
                ),
                skill(skl, "skill-read-file", "# to read a file call host.read_file(path=...)"),
                tool(tol, "read_file", "/tools/read_file.so"),
            ]
            .into_iter()
            .collect(),
        );

        let instruction = BuildInstruction {
            llm_call_required: false,
            variable_patterns: Vec::new(),
            basic_prompt_section_refs: Vec::new(),
            rust_steps: vec![rust_step(
                "0:2",
                vec![ToolBinding {
                    tool_id: tol,
                    tool_name: "read_file".to_string(),
                    params: serde_json::json!({}),
                    error_policy: ErrorPolicy::Fail,
                }],
            )],
            orchestrator_steps: vec![
                orch_step("0:1", vec![pc1], None),
                both_step("0:2", vec![pc2, skl]),
            ],
        };

        let program = compose_program(
            &instruction,
            &resolver,
            &[("slot0".to_string(), "/tmp/notes.txt".to_string())],
        );

        // tier from llm_call_required=false
        assert_eq!(program.tier, "tier0");

        // skills: the one Skill include, deduped, class 1
        assert_eq!(program.skills.len(), 1);
        assert_eq!(program.skills[0].id, skl);
        assert_eq!(program.skills[0].class_code, 1);
        assert_eq!(program.skills[0].name, "skill-read-file");

        // steplist: 2 steps in order
        assert_eq!(program.steplist.len(), 2);

        // step 0:1 — orchestrator-only, no rust counterpart → empty tool_bindings
        assert_eq!(program.steplist[0].step_id, "0:1");
        assert_eq!(program.steplist[0].instructions, "Read the file given by slot0");
        assert_eq!(
            program.steplist[0].executable_code,
            "data = host.read_file(path=\"/tmp/notes.txt\")\nprint(data)"
        );
        assert!(program.steplist[0].tool_bindings.is_empty());

        // step 0:2 — Both, picks up the rust counterpart's tool_bindings
        assert_eq!(program.steplist[1].step_id, "0:2");
        assert_eq!(program.steplist[1].instructions, "Print the file data");
        assert_eq!(program.steplist[1].executable_code, "print(data)");
        assert_eq!(program.steplist[1].tool_bindings.len(), 1);
        assert_eq!(program.steplist[1].tool_bindings[0].tool_name, "read_file");

        // rust_directives: the one tool, with resolved artifact_path
        assert_eq!(program.rust_directives.len(), 1);
        assert_eq!(program.rust_directives[0].tool_id, tol);
        assert_eq!(program.rust_directives[0].tool_name, "read_file");
        assert_eq!(
            program.rust_directives[0].artifact_path,
            "/tools/read_file.so"
        );

        // variables carried through
        assert_eq!(
            program.variables,
            vec![("slot0".to_string(), "/tmp/notes.txt".to_string())]
        );

        // assembled_program = step1 code (bound) + "\n\n" + step2 code
        assert_eq!(
            program.assembled_program,
            "data = host.read_file(path=\"/tmp/notes.txt\")\nprint(data)\n\nprint(data)"
        );
    }

    #[test]
    fn empty_instruction_yields_empty_program() {
        let resolver = FixtureResolver(HashMap::new());
        let instruction = BuildInstruction {
            llm_call_required: true,
            variable_patterns: Vec::new(),
            basic_prompt_section_refs: Vec::new(),
            rust_steps: Vec::new(),
            orchestrator_steps: Vec::new(),
        };
        let program = compose_program(&instruction, &resolver, &[]);
        assert!(program.skills.is_empty());
        assert!(program.steplist.is_empty());
        assert!(program.rust_directives.is_empty());
        assert_eq!(program.tier, "tier1");
        assert_eq!(program.assembled_program, "");
    }

    #[test]
    fn unresolved_includes_are_skipped_silently() {
        let pc = uuid("pycode-only-------");
        let resolver = FixtureResolver(
            [py(pc, "pc", "do thing", "print(1)")].into_iter().collect(),
        );
        // step 0:1 references a UUID the resolver does not know
        let instruction = BuildInstruction {
            llm_call_required: false,
            variable_patterns: Vec::new(),
            basic_prompt_section_refs: Vec::new(),
            rust_steps: Vec::new(),
            orchestrator_steps: vec![
                orch_step("0:1", vec![uuid("missing----------")], None),
                orch_step("0:2", vec![pc], None),
            ],
        };
        let program = compose_program(&instruction, &resolver, &[]);
        assert_eq!(program.steplist.len(), 2);
        assert_eq!(program.steplist[0].executable_code, "");
        assert_eq!(program.steplist[0].instructions, "");
        assert_eq!(program.steplist[1].executable_code, "print(1)");
        assert_eq!(program.steplist[1].instructions, "do thing");
    }

    #[test]
    fn skills_are_deduped_across_steps() {
        let skl = uuid("skill-shared------");
        let pc1 = uuid("pycode-a----------");
        let pc2 = uuid("pycode-b----------");
        let resolver = FixtureResolver(
            [
                skill(skl, "skill-shared", "body"),
                py(pc1, "pc-a", "a", "print(1)"),
                py(pc2, "pc-b", "b", "print(2)"),
            ]
            .into_iter()
            .collect(),
        );
        let instruction = BuildInstruction {
            llm_call_required: false,
            variable_patterns: Vec::new(),
            basic_prompt_section_refs: Vec::new(),
            rust_steps: Vec::new(),
            orchestrator_steps: vec![
                orch_step("0:1", vec![pc1, skl], None),
                orch_step("0:2", vec![pc2, skl], None),
            ],
        };
        let program = compose_program(&instruction, &resolver, &[]);
        assert_eq!(program.skills.len(), 1);
        assert_eq!(program.skills[0].id, skl);
    }
}
