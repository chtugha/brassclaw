# Subplan — F.5 stub fix: `orchestrator_content` prose format + `formatted_content` JSON→prose (FINDING F)

Parent plan: `saved_plan_to_v3.md` → Phase F (`lines 5004–5221`), Tests (`lines 5211–5221`).
Parent subplan: `docs/agents-v3/subplan_problem_stepF_of_saved_plan_to_v3.md` (F.5).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase F step `45d899c7-d3ef-4dae-a07b-85f4affc939c`,
after the Phase F problem substep `a31d567d-...`.

---

## 1. Why this subplan exists — F.5 emitted JSON, the plan mandates prose

F.5 (commit `46d64d31`) upgraded the dormant `handle_assemble_prior_knowledge` `SplitResult`
arm to emit `orchestrator_content` + `formatted_content` (alias). **But it set both to the
JSON string** produced by `assemble_component_strings` (`{"prior_knowledge":[...],
"matched_components":[...]}`). Grounding Phase F tests against plan §0.9 revealed this is a
**stub/incomplete implementation** of the real format:

- Plan §0.9 line 780–786: `orchestrator_content` is a **prose StepContextSpec-headed block**:
  ```
  ## [Skill: ls]
  <skill body>

  ## [PythonCode: ls-result-handler]
  <pythoncode body>
  ```
- Plan §0.9 line 753–760: the class-code → heading map (`Skill`/`Spec`/`Recipe`/`PythonCode`/
  `Catalogue`; class 13 ToolSkill **never** in orchestrator channel; `type:"text"` emits
  nothing).
- Plan FINDING F (line 1334–1350): **`formatted_content` must transition from a JSON-encoded
  object to a prose string = `orchestrator_content`** (a breaking shape change). `default.py`
  is unaffected (it uses `formatted_content` as a string — verified, no `json.loads`). The
  plan mandates documenting this in the Phase F change notes + the public changelog.
- Plan test #4 (line 5216): the `Components` (no-match) arm must emit `orchestrator_content`
  containing all items ("baseline preserved"). F.5 did not touch the `Components` arm — it
  still returns the JSON `assemble_from_component_items` dict with **no `orchestrator_content`
  key**.

So F.5's `orchestrator_content`/`formatted_content` are a JSON stub of the real prose format,
and the `Components` arm lacks `orchestrator_content` entirely. This subplan replaces the stub
with the complete real functionality.

## 2. User design decisions (confirmed via ask_user)

1. **Q-F7-1 (Components arm scope):** `orchestrator_content` in the `Components` broad-scan
   arm includes **ALL retrieved classes** (Skills, Extensions, Specs, Plans, Actions, Recipes,
   PythonCode, Catalogues, …) — "all items" literal; baseline = pre-v3 broad-scan returned
   everything. (Not filtered to the 5 orchestrator-channel classes.)
2. **Q-F7-2 (heading for unlisted classes):** use a **Capitalized category label** for the
   prose heading — `## [Skill: name]` (1–3), `## [Spec: name]` (12), `## [Recipe: name]`
   (21), `## [PythonCode: name]` (22), `## [Catalogue: name]` (23), and for unlisted
   `## [Extension: name]` (4–9), `## [Plan: name]` (14), `## [Action: name]` (16),
   `## [Summary: name]` (15), `## [Orchestrator: name]` (10), `## [Scaffold: name]` (50),
   `## [Docu/Lesson/Issue/Note: name]` (17–20). This matches the user's examples + the plan
   StepContextSpec style. (Note: the literal `class_label()` returns lowercase specific
   subtypes like `skill_rusty`/`extension_worker`/`action` — NOT used; a new
   `step_context_label` helper provides the Capitalized category labels.)
3. **Class 13 (ToolSkill):** plan-specified "never in orchestrator channel" → the formatter
   **skips** class-13 items (never emitted into `orchestrator_content`). Not a decision.
4. **FINDING F (formatted_content JSON→prose):** apply the shape change in the v3 arms
   (`SplitResult` + `Components`); keep the legacy `retrieve_context` path's JSON
   `formatted_content` unchanged (Phase K removes it); document the breaking change in
   `CHANGELOG.md` (which exists). Plan-mandated, not a decision.

## 3. Ordered substeps (run strictly one after another)

### SF5.1 — `StepContextSpec` enum + `step_context_label` + `format_orchestrator_content`

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs`

- Add the `StepContextSpec` enum (plan line 767–774) **extended** to cover all classes per
  Q-F7-1: `Skill, Spec, Recipe, PythonCode, Catalogue, Annotation, Extension, Orchestrator,
  Plan, Summary, Action, Docu, Lesson, Issue, Note, Scaffold, Tool, Component`.
  - `Annotation` is plan-faithful (type:"text" step — never produced from a `ComponentItem`).
- `StepContextSpec::from_class_code(code: i32) -> Option<StepContextSpec>`: returns `None`
  for class 13 (skip) and class 11 (reserved → skip); maps every other class to its variant
  (1–3→Skill, 4–9→Extension, 10→Orchestrator, 12→Spec, 14→Plan, 15→Summary, 16→Action,
  17→Docu, 18→Lesson, 19→Issue, 20→Note, 21→Recipe, 22→PythonCode, 23→Catalogue,
  50→Scaffold, 0→Tool, _→Component).
- `StepContextSpec::heading(&self) -> &'static str`: `Skill`→"Skill", `Spec`→"Spec",
  `Recipe`→"Recipe", `PythonCode`→"PythonCode", `Catalogue`→"Catalogue",
  `Extension`→"Extension", `Orchestrator`→"Orchestrator", `Plan`→"Plan", `Summary`→"Summary",
  `Action`→"Action", `Docu`→"Docu", `Lesson`→"Lesson", `Issue`→"Issue", `Note`→"Note",
  `Scaffold`→"Scaffold", `Tool`→"Tool", `Component`→"Component", `Annotation`→"Annotation".
- `format_orchestrator_content(items: &[crate::memory::ComponentItem]) -> String`: iterate
  items; for each, `StepContextSpec::from_class_code(item.class_code)`; **skip `None`**
  (class 13/11); emit `## [{heading}: {name}]\n{effective_content}`; join blocks with
  `\n\n`. Empty `effective_content` → block is `## [{heading}: {name}]\n` (heading only,
  e.g. a Recipe with no body) — matches plan line 758 ("Recipe" heading, body often empty).

### SF5.2 — Rework the `SplitResult` arm (`orchestrator_content` = prose)

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs` (~line 2663–2689)

- Replace `let (orchestrator_content, _, _) = assemble_component_strings(&orchestrator_items);`
  with `let orchestrator_content = format_orchestrator_content(&orchestrator_items);`
  (prose). `formatted_content` stays the alias (`= orchestrator_content`).
- Keep all routing fields (`tier_zero`, `matched_component_ids`, `override_prompt_creation`,
  `rust_items` serialized, `variant_label`, `step_link`, `wilson_lower`,
  `llm_call_required`, `tier0_eligible`) unchanged.
- `rust_items` serialization (`{id,class_code,name,content}`) is separate from
  `orchestrator_content` (informational; the Python side never applies rust_items directly —
  §0.9 note). Keep it as-is.

### SF5.3 — Rework the `Components` arm (`assemble_from_component_items`)

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs` (`assemble_from_component_items`
~line 2835, called by the `Components` arm at ~line 2610)

- **Solution Override sub-path** (exactly 1 item with `override_prompt_creation == true`):
  emit `orchestrator_content = item.effective_content` (verbatim body, no heading — plan
  line 1310–1311 uses it as the whole user message), `formatted_content` = same alias,
  `content` = `item.effective_content`, `override_prompt_creation: true`,
  `matched_component_ids: [item.id]`, `action_short_circuit: false`, `disambiguation: false`.
- **Normal multi-component path**: `orchestrator_content = format_orchestrator_content(items)`
  (prose), `formatted_content` = same alias, `content` = raw PKC plain-text concatenation
  (keep the existing raw format from `assemble_component_strings`'s raw_content), 
  `override_prompt_creation: false`, `matched_component_ids` = all item ids,
  `action_short_circuit: false`, `disambiguation: false`.
- Refactor `assemble_component_strings` to return `(String /* raw_content */,
  Vec<String> /* matched_ids */)` — drop the now-dead JSON `formatted` output (after SF5.2/SF5.3
  no caller uses it). Update both call sites. This is a clean refactor necessitated by the
  prose rework (removes dead computation, not unsolicited).
- The `Disambiguation` + `ActionShortCircuit` arms are unchanged (already §0.9-correct from
  F.5; `ActionShortCircuit` emits `orchestrator_content: ""` + `formatted_content: ""`).
- The legacy `retrieve_context` fallback (~line 2720–2750) is **unchanged** — keeps JSON
  `formatted_content` (Phase K removes it). Existing test
  `assemble_prior_knowledge_returns_both_surfaces` (orchestrator.rs:7269) calls with
  `None,None` → this legacy path → still passes (JSON `formatted_content` preserved there).

### SF5.4 — CHANGELOG.md: document the `formatted_content` JSON→prose breaking change

**File:** `CHANGELOG.md` (exists)

- Add an entry under the v3 / Phase F section: `__assemble_prior_knowledge__`'s
  `formatted_content` field changes shape from a JSON-encoded object
  (`{"prior_knowledge":[...],"matched_components":[...]}`) to a **prose string** = the new
  `orchestrator_content` (StepContextSpec-headed block). Custom orchestrators that do
  `json.loads(pkr["formatted_content"])` must switch to using `pkr["orchestrator_content"]`
  (or `formatted_content`) as a string. The built-in `default.py` is unaffected. Legacy
  `retrieve_context` path retains JSON `formatted_content` until Phase K. Reference plan
  FINDING F (§0.9 line 1334–1350).

### SF5.5 — Update F.5 doc comments + subplan §4

- Update the `SplitResult` arm + `assemble_from_component_items` doc comments to record the
  prose format + FINDING F shape change.
- Update `subplan_problem_stepF_of_saved_plan_to_v3.md` §4 verification state.

### Verify (both configs — default + `--features brassclaw_engine/skills-db`)

- `cargo fmt --all -- --check`
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default + skills-db)
- `cargo test -p brassclaw_engine --lib` (default + skills-db — no regression; the existing
  `assemble_prior_knowledge_returns_both_surfaces` test uses `None,None` → legacy path,
  unaffected by the v3-arm prose change)

### Commit + push to `origin/main` before resuming F.7.

## 4. Verification state

- SF5.1 — `StepContextSpec` extended in `instruction_builder.rs` (6 → 18
  variants) + `from_class_code(i32) -> Option<Self>` (None for class 13/11) +
  `heading() -> &'static str`; unit test
  `step_context_spec_from_class_code_maps_all_classes_and_skips_toolskill`
  added. `step_context_label` + `format_orchestrator_content` added in
  `orchestrator.rs`. **Done.**
- SF5.2 — `SplitResult` arm now `let orchestrator_content =
  format_orchestrator_content(&orchestrator_items);` (prose); `formatted_content`
  alias unchanged. **Done.**
- SF5.3 — `assemble_from_component_items` (Components arm) now emits the full
  §0.9 dict (`orchestrator_content` prose + `formatted_content` alias + `content`
  raw + `override_prompt_creation` + `matched_component_ids` +
  `action_short_circuit:false` + `disambiguation:false`); Solution Override
  sub-path emits verbatim body as `orchestrator_content`/`formatted_content`/
  `content`. `assemble_component_strings` refactored to `(String, Vec<String>)`
  (raw_content, matched_ids) — dead JSON `formatted` dropped. **Done.**
- SF5.4 — `CHANGELOG.md` `## [Unreleased]` section added: breaking
  `formatted_content` JSON→prose change (FINDING F) + `orchestrator_content` /
  §0.9 dict / `StepContextSpec` extension. **Done.**
- SF5.5 — doc comments updated (dispatch site, `handle_assemble_prior_knowledge`
  doc, `assemble_from_component_items` doc, `assemble_component_strings` doc,
  `step_context_label` / `format_orchestrator_content` docs); this §4 updated.
  **Done.**

**Verify (both configs):**
- `cargo fmt --all -- --check` — clean.
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default) —
  clean (fixed one `doc_lazy_continuation` from a `+`-prefixed doc line).
- `cargo clippy -p brassclaw_engine --features brassclaw_engine/skills-db
  --all-targets -- -D warnings` — clean.
- `cargo test -p brassclaw_engine --lib` (default) — **669 passed** (was 668;
  +1 new `step_context_spec` test), 0 failed.
- `cargo test -p brassclaw_engine --features brassclaw_engine/skills-db --lib`
  — **680 passed** (was 679; +1), 0 failed.
- No regression: the legacy `retrieve_context` path (test
  `assemble_prior_knowledge_returns_both_surfaces`, `None,None`) is unchanged
  and still emits JSON `formatted_content` (Phase K removes it).
