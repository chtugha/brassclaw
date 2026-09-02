# Subplan — Problem Step C.2 of `saved_plan_to_v3.md` (builtin host-component seed)

> **Scope locked by the user (2026-09-03):** C.2 = **spec + seed (data only, no Rust)**.
> The Step 27 spec in `builtin_stuff_v3.md` is complete (the "migration" is done). C.2
> builds an **idempotent boot seed** (`seed_builtin_host_components`) that inserts the
> Step 27 component stacks into the DB at startup. No Rust dispatch arms, no handler
> changes — those belong to **C.6** (the new cross-turn-persistent driver fn).
>
> **Placement (β, locked):** `execute_orchestrator` IS deleted in C.7. The C.1 `host.*`
> arms + `host` namespace injection currently live in `execute_orchestrator` only because
> it is the sole Monty-driver fn with a `FunctionCall` match today; they are **dormant**
> (execute_orchestrator is Model-A, not reached by the agent-loop) and **move to the new
> C.6 driver fn** when it is built. C.2 does NOT touch `execute_orchestrator`.

## Goal

A single idempotent boot seed that materialises every Step 27 component as a validated
DB row, so the C.6 driver (and the intent/retrieval systems) can resolve them by name
without a kohai/sempai validation pass. Builtins bypass Q1 (they ship validated).

## Component inventory (from `builtin_stuff_v3.md` Step 27)

Per the LOCKED classification — RETIRED/DROPPED items are NOT seeded.

### 8 `host.*` Tools — each a 5-component stack (Tool cl.0 + ToolSkill cl.13 + PythonCode cl.22 + Leaf Skill cl.1 + Recipe cl.21)

| Substep | Tool | Handler exists? |
|---------|------|-----------------|
| 27.1 | `host.resolve_intent` | yes (C.1) |
| 27.2 | `host.compose_orchestrator` (rewrite) | yes (pre-existing; rewrite deferred — seed spec as-is) |
| 27.3 | `host.post_reply` | yes (C.1) |
| 27.7.2 | `host.fetch_component` | yes |
| 27.7.3 | `host.resolve_component_by_name` | yes |
| 27.7.4 | `host.validate_component` | yes |
| 27.9.1 | `host.check_signals` | yes |
| 27.10.3 | `host.kohai_complete` | deferred (new logic) — seed spec as-is, handler lands in its own substep |

### 3 Recipes (no new Tool row) + their PythonCode formatters

| Substep | Recipe | Notes |
|---------|--------|-------|
| 27.4 | `host-save-history` (cl.21) + `pc-host-history-format` (cl.22) | over `builtin.memory_write` (Step 11) — no new Tool |
| 27.10.1 | `host-assemble-prior-knowledge` (cl.21, Tier 1 fallback) | no retrieval verbs |
| 27.10.2 | `host-non-match-llm-answer` (cl.21, Tier 2 Kohai-mediated) | over `host.kohai_complete` |

### 1 ExtensionCatalogue

| Substep | Catalogue | Children |
|---------|-----------|----------|
| 27.11 | `builtin-host` (cl.23) | the 8 `host.*` tool component ids + 3 recipe ids |

### Reused (no new component row — already exist)

`builtin.memory_write` (Step 11), `first_party_tools/http` (Step 20), `builtin.skill_list`
(Step 16), `pc-regex-match` (Step 20.x.2). These are referenced by the recipes but NOT
re-seeded.

### Stale spec to correct (NOT seeded)

- **27.6.1 `pc-host-execute-parallel`** — STALE. The user retired
  `__execute_actions_parallel__` entirely (portion 102/103: Monty is single-threaded, a
  parallel helper would degrade to sequential). Correct 27.6.1 in `builtin_stuff_v3.md`
  to **RETIRED**, do NOT seed.

## Slices (one-by-one; clippy green both configs + commit + push each)

- **Slice 0 — finish mechanism grounding + doc corrections.**
  - Ground the class-0 (Tool) store + insert API + which table (retrieval returns None
    for class 0, but the row lives somewhere — find the `reborn_*` table + `New*` struct).
  - Ground the class-1 (Skill) store (`reborn_skills`) + `New*` + insert.
  - Ground the class-13 (ToolSkill) store (`reborn_tool_skills`) + `New*` + insert.
  - Ground the **validated-status mechanism**: how to insert builtins as `validated`
    (bypass Q1 pending). Check `update_validation_status` / a `source = "builtin"` fast-path.
  - Ground the **boot wiring point**: where `seed_builtin_providers` is called
    (`webui.rs:143`) → add `seed_builtin_host_components` alongside (or `factory.rs`).
  - Correct stale placement notes: `saved_plan_to_v3.md` C.1/C.6 text + this subplan's
    C subplan `:86-87` is already correct under β; mark portion-101 placement stale.
  - Correct `builtin_stuff_v3.md` 27.6.1 → RETIRED.
- **Slice 1 — seed fn skeleton + `builtin-host` catalogue row (27.11).**
  `seed_builtin_host_components(pool)`; idempotent (upsert by name+project or
  check-exists-then-insert); inserts the class-23 `builtin-host` row with empty
  `child_component_ids` (filled incrementally as the tool/recipe ids are minted).
  Wire into boot. clippy + commit.
- **Slices 2–9 — one per `host.*` tool (27.1, 27.2, 27.3, 27.7.2, 27.7.3, 27.7.4,
  27.9.1, 27.10.3).** Each: read that substep's spec, encode the 5 components (Tool +
  ToolSkill + PythonCode + Leaf Skill + Recipe) as `New*` rows, insert idempotently,
  append the minted ids to `builtin-host.child_component_ids`.
- **Slices 10–12 — the 3 Recipes (27.4, 27.10.1, 27.10.2) + their PythonCode formatters.**
  Append recipe ids to `builtin-host.child_component_ids`.
- **Slice 13 — final verification.** clippy `--all-targets -D warnings` (default +
  skills-db); `cargo test --lib`; `cargo check -p brassclaw_reborn_composition`; an
  integration test that boots + asserts the 45 rows exist + are validated + the
  catalogue children resolve. Mark C.2 complete; commit + push; proceed to **C.3**.

## Environment (carried)

`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build/test/clippy/check.
`df -h /Users/ollama/brassclaw-target` first — scoped `cargo clean -p
brassclaw_reborn_composition` (or `-p brassclaw_engine`) if Avail<15GB or >90%.
Both configs: default features `["postgres","root-llm-provider"]` + `--features skills-db`.
Docker/testcontainers NOT available → DB integration tests that need a live Postgres
SKIP locally (note in commit).

## Out of scope (explicit)

- Any Rust dispatch arm / handler change (→ C.6).
- The `compose_orchestrator` rewrite + `kohai_complete` new logic (seed spec as-is; the
  handler rework is its own substep).
- cdylib dynamic loading (→ C.3).
- Mode-driven security + WebUI panel (→ C.4).
