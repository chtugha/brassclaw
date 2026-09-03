# Step C.4.5 — Common Component Syntax + Composition System

Subplan of `./saved_plan_to_v3.md` (Step C block, folded into Step C per F5=B;
keeps C.5/C.6/C.7 numbering). Prerequisite for the `host.compose_orchestrator`
rewrite (deferred from C.5/C.6) and therefore for the C.5 basic-mode
orchestrator script: the composition system can only emit exact, reproducible
per-step Python once every component class shares one unambiguous machine-
readable syntax.

## Objective

Extend the **Phase HI dual-nature recipe syntax** (Step B — DONE) from
`Recipe` (class 21) to **every component class**, so that:

1. Every component carries a **machine-readable exact-logic** form that
   **always reproduces the same** orchestrator + Rust results, AND a concise
   **human-readable explanation**.
2. The **composition system** (`host.compose_orchestrator` rewrite) reads any
   component's instructions, **splits rust / orchestrator**, compiles what the
   Rust executioner needs, and provides Monty the parts to assemble + run.
3. Behaviour changes need **no code changes — only the component is altered**.

## Locked decisions (F1–F5)

- **F1=A — universal machine form.** IBS `BuildInstruction` (`rust_steps` +
  `orchestrator_steps`) + per-class `step_link` is the universal machine-
  readable form for ALL classes (extended from Recipe). One machine-side shape
  everywhere; `step_link` resolves to it per class.
- **F2=C — first class + slice order.** `recipe` (21) first (the Step B
  dual-nature reference) — align it to the common contract — then extend to
  the other classes.
- **F3=A — PythonCode component-includes.** Structural include resolved at
  compose time: a PythonCode body's component-placeholder is inlined by the
  composer with the referenced mini-PythonCode component's body (one function
  each, like an include). `{{vars.slotN}}` / `{{user_input}}` stay for variable
  substitution (prompt data).
- **F4=A (modified) — composer scope + Matching/Non-Matching.** The composer
  (`host.compose_orchestrator`) returns the **parts** — skills + program parts
  (PythonCode pieces) + variable bindings — NOT a baked program string. Monty
  receives them, assembles, **writes code** (with LLM help via
  `host.kohai_complete`), and **runs** it via `host.run_program`. **All
  orchestration happens from within Monty, not from anywhere else.** The
  LLM-helped code writing feeds back to **optimise a recipe** or **mint new
  Python-code components** usable next time, after Q1 validation (the
  kohai-sempai new-component-creation loop). Matching-Mode: parts come from the
  matched recipe. Non-Matching-Mode: Monty writes from skill instructions.
- **F4-refinement (user, 2026-09-03) — the composition system IS the IBS;
  `compose_orchestrator` is a ToolSkill/Skill OVER the IBS, not bespoke Rust.**
  The **IBS** (`instruction_builder.rs`, the Phase A `build_instruction` →
  `BuildInstruction{rust_steps, orchestrator_steps}` machinery) **IS the
  composition system.** `host.compose_orchestrator` is **reclassified** (from
  C.2's "net-new host.* Tool" seed) into a **ToolSkill/Skill component** that
  drives the IBS — not a bespoke Rust handler. The IBS, when driven, **creates
  the Rust dynamic plugin** (cdylib load directives for the Rust executioner,
  via the C.3 `DynamicToolLoader`) **+ the Monty stuff**, and **gives Monty** a
  **predefined structured output** so Monty can reliably handle what it is
  handed:
  `{ skills: [...], steplist: [{ step_id, instructions, executable_code, ... }], rust_directives: [...], variables: {...} }`.
  Monty iterates the steplist, uses the skills, runs each step's
  `executable_code` via `host.run_program`, and loads the rust plugin per
  `rust_directives`. **This means less new Rust** — the IBS core already exists
  (Phase A); C.4.5.17 extends it to emit the predefined Monty-facing structure +
  rust-plugin directives + variable/include resolution, and mints the
  `compose_orchestrator` Skill/ToolSkill that thin-calls it.
- **F5=B — numbering.** Folded into Step C as **C.4.5**; C.5 (basic-mode
  script), C.6 (driver), C.7 (retirement) keep their numbering.

## Prerequisite (DONE)

Phase HI / Step B — dual-nature recipe syntax:
- Machine-readable (untouched): `RecipeVariant.step_link` → IBS
  `build_instruction` → `BuildInstruction` (`rust_steps` + `orchestrator_steps`).
- Human-readable: `Recipe.description`, `RecipeVariant.description`,
  `StepDescriptionEntry.label`, `StepEntry.goal`.
- Q1 gate: `check_variant_descriptions` (v3 variants require a variant
  description ≤ 512 chars; legacy `step_link == None` exempt).
- Shipped: B.1–B.5 (field + round-trip tests + Q1 gate + docs + both configs).

## Component class inventory

From `intent_system::class_label` + `retrieval_source::class_code_to_table`:

| Code | Label | Table | Group |
|------|-------|-------|-------|
| 0 | tool | (no prompt text) | execution primitive (Rust) |
| 1 | skill_rusty | reborn_skills | skill |
| 2 | skill_monty | reborn_skills | skill |
| 3 | skill_llm | reborn_skills | skill |
| 4–9 | extension_{worker,cron,trigger,webhook,plan,revision} | reborn_extensions_unified | extension |
| 10 | orchestrator | reborn_skills | skill |
| 11 | reserved | — | — |
| 12 | spec | reborn_specs | memory/instruction |
| 13 | tool_skill | reborn_tool_skills | execution primitive (Rust) |
| 14 | plan | reborn_plans | memory/instruction |
| 15 | summary | reborn_summaries | memory/instruction |
| 16 | action | reborn_actions | instruction |
| 17 | docu | reborn_docus | memory/instruction |
| 18 | lesson | reborn_lessons | memory/instruction |
| 19 | issue | reborn_issues | memory/instruction |
| 20 | note | reborn_notes | memory/instruction |
| 21 | recipe | reborn_recipes | instruction (dual-nature DONE — reference, aligned first) |
| 22 | python_code | reborn_python_code | execution unit (Monty runs) |
| 23 | extension_catalogue | — | extension |
| 50 | scaffold | reborn_skills | scaffold |

The C.2 seed (`seed_builtin_host.rs`) mints the 5-component stack (Tool +
ToolSkill + PythonCode + leaf Skill + Recipe) per `host.*` tool — the reference
shape for the common syntax.

## Common-syntax contract (the a–i items)

- **a) Exact building instructions** — how code-snippet modules assemble into
  the exact described Rust-executioner content; standardised solutions for
  common problems. → IBS `BuildInstruction.rust_steps` (`{tool, tool_skill}`) is
  the universal rust-side build form; per-class `step_link` resolves to it.
- **b) Formatting conventions** — one canonical shape per class (field set,
  ordering, units), enforced at Q1 validation.
- **c) Python-code placeholders** — repeating code (functions/classes) factored
  into reusable mini PythonCode components (one function each). A PythonCode
  body carries `{{vars.slotN}}`/`{{name}}` placeholders the composer bakes in;
  a component-placeholder is inlined at compose time with the referenced mini-
  PythonCode's body (F3=A). Chain-loading mirrors the skill chain-load in
  `select_skills`.
- **d) Architecture documentation** — how each component + system works + is
  standardised.
- **e) Command syntax** — uniform skill-design + tool-call conventions across
  all components (`host.<tool>(kw=...)`; one leaf skill per approach).
- **f) Key-system documentation** — f1 intent-matching, f2 python-orchestrator,
  f3 validation, f4 components (extension-recipe-skill-toolskill-tool with the
  converted builtin tools as examples), f5 rust-executioner (orchestrator-
  command-driven executions rule + exceptions), f6 LLM-prompt creation + the
  prefix system, f7 kohai-sempai (duties, with/without sempai), f8 new-
  component-creation per class + exact validation criteria.
- **g) Database-structure standardisation** — one schema convention per class
  across the `reborn_*` tables: scope keys, content column, prior-knowledge
  column, `class_code`, `prompt_uid` ordering.
- **h) Variables** — the `{{vars.slotN}}` / `{{user_input}}` substitution
  contract: who bakes what, when, with what escaping.
- **i) Everything else clever/necessary** — added per-slice as discovered.

## Placeholder grammar (items c + h — F-HI-2=A; formalised C.4.5.1)

The composer (C.4.5.17) bakes three placeholder kinds into orchestrator-side
content (PythonCode bodies + recipe orchestrator-side step content). All are
`{{ ... }}` mustache-style; the machine-readable form (`StepEntry.include` +
`VariablePattern`) is the unambiguous source the composer resolves them from.

- **`{{vars.NAME}}`** — slot-variable substitution. `NAME` matches a
  `RecipeVariant.variable_patterns[].name`; the composer bakes the extracted
  slot value. `{{vars.slotN}}` (N = 1-based ordinal) is the positional form
  used when `variable_patterns` is empty (positional auto-extraction, §0.17.3).
  Data substitution only — never code.
- **`{{user_input}}`** — the raw user input string (data substitution only).
- **`{{component_name}}`** — structural include. The composer inlines the
  referenced mini-PythonCode component's body (one function each, like an
  include) at that slot. The machine form carries the resolved component as a
  UUID in `StepEntry.include` (recipe) or the PythonCode include list; the
  `{{component_name}}` template is the human-readable authoring surface that
  resolves to that UUID at compose time. F3=A.

**Baking rules (h):** the composer (C.4.5.17) is the sole baker; Q1 validates
structure only (`variable_patterns` regex compileability + non-nil include
UUIDs — C.4.5.1 gate), never bakes. Escaping: baked slot values are inserted
as Python string literals (JSON-encoded) to prevent injection. `{{user_input}}`
is never baked into raw code — only into string-literal positions.

## Composition system contract (`host.compose_orchestrator` rewrite)

- **Input:** `component_id` (UUID or well-known name) + `user_input` (carries
  prompt variables — e.g. a path — to bind).
- **Behaviour:** fetch the component by id → read its machine-readable form
  (IBS `BuildInstruction`, F1=A) → split **rust part / orchestrator part** →
  compile the rust part (load directives for the Rust executioner: built-in
  tools + cdylib `dlopen` via the C.3 `DynamicToolLoader`) → resolve component-
  includes (F3=A) + bind variables (h) → return the **parts** (skills +
  PythonCode pieces + variable bindings) to Monty.
- **Identity (F4-refinement):** the **IBS IS the composition system.**
  `host.compose_orchestrator` is a **ToolSkill/Skill component** that thin-calls
  the IBS — **not** a bespoke Rust handler (reclassified from C.2's "net-new
  host.* Tool" seed). The IBS creates the rust dynamic plugin + the Monty stuff.
- **Output (predefined structure — F4-refinement):**
  `{ skills: [...], steplist: [{ step_id, instructions, executable_code, tool_bindings, ... }], rust_directives: [...], variables: {...} }`.
  **The composer does NOT run anything + does NOT bake a single program
  string.** Monty iterates the steplist, uses the skills, runs each step's
  `executable_code` via `host.run_program` (Monty 0.0.16 has no
  `exec`/`eval`/`compile` builtin — verified in the git checkout; a host
  callable is the only way for Monty to run a dynamic code string), and loads
  the rust plugin per `rust_directives`. All orchestration is inside Monty
  (F4=A-modified).
- **Feedback loop:** Monty's LLM-helped code writing → recipe optimisation OR
  new python-code components, after Q1 validation (kohai-sempai).

## Slices (one commit each; one component class at a time to avoid OOM)

- **C.4.5.0** — Common-syntax contract spec finalised (this doc's machine side
  + the F1–F5 locks) + composition contract. The reference every later slice
  conforms to.
- **C.4.5.1** — `recipe` (21) — align the Step B dual-nature to the common
  contract (reference class; mostly verification + gap-close: confirm
  `step_link`→IBS is the universal form, add component-include + variable
  conformance).
- **C.4.5.2** — `python_code` (22) + placeholder/include mechanism (c/h, F3=A).
- **C.4.5.3** — `tool_skill` (13).
- **C.4.5.4** — `tool` (0).
- **C.4.5.5** — `skill` (1/2/3/10) + command syntax (e).
- **C.4.5.6** — `action` (16).
- **C.4.5.7–C.4.5.13** — memory/instruction: `spec`(12), `plan`(14),
  `summary`(15), `lesson`(18), `issue`(19), `note`(20), `docu`(17).
- **C.4.5.14–C.4.5.16** — extensions (4–9, 23) + `scaffold`(50).
- **C.4.5.17** — Composition system: the **IBS IS the composition system**
  (F4-refinement). Extend `instruction_builder.rs` (`build_instruction`) to emit
  the **predefined Monty-facing structure** `{ skills, steplist[{step_id,
  instructions, executable_code, tool_bindings}], rust_directives, variables }`,
  create the rust dynamic plugin (cdylib load directives via C.3
  `DynamicToolLoader`), resolve component-includes (F3=A) + bind variables (h).
  Mint `host.compose_orchestrator` as a **ToolSkill/Skill component** that
  thin-calls the IBS (reclassified from C.2's "net-new host.* Tool" seed — NOT
  bespoke Rust). Add `host.run_program` (Monty runs each step's
  `executable_code` — Monty 0.0.16 has no exec/eval/compile; precedent: retired
  `__execute_code_step__`/`execute_code` scripting.rs:519). Unit-tested in
  isolation: deterministic predefined structure from a fixture component.
  **Status grounded 2026-09-03: NOT yet implemented** — the IBS core exists
  (Phase A, Recipe-only `step_link`→`BuildInstruction`); `compose_orchestrator`
  is a C.2 seed placeholder (no handler in orchestrator.rs); `host.run_program`
  is net-new.
- **C.4.5.18** — Comprehensive docs (d, f1–f8, g).
- **C.4.5.19** — Both configs green (default + `--features skills-db`); DB-
  schema standardisation verification; mark C.4.5 done; commit + push; resume
  C.5 (basic-mode orchestrator script, now that compose_orchestrator exists).

## Status

[x] C.4.5.0 DONE (contract spec + F1–F5 locks). [x] C.4.5.1 DONE (2026-09-03 —
    `recipe`(21) aligned: `{{...}}` placeholder grammar formalised in the
    contract (items c/h); explicit Q1 gate `check_variant_machine_form` added
    to recipe_validator.rs (variable_patterns name+regex + non-nil include
    UUIDs + step_descriptions parse, legacy exempt); 5 gate tests + 1 IBS
    round-trip; no schema change; both configs clippy-clean + 47 recipe tests
    green). F1–F5 locked (F1=A universal IBS form; F2=C recipe-first; F3=A
    template-vars + component-includes; F4=A-modified + F4-refinement
    composer-returns-parts / IBS-IS-the-composition-system /
    compose_orchestrator-is-a-Skill-over-IBS / predefined output structure;
    F5=B folded into Step C as C.4.5). [x] C.4.5.2 DONE (2026-09-03 —
    `python_code`(22) + placeholder/include mechanism (c/h, F3=A): migration
    V069 adds JSONB `includes` column (`Vec<Uuid>`, machine form of
    `{{component_name}}` structural includes — mirrors recipe `StepEntry.include`,
    DEFAULT '[]' so pre-existing rows stay valid); `PgPythonCode`/`NewPgPythonCode`
    extended (includes field) + `PYTHON_CODE_SELECT` (index 25) +
    `decode_python_code_row` (JSONB→Vec<Uuid>) + INSERT ($14); 12 seed
    `NewPgPythonCode` sites updated (`includes: vec![]`); explicit Q1 gate
    `validate_python_code_placeholders` added to component_validator class-22 arm
    (balanced `{{ }}` + recognised kinds vars.NAME/vars.slotN/user_input/
    component_name + non-nil include UUIDs read from `GenericComponent::extra`
    `{"includes":[...]}`; referential placeholder↔include match deferred to
    Phase I/N — Fork 2-B=B); 6 engine gate tests + composition store round-trip
    (non-empty includes assertion, docker-gated) + column-count pin (26 cols);
    both configs clippy-clean (engine + composition, default + --features
    skills-db); 9 class22 + 47 recipe + 6 composition tests green). [x]
    **C.4.5.3 DONE** (2026-09-03 — `tool_skill`(13) + DB-structure standardization
    (item g): migration V070 drops the 2 legacy pre-centralization remnants
    `queue_code`+`validation_errors` from `reborn_tool_skills` (V051 documents
    them as slated-for-V059/Phase-N drop; no Rust reader — retrieval_source
    selects neither, pg_tool_skill_store has no decode SELECT) + adds JSONB
    `includes` (machine form of `{{component_name}}` structural-include
    placeholders, mirrors PythonCode V069 + recipe `StepEntry.include`, DEFAULT
    '[]' keeps pre-existing rows valid); `ToolSkill` struct gains
    `#[serde(default)] includes: Vec<Uuid>` + 5 test fixtures; `NewPgToolSkill`
    + INSERT → 17 params ($17 includes) + 8 `ts-host-*` seed sites (`includes:
    vec![]`); no decode SELECT → read path untouched (composer C.4.5.17 fetches
    includes separately); explicit Q1 gate — catch-all `12..=14 | 17..=20` →
    `12 | 14 | 17..=20`, new dedicated `13 =>` arm = `validate_tool_skill` +
    `validate_tool_skill_placeholders` (Generic/Recipe → error); extracted shared
    `validate_placeholder_grammar` helper (python_code refactored to use it,
    behavior-preserving) + `validate_includes_non_nil_uuids`; placeholder grammar
    scanned on description/preconditions/error_handling/code_snippet/serialized
    param_template/each param_schema[].description; 6 engine gate tests (valid
    4-kind + non-nil include passes; unbalanced / unrecognised `{{bogus}}` /
    nil-include / Generic-payload-errors / empty-tool_name-fails — proves
    `validate_tool_skill` is called). ToolSkill has NO `variable_patterns`
    (vars from recipe/caller; `{{vars.name}}` lives in param_template). Both
    configs clippy-clean; engine 567 + composition 682/689 lib tests green). [x]
    **C.4.5.4 DONE** (2026-09-03 — `tool`(0) + DB-structure standardization (item g)
    + the rust-side common form: migration V071 `reborn_tools_syntax` — drops the
    full 5-col legacy set (queue_code, validation_errors, review_feedback,
    review_attempts, rejected_at; all DDL-default-only, central queue tracks
    lifecycle, graduation UPDATE only sets validation_status, no Rust reader) +
    drops `cdylib_artifact_path` (REVERSES V067 — dead, zero Rust readers; the
    cdylib load directive is a composition concern that moves to recipe-step/
    toolskill in C.4.5.17; builtin_stuff_v3.md Step 1.1 plans Tool rows with NO
    cdylib_artifact_path) + drops the V067 partial index `reborn_tools_cdylib_path_idx`
    + adds `capability_id TEXT NOT NULL DEFAULT ''` (the Executioner's dispatch id
    — "builtin.shell" / "host.resolve_intent"; the rust-side common form of a Tool)
    + backfill `UPDATE ... SET capability_id = name WHERE capability_id = ''`
    (idempotent; for seeded host.* tools capability_id == name); `NewPgTool` gains
    `capability_id: String` + INSERT → 15 params ($15); all 8 host.* seed Tool
    rows updated; Q1 gate — class-0 arm now runs `validate_tool_skill` (whose
    tool_name-non-empty check IS the capability_id-non-empty gate, since for
    class 0 `tool_name` carries the capability_id) + `validate_tool_skill_placeholders`
    (placeholder grammar + non-nil includes, mirrors class 13; concrete for valid
    tools → no-op); 3 engine gate tests (valid capability_id passes; unbalanced
    `{{` fails; empty capability_id fails). Both configs clippy-clean; engine 570
    (567+3 new) + composition 689 lib tests green). [x]
    **C.4.5.5 DONE** (2026-09-03 — `skill`(1/2/3/10) + command syntax (item e) +
    DB-structure standardization (item g): migration V072 `reborn_skills_syntax`
    — drops the 5 legacy pre-centralization lifecycle cols (validation_errors,
    review_feedback, review_attempts, rejected_at, queue_code; the SAME set V071
    dropped from reborn_tools, BUT these WERE actively read/written by
    `DbSkillStore` under the `db-store` feature enabled by composition's
    `skills-db` feature — so this migration is PAIRED with a `DbSkillStore`
    refactor that removes all 24 legacy-col references: `DbSkillRow` struct
    fields, INSERT col+`'q1_auto'` literal, 3 identical SELECT lists,
    `row_from_pg` decoder, 4 UPDATE sets; `mark_auto_failed` signature drops the
    now-unused `errors: &[String]` arg — zero callers) + widens the class_code
    CHECK from `IN (1,2,3)` to `IN (1,2,3,10,50)` (fixes the latent
    contradiction: `retrieval_source.rs:1052` + V061 registry assumed 10/50 live
    in `reborn_skills` but V027's CHECK rejected them → class 10 orchestrator
    scripts can now be INSERTed, enabling C.5/C.6; constraint dropped by
    Postgres-assigned name `reborn_skills_class_code_check`, `IF EXISTS` guards);
    `db_skill_loader.rs sample_row()` updated (5 field initializers dropped;
    `row_to_json` unchanged); Q1 gate — class `10 | 50` arm =
    `validate_soft_budget_named` (BUDGET_ORCHESTRATOR=50,000) +
    `validate_placeholder_grammar` (the class-10 orchestrator script carries
    `{{vars.NAME}}`/`{{vars.slotN}}`/`{{user_input}}`/`{{component_name}}`
    placeholders; skills 1-3 are pure narrative → NOT gated); command-syntax
    item e = docs-only (C.4.5.18 documents the `ts-`/`skill-`/`pc-`/`shell-`
    backtick-name convention; Q1 does NOT enforce narrative); 3 engine gate
    tests (valid placeholders pass; unbalanced `{{` fails; unrecognised
    `{{bogus}}` fails). Both configs clippy-clean; engine 573 default / 584
    skills-db + brassclaw_skills db-store 231 + composition skills-db 689 green). [x]
    **C.4.5.6 DONE** (2026-09-03 — `action`(16) DB-structure standardization ONLY,
    F6=A: migration V073 `reborn_actions_syntax` drops the 5 legacy
    pre-centralization lifecycle cols (validation_errors, review_feedback,
    review_attempts, rejected_at, queue_code — the SAME set V071 dropped from
    reborn_tools + V072 from reborn_skills) from `reborn_actions`;
    `parent_mission_id` already dropped by V064; `queue_code`'s CHECK auto-drops
    with the column. UNLIKE V072 (reborn_skills, where the cols were actively
    used by `DbSkillStore`), `reborn_actions` has NO dedicated store (no
    `PgAction`/`pg_action_store`) — the 5 cols have ZERO Rust readers/writers
    (the class-16 SELECT projection in retrieval_source.rs reads
    id/class_code/prompt_uid/name/description/effective_content/
    override_prompt_creation/steps/allowed_tools; NONE of the 5 legacy) and NO
    INSERT names them (all 6 `INSERT INTO reborn_actions` specify only
    id/scope/name/description/class_code|validation_status|steps|allowed_tools)
    → NO paired Rust refactor + NO test changes needed. F6=A DEFERRED the Action
    step-machine — the `steps` JSONB 13-step-type model (tool_call/conditional/
    set_var/loop/return/evaluate/call_skill/try_catch/parallel/call_action/
    spawn_subprocess/wait/emit_event) + the `ActionShortCircuit` intent path
    (retrieval_source.rs:727, documented "vestigial under Q2") — driven by the
    retired `default.py`/`__execute_action__`; its retire-vs-reformulate fate is
    entangled with C.4.5.17 (composition system, not built) + C.5/C.6/C.7
    (retirement, not done), so the step-machine/Q1-gate/syntax alignment is
    deferred to those slices. `cargo check -p brassclaw_pg` confirms the
    migration embeds cleanly via `refinery::embed_migrations!`; no Rust touched.
    NO dedicated Zenflow step exists for action(16) — the plan jumps HI.5 skill
    → C.4.5.1 recipe → HI.7 composition; tracked in this subplan + mindmap
    only). V073 shipped `ca276c16`.
    **Remaining: C.4.5.14–C.4.5.19 not implemented** (IBS core exists Phase A but
    Recipe-only; compose_orchestrator is a C.2 placeholder; host.run_program
    net-new; per-class syntax upgrades + predefined structure + rust-plugin
    directives all pending). [x]
    **C.4.5.7 DONE** (2026-09-03 — memory/instruction classes (12/14/15/17/18/19/20)
    DB-structure standardization (item g), F7=A BATCHED all 7 uniform narrative
    classes into one slice (C.4.5.8–C.4.5.13 COLLAPSE into C.4.5.7): migration
    V074 `reborn_memory_classes_syntax` drops the 5 legacy pre-centralization
    lifecycle cols (validation_errors, review_feedback, review_attempts,
    rejected_at, queue_code) from all 7 tables — reborn_specs(12)/reborn_plans(14)/
    reborn_summaries(15)/reborn_docus(17)/reborn_lessons(18)/reborn_issues(19)/
    reborn_notes(20); `parent_mission_id` already dropped by V064; all 7 tables'
    `queue_code` is plain `TEXT` with NO inline CHECK (verified V036–V043), so no
    explicit constraint drop. The 5 cols have ZERO Rust readers/writers — NO paired
    Rust refactor + NO test changes: the class `12|14|17..=20` Q1 gate
    (component_validator.rs:301 `validate_soft_budget_named`) touches no DB col;
    retrieval SELECTs read id/class_code/prompt_uid/name/description/
    effective_content (none of the 5); component_import.rs INSERT names only
    scope/name/description/content/content_hash/consumer_tags/intent_examples/
    source/validation_status; the only literal INSERT (validation_queue.rs:954 into
    reborn_notes) names only scope/name/validation_status; the legacy-col names
    elsewhere in .rs (similarity_checker/recipe_matcher/recipe_validator) are the
    RETIRING `MemoryDoc` struct (types/memory.rs confirms legacy), NOT reads of
    these tables. Q1 GATE UNCHANGED — the catch-all `12|14|17..=20` soft-budget
    (10k) is correct for narrative memory content; these classes carry NO `{{ }}`
    placeholders (authored text, not assembled code) → no placeholder-grammar gate
    (unlike class 10/13/22); narrative-formatting conventions (item b) +
    command-syntax references (item e) are docs-only (C.4.5.18). `cargo check -p
    brassclaw_pg` confirms V074 embeds cleanly via `refinery::embed_migrations!`
    (zero Rust touched; DB integration tests skip locally — no Docker). V074
    shipped `ca8a12b7`. **Zenflow step `cbd206ef` (HI.8, sort 39) marked
    Completed.** Next slice C.4.5.14 (extensions 4–9 + ExtensionCatalogue 23 +
    `scaffold`(50)).
    This subplan is the canonical one (the prior `subplan_problem_phaseHI_…md`
    is a redirect stub).
