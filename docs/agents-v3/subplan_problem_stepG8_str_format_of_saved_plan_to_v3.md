# Subplan — Step G.8 problem: `str.format()` is not Monty-supported

Parent plan: `saved_plan_to_v3.md` → Phase G (Recipe System Finalisation).
Phase G subplan: `docs/agents-v3/subplan_problem_stepG_of_saved_plan_to_v3.md` (substep G.8).
G-STUB subplan: `docs/agents-v3/subplan_stub_stepG_action_steps_of_saved_plan_to_v3.md`.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Discovered while implementing **G.8** (the Phase G test substep): G.8 test #4 drove
`handle_disambiguation` through the Monty VM for the first time and exposed a latent
production crash. Inserted as a sub-substep before G.8 resumes, because G.8's
disambiguation unit test cannot pass while `handle_disambiguation` crashes the VM.

---

## 1. The problem

`orchestrator/default.py` builds human-readable / error strings with the
`"...".format(...)` method in **5** places:

| Line | Code | Reachability |
|------|------|--------------|
| 973 | `handle_disambiguation` response line: `"{} (class {}, score {})".format(...)` | **Live** — every intent-disambiguation result (§3.12 / G.3). |
| 736 | `_execute_action_steps` `tool_call` blocked: `"...'{}' not in allowed_tools (SEC-07)".format(tool_name)` | Action error path. |
| 804 | `_execute_action_steps` `call_skill` blocked: `"...'{}' not in allowed_tools (SEC-07)".format(skill_name)` | Action error path. |
| 833 | `_execute_action_steps` `parallel` tool blocked: `"...'{}' not in allowed_tools (SEC-07)".format(tool_name)` | Action error path. |
| 857 | `_execute_action_steps` `call_action` unresolvable: `"...Action '{}' not resolvable...".format(nested_name)` | Action error path. |

The file's own style guide (`default.py:241`: "no f-strings (use `"...".format(...)`)")
and the `handle_disambiguation` docstring (`default.py:965`: "`.format()` ...
established Monty-supported patterns, default.py:103") both **assert** that
`str.format()` is Monty-supported. It is not.

**Proof:** `crates/monty/src/types/str.rs:324-327` lists `format()` /
`format_map()` among the str methods that are **deliberately unimplemented**
("Requires implementing the format spec mini-language (PEP 3101), which is
complex..."). Driving `handle_disambiguation` through Monty raises:

```
AttributeError: 'str' object has no attribute 'format'
  File "test.py", line 1540, in <module>
    result = run_loop(context, goal, actions, state, config)
  File "test.py", line 1118, in run_loop
    return handle_disambiguation(pkr.get("candidates", []), state)
  File "test.py", line 973, in handle_disambiguation
```

This is the "written half-way and then silenced" anti-pattern the task calls
out: G.3 added the disambiguation path but **no test ever executed it through
Monty**, so the unsupported-method crash was never observed. The 4 action-step
error-path uses are the same bug class — they crash the VM the moment a
blocked-tool / unresolvable-call_action error is hit during deterministic
Action execution.

`str.format()` is the only unsupported method in play; `str.join` (used at
`default.py:1013` `",".join(skill_names)` and at `handle_disambiguation`'s
`"\n".join(lines)`), `str()` (used at `default.py:1513` `str(...)`), and `+`
string concatenation are all Monty-supported and already used throughout
`default.py`.

---

## 2. Fix design

Replace each `"...".format(arg)` with Monty-safe `+` string concatenation,
wrapping non-string interpolands in `str(...)` (Monty raises on `str + int`).

- **Line 973 (`handle_disambiguation`)** — the response line:
  ```python
  lines.append("{} (class {}, score {})".format(
      c.get("class_label", ""),
      c.get("component_class_code", ""),
      c.get("score", ""),
  ))
  ```
  becomes
  ```python
  lines.append(
      c.get("class_label", "")
      + " (class " + str(c.get("component_class_code", "")) + ", score "
      + str(c.get("score", "")) + ")"
  )
  ```
  `class_label` is already a str; `component_class_code` is an int and `score`
  is a float, so both are wrapped in `str()`. `"\n".join(lines)` is unchanged
  (Monty-supported).

- **Lines 736 / 804 / 833 / 857 (`_execute_action_steps` error paths)** — each
  `"error": "....'{}'....".format(name)` becomes
  `"error": "....'" + name + "'...."` (`tool_name` / `skill_name` /
  `nested_name` are already strings from `step_def.get(...)`).

- Update the two stale docstring/comment claims (`default.py:241` style guide
  and `handle_disambiguation:965`) so they no longer assert `.format()` is
  Monty-supported — point at `+` concatenation + `str()` instead. (Per the
  task rule: do not blindly remove, repair/complete instead of deleting.)

No other file changes. No migration. No API change.

## 3. Files touched

- `crates/brassclaw_engine/orchestrator/default.py`
  - `handle_disambiguation` response-line build (line ~973) + its docstring
    (line ~965).
  - `_execute_action_steps` 4 error-string builds (lines ~736, 804, 833, 857).
  - Style-guide comment (line ~241).

## 4. Tests

- **G.8 unit test #4** (`step0_disambiguation_surfaces_candidates_without_llm`
  in `orchestrator.rs::tests`) already drives `handle_disambiguation` through
  Monty and asserts the response lists `Tool: foo` / `Action: bar` — this
  becomes the end-to-end regression for the line-973 fix.
- **New G.8 unit test #6**
  (`step0_action_tool_call_blocked_returns_error_outcome`): an
  `action_short_circuit` pkr + a fetched action doc whose single step is a
  `tool_call` to a tool NOT in `allowed_tools` → `execute_action_procedure`
  returns outcome `error` with the SEC-07 message built via the (now
  concatenation) line-736 path, and NO `__llm_complete__`. This is the
  end-to-end regression for the action-step error-path fixes.

## 5. Verification (both configs — default + `--features brassclaw_engine/skills-db`)

- `cargo fmt --all -- --check`
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default + skills-db)
- `cargo test -p brassclaw_engine --lib step0_` (default + skills-db) — the 6
  G.8 unit tests, all green.
- `cargo test -p brassclaw_engine --lib` (default + skills-db) — full suite,
  no regressions vs the 678 default / 689 skills-db baseline.

## 6. Sequencing

Execute this subplan BEFORE resuming G.8. G.8 test #4 (disambiguation) crashes
without the line-973 fix; the new test #6 validates the action-step error-path
fixes. After this subplan completes, resume G.8 (run the full G.8 unit suite +
the composition integration test).

---

## 7. Status

- **Decision:** fix all 5 `str.format()` usages with Monty-safe `+`
  concatenation + `str()`; repair the 2 stale docstring/comment claims.
- **Done:**
  - `default.py` `handle_disambiguation` response line (line ~973) → `+` +
    `str()`; docstring (line ~964) repaired to point at `+`/`str()` and note
    `str.format()` is unimplemented.
  - `default.py` `_execute_action_steps` 4 error paths (lines ~736, 804, 833,
    857) → `+` concatenation.
  - `default.py` style-guide comment (line ~241) repaired: "no f-strings and
    NO `\"...\".format(...)` — `str.format()` is unimplemented in Monty; use
    `+` concatenation + `str()`".
  - G.8 unit test #6 `step0_action_tool_call_blocked_returns_error_outcome`
    added (drives the line-736 SEC-07 block path through Monty; asserts
    outcome `error` + the blocked-tool message + no `__llm_complete__`).
  - Verified both configs: `cargo fmt --all -- --check` clean;
    `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` clean
    (default + `--features brassclaw_engine/skills-db`);
    `cargo test -p brassclaw_engine --lib` = **684 default / 695 skills-db**
    (0 failed both; +6 vs the 678/689 baseline = the 6 G.8 `step0_` tests).
    The previously-failing G.8 test #4 (disambiguation) now passes; test #6
    passes.
- **Adjacent repair (encountered during verification, not in the original
  subplan):** the skills-db full-lib run surfaced an intermittent failure of
  `load_reduction_rules_db_error_returns_empty_and_caches`. Root cause:
  `invalidate_reduction_rules_cache()` is a process-wide flush (clears EVERY
  cached slot), and 4 reduction-rules tests call it while `cargo test` runs
  them in parallel — a sibling's flush can clear this test's slot between its
  two `load_reduction_rules` calls, turning the "DB called once" assertion
  into a spurious second query. Pre-existing flake, unrelated to the
  `str.format()` fix. Per the task rule (resolve, don't suppress), fixed with
  a test-only `std::sync::Mutex` serialization lock
  (`REDUCTION_RULES_TEST_LOCK` in `orchestrator.rs::tests`) acquired by all 4
  reduction-rules cache tests; the 3 async tests hold it across `.await` with
  `#[allow(clippy::await_holding_lock)]` (current-thread `#[tokio::test]`
  runtime → no self-deadlock), matching the existing oauth/hooks convention
  (16 prior uses). No new dependency, no production code change. Confirmed
  fixed: skills-db full lib run twice → 695 passed / 0 failed both times.

