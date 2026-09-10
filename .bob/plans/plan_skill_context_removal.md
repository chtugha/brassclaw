# Subplan: Remove v1 Skill-Context Injection Machinery

> **Status:** Draft — not started.
> **Parent plan:** `saved_plan_to_v3.md` Phase P.1 (VFS skills removal continuation)
> **Triggered by:** architectural audit — user confirmed v3 is fully orchestrator-based;
> "use skill coding" → intent match → Recipe execution; there is no per-turn SKILL.md
> injection path in v3.

---

## Background — two paths inside the same struct

`SelectableSkillContextSource<S>` serves two completely different purposes:

| Method | Purpose | v3 Status |
|--------|---------|-----------|
| `record_user_message` / `peek_message_text` | Stores raw user text keyed by `AcceptedMessageRef` → feeds `InputStage::resolve_message_text` → `state.last_user_text` → `RecipeStage` intent matching | **LIVE — needed** |
| `load_skill_context_candidates` (via `HostSkillContextSource` impl) | Reads SKILL.md from VFS → `HostSkillContextCandidate` blobs → `LoopContextSnippet` → LLM context injection | **DEAD — v1 only** |

The `HostSkillContextSource` trait is entirely the dead half.  It is implemented by:
- `SelectableSkillContextSource` (`first_party_extension_ports/src/activation.rs:791`)
- `SkillBundleContextSource` (`loop_support/src/skill_bundle_context_source.rs:58`)

Both implementations exist only to serve the dead SKILL.md-injection path.

## What is dead and why

In v3, when a user says "use skill coding":
1. `InputStage::drain` calls `resolve_message_text` → `peek_message_text` → raw text
2. `RecipeStage::process` calls `fetch_for_turn(raw_text)` → intent match in DB
3. If matched: `TierZeroStage` runs the Recipe → reply (no LLM)
4. If not matched: Tier 1/2 fall-through → `PromptStage` → LLM with DB-assembled base-prompt bundle (from `PgBasicPromptStore`)

In step 4, the `instruction_snippets` from `HostSkillContextSource::load_skill_context_candidates`
WOULD be injected into the LLM context window — **but**:
- Phase P.1 already moved all skills to DB rows (`reborn_skills`) seeded in `builtin_bootstrap.rs`
- The VFS SKILL.md files are no longer the source of truth
- Skills in v3 are DB components injected via the `PgBasicPromptStore` prefix bundle (Phase K.1),
  NOT via per-turn `load_skill_context_candidates` calls
- The local-dev `FilesystemSkillBundleSource` reads from a VFS that no longer has meaningful content
  (the old `skills/` directory's SKILL.md files reference v1 concepts: `MemoryDocs`, `Missions`,
  v1 `skill_activate` API — none of which exist in v3)

**Conclusion:** `HostSkillContextSource::load_skill_context_candidates` is dead. It either:
- Returns empty (VFS has no skills installed) → `instruction_snippets = []` → no-op
- Returns stale v1 blobs (from old SKILL.md files) → LLM sees broken v1 instructions

Both outcomes are wrong. The safe v3 path is to remove the injection path entirely.

## What stays

`SelectableSkillContextSource` is NOT fully removed. The `messages_by_run` store and
`peek_message_text` / `record_user_message` methods stay — they are the mechanism by
which `SkillActivationMessageTextResolver::resolve_message_text` recovers the raw user
text for `InputStage`. This is the Tier-0 intent-matching prerequisite.

After removal, `SelectableSkillContextSource` becomes a pure message-text-recorder with
no VFS or SKILL.md dependency. Its generic `<S: SkillBundleSource>` type parameter also
goes away once the `load_skill_context_candidates` path is removed.

## Removal sequence (one commit per step)

### Step 1 — Remove `HostSkillContextSource` field from `ThreadBackedLoopContextPort`

**Files:**
- `crates/brassclaw_loop_support/src/lib.rs`
  - Remove `skill_context_source: Option<Arc<dyn HostSkillContextSource>>` field (line 206)
  - Remove `with_skill_context_source` builder (lines 283-286)
  - Remove `build_skill_instruction_snippets` call in `load_loop_context` (lines 371-377)
  - Remove `skill_context_source: None` from `Self { ... }` constructor
  - Remove `HostSkillContextSource` from `use` import (line 84) if no longer used here

**Impact:** `skill_snippets` in `LoopContextBundle.instruction_snippets` is now always empty
(the prefix bundle from `system_bundle_source` / `PgBasicPromptStore` still injects at snippet #0).

**Tests to update:**
- `thread_loop_support_contract.rs` — all tests using `with_skill_context_source` must be
  updated: remove the builder call; the `SkillBundleContextSource` mock is no longer needed.
- Any test asserting `instruction_snippet_count > 0` for skill snippets must be updated
  (prefix bundle snippets still work; skill-specific snippet tests should be removed).

### Step 2 — Remove `skill_context_source` from `ThreadBackedLoopModelPort`

**Files:**
- `crates/brassclaw_loop_support/src/lib.rs` lines 898-954
  - Remove `skill_context_source` field (line 898)
  - Remove `with_skill_context_source` builder (lines 953-955)
  - Remove wiring at lines 1366-1378 (`build_skill_instruction_snippets` call in the
    model-port path — same dead path as step 1 but on the model side)

### Step 3 — Remove `skill_context_source` from `RebornLoopDriverHost` and `RuntimeParts`

**Files:**
- `crates/brassclaw_reborn/src/loop_driver_host.rs`
  - Remove `skill_context_source: Option<Arc<dyn HostSkillContextSource>>` field (line 900)
  - Remove `with_skill_context_source` builder (lines 1135-1137)
  - Remove wiring at line 1540-1542 (`context_adapter.with_skill_context_source`)
  - Remove `skill_context_source` from the `RebornLoopDriverHostParts` clone at line 1751
  - Remove import `HostSkillContextSource` (line 31)
- `crates/brassclaw_reborn/src/runtime.rs`
  - Remove `skill_context_source: Option<Arc<dyn HostSkillContextSource>>` (line 137)
  - Remove import (line 12)
- `crates/brassclaw_reborn/src/loop_driver_host/model_gateway.rs`
  - Remove `skill_context_source` field (line 24)
  - Remove `with_skill_context_source` call (line 50)

**Tests to update:**
- `crates/brassclaw_reborn/tests/loop_driver_host.rs` — remove `HostSkillContextSource` import,
  remove `StaticSkillContextSource` mock, remove `with_skill_context_source` calls.

### Step 4 — Remove `skill_context_source` from `RebornRuntimeInput`

**Files:**
- `crates/brassclaw_reborn_composition/src/runtime_input.rs`
  - Remove `skill_context_source: Option<Arc<dyn HostSkillContextSource>>` field (line 282)
  - Remove `with_skill_context_source` builder (lines 474-477)
  - Remove `use brassclaw_loop_support::HostSkillContextSource` (line 30)

**Impact:** `with_skill_context_source` was only used in the test at `runtime.rs:4776-4796`.
That test (`send_user_message_uses_caller_supplied_skill_context_source`) must be removed.

### Step 5 — Remove `LocalDevSkillContextSource.source` and the `configured_skill_context_source` path

**Files:**
- `crates/brassclaw_reborn_composition/src/runtime.rs`
  - `LocalDevSkillContextSource` struct: remove `source: Arc<dyn HostSkillContextSource>` field (line 3186)
  - `local_dev_filesystem_skill_context_source`: remove `source: selectable_skills.host_skill_context_source()` from return (line 3240)
  - The match at lines 2107-2138 (`configured_skill_context_source` branch): The variable
    `skill_context_source` (the `Option<Arc<dyn HostSkillContextSource>>`) that this builds
    is only used downstream to call `host_factory.with_skill_context_source(...)` — once
    `RebornRuntimeInput.skill_context_source` is removed (step 4), this whole branch simplifies.
    Only `skill_activation_source` and `skill_execution_adapter` remain from this match.
  - Remove the `HostSkillContextSource` import (line 47 area)

### Step 6 — Remove `builtin.skill_activate` synthetic capability

This is the v1 LLM-callable tool that let the LLM activate skills by name. In v3, the user says
"use skill coding" → intent match → Recipe; the LLM never calls `skill_activate`.

**Files:**
- `crates/brassclaw_reborn_composition/src/runtime/local_dev/skill_activation.rs` — **DELETE FILE**
- `crates/brassclaw_reborn_composition/src/runtime/local_dev.rs`
  - Remove `mod skill_activation;` declaration (line 54)
  - Remove `use skill_activation::skill_activation_capability;` (line 61)
  - Remove `pub(crate) use skill_activation::SKILL_ACTIVATE_CAPABILITY_ID;` (line 60)
  - Remove `skill_activation_source` parameter from `capability_wiring` (line 93)
  - Remove the `skill_activation_capability(...)` call inside `capability_wiring`

**Tests to update:**
- `local_dev/tests.rs` — remove tests referencing `SKILL_ACTIVATE_CAPABILITY_ID`:
  `local_dev_skill_activate_tool_loads_selected_skill_context` (line 833),
  `capability_wiring_with_skill_activation_source_exposes_skill_activate_capability` (line 966),
  the assertion at line 827, the capability-count check at line 624.
- `factory.rs` tests that reference `SKILL_ACTIVATE_CAPABILITY_ID` (lines 2966, 2988).

### Step 7 — Remove `HostSkillContextSource` trait and its implementations

**Files:**
- `crates/brassclaw_loop_support/src/skill_context.rs`
  - Remove the `HostSkillContextSource` trait definition (lines 22-27)
  - Remove `HostSkillContextCandidate` struct and impls (lines 30-65)
  - Remove `HostSkillContextBuildError` and impl (lines 67-103)
  - Remove `build_skill_instruction_snippets` function (lines 106-125)
  - Remove `build_skill_run_snapshot` function (lines 127-155)
  - Remove `parsed_skill_to_snapshot_entry` (lines 157-170)
  - The `SkillSourceKind` import can stay (used elsewhere)
  - `skill_context_error_to_host_error` can stay (used for the remaining `SkillContextService` tests)
  - Re-evaluate: if `SkillContextService` itself is only used to consume `SkillRunSnapshot`
    built from the dead `HostSkillContextCandidate` path, remove `SkillContextService` too.

- `crates/brassclaw_loop_support/src/skill_bundle_context_source.rs` — **DELETE FILE**
  (entire file is the `SkillBundleContextSource` impl of `HostSkillContextSource`)
- `crates/brassclaw_loop_support/src/lib.rs`
  - Remove `pub use skill_bundle_context_source::SkillBundleContextSource;` (line 78)
  - Remove `mod skill_bundle_context_source;` (if mod declared)
  - Remove pub re-exports of the dead types from `skill_context` module (lines 83-88)

- `crates/brassclaw_first_party_extension_ports/src/activation.rs`
  - Remove `impl<S> HostSkillContextSource for SelectableSkillContextSource<S>` (line 791+)
  - Remove the `load_skill_context_candidates` method body
  - The `SelectableSkillContextSource` struct itself stays (it still has `record_user_message` / `peek_message_text`)

**Tests to remove:**
- All tests in `thread_loop_support_contract.rs` that use `SkillBundleContextSource` (most of the
  skill-context-related tests — ~20 tests). Replace with simpler tests that validate the
  `messages_by_run` / `peek_message_text` path only.

### Step 8 — Simplify `SelectableSkillContextSource` generic parameter

Currently `SelectableSkillContextSource<S: SkillBundleSource>` is generic over `S` because
`load_skill_context_candidates` needs to call `S::read_skill_bundle_file`. Once that method
is removed, the generic parameter can be dropped (or replaced with a concrete internal type).

**Impact:** `LocalDevSelectableSkillContextSource` type alias and all the `<FilesystemSkillBundleSource<LocalDevRootFilesystem>>` generic instantiations go away. `SkillActivationMessageTextResolver<S>` in `retrieval_lookup_impl.rs` becomes non-generic.

This is the cleanest final state: `SelectableSkillContextSource` becomes a plain struct with no VFS dependency.

### Step 9 — Remove `FilesystemSkillBundleSource` (VFS SKILL.md reader)

Once step 8 removes the generic dependency, `FilesystemSkillBundleSource` is unused.

**Files:**
- `crates/brassclaw_loop_support/src/filesystem_skill_bundle_source.rs` — **DELETE FILE**
- `crates/brassclaw_loop_support/src/lib.rs` — remove `pub use filesystem_skill_bundle_source::...`

### Step 10 — Simplify `FirstPartySkillsExtension`

`crates/brassclaw_first_party_extension_ports/src/skills.rs` currently has:
- `host_skill_context_source()` → feeds the dead `HostSkillContextSource` path
- `activation_source()` → feeds `record_user_message` / `peek_message_text` (LIVE)
- `execution_adapter()` → feeds `execute_skill_message` (test-only)
- `selectable_skill_runtime_with_setup_markers()` → builds all three

After step 8, the struct simplifies significantly. `host_skill_context_source()` method removed;
`selectable_skill_runtime_with_setup_markers` still needed for `activation_source()`.

### Step 11 — Remove `SkillBundleSource` trait (if orphaned)

`crates/brassclaw_loop_support/src/skill_bundle_source.rs` defines the `SkillBundleSource` trait
with two methods: `list_skill_bundles` and `read_skill_bundle_file`. Both are only called by
`FilesystemSkillBundleSource` (deleted in step 9) and `SelectableSkillContextSource::load_skill_context_candidates` (deleted in step 7). If no other consumer remains, delete this file too.

---

## Not removed

- `SelectableSkillContextSource::record_user_message` / `peek_message_text` / `clear_accepted_message` → STAYS (feeds `SkillActivationMessageTextResolver`)
- `SkillActivationMessageTextResolver` in `retrieval_lookup_impl.rs` → STAYS (feeds `InputStage`)
- `SkillExecutionAdapter` / `execute_skill_message` → STAYS (test-only API for skill-aware CLI, not actively dead yet; mark as `#[cfg(any(test, feature = "test-support"))]` if appropriate)
- `builtin.skill_list` / `builtin.skill_install` / `builtin.skill_remove` → STAYS (DB-backed skill management tools; exposed to orchestrator as first-class callables; covered by builtin_bootstrap.rs step)
- `handle_list_skills` in `orchestrator.rs` → STAYS (callable from custom orchestrators even though basic-mode no longer issues it)

---

## Test impact summary

| File | Action |
|------|--------|
| `thread_loop_support_contract.rs` | Remove ~20 skill-context tests; keep `messages_by_run` / `peek_message_text` tests |
| `loop_driver_host.rs` (tests) | Remove `StaticSkillContextSource` mock; remove `with_skill_context_source` usage |
| `runtime.rs` (tests) | Remove `send_user_message_uses_caller_supplied_skill_context_source` test |
| `local_dev/tests.rs` | Remove `skill_activate`-related tests (3-4 tests) |
| `factory.rs` (tests) | Remove `SKILL_ACTIVATE_CAPABILITY_ID` assertions |
| `skill_bundle_context_source.rs` tests | Deleted with file |
| `filesystem_skill_bundle_source.rs` tests | Deleted with file |
| `activation.rs` tests | Prune tests for `load_skill_context_candidates`; keep `record_user_message`/`peek_message_text` tests |

---

## Risk assessment

- **Low risk:** Steps 1-6 (remove wiring, remove dead capability). The `None` path is already the
  production default on the PG/hosted path — the code is already written to handle absence gracefully.
- **Medium risk:** Steps 7-8 (remove trait + simplify generic). Many test mocks use `SkillBundleContextSource`.
  Careful test updates required.
- **Low risk after 7-8:** Steps 9-11 (delete files). Mechanical once deps are gone.

---

## Saved plan update

After all 11 steps complete, update `saved_plan_to_v3.md` Phase P.1 with an additional "Step C"
describing the VFS-layer cleanup as a follow-on to Steps A and B (which covered `bundled_skills.rs`
and `management.rs` `SkillSource::System` respectively).
