# Subplan — Phase G problem resolution (`saved_plan_to_v3.md`)

Parent plan: `saved_plan_to_v3.md` → Phase G (`lines 5244–5349`) + §0.9 pseudocode (`lines 1260–1389`).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase G step `86628813-572e-4505-99f0-6cbc83da3c6d`.

---

## 1. Why this subplan exists — the pre-Phase-G plan vs post-Phase-F codebase gap

Phase G upgrades `default.py` step-0 to the §0.9 v3 flow and migrates `call_action`
to UUID-based fetch. Grounding the live codebase after Phase A–F revealed a set of
gaps + design forks that the plan body does not resolve on its own. All were raised
as design questions and answered by the user before implementation (§3). This subplan
records the answers + the exact implementation sequence (§4).

**Grounding findings (confirmed against current code):**

1. **`__fetch_component__` is registered (Phase F.6)** and returns a **dict-or-None**
   `{id, class_code, name, description, content, override_prompt_creation}`
   (handler at `orchestrator.rs:2783`, cfg-gated `pg_pool`, returns `Value::Null`
   when no pool / not found). The plan's `call_action` Option A path can use it
   directly.

2. **`__resolve_component_by_name__(name, class_code)` does NOT exist.** The plan's
   Option B fallback host function is absent. Q-G4 → add it now.

3. **`handle_disambiguation`, `_set_active_skills_from_matched_ids`,
   `execute_recipe_orchestrator_channel` do NOT exist** in `default.py`
   (`grep "disambiguat"` in `orchestrator/` → 0 results). §0.9 line 1302 calls
   `handle_disambiguation`; the plan test list references it. Q-G2 → implement it.
   `execute_recipe_orchestrator_channel` is **Phase H** wiring (Q-G1 → defer).

4. **The current step-0 block (`default.py:994–1060`) is OLD pre-v3 code:**
   - `pkr = __assemble_prior_knowledge__(goal, token_budget, "02")` (line 997) —
     PRIMARY call, **stays**.
   - override / `formatted_content` branch (998–1008) — **extends** to the §0.9
     `override_prompt_creation` / `orchestrator_content` flow.
   - dead `docs = __retrieve_docs__(goal, 5)` Action-detection shim (1010–1028) —
     **REMOVE** (Phase G; documented dead since §0.9 Problem 1).
   - `all_skills = __list_skills__()` + `select_skills(...)` block (1030–1059) —
     **REMOVE** (Phase G; IBS already selected Skills by UUID, §0.9 Problem 2).

5. **`ActiveSkillProvenance` (`thread.rs:196–204`)** is
   `{doc_id: DocId, name: String, version: u32, snippet_names: Vec<String>
   (#[serde(default)]), force_activated: bool (#[serde(default)])}` — `version`
   is **u32** (required, no default).

6. **`DbSkillRow.version` is a `String`** (e.g. `"1.0.0"`, `db_store.rs:195`). The
   existing skills-db `__set_active_skills__` step-0 path builds
   `"version": s_meta.get("version", 1)` where `metadata.version` is the **String**
   `"1.0.0"` → deserializing into `ActiveSkillProvenance` (u32) **silently fails**
   → `handle_set_active_skills` returns `None` and writes nothing. **Latent
   pre-existing bug** that Phase G surfaces + fixes: the new
   `fetch_skill_provenance_by_ids` helper parses the major version String → u32
   (split `'.'`, parse first, default 1) so `__set_active_skills__` succeeds.

7. **`reborn_skills` has NO `code_snippets` / `snippet` column** (grep of the V027
   migration → 0). So `snippet_names = []` is **correct** for skills-db skills
   (matches the existing `row_to_json` which omits `code_snippets`; the old
   `select_skills` path defaulted to `[]`). Q-G3 → emit `snippet_names: []`.

8. **`__set_active_skills__` → `handle_set_active_skills`** (`orchestrator.rs:3545`)
   deserializes `Vec<ActiveSkillProvenance>` then `thread.set_active_skills(&skills)`
   (persists to thread metadata for post-run learning provenance).

9. **Dispatch site** (`orchestrator.rs:640–730`) has `pg_pool` (cfg-gated) +
   `store` available → `pg_pool` can be threaded into
   `handle_assemble_prior_knowledge` exactly like `handle_list_skills` /
   `handle_fetch_component` do → **NO `RetrievalSource` trait change needed**
   for Q-G3.

10. **Monty VM = `pydantic/monty v0.0.16`** (git dep, `Cargo.toml:34`). Its README
    says "no classes, exceptions, async" BUT brassclaw's `default.py` already uses
    `try`/`except Exception as exc:` (line 789–794) + `raise` + `async`/`await`
    (scripting.rs tests) → Monty v0.0.16 **does** support built-in exceptions +
    try/except + async in practice, but **does NOT support custom `class`
    definitions** (no `class` appears in any existing script). So a custom
    `class _FallBackToTier2(Exception)` is **NOT possible** → Q-G5 uses a
    Monty-safe **result-dict marker** `{"error": ..., "unresolvable_action": True}`
    instead of a custom exception (§3 Q-G5).

11. **`run_loop` is NOT directly unit-tested today.** The Monty test harness
    `run_python_final` (`orchestrator.rs:4487`) only mocks `__regex_match__` +
    `FINAL` (returns `None` for all other host fns, `Undefined` for name lookups).
    The plan's Phase G step-0 behavior tests therefore require **extending the
    harness** to mock the step-0 host functions + run `run_loop` (§4 G.8).

---

## 2. Tier-zero deferral (Q-G1)

The Zenflow Phase G step description mentions adding the `tier_zero` early-return
branch, but plan §0.9 (lines 1288–1293) explicitly states `tier_zero` +
`execute_recipe_orchestrator_channel` are NEW wiring landing in **Phase H**;
Phase G's own section (`lines 5244–5349`) + test list do not mention `tier_zero`.
**Decision: Phase H.** Phase G does **NOT** add the `tier_zero` branch. The
`handle_assemble_prior_knowledge` `SplitResult` arm already emits `tier_zero`
(Q-F4, Phase F.5) — Phase G's step-0 simply does not branch on it yet.

---

## 3. Design questions + user answers

| # | Question | Answer |
|---|----------|--------|
| Q-G1 | Where does `tier_zero` branch land? | **Phase H** (defer; Phase G does not add it). |
| Q-G2 | `handle_disambiguation` (called by §0.9:1302) doesn't exist — implement? | **Yes.** Emit `__emit_event__("disambiguation_required", candidates=...)` + `__transition_to__("disambiguation", "awaiting user choice")` + `return complete_result(state, "disambiguation", response=<candidate list>, extra={"candidates": candidates})` so the turn ends for user choice. |
| Q-G3 | `_set_active_skills_from_matched_ids` needs `ActiveSkillProvenance{doc_id,name,version,snippet_names}` but only has bare UUIDs; `__fetch_component__` returns `ComponentItem` (no version/snippet_names); plan says NO `__list_skills__`. Metadata source? | **Rust `handle_assemble_prior_knowledge` emits a new `active_skills` field** (skill-class provenance list) in the pkr dict for the `SplitResult` + `Components` arms; the Python helper passes it to `__set_active_skills__`. New `fetch_skill_provenance_by_ids(pool, scope, ids)` in `db_skill_loader` (targeted `SELECT id, name, version FROM reborn_skills WHERE id=ANY($1) AND scope + validation gate`); version = major-version u32 parsed from DB String; `snippet_names=[]` (reborn_skills has no code_snippets column). Non-skills-db path (no pool) → `active_skills=[]`. |
| Q-G4 | `__resolve_component_by_name__` (Option B fallback host fn) doesn't exist; plan says "Both paths must be implemented." Add it? | **Yes.** Add `__resolve_component_by_name__(name, class_code)` host function now: Rust handler + dispatch arm, scope+validation-gated, returns dict-or-None matching `__fetch_component__`'s shape. Needs new `fetch_component_by_name(pool, scope, name, class_code)` helper in `retrieval_source.rs` (skills-db gated), mirroring `fetch_component_by_id`. |
| Q-G5 | `call_action` non-null `action_id` but `__fetch_component__` returns empty/None — fall back to Option B name lookup, error, or Tier-2? | **Fall back to Tier-2 (llm + prompt creation).** NOT Option B name lookup, NOT a hard error. Monty-safe propagation: the unresolvable result is a **result-dict marker** `{"error": "call_action: Action '{}' not resolvable...".format(name), "unresolvable_action": True}` (NOT a custom exception — Monty "no classes"). It propagates via `return` from `_execute_action_steps`; `execute_action_procedure` converts `unresolvable_action` → `complete_result(state, "fall_back_to_tier2", extra={"reason": ...})` BEFORE the `if "error" in result:` check; step-0's `action_short_circuit` branch then falls through to Tier-2. |
| Q-G6 | The call_action step-name→UUID data-migration SQL location — plan said "NOT a Flyway migration (data-only)". | **Make it a Flyway migration after all** (overrides the plan; runs automatically on deploy). New migration **V062**. |

**Monty-safe propagation rationale (Q-G5):** The `{"error":..., "unresolvable_action": True}`
marker propagates via `return` from `_execute_action_steps` (exits the function
immediately). It is caught by `try_catch` blocks (which check `"error" in sub_result`
→ run the catch arm — preserves existing behavior, the action author handles it).
When NOT in a try_catch (top-level or one level deep), it reaches
`execute_action_procedure` which converts it to `outcome:"fall_back_to_tier2"` →
step-0 falls through to Tier-2. Deep-nested `call_action` chains (call_action
inside another call_action's steps) swallow the marker in `_last_result`
(pre-existing behavior, documented as a known limitation — NOT introduced by
Phase G). Custom exception classes are impossible (Monty "no classes"), so the
result-dict marker is the only Monty-safe mechanism.

---

## 4. Implementation substeps (one-by-one; each verified + committed + pushed)

**Execution order:** Rust (G.1, G.2) → Python helpers (G.3, G.4) → Python step-0
core (G.5) → Python call_action (G.6) → migration (G.7) → tests (G.8).

### G.1 — Rust: `active_skills` provenance fetch + emit (Q-G3)

- Add `fetch_skill_provenance_by_ids(pool, scope, skill_ids) -> Result<Vec<serde_json::Value>, DbSkillStoreError>`
  to `crates/brassclaw_engine/src/executor/db_skill_loader.rs` (`#[cfg(feature="skills-db")]`).
  Targeted query:
  ```sql
  SELECT id::text, name, version
  FROM reborn_skills
  WHERE id = ANY($1)
    AND tenant_id  = $2
    AND user_id    = $3
    AND agent_id   = $4
    AND project_id = $5
    AND validation_status = 'validated'
    AND '05:validator' != ALL(consumer_tags)
  ORDER BY class_code, prompt_uid
  ```
  Build `[{doc_id, name, version: <major u32>, snippet_names: [], force_activated: false}]`.
  Parse `version: String` → major u32 (`v.split('.').next()` → parse → default 1).
- Thread `pg_pool` (cfg-gated) into `handle_assemble_prior_knowledge`
  (signature + dispatch arm at `orchestrator.rs:711`), exactly like
  `handle_list_skills` / `handle_fetch_component`.
- In `handle_assemble_prior_knowledge`, for the **`SplitResult`** arm: extract
  skill-class ids (class 1–3) from `orchestrator_items`, call
  `fetch_skill_provenance_by_ids`, emit `active_skills` in the pkr dict.
  Same for the **`Components`** arm (extract from `items`). The
  `ActionShortCircuit` + `Disambiguation` arms emit `active_skills: []`
  (no skills active). Non-skills-db path (no pool) → `active_skills: []`.
- **Test (this substep):** Rust unit test in `orchestrator.rs::tests` asserting
  the `SplitResult` + `Components` pkr dicts carry an `active_skills` key (shape
  `[{doc_id,name,version(int),snippet_names:[],force_activated:false}]`), via the
  existing `MockRetrievalSource` (Phase F.7) + a mock/skip path for the DB helper
  (skills-db gated; skip-if-no-pool — assert `active_skills: []` on the
  no-pool path, and the helper's version-parse logic via a focused unit test).

### G.2 — Rust: `__resolve_component_by_name__` host function (Q-G4)

- New `fetch_component_by_name(pool, scope, name, class_code) -> Result<Vec<ComponentItem>, RetrievalSourceError>`
  in `crates/brassclaw_engine/src/memory/retrieval_source.rs` (`#[cfg(feature="skills-db")]`),
  mirroring `fetch_component_by_id` (`retrieval_source.rs:1023`): same
  `class_code_to_table` mapping + scope + SEC-01 validation gate, but
  `WHERE name = $1` (+ scope + validation) `LIMIT 1` (bind order: name first,
  then scope tuple).
- New handler `handle_resolve_component_by_name(args, thread, #[cfg(feature="skills-db")] pg_pool)`
  in `orchestrator.rs`, returning the same dict-or-`Value::Null` shape as
  `handle_fetch_component` (`orchestrator.rs:2783`).
- Register dispatch arm `"__resolve_component_by_name__"` in the host-fn dispatch
  (`orchestrator.rs:721` region), threading `pg_pool` (cfg-gated).
- **Test (this substep):** Rust unit test asserting
  `handle_resolve_component_by_name` returns `Value::Null` on the no-pool path
  (cfg default) + a DB-integration test (composition `tests/`, skip-if-no-docker)
  mirroring `fetch_component.rs` for the named-lookup path.

### G.3 — Python: `handle_disambiguation(candidates, state)` (Q-G2)

- New function in `default.py` (helpers region, before `run_loop`):
  emit `__emit_event__("disambiguation_required", candidates=candidates)`;
  `__transition_to__("disambiguation", "awaiting user choice")`;
  `return complete_result(state, "disambiguation", response=<candidate list>,
  extra={"candidates": candidates})`.
- The `<candidate list>` is the human-readable rendering of `candidates`
  (each candidate is `{component_id, component_class_code, class_label, score}`
  per the `Disambiguation` arm). Build a Monty-safe plain list of strings
  (no list-comprehension-with-if) — `"{} (class {}, score {})".format(...)`.
- Not yet called from step-0 (wired in G.5).

### G.4 — Python: `_set_active_skills_from_matched_ids(matched_component_ids, state, active_skills)` (Q-G3)

- New helper in `default.py` (helpers region). **3-arg signature** (documented
  minor deviation from the plan's 2-arg `_set_active_skills_from_matched_ids(
  matched_component_ids, state)` — justified by Q-G3 putting `active_skills` in
  pkr; recorded here + in the saved_plan reference).
- Body: if `active_skills` is non-empty → `__set_active_skills__(active_skills)`;
  build `skill_names` (Monty-safe loop, no list-comp-with-if) →
  `__emit_event__("skill_activated", skill_names=",".join(skill_names))`;
  `state["active_skill_ids"] = matched_component_ids`;
  `state["skill_snippet_names"] = []` (reborn_skills has no code_snippets —
  §1 finding 7). If `active_skills` empty → still set
  `state["active_skill_ids"] = matched_component_ids` + `skill_snippet_names=[]`
  (no `__set_active_skills__` call, no event).
- Not yet called from step-0 (wired in G.5).

### G.5 — Python: restructure step-0 to the §0.9 v3 flow (the core)

Replace `default.py:994–1060` with the §0.9 v3 flow **minus `tier_zero`** (Q-G1):
```python
token_budget = config.get("prior_knowledge_token_budget", 100000) if isinstance(config, dict) else 100000
pkr = __assemble_prior_knowledge__(goal, token_budget, "02")
active_skills = []
matched_ids = []
if isinstance(pkr, dict):
    matched_ids = pkr.get("matched_component_ids", [])
    active_skills = pkr.get("active_skills", [])
    if pkr.get("action_short_circuit"):
        __emit_event__("action_started", action_name=pkr.get("action_name", ""))
        __transition_to__("running", "action execution")
        action_doc = __fetch_component__(pkr.get("action_component_id", ""), 16)
        if isinstance(action_doc, dict):
            action_result = execute_action_procedure(action_doc, goal, state)
            if action_result.get("outcome") == "fall_back_to_tier2":
                __emit_event__("action_unresolved", action_name=pkr.get("action_name", ""))
                __transition_to__("prompting", "action unresolved -> tier-2")
                # fall through to Tier-2 (override/orchestrator_content path below)
            else:
                __transition_to__("completed", "action completed")
                return action_result
        else:
            __emit_event__("action_unresolved", action_name=pkr.get("action_name", ""))
            __transition_to__("prompting", "action not fetched -> tier-2")
            # fall through to Tier-2
    elif pkr.get("disambiguation"):
        return handle_disambiguation(pkr.get("candidates", []), state)
    elif pkr.get("override_prompt_creation"):
        working_messages = [{"role": "User",
                              "content": pkr.get("orchestrator_content", pkr.get("formatted_content", ""))}]
    elif pkr.get("orchestrator_content"):
        insert_as_user_message_at_n_minus_1(working_messages, pkr["orchestrator_content"])
# Outside `if isinstance(pkr, dict):` — always run (baseline preserved when pkr
# is not a dict, e.g. legacy non-dict return).
insert_volatile_context_at_n_minus_1(working_messages)
_set_active_skills_from_matched_ids(matched_ids, state, active_skills)
```
- **Removes** the dead `docs = __retrieve_docs__(goal, 5)` shim (1010–1028) +
  the `__list_skills__()` / `select_skills()` block (1030–1059) — these are the
  same edit (the old block is replaced wholesale).
- **Tier-2 fallback works** because `ActionShortCircuit` pkr has
  `orchestrator_content=""` + `override_prompt_creation=false`, so the
  `elif orchestrator_content:` branch is skipped (no-op) and control reaches
  `__llm_complete__` with the un-augmented `working_messages`.
- **Backward compat:** `pkr["formatted_content"]` stays supported as an alias
  (the `override_prompt_creation` arm reads `orchestrator_content` first, falls
  back to `formatted_content`).

### G.6 — Python: migrate `call_action` + `execute_action_procedure` (Q-G4/Q-G5)

- `call_action` (`default.py:839–857`): replace
  `nested_docs = __retrieve_docs__(nested_name, 1)` with:
  ```python
  nested_action_id = step_def.get("action_id", "")
  nested_action = None
  if nested_action_id:
      fetched = __fetch_component__(nested_action_id, 16)
      nested_action = fetched if isinstance(fetched, dict) else None
  else:
      resolved = __resolve_component_by_name__(nested_name, 16)
      nested_action = resolved if isinstance(resolved, dict) else None
  if not nested_action:
      return {"error": "call_action: Action '{}' not resolvable (no action_id and name lookup failed)".format(nested_name), "unresolvable_action": True}, step_counter
  ```
  After the nested `_execute_action_steps` call, propagate the marker:
  if `sub_result is not None and sub_result.get("unresolvable_action")`:
  `return sub_result, step_counter` (bubble the `fall_back_to_tier2` signal up
  through nested call_action chains to `execute_action_procedure`).
- `execute_action_procedure` (`default.py:901`): insert BEFORE
  `if "error" in result:`:
  ```python
  if result is not None and result.get("unresolvable_action"):
      return complete_result(state, "fall_back_to_tier2",
                             extra={"reason": result.get("error", "")})
  ```

### G.7 — V062 Flyway migration (Q-G6)

- New `crates/brassclaw_pg/migrations/V062__call_action_action_id_resolution.sql`
  — the call_action step-name → `action_id` UUID data migration SQL (from plan
  lines 5284–5311), made **idempotent** (`AND step->>'action_id' IS NULL` in the
  CASE condition so re-running is safe) + the post-migration audit query as a
  trailing `\echo`-style comment (Flyway runs the UPDATE; the audit SELECT is
  documented in the migration header comment for operators to run manually
  after deploy, since Flyway does not return SELECT rows).

### G.8 — Phase G tests (plan list, `saved_plan:5343–5347`)

- **Extend the Monty test harness** (`orchestrator.rs::tests`): add a
  `run_python_step0` helper (sibling of `run_python_final`) that runs
  `run_loop(context, goal, actions, state, config)` with `step==0` and mocks the
  step-0 host functions:
  - `__assemble_prior_knowledge__` → a preset pkr dict (per test case).
  - `__fetch_component__` → a preset action doc (or `None`).
  - `__resolve_component_by_name__` → a preset doc (or `None`).
  - `__emit_event__` / `__transition_to__` / `__set_active_skills__` /
    `insert_volatile_context_at_n_minus_1` / `insert_as_user_message_at_n_minus_1`
    → capture into a recorder vec (assert call sites + args).
  - `__llm_complete__` → returns a terminal "complete" outcome; captures the
    `working_messages` it received (assert orchestrator_content was injected).
  - `max_iterations = 1` so only step-0 runs.
- **Pure-unit tests (both configs):**
  1. step-0 with upgraded pkr (`orchestrator_content` set, no short-circuit) →
     `orchestrator_content` injected at N-1; `__list_skills__` NOT called;
     `__retrieve_docs__` shim NOT called.
  2. pkr `action_short_circuit: true` + `__fetch_component__` returns a doc →
     `execute_action_procedure` runs (action doc fetched by UUID), NO
     `__llm_complete__` call; outcome `completed`.
  3. pkr `action_short_circuit: true` + `__fetch_component__` returns None →
     `action_unresolved` event + falls through to Tier-2 (`__llm_complete__`
     called).
  4. pkr `disambiguation: true` → `handle_disambiguation` runs (event
     `disambiguation_required` + outcome `disambiguation`); NO
     `__llm_complete__`.
  5. no-match pkr (`Components` broad-scan, `orchestrator_content` set) →
     `orchestrator_content` injected + `_set_active_skills_from_matched_ids`
     called with the pkr `active_skills`.
- **Integration test (composition `tests/`, skip-if-no-docker):**
  `call_action` using `__fetch_component__(action_id, 16)` → correct Action
  fetched by UUID (mirror `fetch_component.rs`).
- If extending the harness proves too complex mid-implementation, write a
  `subplan_problem_stepG8_*.md` per the task rules and execute it before
  resuming.

**Verification (both configs — default + `--features brassclaw_engine/skills-db`
for engine; default + `--features brassclaw_reborn_composition/skills-db` for
composition tests):**
- `cargo fmt --all -- --check`
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default + skills-db)
- `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  (default + `--features brassclaw_reborn_composition/skills-db`)
- `cargo test -p brassclaw_engine --lib` (default + skills-db)
- DB-integration tests skip on this host (no docker) — correct-by-grounding
  against migrations + the handler SQL.

**Commit + push** each completed substep to `origin/main` before the next.

---

## 5. Verification state

(updated as substeps complete)

- G.1 — Done. `fetch_skill_provenance_by_ids` in `db_skill_loader.rs` +
  `skill_provenance_for_items` helper + `handle_assemble_prior_knowledge`
  `pg_pool` threading + `assemble_from_component_items` `active_skills` arg
  + all four `FetchForTurnResult` arms emit `active_skills` +
  `phase_g1_active_skills_emitted_in_every_arm` test. fmt/clippy/test green
  both configs (677/688 lib tests). Committed+pushed `e7c2ce31`.
- G.2 — Done. `fetch_component_by_name` in `retrieval_source.rs`
  (name=$1 + scope + SEC-01 gate + LIMIT 1) +
  `handle_resolve_component_by_name` handler (dict-or-Null) +
  `__resolve_component_by_name__` dispatch arm (cfg-gated pg_pool) +
  `phase_g2_resolve_by_name_returns_null_on_unresolvable_paths` unit test
  (both configs) + composition `fetch_component_by_name_resolves_action_item`
  / `fetch_component_by_name_is_tenant_scoped` (DB-integration, skip-if-no-
  docker). fmt/clippy/test green both configs (678/689 lib). Committed+pushed
  `da1bee7b`.
- G.3 — Done. `handle_disambiguation(candidates, state)` in `default.py`
  (helpers region, before `run_loop`): emits `disambiguation_required`,
  transitions to `disambiguation`, returns `complete_result(outcome=
  "disambiguation", response=<candidate list>, extra={"candidates": ...})`.
  Monty-safe (for-loop + `.format()` + `.append()` + `"\n".join`). Not yet
  called (wired in G.5). Verified: ast.parse clean; clippy clean both configs;
  96 orchestrator helper tests pass both configs (Monty parses it).
  Committed+pushed `0b02d024`.
- G.4 — pending.
- G.5 — pending.
- G.6 — pending.
- G.7 — pending.
- G.8 — pending.
