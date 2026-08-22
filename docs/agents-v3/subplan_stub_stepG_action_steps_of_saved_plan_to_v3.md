# Subplan — STUB: action_short_circuit fetches a component view without executable `steps`

Parent plan: `saved_plan_to_v3.md` → Phase G (Recipe System Finalisation).
Phase G subplan: `docs/agents-v3/subplan_problem_stepG_of_saved_plan_to_v3.md`.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Discovered while grounding **G.8** (the Phase G test substep). Inserted as a
sub-substep before G.8 because G.8's `action_short_circuit` test cannot
meaningfully verify execution while the production path silently no-ops.

---

## 1. The stub / gap

G.5 wired the §0.9 `action_short_circuit` step-0 branch
(`orchestrator/default.py:1098-1116`):

```python
action_doc = __fetch_component__(pkr.get("action_component_id", ""), 16)
if isinstance(action_doc, dict):
    action_result = execute_action_procedure(action_doc, goal, state)
    ...
```

`execute_action_procedure` → `_execute_action_steps(action, …)` reads
`action.get("steps", [])` (`default.py:713`) and `action.get("allowed_tools", [])`
(`default.py:717`).

But `__fetch_component__` (and `__resolve_component_by_name__`) return a
**component view** dict — `{ id, class_code, name, description, content,
override_prompt_creation }` (`orchestrator.rs:2857-2864` and `2928-2935`).
For class 16 the `content` column is
`COALESCE(prior_knowledge_content, description)` (`retrieval_source.rs:977`) —
the LLM-readable description, **not** the executable `steps` JSONB. The dict has
**no `steps` key and no `allowed_tools` key**.

Result: `_execute_action_steps` gets `steps = []` → the `for step_def in steps:`
loop runs zero iterations → `result = None` → `execute_action_procedure` returns
`complete_result(state, "completed", "Action completed.")`. **The action's real
procedure is never fetched nor executed.** This is a silent no-op, exactly the
"written half-way and then silenced" anti-pattern the task calls out.

### Reachability (this is a live production path, not dead code)

`handle_assemble_prior_knowledge`'s `ActionShortCircuit` arm
(`orchestrator.rs:2688-2694`) emits:

```json
{ "action_short_circuit": true, "action_component_id": "<uuid>",
  "action_name": "<name>", "orchestrator_content": "", "active_skills": [],
  "override_prompt_creation": false, "formatted_content": "",
  "matched_component_ids": ["<uuid>"] }
```

The Phase F.7 #3 test (`orchestrator.rs:7768-7786`) already asserts this. So
whenever the intent system short-circuits to an Action, G.5's branch runs and
silently no-ops. The `fall_back_to_tier2` signal G.6 added is also never
produced (zero-step "completed" is returned instead).

---

## 2. Fix design — decision required (Q-G-STUB1)

The component fetch must provide the executable `steps` (+ `allowed_tools`) for
class-16 Actions so `execute_action_procedure` can run them. Two options:

### Option A (recommended) — extend the class-16 fetch to return `steps` + `allowed_tools`

- `retrieval_source.rs`: add `steps: Option<serde_json::Value>` and
  `allowed_tools: Option<serde_json::Value>` to `ComponentItem` (populated only
  for class 16; `None` for every other class).
- `fetch_component_by_id` + `fetch_component_by_name`: add a **class-16-specific
  query branch** that SELECTs the `steps` and `allowed_tools` JSONB columns from
  `reborn_actions` (these columns exist only on `reborn_actions`, so they cannot
  be in the generic `class_code_to_table` content_expr SELECT). Parse both via
  `serde_json::from_value`/`row.get::<_, serde_json::Value>`.
- `orchestrator.rs`: `handle_fetch_component` + `handle_resolve_component_by_name`
  emit `steps` + `allowed_tools` in the returned dict **when present** (class 16).
- `default.py`: **no change** — `execute_action_procedure`/`_execute_action_steps`
  already read `steps` + `allowed_tools`.

**Pros:** single fetch; faithful to the plan's `action_doc = __fetch_component__(…)`
+ `execute_action_procedure(action_doc, …)` wiring (FIND-P7-02, plan line 5345);
the action component's executable steps are part of the component (v3 shape).
**Cons:** `ComponentItem` gains two class-16-specific `Option` fields; the fetch
fns gain a class-16 branch (moderate).

### Option B — dedicated host fn `__fetch_action_for_execution__(uuid)`

- New retrieval fn returning `{steps, allowed_tools}` (parsed JSONB) from
  `reborn_actions`.
- New host fn + dispatch arm.
- Either G.5 step-0 calls it after `__fetch_component__` and merges, or
  `execute_action_procedure` calls it.

**Pros:** `__fetch_component__` stays a generic component view; clean separation.
**Cons:** deviates from the plan's wiring (adds a second API + a second
round-trip); more surface area; the action doc passed to
`execute_action_procedure` no longer comes from a single `__fetch_component__`.

### Recommendation: **Option A** — it matches the plan's stated wiring, keeps a
single fetch, and treats the executable `steps` as part of the Action component
(the v3 invariant). `__resolve_component_by_name__` is updated symmetrically so
the G.6 Option B fallback path also executes real steps.

---

## 3. Files touched (Option A)

- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  - `ComponentItem`: + `steps: Option<serde_json::Value>`,
    `allowed_tools: Option<serde_json::Value>`.
  - `fetch_component_by_id`: class-16 branch SELECTing `steps` + `allowed_tools`.
  - `fetch_component_by_name`: same class-16 branch.
  - All other `ComponentItem` constructors: initialize the two new fields to
    `None`.
- `crates/brassclaw_engine/src/executor/orchestrator.rs`
  - `handle_fetch_component`: emit `steps` + `allowed_tools` when `Some` (class 16).
  - `handle_resolve_component_by_name`: same.
- No `default.py` change.
- Tests (see §4).

## 4. Tests

- **Unit (engine, both configs):** extend the G.2
  `phase_g2_resolve_by_name_returns_null_on_unresolvable_paths` family +
  add a class-16 `steps`-emission assertion where a pool/fixture is available
  (the existing handler unit tests are Null-path only because no pool is wired
  in-engine; the real `steps`-emission is covered by the composition
  integration test below).
- **Composition integration (`tests/fetch_component.rs`, skip-if-no-docker):**
  `fetch_component_by_id_returns_action_steps` — insert an Action with a known
  `steps` JSONB + `allowed_tools`, fetch by id (class 16), assert
  `ComponentItem.steps` / `allowed_tools` parse to the expected JSON. Mirror the
  existing `fetch_component_by_name_resolves_action_item` helpers.

## 5. Verification (both configs — default + `--features brassclaw_engine/skills-db`;
composition default + `--features brassclaw_reborn_composition/skills-db`)

- `cargo fmt --all -- --check`
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default + skills-db)
- `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (default + skills-db)
- `cargo test -p brassclaw_engine --lib` (default + skills-db)
- DB-integration tests skip on this host (no docker) — correct-by-grounding
  against `reborn_actions` schema (`steps JSONB`, `allowed_tools` — verify the
  column exists) + the handler SQL.

## 6. Sequencing

Execute this subplan BEFORE resuming G.8. G.8's `action_short_circuit` test
(test #2: "fetch returns a doc → execute_action_procedure runs, outcome
completed") can then mock `__fetch_component__` returning a dict WITH `steps`
and verify the Python executor runs them; the composition integration test
verifies the real fetch returns `steps`. After this subplan completes, resume
G.8.

---

## 7. Status

- **Decision (Q-G-STUB1): Option A** — extend the class-16 fetch path to
  SELECT + parse `steps` (JSONB) + `allowed_tools` (TEXT[]) onto
  `ComponentItem` and emit them in the returned dict for class 16. Q2
  (also return `timeout_secs`?) was left unanswered → minimal scope taken:
  `steps` + `allowed_tools` only, NO `timeout_secs`.
- **Implemented:**
  - `retrieval_source.rs`: `ComponentItem` gained
    `steps: Option<serde_json::Value>` + `allowed_tools: Option<serde_json::Value>`
    (populated only for class 16; `None` otherwise). New
    `component_item_from_row` helper reads the 9-column fetch row (cols 7–8
    as `Option` → NULL-safe; `steps` surfaced only when the JSONB is an
    array). `fetch_component_by_id` + `fetch_component_by_name` gained the
    class-16-specific projection (`steps`, `allowed_tools` for class 16;
    `NULL::jsonb` / `NULL::text[]` otherwise → uniform 9-column shape) and
    build items via the helper. The three prompt-assembly constructors
    (RamSource legacy, broad-scan `Components`, batched `fetch_components_
    by_ids`) deliberately set `steps: None, allowed_tools: None` — they
    build `orchestrator_content`, not an executable doc.
  - `orchestrator.rs`: `handle_fetch_component` + `handle_resolve_component_
    by_name` emit `steps` + `allowed_tools` in the returned Python dict
    when `Some` (class 16). `phase_f7_item` test fixture got the two
    `None` fields.
  - `retrieval_lookup_impl.rs`: 4 composition mapping fixtures got the two
    `None` fields.
  - `tests/fetch_component.rs`: added `insert_action_with_procedure` seed
    helper + `fetch_component_by_id_returns_action_steps` +
    `fetch_component_by_name_returns_action_steps` (skip-if-no-docker);
    header migration range bumped V061 → V062.
- **Verified:** `cargo fmt --all -- --check` clean; engine clippy clean
  (default + skills-db); composition clippy clean (default + skills-db);
  engine lib tests 678 default / 689 skills-db (0 failed both — no
  regressions); `fetch_component` integration tests compile + pass (skip on
  this no-docker host). No migration changed, so the G.7 embedded-PG
  `full_boot_cycle_from_scratch` (V000–V062) verification still stands.
- **Commit:** `07165137` — code + this subplan doc together (pushed to
  `origin/main`, `ab059b7b..07165137`).
