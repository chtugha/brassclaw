# Subplan — H8.4a `active_skills` provenance mechanism obsolescence

Parent: `./subplan_problem_stepH8_of_saved_plan_to_v3.md` step **H8.4** (delete the dormant
Model A PK path). Zenflow task `e81125fc-ce63-449e-922a-dfa80b964019`, chat
`be1470ab-f612-4526-bc95-e1e37c8f4527`.

## 1. Problem encountered during H8.4

H8.4 deletes the dormant Model A prior-knowledge path. The cascade verification
(workspace grep) found that this makes the **entire `active_skills` provenance
mechanism dormant** — and the mechanism is **not re-wired on the live Model B/C
agent-loop path**:

- `__set_active_skills__` dispatch arm + `handle_set_active_skills`
  (`crates/brassclaw_engine/src/executor/orchestrator.rs:761-762`, `:3887-3910`) — still
  present, but **no caller** after H8.4.
- `_set_active_skills_from_matched_ids` (`crates/brassclaw_engine/orchestrator/default.py:992`)
  — still present, but its **only caller** (the `default.py` step-0 PK block) was deleted by
  H8.4.
- `thread.set_active_skills()` / `active_skills()` / `ActiveSkillProvenance` /
  `ACTIVE_SKILLS_METADATA_KEY` (`crates/brassclaw_engine/src/types/thread.rs:194-378`) — still
  present, **no Model B/C path populates them** (confirmed: zero `set_active_skills` /
  `__set_active_skills__` calls in `brassclaw_reborn_composition` or `brassclaw_agent_loop`;
  the `brassclaw_first_party_extension_ports` hits are an unrelated extension-activation
  concept, `SkillActivationSelector`).
- `fetch_skill_provenance_by_ids` (`crates/brassclaw_engine/src/executor/db_skill_loader.rs:97-177`)
  — dead; its **only caller** `skill_provenance_for_items` was deleted by H8.4.
- `crates/brassclaw_engine/prompts/mission_skill_repair.md` — orphaned (no `include_str!`
  reference; the mission system that loaded it was deleted in H.5 O2.3).
- `tests/engine_v2_skill_codeact.rs::skill_codeact_persists_active_skill_provenance`
  (`:736-873`, `#[cfg(feature="skills-db")]` + docker-gated) — verifies the now-deleted path
  (`skill_provenance_for_items` → `__set_active_skills__` via step-0
  `__assemble_prior_knowledge__`). It compiles + **skips locally (no docker) → GREEN**, but is a
  **latent CI break**: with docker it `panic!("expected github skill provenance...")` because
  step-0 no longer calls `__set_active_skills__`. Its private `pg_rig` helper module
  (`:666-734`) is exclusive to this test.

The plan (`saved_plan_to_v3.md:1318-1320`) intended `active_skills` to stay populated via
`_set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []))` on the Model A
step-0 path. H8.4 removed that call site (correct — Q1=delete dormant Model A), and Model B/C
never wired a replacement.

## 2. User decision (lock)

**Q-active_skills = B — obsolete, delete.** The `active_skills` /
`ActiveSkillProvenance` / `__set_active_skills__` / `handle_set_active_skills` /
`_set_active_skills_from_matched_ids` / thread.rs metadata key / `mission_skill_repair.md`
prompt / the docker test are all obsolete — superseded by the orchestrated Sempai validation
system. Write this obsolescence subplan (like H.5 O1–O5) and delete them all. This folds into /
follows H8.4.

## 3. Deletion map

All in `crates/brassclaw_engine/` (unco-mingled with the user's concurrent prefix-cache WIP in
other crates) + `tests/engine_v2_skill_codeact.rs` + one prompt file.

### 3.1 Rust — `crates/brassclaw_engine/src/executor/orchestrator.rs`
- **D1** — `__set_active_skills__` dispatch arm: the `// __set_active_skills__(skills)` comment
  + `"__set_active_skills__" => handle_set_active_skills(args, thread),` (`:761-762`). Keep the
  `__regex_match__` arm above and `__validate_component__` arm below.
- **D2** — `"skill_activated" => { ... }` event-name match arm in `handle_emit_event`
  (`:2471-2479`). Dead once the only emitter (`_set_active_skills_from_matched_ids`,
  `default.py:1017`) is deleted. The string match has a `_ =>` default arm, so removal is
  compile-safe. Keep the `"budget_warning"` arm below.
- **D3** — `handle_set_active_skills` fn + doc (`:3887-3910`). Keep `handle_validate_component`
  below (`:3912+`).
- **D4** — remove `ActiveSkillProvenance` from
  `use crate::types::thread::{ActiveSkillProvenance, Thread, ThreadState};` (`:53`) →
  `{Thread, ThreadState}`.

### 3.2 Rust — `crates/brassclaw_engine/src/types/thread.rs`
- **D5** — `ActiveSkillProvenance` struct + doc (`:194-204`).
- **D6** — `ACTIVE_SKILLS_METADATA_KEY` const (`:206`).
- **D7** — `set_active_skills` + `active_skills` fns + docs (`:350-378`).
- **D8** — `active_skill_provenance_roundtrips_through_metadata` test (`:686-700`).

### 3.3 Rust — `crates/brassclaw_engine/src/lib.rs`
- **D9** — remove `ActiveSkillProvenance` from
  `pub use types::thread::{ActiveSkillProvenance, Thread, ThreadConfig, ThreadId, ThreadState, ThreadType};`
  (`:60-62`). Confirmed no cross-crate import (`ActiveSkillProvenance` is not referenced outside
  `brassclaw_engine`).

### 3.4 Rust — `crates/brassclaw_engine/src/executor/db_skill_loader.rs`
- **D10** — `fetch_skill_provenance_by_ids` fn + doc (`:97-177`). Dead (only caller
  `skill_provenance_for_items` deleted in H8.4). `SkillScope` import stays (used by
  `scope_from_thread_ids` / `fetch_llm_skills_as_json` / `fetch_monty_skills_as_json`).
- **D11** — remove `fetch_skill_provenance_by_ids` from the `pub use inner::{...}` re-export
  (`:439`).

### 3.5 Python — `crates/brassclaw_engine/orchestrator/default.py`
- **D12** — `_set_active_skills_from_matched_ids` def + docstring + body (`:992-1021`) + the
  trailing blank lines. Dead (only caller, the step-0 PK block, deleted by H8.4). Keep
  `_parse_orchestrator_channel_steps` below (`:1024+`).

### 3.6 Prompt file
- **D13** — delete `crates/brassclaw_engine/prompts/mission_skill_repair.md` (orphaned; no
  `include_str!` reference, mission system deleted in H.5 O2.3).

### 3.7 Test — `tests/engine_v2_skill_codeact.rs`
- **D14** — `pg_rig` module + its doc comment (`:666-734`). Exclusive to the deleted test
  (only `pg_rig::Rig::start()` reference is `:780`, inside the deleted test).
- **D15** — `skill_codeact_persists_active_skill_provenance` test fn + doc (`:736-873`).
  Shared helpers (`make_github_skill_doc`, `canned_github_issues`, `HttpMockEffects`,
  `ScriptedLlm`, `TestStore`) STAY — used by `non_matching_goal_skips_skill_codeact` (`:877+`).
  `ThreadManager::with_pg_pool` STAYS — still consumed by the 3 live dispatch arms
  (`__list_skills__` / `__fetch_component__` / `__resolve_component_by_name__`).

### 3.8 Doc-comment updates (retired-symbol refs)
- **DC1** — `crates/brassclaw_engine/src/memory/retrieval_source.rs:113-114` —
  `matched_component_ids` doc "the `_set_active_skills` identity set for the turn" →
  "the orchestrator-channel identity set for the turn (Wilson scoring /
  `record_recipe_outcome`)".
- **DC2** — `crates/brassclaw_engine/src/memory/retrieval_source.rs:939-941` — same ref in the
  `PostgresSource` routing comment.

## 4. Deferred cascades (DOCUMENT, do NOT delete — cross-crate breaking)

- **`SkillActivated` event variant** in `crates/brassclaw_common/src/event.rs:422,732` — a `pub`
  enum variant (`EventKind::SkillActivated { skill_names }`, `#[serde(rename="skill_activated")]`).
  After D2 it is never constructed in `brassclaw_engine`. Removing a `pub` serde-tagged enum
  variant is a **serialization-breaking API change** (old persisted/audited events with
  `"skill_activated"` would fail to deserialize) and is out of scope for H8.4a. rustc/clippy do
  not warn on unused `pub` enum variants. **Left in place + documented.** A separate
  `brassclaw_common` event-enum cleanup decides its fate.
- **`tests/e2e/scenarios/test_skill_oauth_flow.py:458`** — `has_skill_event = "skill_activated"
  in event_types`. With the emission gone (`default.py:1017` deleted via D12), this e2e
  assertion becomes a **latent CI break** (docker-gated, skips locally without docker). The test
  is a broader OAuth-skill-flow scenario, not solely an `active_skills` test; deleting it is out
  of scope. **Left in place + documented** for a separate e2e cleanup decision.
- **Historical docs** — `saved_plan_to_v3.md` (active_skills refs at `:1106`, `:1318-1326`,
  `:5250`, `:5486-5487`, `:9444`), `docs/agents-v3/04-ibs.md:235`,
  `docs/agents-v3/05-skills-system.md:187`, `docs/agents-v3/13-orchestrator-default-py.md`,
  `docs/agents-v3/subplan_problem_stepG_of_saved_plan_to_v3.md`,
  `docs/agents-v3/subplan_problem_stepH4_8_of_saved_plan_to_v3.md`,
  `MESSAGE_FLOW_AND_PLAN_AUDIT.md:360`, `docs/plans/Recipe_Reviewed_v3_plan.md` — historical
  design/audit text. **Left as-is** (do not rewrite history); the obsolescence is recorded here
  + in the `saved_plan_to_v3.md` H8.4 substep note.

## 5. Status tracker

- H8.4a — Pending. Execute D1–D15 + DC1–DC2; verify
  `cargo check -p brassclaw_engine` (default + `--features skills-db`) +
  `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (both) +
  `cargo test -p brassclaw_engine` (both) GREEN; ships in the H8.4 commit (the active_skills
  deletion is a direct consequence of H8.4 deleting the step-0 call site — both touch
  `orchestrator.rs`, so they land in one GREEN commit).
