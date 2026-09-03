# Phase HI — Common Component Syntax + Composition System

Subplan of `./saved_plan_to_v3.md`. Inserted between **Step B** (DONE —
dual-nature recipe syntax) and **Step C.5** (basic-mode orchestrator script).
**Why here:** `host.compose_orchestrator` (C.5/C.6) can only emit exact,
reproducible per-step Python once every component class shares one
unambiguous machine-readable syntax + the composition rules that turn it into
rust directives + an exact python program. Phase HI is that definition + the
composition-system that enforces it. The pre-existing Zenflow plan step
`d61dca66-…` ("Phase HI - Syntax Invention") is this subplan's tracker; its
original Step-B scope is DONE and superseded by this expanded scope.

## Objective

Extend the **Phase HI / Step B dual-nature recipe syntax** (Recipe, class 21)
to **every component class**, so that:

1. Every component carries a **machine-readable exact-logic** form that
   **always reproduces the same** orchestrator + Rust results, AND a concise
   **human-readable explanation**.
2. The **composition system** (`host.compose_orchestrator` rewrite) reads any
   component's instructions, **splits rust / orchestrator**, composes/compiles
   what the Rust executioner needs, and **emits exact Python** for the
   orchestrator with concrete instructions for every step.
3. Behaviour changes need **no code changes — only the component is altered**.

## Prerequisite (DONE)

Phase HI / Step B — dual-nature recipe syntax:
- Machine-readable (untouched): `RecipeVariant.step_link` → IBS
  `build_instruction` → `BuildInstruction` (`rust_steps` + `orchestrator_steps`).
- Human-readable: `Recipe.description`, `RecipeVariant.description`,
  `StepDescriptionEntry.label`, `StepEntry.goal`.
- Q1 gate: `check_variant_descriptions` (v3 variants require a variant
  description ≤ 512 chars; legacy `step_link == None` exempt).
- Shipped: B.1–B.5 (field + round-trip tests + Q1 gate + docs + both configs).

## Component class inventory (verified this portion + HI.1 audit)

Verified codes (from `doc_type_to_class_code` retrieval_source.rs:1449 +
`db_tool_source.rs` + C.2/C.3 seeds):

| Code | Label | Table | Group |
|------|-------|-------|-------|
| 0  | tool                | (no prompt text)        | execution primitive (Rust) |
| 3  | skill               | reborn_skills           | skill (orchestrator-facing narrative) |
| 12 | spec                | reborn_specs            | memory/instruction |
| 13 | tool_skill          | reborn_tool_skills      | execution primitive (Rust) |
| 14 | plan                | reborn_plans            | memory/instruction |
| 15 | summary             | reborn_summaries        | memory/instruction |
| 18 | lesson              | reborn_lessons          | memory/instruction |
| 19 | issue               | reborn_issues           | memory/instruction |
| 20 | note                | reborn_notes            | memory/instruction |
| 21 | recipe              | reborn_recipes          | instruction (dual-nature DONE — reference) |
| 22 | python_code         | reborn_python_code      | execution unit (Monty runs) |
| 23 | extension_catalogue | —                       | extension container |

**HI.1 audit** will enumerate any additional codes surfaced by
`intent_system::class_label` / `class_code_to_table` (e.g. skill sub-kinds,
extension variants, action/docu/scaffold) and fold them into this table; only
verified codes are listed above to avoid propagating unverified labels.

The C.2 seed (`seed_builtin_host.rs`) already mints the 5-component stack
(Tool + ToolSkill + PythonCode + leaf Skill + Recipe) per `host.*` tool — the
reference shape for the common syntax.

## Common-syntax contract (the a–i items)

- **a) Exact building instructions** — how code-snippet modules are assembled
  to produce the exact described Rust-executioner content; standardised
  solutions for common problems. → the IBS `BuildInstruction.rust_steps`
  (`{tool, tool_skill}`) is the universal rust-side build form; per-class
  `step_link` resolves to it.
- **b) Formatting conventions** — one canonical shape per class (field set,
  ordering, units), enforced at Q1 validation.
- **c) Python-code placeholders** — repeating code parts (functions/classes)
  factored into **reusable mini PythonCode components** (one function each,
  like includes). A PythonCode body carries `{{vars.slotN}}` / `{{name}}`
  placeholders the composition system bakes in; a placeholder may itself
  resolve to another PythonCode component (chain-loading, mirroring the
  existing skill chain-load in `select_skills`).
- **d) Architecture documentation** — how each component + system works + is
  standardised.
- **e) Command syntax** — uniform skill-design + tool-call conventions across
  all components (`host.<tool>(kw=...)` form; one leaf skill per approach).
- **f) Key-system documentation** — f1 intent-matching, f2 python-orchestrator,
  f3 validation, f4 components (extension-recipe-skill-toolskill-tool with the
  converted builtin tools as examples), f5 rust-executioner (orchestrator-
  command-driven executions rule + exceptions), f6 LLM-prompt creation + the
  prefix system, f7 kohai-sempai (duties, with/without sempai), f8 new-
  component-creation per class + exact validation criteria.
- **g) Database-structure standardisation** — one schema convention per class
  (the `reborn_*` tables): scope keys, content column, prior-knowledge column,
  class_code, prompt_uid ordering.
- **h) Variables** — the `{{vars.slotN}}` / `{{user_input}}` substitution
  contract: who bakes what, when, with what escaping.
- **i) Everything else clever/necessary** — added per-slice as discovered.

## Composition system contract (`host.compose_orchestrator` rewrite)

- **Input:** `component_id` (UUID or well-known name) + `user_input` (carries
  prompt variables — e.g. a path — to bake in).
- **Behaviour:** fetch the component by id → read its machine-readable form →
  split **rust part / orchestrator part** → compile the rust part (load
  directives for the Rust executioner: built-in tools + cdylib dlopen via the
  C.3 `DynamicToolLoader`) → assemble the **exact, self-contained Python
  program** (skills' instructions + PythonCode pieces + prompt variables
  baked in via c/h).
- **Output:** `{ program: <self-contained python string>, rust_directives: [...] }`.
- **The composer does NOT run the program.** Monty runs it via `host.run_program`
  (Monty 0.0.16 has no `exec`/`eval`/`compile` builtin — verified in the git
  checkout; a host callable is the only way for Monty to run a dynamic code
  string). The composer only assembles; **Monty reads the skill instructions +
  writes/runs the code** (per user: "skills contain instructions for monty to
  create functions/code ... write the python code for itself and run it").

## Slices (one commit each; one component class at a time to avoid OOM)

- **HI.0** — Common-syntax contract spec + composition contract (this doc's
  machine-side, finalised into a spec; the reference every later slice
  conforms to). Resolves the open forks below. **No code yet.**
- **HI.1** — Audit current syntax per class; finalise the component-class
  inventory (fold in any extra codes from `intent_system::class_label`).
  Output: an audit table appended here.
- **HI.2** — `python_code` (22) + placeholder/include mechanism (c/h).
- **HI.3** — `tool_skill` (13).
- **HI.4** — `tool` (0).
- **HI.5** — `skill` (3, + sub-kinds from HI.1) + command syntax (e).
- **HI.6** — `recipe` (21) — align the Step B dual-nature to the common
  contract (mostly verification + gap-close; already dual-nature).
- **HI.7** — Composition system: `host.compose_orchestrator` rewrite (the
  consumer of the standardised syntax) + `host.run_program` (Monty runs the
  assembled code string). Unit-tested in isolation (deterministic rust +
  python output from a fixture component).
- **HI.8** — memory/instruction classes: `spec`(12), `plan`(14),
  `summary`(15), `lesson`(18), `issue`(19), `note`(20) + any extra
  instruction classes from HI.1.
- **HI.9** — `extension_catalogue` (23) + extension variants from HI.1.
- **HI.10** — Comprehensive docs (d, e, f1–f8, g, h, i). Both configs green
  (default + `--features skills-db`); DB-schema standardisation verification;
  mark Phase HI done; commit + push; resume C.5 (basic-mode orchestrator
  script, now that compose_orchestrator exists).

## Open forks (resolve before HI.2)

- **F-HI-1 — Universal machine form.** Is the IBS `BuildInstruction`
  (`rust_steps` + `orchestrator_steps`) + per-class `step_link` the universal
  machine-readable form for ALL classes (extended from Recipe), or does each
  class keep its own machine form unified only by a shared contract?
  - A: extend the existing IBS `step_link`/`build_instruction` shape to ALL
    classes — each declares its build instructions in the same structured form.
  - B: a new unified declarative schema (JSON/YAML) every class uses, compiled
    to rust + python by the composition-system.
  - C: each class keeps its own machine form, unified only by a shared
    contract/spec (no single struct).
- **F-HI-2 — Placeholder/include mechanism (c).** How do PythonCode
  components carry placeholders (variables that are small PythonCode
  components)?
  - A: `{{vars.slotN}}` / `{{component_name}}` template syntax the composer
    bakes in before execution (IBS precedent).
  - B: a structural include/import of mini-PythonCode components resolved at
    compose time (explicit `placeholders: [{name, source_component, …}]`
    field).
  - C: other.
- **F-HI-3 — Composition output shape.** What does `compose_orchestrator`
  return?
  - A: `{program: string, rust_directives: [...]}` — a self-contained program
    string Monty runs via `host.run_program` + rust load directives.
  - B: a structured step list the orchestrator iterates (no single program
    string).
- **F-HI-4 — Composition-system identity.** Is the composition-system the
  `host.compose_orchestrator` rewrite itself, or a separate Rust service that
  `compose_orchestrator` thin-calls?
  - A: `compose_orchestrator` IS the composition-system (one host callable).
  - B: a separate composition service (Rust, no Monty) that
    `compose_orchestrator` thin-calls.
- **F-HI-5 — First class + slice order.** Which class is upgraded first
  (drives the common-syntax reference the rest conform to)?
  - A: `python_code`(22) — the unit Monty runs + placeholders c/h.
  - B: `tool`(0) — most foundational.
  - C: `recipe`(21) — the dual-nature reference (already done; mostly verify).

## Out of scope (explicit)

- The Monty VM, the `host.*` callable registry (C.1), the cdylib load/unload
  primitives (C.3), the security-settings store/panel (C.4) — all kept.
- The basic-mode orchestrator script (C.5) + production driver (C.6) +
  Model-A retirement (C.7) — sequenced AFTER Phase HI.

## Status

[ ] Pending — forks F-HI-1…F-HI-5 open before HI.2; HI.0 spec + HI.1 audit
    land after fork resolution.
