# deduplication-plan.md — Prompt-Path Deduplication Plan

> **Scope:** Resolve the duplications and dead-code leftovers found in the
> Reborn prompt-creation path (engine v2 `ExecutionLoop` + Python orchestrator).
> This plan is **prompt-path-only**. It does not touch the LLM provider layer,
> the kernel, or product surfaces beyond what is required to remove dead
> prompt-assembly code.
>
> **Status:** Plan only — no code changed.
>
> **Predecessor plan:** `integrate-postgres.md` is currently being implemented.
> Several findings below interact with that migration. Each finding is tagged
> with a **Migration interaction** section and a **Sequencing** directive. The
> global rule is: **findings marked `AFTER-PG` must wait until the relevant
> `integrate-postgres.md` phase ships; findings marked `INDEPENDENT` may be done
> in parallel with the migration.** Do not start any `AFTER-PG` item before
> confirming the corresponding Postgres phase is in the tree.

---

## 0. Background: the intended split

The Reborn prompt path has a deliberate Rust/Python boundary:

- **Rust** owns *mechanism and side effects* — building the stable system
  prompt, calling the LLM, executing tools, hosting the Monty VM, DB I/O.
- **Python** (the orchestrator in `crates/brassclaw_engine/orchestrator/default.py`)
  owns *decisions and policy* — which docs/skills to inject, when to nudge,
  when to compact/reduce, how to route the LLM response. Python is chosen so
  the self-improvement mission can patch the loop at runtime via a `MemoryDoc`
  titled `orchestrator:default`.

Some duplication is the *intentional* cost of that boundary (a helper must be
re-implemented in Python so it can be self-modified). Other duplication is
*leftover* from the v1→v2 migration and is safe to remove. This plan
distinguishes the two and only acts on the leftover kind, plus adds drift
guards for the intentional kind.

---

## 1. Findings

### Finding 1 — `build_step_context()` is dead in the v2 loop

**Class:** Leftover dead code. **Sequencing:** `AFTER-PG` (Phase 4 memory wiring).

**Description.** `build_step_context()` in
`./crates/brassclaw_engine/src/executor/context.rs:23` formats retrieved
memory docs as `## Prior Knowledge (from completed threads)` and injects them
as a synthetic **User** message just before the last user turn. The stated
purpose (in the file's own comments) is KV-cache hygiene: volatile per-turn
retrieval content must not mutate the stable system prefix.

**Evidence it is dead.** A repo-wide grep for `build_step_context` returns
matches only in:
- `./crates/brassclaw_engine/src/executor/context.rs:23` (definition)
- `./crates/brassclaw_engine/src/executor/context.rs:175,225,265` (the file's
  own `#[cfg(test)]` callers)

No production call site exists. The active v2 loop is
`ExecutionLoop::run()` (`./crates/brassclaw_engine/src/executor/loop_engine.rs:374`)
which hands off to the Python orchestrator. The orchestrator does its own
prior-knowledge formatting (`format_docs`, `default.py:234`) and injects via
`append_system_append` (appends to the **System** message, not User).

**Impact.** Two consequences:
1. The KV-cache-friendly User-message injection path the tests describe is
   *not* running in production. The active path appends prior knowledge to the
   System message on step 0, which mutates the stable prefix and defeats the
   per-turn prefix-cache benefit for that content. This is a real regression
   from the documented design, not just dead code.
2. The file ships a parallel `format_docs_as_context()` that duplicates the
   Python `format_docs()` output shape (Finding 5).

**Migration interaction.** `integrate-postgres.md` migrates the memory backend
behind the `Store` / `RetrievalEngine` seams (`brassclaw_memory_docs` §4.17,
`brassclaw_memory_chat_records` §4.29, file-less chunk indexing §4.30.1,
`EmbeddingRoleAdapter` §3). `build_step_context` consumes
`RetrievalEngine::retrieve_context`, so it transparently rides the new
Postgres backend once Phase 4 is wired. The decision here is about *call
sites*, not backends — but if the recommendation is to **resurrect** the
User-message injection path, it must be done after the Postgres retrieval is
wired so the resurrected path exercises the new backend, not the libSQL one.

**Recommended action.** Choose **one** of:

- **Option A (resurrect — recommended):** Wire `build_step_context` (or its
  equivalent) back into the v2 loop so prior knowledge is injected as a User
  message before the last user turn, restoring the KV-cache invariant the
  tests already assert. Concretely: have the orchestrator call a new host
  function `__inject_prior_knowledge__(docs)` that performs the User-message
  insertion via Rust, rather than `append_system_append`. Keep the
  formatting in Rust (`format_docs_as_context`) as the canonical formatter
  and have Python call through to it. This makes Finding 5 collapse into a
  single formatter.
- **Option B (delete):** Remove `context.rs` entirely and accept that prior
  knowledge mutates the system prefix on step 0. Update the engine `CLAUDE.md`
  to reflect the actual behavior. This is the smaller diff but keeps the
  prefix-cache regression.

Either way, **delete the stale claim** in `./crates/brassclaw_engine/CLAUDE.md`
that compaction lives in `executor/compaction.rs` (see Finding 2).

**Verification.**
- Option A: add an integration test that drives `ExecutionLoop::run` with a
  populated `Store` and asserts the prior-knowledge docs appear in a **User**
  message at position N-1, and that the System message is byte-identical
  across two turns with different retrieval results. The existing
  `context_injects_docs_before_last_user_message` test in `context.rs` is the
  shape; lift it to the integration tier.
- Option B: `cargo test -p brassclaw_engine` passes after deletion; grep
  confirms no remaining references.

---

### Finding 2 — `executor/compaction.rs` is a phantom; CLAUDE.md is stale

**Class:** Stale documentation. **Sequencing:** `INDEPENDENT`.

**Description.** `./crates/brassclaw_engine/CLAUDE.md` module map lists
`executor/compaction.rs` as "Context compaction when approaching model context
limit." The file does not exist (Glob `crates/brassclaw_engine/src/executor/compaction.rs`
returns no results). Compaction lives entirely in the Python orchestrator
(`compact_if_needed`, `default.py`).

**Impact.** An implementer reading `CLAUDE.md` will look for compaction logic
in the wrong place. Low severity, but it misdirects the next agent.

**Migration interaction.** None. `integrate-postgres.md` does not touch
compaction or the engine `CLAUDE.md`.

**Recommended action.** Update `./crates/brassclaw_engine/CLAUDE.md` module
map: remove the `compaction.rs` row and add a note that compaction is owned by
the Python orchestrator (`orchestrator/default.py:compact_if_needed`). While
there, also update the `context.rs` row to reflect Finding 1's resolution.

**Verification.** `grep compaction.rs crates/brassclaw_engine/CLAUDE.md`
returns nothing; `grep compact_if_needed crates/brassclaw_engine/CLAUDE.md`
returns the new note.

---

### Finding 3 — `brassclaw_skills::selector` (Rust) vs `default.py::score_skill` (Python)

**Class:** Intentional duplication (self-modification boundary) — **keep both,
add a drift guard.** **Sequencing:** `INDEPENDENT`.

**Description.** Skill scoring exists in two languages:
- Rust: `./crates/brassclaw_skills/src/selector.rs:312` (`score_skill`) and
  `prefilter_skills_with_options` at `:177`.
- Python: `./crates/brassclaw_engine/orchestrator/default.py:678`
  (`score_skill`) and `select_skills` at `:816`.

The Python file's own docstrings state the duplication:
- `default.py:681` — *"Scoring is aligned with the v1
  `brassclaw_skills::selector::score_skill`"*
- `default.py:819` — *"Mirrors the v1 Rust
  `brassclaw_skills::selector::prefilter_skills`"*

And `./crates/brassclaw_skills/src/lib.rs:20-21` explicitly acknowledges it:
*"`selector` — Rust-side deterministic scoring (`prefilter_skills`). In v2,
the equivalent logic lives in `orchestrator/default.py:score_skill()`."*

**Evidence both are live.** The Rust selector is **not** dead. It is called
by `./crates/brassclaw_first_party_extension_ports/src/activation.rs:951`
(`prefilter_skills_with_options`) for the extension-activation path. The
Python port is called by the v2 engine orchestrator. So both are production
code paths.

**Impact.** The two implementations can silently drift. A scoring change in
one (e.g. a new exclude-keyword, a different cap) will not propagate to the
other, producing different skill selection between the extension-activation
path and the engine orchestrator. This is the highest-risk duplication in the
prompt path because it changes *which* skills end up in the prompt.

**Migration interaction.** None directly. `integrate-postgres.md` migrates
where skill `MemoryDoc`s are stored, not the scoring algorithm. The
`__list_skills__` host call reads through the `Store` seam and will move to
Postgres transparently. The scoring logic itself is backend-independent.

**Recommended action.** Do **not** delete either implementation — the
boundary is intentional. Instead, add a **drift-corpus test** that pins the
two implementations to the same behavior:

1. Create a shared fixture file
   `./crates/brassclaw_skills/tests/fixtures/scoring_corpus.json` containing
   ~20 skill-manifest + user-message pairs with expected scores (covering:
   exact-word hit, substring hit, tag hit, regex hit, exclude-keyword veto,
   extracted-skill confidence factor, implicit-name matching).
2. Add a Rust test in `brassclaw_skills` that runs the corpus through
   `prefilter_skills_with_options` and asserts the expected selection.
3. Add a Python test runner (Monty-hosted, invoked from a
   `brassclaw_engine` integration test) that runs the same corpus through
   `orchestrator/default.py:select_skills` and asserts the same expected
   selection.
4. The two tests share the **same fixture file** and the **same expected
   outputs**, so any drift fails one or the other.

Additionally, add a one-line invariant comment at the top of both
`score_skill` functions: *"Keep in sync with
`brassclaw_skills::selector::score_skill` / `default.py::score_skill` — see
`scoring_corpus.json`."*

**Verification.** Both drift tests pass. Manually mutate one cap (e.g.
keyword cap 30 → 35) in one implementation and confirm the relevant test
fails; revert.

---

### Finding 4 — `llm_signals_tool_intent()` is dead in v2

**Class:** Leftover dead code. **Sequencing:** `INDEPENDENT`.

**Description.** `./crates/brassclaw_llm/src/reasoning.rs:48`
(`pub fn llm_signals_tool_intent`) is the v1 Rust tool-intent detector. The
v2 Python port is `./crates/brassclaw_engine/orchestrator/default.py:101`
(`signals_tool_intent`), whose docstring states *"Ported from V1 Rust
llm_signals_tool_intent()"*.

**Evidence it is dead in v2.** A repo-wide grep for `llm_signals_tool_intent`
returns matches only inside `./crates/brassclaw_llm/src/reasoning.rs` — the
definition (line 48) and its tests (lines 3449–3511). No production call site
exists. The active v2 loop uses the Python port.

**Caveat before deletion.** `brassclaw_llm` is a shared v2 crate (it lives in
`crates/`, not the removed `src/` v1 tree). Before deleting, confirm no
downstream v2 consumer references it via a re-export. The grep above already
covers the workspace; also check `Cargo.toml` feature flags for any
`llm_signals_tool_intent`-gated export. If clean, delete.

**Impact.** ~60 lines of dead production code plus ~70 lines of dead tests,
maintained and linted for nothing. More importantly, an implementer who
greps for "tool intent" will find the Rust version and assume it is active,
risking a change to dead code.

**Migration interaction.** None. `integrate-postgres.md` does not touch
`brassclaw_llm` reasoning logic.

**Recommended action.** Delete `llm_signals_tool_intent` and its tests from
`./crates/brassclaw_llm/src/reasoning.rs`. If a v1 compatibility shim is
still required (it should not be — `AGENTS.md` says v1 `src/` was removed in
Phase 6), gate it behind a `#[cfg(feature = "v1-compat")]` flag instead and
document the gate. Run `cargo clippy -p brassclaw_llm --all-targets -- -D
warnings` after deletion.

**Verification.** `cargo test -p brassclaw_llm` and `cargo clippy -p
brassclaw_llm --all-targets -- -D warnings` both pass. Grep
`llm_signals_tool_intent` across the workspace returns zero hits.

---

### Finding 5 — Prior-knowledge formatting exists in two languages

**Class:** Collapses with Finding 1. **Sequencing:** `AFTER-PG` (follows
Finding 1's resolution).

**Description.** Two formatters produce nearly identical
`## Prior Knowledge (from completed threads)` output:
- Rust: `format_docs_as_context` in
  `./crates/brassclaw_engine/src/executor/context.rs:78`
- Python: `format_docs` in
  `./crates/brassclaw_engine/orchestrator/default.py:234`

Both emit a `## Prior Knowledge` header, a per-doc `[LESSON|KNOWN ISSUE|SKILL|…]`
label, the doc title, and a 500-char-truncated body. The only difference is
the injection target (User message in Rust, System append in Python).

**Impact.** Output shape can drift (label set, truncation length, header
text). Today they happen to match.

**Migration interaction.** None directly — both read `MemoryDoc`s through
the same `Store` seam, which `integrate-postgres.md` moves to Postgres. The
formatters themselves are backend-independent.

**Recommended action.** This finding is resolved by Finding 1's decision:
- If Finding 1 → Option A (resurrect User-message injection): make the Rust
  `format_docs_as_context` the **single canonical formatter**, exposed to
  the orchestrator via a `__format_prior_knowledge__(docs)` host function.
  Delete the Python `format_docs` and have the orchestrator call through.
- If Finding 1 → Option B (delete `context.rs`): delete
  `format_docs_as_context` along with it; the Python `format_docs` becomes
  canonical by default.

Either way, exactly one formatter survives.

**Verification.** Grep for `## Prior Knowledge (from completed threads)`
returns exactly one source location.

---

### Finding 6 — System-prompt "find-and-append" pattern in two languages

**Class:** Intentional duplication — **no action, document only.**
**Sequencing:** `INDEPENDENT`.

**Description.** Two functions implement the "find the System message and
append to its content, else insert one at index 0" pattern:
- Rust: `upsert_codeact_system_prompt` in
  `./crates/brassclaw_engine/src/executor/prompt.rs:268`
- Python: `append_system_append` in
  `./crates/brassclaw_engine/orchestrator/default.py:270`

**Why this is intentional.** They serve different *content* on different
*owners*: Rust upserts the engine-owned stable system prompt (preamble +
capabilities + tools + postamble); Python appends per-turn prior knowledge
and active skills. They must not be merged — the Python one exists so the
self-modifiable orchestrator can manage its own appends without a Rust
recompile.

**Impact.** None functional. The only risk is an implementer "fixing" the
duplication by merging them and breaking the self-modification boundary.

**Migration interaction.** None.

**Recommended action.** Add a one-line comment to each pointing at the other
and stating the boundary is intentional:
- `prompt.rs:268` — *"Mirror of `orchestrator/default.py:append_system_append`.
  Rust owns the stable system prompt; Python owns per-turn appends. Do not
  merge — see deduplication-plan.md Finding 6."*
- `default.py:270` — *"Mirror of prompt.rs:upsert_codeact_system_prompt.
  Intentional split for self-modification; do not merge."*

No code change.

**Verification.** Comments present; no behavior change.

---

### Finding 7 — `brassclaw_engine/CLAUDE.md` module map is stale

**Class:** Stale documentation. **Sequencing:** `INDEPENDENT` (but do it
alongside Finding 2 to keep the doc fix atomic).

**Description.** The module map in `./crates/brassclaw_engine/CLAUDE.md`
lists `context.rs` as "Context builder (messages + actions from leases +
memory docs)" without noting it is not on the active v2 loop path (Finding 1),
and lists `compaction.rs` which does not exist (Finding 2).

**Migration interaction.** None.

**Recommended action.** Update the module map to reflect Finding 1's
resolution and remove the `compaction.rs` row (Finding 2). Add a row noting
the Python orchestrator owns compaction and per-turn prior-knowledge/skill
injection.

**Verification.** The module map rows match the actual `executor/` directory
listing produced by `ls crates/brassclaw_engine/src/executor/`.

---

## 2. Sequencing summary

| Finding | Class | Sequencing | Depends on |
|---------|-------|------------|------------|
| 1 — dead `build_step_context` | Leftover | `AFTER-PG` Phase 4 | Postgres memory wiring |
| 2 — phantom `compaction.rs` doc | Stale doc | `INDEPENDENT` | — |
| 3 — skill scoring drift | Intentional | `INDEPENDENT` | — |
| 4 — dead `llm_signals_tool_intent` | Leftover | `INDEPENDENT` | — |
| 5 — prior-knowledge formatting | Collapses with 1 | `AFTER-PG` | Finding 1 |
| 6 — system-prompt append pattern | Intentional | `INDEPENDENT` | — |
| 7 — stale CLAUDE.md module map | Stale doc | `INDEPENDENT` | Findings 1, 2 |

**Recommended execution order:**

1. **Batch 1 (INDEPENDENT, do anytime):** Findings 4, 6. Pure dead-code
   removal and comment additions. No migration dependency.
2. **Batch 2 (INDEPENDENT, do anytime):** Findings 3. Adds the drift-corpus
   test fixture and two consumers. No migration dependency.
3. **Batch 3 (after integrate-postgres Phase 4 ships):** Finding 1 (decide
   Option A or B), then Finding 5 (collapses from 1), then Finding 2 +
   Finding 7 (update CLAUDE.md to match the chosen resolution).

Batch 3 is gated because Finding 1's Option A resurrects a retrieval path
that should exercise the new Postgres backend, not the libSQL one. Starting
it before Phase 4 would mean re-validating against a backend that is about
to be removed.

---

## 3. Global guardrails for the implementing agent

- **Do not weaken prefix-cache stability.** The alphabetized capability and
  tool rendering in `prompt.rs` is load-bearing for vLLM/Anthropic prefix
  cache hits. Finding 1 Option A must not reorder the stable system prompt.
- **Do not merge intentional splits** (Findings 3, 6). The Python
  orchestrator is self-modifiable; merging its logic into Rust breaks the
  self-improvement mission's ability to patch the loop at runtime.
- **Respect the `AGENTS.md` rule:** no `.unwrap()`/`.expect()` in production,
  `thiserror` for errors, `cargo clippy --all --benches --tests --examples
  --all-features -- -D warnings` clean.
- **Test through the caller.** For Finding 1, the regression test must drive
  `ExecutionLoop::run`, not call `build_step_context` directly — the bug is
  that the caller stopped calling it.
- **Do not touch `integrate-postgres.md`'s scope.** If a finding's
  resolution requires a memory-backend change that the Postgres plan already
  owns, defer to the Postgres plan and only adjust call sites here.
- **After each batch:** run the targeted `cargo test -p <crate>` and
  `cargo clippy -p <crate> --all-targets -- -D warnings` for every crate
  touched, then the full `cargo clippy --all … -D warnings` once at the end.
