# Clever Prompts — Model-Aware Context Budget Distribution

**Status:** Design plan (not yet implemented)  
**Author:** Bob  
**Scope:** `crates/` only (v1 / `brassclaw_engine` is dead — no changes there)

---

## Problem Statement

Seven independent, model-blind token/byte limits exist across the codebase. Each
consumer enforces its own ceiling with no knowledge of (a) the model's actual
context window or (b) what other consumers have already reserved. This causes two
classes of visible failures today:

1. **Hard kills on budget overflow.** `SkillContextError::ContextBudgetExceeded`
   and `HostSkillContextBuildError::ContextBudgetExceeded` propagate all the way up
   as `LoopFailureKind::context_build_failed`, surfacing as a failed turn with no
   reply rather than a graceful degradation.

2. **Silent data loss.** `MAX_PROMPT_OVERLAY_CHARS = 4_000` and
   `CODE_EXECUTED_MAX_BYTES = 8_000` (both v1 — dead code, noted only for context)
   truncate content mid-sentence/mid-JSON. The Reborn equivalents
   (`LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES = 64 KiB`,
   `LOOP_CONTEXT_TOTAL_MODEL_CONTENT_MAX_BYTES = 256 KiB`) hard-error instead of
   degrading gracefully.

The underlying cause is a missing **central budget object** that pre-divides the
model's context window before distributing slices to each consumer.

---

## Design Principles (from answers)

**1. Soft slices, hard total.**  
The slice labels (`skill_snippet_tokens`, `history_tokens`, `tool_output_tokens`) are
**advisory defaults**, not strict per-category hard limits. The only hard constraint is
the model's total context window (`model_context_window`). If a turn has very few chat
messages but needs a lot of skill context, skill context is allowed to expand into the
history slice as long as the aggregate stays under the window. The budget object
hands each consumer its *starting* allocation; they report back how much they actually
used, and the next consumer can absorb any headroom.

**2. Model role architecture (future-facing).**  
Multi-model failover is not planned, but a model-role system is: each provider/model
can be assigned a **role** (e.g., `primary`, `skill_author`, `teacher`,
`domain_specialist`). `TurnContextBudget` is built from the **active role's** model
context window at turn start. The registry schema must accommodate `context_window_tokens`
per model entry (not just per provider), so that when a `skill_author` role is assigned
a different model, the budget is derived from that model's window rather than the
primary's.

**3. One lever: total token budget.**
There is no separate profile override or admin unlock. The allocation fractions are
**soft starting points** — any consumer that needs more simply takes headroom left by
others (up to the total window). To give everything more room, operators increase the
total token budget via the provider config (e.g. raise `context_window_tokens` for
their provider, or switch to a larger model). No special flag or profile concept is
needed. The implementation must never hard-block a consumer that exceeds its slice
fraction as long as the aggregate stays under the window.

---

## Current State — Hardcoded Constants (Reborn only)

| # | Constant | Value | File | Problem |
|---|----------|-------|------|---------|
| F1 | `MAX_SKILL_CONTEXT_TOKENS` | 4 000 tokens | `brassclaw_skills/src/selector.rs:17` | Model-blind pre-filter; rejects skills that would fit |
| F2 | `DEFAULT_MAX_CONTEXT_TOKENS` | 8 000 tokens | `brassclaw_agent_loop/src/strategies/context.rs:98` | Uncoordinated ceiling; ignores actual window |
| F3 | `AVG_TOKENS_PER_MESSAGE` | 200 | `strategies/context.rs:156`, `executor/prompt.rs:595` | Magic constant → wrong message counts |
| F4 | `LOOP_CONTEXT_TOTAL_MODEL_CONTENT_MAX_BYTES` | 256 KiB | `brassclaw_turns/src/run_profile/host.rs:716` | Hard-errors instead of graceful stop |
| F5 | `LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES` | 64 KiB | `brassclaw_turns/src/run_profile/host.rs:715` | Per-skill hard-reject instead of fallback to safe_description |

---

## Proposed Architecture — `TurnContextBudget`

### New type: `TurnContextBudget`

Location: **`crates/brassclaw_agent_loop/src/context_budget.rs`** (new file)

```rust
/// Pre-computed, per-turn advisory budget derived from the active model's context window.
///
/// Slices are **soft defaults**, not per-category hard limits. The only hard constraint
/// is `model_context_window`. Consumers receive their starting allocation, report actual
/// usage back, and the next consumer absorbs any headroom.
///
/// Built once at the start of `build_prompt_bundle_for_surface` from the active
/// model's registered `context_window_tokens`, then threaded to every consumer.
#[derive(Debug, Clone, Copy)]
pub struct TurnContextBudget {
    /// Total context window (tokens). Hard ceiling — never exceeded.
    pub model_context_window: u32,

    /// Tokens reserved for the model's own reply output. Always held back.
    pub output_reserved: u32,

    /// Starting advisory allocation for system instructions + skill snippets.
    pub skill_snippet_tokens: u32,

    /// Starting advisory allocation for conversation history (messages).
    pub history_tokens: u32,

    /// Starting advisory allocation for tool output text.
    pub tool_output_tokens: u32,
}

impl TurnContextBudget {
    /// Build from a resolved model context window using default allocation fractions.
    ///
    /// Default fractions (tunable in Phase 4 via DB-backed settings):
    ///   - output_reserved : 15 % of window (floor: 4 096, ceiling: window)
    ///   - skill_snippets  : 20 % of remaining
    ///   - tool_output     : 15 % of remaining
    ///   - history         : remaining ~65 %
    ///
    /// These are *starting* allocations. Budget accounting allows a consumer that
    /// uses less than its slice to donate the surplus to the next consumer.
    pub fn from_context_window(context_window_tokens: u32) -> Self {
        Self::from_context_window_with_allocation(
            context_window_tokens,
            &BudgetAllocation::default(),
        )
    }

    pub fn from_context_window_with_allocation(
        context_window_tokens: u32,
        allocation: &BudgetAllocation,
    ) -> Self {
        let output_reserved = ((context_window_tokens as u64
            * allocation.output_reserved_pct as u64)
            / 100) as u32;
        let output_reserved = output_reserved.max(4_096).min(context_window_tokens);
        let remaining = context_window_tokens.saturating_sub(output_reserved);
        let skill_snippet_tokens =
            (remaining as u64 * allocation.skill_snippet_pct as u64 / 100) as u32;
        let tool_output_tokens =
            (remaining as u64 * allocation.tool_output_pct as u64 / 100) as u32;
        let history_tokens = remaining
            .saturating_sub(skill_snippet_tokens)
            .saturating_sub(tool_output_tokens);
        Self {
            model_context_window: context_window_tokens,
            output_reserved,
            skill_snippet_tokens,
            history_tokens,
            tool_output_tokens,
        }
    }

    /// Bytes equivalent of `skill_snippet_tokens` (4 chars ≈ 1 token).
    pub fn skill_snippet_bytes(&self) -> usize {
        self.skill_snippet_tokens as usize * 4
    }

    /// Bytes equivalent of `history_tokens`.
    pub fn history_bytes(&self) -> usize {
        self.history_tokens as usize * 4
    }

    /// Bytes equivalent of `tool_output_tokens`.
    pub fn tool_output_bytes(&self) -> usize {
        self.tool_output_tokens as usize * 4
    }

    /// Estimated maximum messages that fit in `history_tokens`.
    pub fn max_messages_estimate(&self, avg_tokens_per_message: u32) -> u32 {
        self.history_tokens / avg_tokens_per_message.max(1)
    }
}

/// Advisory allocation percentages for a `TurnContextBudget`.
///
/// These are **starting-point hints**, not hard per-category limits. Any consumer
/// that exhausts its slice can absorb headroom from others as long as the aggregate
/// stays under `model_context_window`. Operators who want more room for everything
/// simply increase `context_window_tokens` in the provider config or switch to a
/// larger model — no special flag or override mechanism exists.
///
/// Stored in DB-backed settings (Phase 4).
#[derive(Debug, Clone, Copy)]
pub struct BudgetAllocation {
    pub output_reserved_pct: u8,
    pub skill_snippet_pct: u8,
    pub tool_output_pct: u8,
    // history_pct is implied: 100 - output_reserved_pct - skill_snippet_pct - tool_output_pct
}

impl Default for BudgetAllocation {
    fn default() -> Self {
        Self {
            output_reserved_pct: 15,
            skill_snippet_pct: 20,
            tool_output_pct: 15,
        }
    }
}

/// Default fallback context window when the model's value is not yet known.
///
/// Conservative minimum (works on Claude Haiku, GPT-3.5-class models).
pub const DEFAULT_FALLBACK_CONTEXT_WINDOW: u32 = 16_000;
```

### `ObservedMessageAverage` — replace `AVG_TOKENS_PER_MESSAGE = 200`

Location: same file, `crates/brassclaw_agent_loop/src/context_budget.rs`

```rust
/// Rolling exponential moving average of observed tokens per message.
///
/// Updated after each turn by the executor with
/// `actual_prompt_tokens / messages_in_bundle` from the model usage response.
/// Thread-safe; shared via Arc across the loop. Alpha = 0.25 (favours recent data).
pub struct ObservedMessageAverage {
    // EMA stored as fixed-point (tokens × 100) for lock-free integer arithmetic.
    value: Arc<AtomicUsize>,
}

impl ObservedMessageAverage {
    /// Starting estimate. Replaced quickly once real usage data flows in.
    pub const DEFAULT_AVG_TOKENS_PER_MESSAGE: u32 = 300;

    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicUsize::new(
                Self::DEFAULT_AVG_TOKENS_PER_MESSAGE as usize * 100,
            )),
        }
    }

    /// Read current estimate in whole tokens.
    pub fn get_tokens(&self) -> u32 {
        (self.value.load(Ordering::Relaxed) / 100) as u32
    }

    /// Update with a new observed average (EMA α = 0.25).
    pub fn update(&self, observed_tokens_per_message: u32) {
        let prev = self.value.load(Ordering::Relaxed) as u64;
        let next =
            (prev * 75 + (observed_tokens_per_message as u64 * 100) * 25) / 100;
        self.value.store(next as usize, Ordering::Relaxed);
    }
}
```

### Source of truth — `context_window_tokens` per model entry

The provider registry (`crates/brassclaw_llm/src/registry.rs`) stores
`ProviderDefinition` objects loaded from `providers.json`. Today the registry is
**per-provider**; once the model-role system arrives, it will need to be **per-model**
or have per-model overrides. The schema must support both now so Phase 0 doesn't
need to be redone.

**Step A — Add `context_window_tokens: Option<u32>` to `ProviderDefinition`**

```rust
// crates/brassclaw_llm/src/registry.rs
pub struct ProviderDefinition {
    // ... existing fields ...

    /// Context window in tokens for the default model of this provider.
    ///
    /// For the model-role system: per-model overrides will be stored in
    /// a `model_overrides: HashMap<String, ModelOverride>` added in a
    /// later phase — this field covers the "use provider default model" case.
    ///
    /// `None` → `TurnContextBudget` uses `DEFAULT_FALLBACK_CONTEXT_WINDOW`.
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
}
```

Known values to populate in `providers.json`:

| Provider / model | `context_window_tokens` |
|------------------|------------------------|
| `anthropic` (claude-sonnet-4-x, claude-opus-4-x) | 200 000 |
| `openai` (gpt-4o, gpt-4o-mini) | 128 000 |
| `openai` (o1, o3) | 200 000 |
| `ollama` (default — conservative) | 8 000 |
| `groq` (llama-3.1-70b, llama-3.3-70b) | 128 000 |
| `deepseek` (deepseek-chat, deepseek-r1) | 64 000 |
| `gemini` (gemini-2.5-pro, gemini-2.0-flash) | 1 000 000 |
| `openrouter` (passthrough — use `null`, resolved per model) | `null` |

**Step B — Expose `context_window_tokens()` on `LlmHost`**

```rust
// crates/brassclaw_llm/src/host.rs
impl LlmHost {
    pub fn context_window_tokens(&self) -> u32 {
        self.registry
            .find(self.resolved_provider_id())
            .and_then(|def| def.context_window_tokens)
            .unwrap_or(DEFAULT_FALLBACK_CONTEXT_WINDOW)
    }
}
```

---

## Implementation Plan — 5 Phases

Phase 4 (v1 engine) has been removed. v1 / `brassclaw_engine` is dead code.

### Phase 0 — Groundwork (no behavior change)

**Goal:** add new types and wire them through without changing any limit values.

1. Create `crates/brassclaw_agent_loop/src/context_budget.rs` with `TurnContextBudget`,
   `BudgetAllocation`, `ObservedMessageAverage`, and `DEFAULT_FALLBACK_CONTEXT_WINDOW`
   as specified above.
2. Add `context_window_tokens: Option<u32>` to `ProviderDefinition` in
   `crates/brassclaw_llm/src/registry.rs` (JSON-serde field, default `null`).
3. Populate `providers.json` with context window values per the table above.
4. Add `context_window_tokens()` accessor to `LlmHost` (`crates/brassclaw_llm/src/host.rs`).
5. Build a `TurnContextBudget` at the top of `build_prompt_bundle_for_surface`
   (`crates/brassclaw_agent_loop/src/executor/prompt.rs`), initially from the
   fallback window. Real values are wired in Phase 2 when `LlmHost` is accessible
   at that call site.
6. Add unit tests for `TurnContextBudget::from_context_window` and
   `from_context_window_with_allocation` with window sizes: 8 K, 16 K, 32 K, 128 K,
   200 K, 1 M. Assert that slices sum to `model_context_window`, `history_tokens > 0`
   for all inputs, and `output_reserved >= 4_096`.
7. `cargo test -p brassclaw_agent_loop -p brassclaw_llm && cargo clippy -p brassclaw_agent_loop -p brassclaw_llm -- -D warnings`

### Phase 1 — Fix F4/F5: graceful budget stop instead of hard error

**Goal:** `SkillContextError::ContextBudgetExceeded` must never propagate as a turn
failure. When the aggregate budget is full, return what was collected so far.

**Files:**
- `crates/brassclaw_turns/src/run_profile/skill_context.rs`
- `crates/brassclaw_loop_support/src/skill_context.rs`

**Changes:**

1. In the snippet collection loop in `SkillContextService`, replace the
   `checked_context_total_bytes(…)?` propagation with a `break` — collect all
   snippets that fit within the budget, then return them gracefully instead of
   failing the entire build.

2. For the per-snippet `LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES` check: when a
   single skill's `prompt_content` exceeds the per-snippet cap, **fall back to
   `safe_description` only** for that skill rather than returning `Err`. The skill
   remains visible to the model — only its full prompt is omitted. Record which
   skills were description-only in the result.

3. Add `snippets_truncated: bool` and `description_only_count: usize` to the
   service result type so the caller can emit a `debug!` log when truncation occurred.

4. In `crates/brassclaw_loop_support/src/skill_context.rs`, remove the
   `HostSkillContextBuildError::ContextBudgetExceeded` variant (it can no longer
   be reached after Step 1). If removing it would be a breaking API change, gate
   it `#[deprecated]` and `unreachable!()` the arm.

5. Update all tests that previously asserted the error is returned on overflow to
   assert truncated-but-successful results instead.

6. `cargo test -p brassclaw_turns -p brassclaw_loop_support && cargo clippy -p brassclaw_turns -p brassclaw_loop_support -- -D warnings`

### Phase 2 — Wire `TurnContextBudget` to the selector and context strategy

**Goal:** replace F1 (`MAX_SKILL_CONTEXT_TOKENS = 4_000`) and F2
(`DEFAULT_MAX_CONTEXT_TOKENS = 8_000`) with values derived from the budget. Replace
F3 (`AVG_TOKENS_PER_MESSAGE = 200`) with the live `ObservedMessageAverage`.

**Files:**
- `crates/brassclaw_skills/src/selector.rs`
- `crates/brassclaw_agent_loop/src/strategies/context.rs`
- `crates/brassclaw_agent_loop/src/executor/prompt.rs`
- `crates/brassclaw_agent_loop/src/executor/canonical.rs`
- `crates/brassclaw_llm/src/host.rs`

**Changes:**

1. Resolve `LlmHost::context_window_tokens()` at the start of
   `build_prompt_bundle_for_surface` and build a real `TurnContextBudget` from it
   (replacing the fallback-only budget from Phase 0).

2. Thread `budget.skill_snippet_tokens` into `prefilter_skills_with_options`
   as a `token_budget_tokens: u32` parameter, replacing the hardcoded
   `MAX_SKILL_CONTEXT_TOKENS`. Delete the constant.

3. Change `ContextStrategy::plan_context_request` signature to accept
   `budget: &TurnContextBudget`. In `DefaultContextStrategy`, replace
   `DEFAULT_MAX_CONTEXT_TOKENS` with `budget.history_tokens`. Delete the constant.

4. Add `Arc<ObservedMessageAverage>` to `DefaultContextStrategy`. Initialize the
   slot in the composition root and share it via `Arc`. Use
   `budget.max_messages_estimate(avg.get_tokens())` in place of the
   `AVG_TOKENS_PER_MESSAGE`-based estimate. Delete the `AVG_TOKENS_PER_MESSAGE`
   constant from both sites.

5. After each turn completes in `canonical.rs`, update the `ObservedMessageAverage`
   with `usage.prompt_tokens / messages_in_bundle` from the model response usage
   field (skip the update when `messages_in_bundle == 0`).

6. **Soft-slice accounting:** after the context strategy and skill selector each
   report their actual token usage, compute `headroom = budget.history_tokens -
   actual_history_tokens` and add it to `skill_snippet_tokens` before passing the
   budget to the skill context service. This implements the soft-slice rule from
   the design principles.

7. `cargo test -p brassclaw_agent_loop -p brassclaw_skills && cargo clippy -p brassclaw_agent_loop -p brassclaw_skills -- -D warnings`

### Phase 3 — Wire `TurnContextBudget` to the host-side byte budgets

**Goal:** make `LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES` and
`LOOP_CONTEXT_TOTAL_MODEL_CONTENT_MAX_BYTES` derived from the live budget rather
than compiled-in constants. The constants become safety guard-rails only.

**Files:**
- `crates/brassclaw_turns/src/run_profile/host.rs`
- `crates/brassclaw_turns/src/run_profile/skill_context.rs`
- `crates/brassclaw_reborn_composition/src/runtime_input.rs`

**Changes:**

1. Add `SkillContextBudget::from_turn_budget(budget: &TurnContextBudget) -> Self`:
   ```rust
   pub fn from_turn_budget(budget: &TurnContextBudget) -> Self {
       let max_context_bytes = budget.skill_snippet_bytes()
           .min(LOOP_CONTEXT_TOTAL_MODEL_CONTENT_MAX_BYTES);
       let max_snippet_bytes = (budget.skill_snippet_bytes() / 4)
           .min(LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES);
       Self { max_snippet_bytes, max_context_bytes }
   }
   ```

2. In the composition root (`runtime_input.rs`), replace the hardcoded
   `SkillContextBudget::default()` with `SkillContextBudget::from_turn_budget(&budget)`.

3. `LOOP_CONTEXT_SNIPPET_MODEL_CONTENT_MAX_BYTES` and
   `LOOP_CONTEXT_TOTAL_MODEL_CONTENT_MAX_BYTES` remain as `pub const` but their
   role changes: they are **absolute upper-bound guard-rails** (never exceeded
   regardless of model window), not the primary budget. Add doc comments clarifying
   this distinction.

4. `cargo test -p brassclaw_turns && cargo clippy -p brassclaw_turns -- -D warnings`

### Phase 4 — Observability and settings exposure

**Goal:** surface the computed budget in WebUI settings, emit per-turn debug
telemetry, and allow operators to tune allocation fractions.

**Files:**
- `crates/brassclaw_reborn_webui_ingress/` (settings GET/PUT endpoint)
- `crates/brassclaw_webui_v2_static/` (settings UI card)
- `crates/brassclaw_agent_loop/src/context_budget.rs`
- DB settings schema migration

**Changes:**

1. Add `context_budget_allocation` to the DB-backed settings schema:
   ```sql
   ALTER TABLE settings ADD COLUMN context_budget_allocation JSONB
       DEFAULT '{"output_reserved_pct":15,"skill_snippet_pct":20,"tool_output_pct":15}';
   ```
   Both PostgreSQL and libSQL backends must get the migration.

2. Load the allocation in the composition root and pass it to
   `TurnContextBudget::from_context_window_with_allocation(window, &allocation)`.
   Fall through to `BudgetAllocation::default()` when the column is absent or
   unparseable.

3. Emit at the start of every turn:
   ```rust
   debug!(
       model_context_window = budget.model_context_window,
       output_reserved = budget.output_reserved,
       skill_snippet_tokens = budget.skill_snippet_tokens,
       history_tokens = budget.history_tokens,
       tool_output_tokens = budget.tool_output_tokens,
       "turn context budget"
   );
   ```

4. Add a "Context Budget" card to the WebUI v2 settings page showing:
   - The active model's `context_window_tokens` (read-only, source: provider config)
   - The computed `output_reserved`, `skill_snippet_tokens`, `history_tokens`,
     `tool_output_tokens` in tokens
   - Editable sliders for `skill_snippet_pct` and `tool_output_pct` (history_pct
     derived as remainder). These adjust the *starting* allocation hints only.
   - A note: "Any section can exceed its hint if others leave headroom. To increase
     the total available budget, raise `context_window_tokens` in the provider config."

5. `cargo test -p brassclaw_turns -p brassclaw_reborn_webui_ingress && cargo clippy --all -- -D warnings`

---

## Migration Safety

- **Phases 0–3 are behavior-preserving** for any model with a context window ≥ 16 K.
  The only observable change is that turns which previously hard-failed with
  `context_build_failed` now succeed with gracefully truncated skill context (Phase 1),
  and skill prefiltering becomes less restrictive for large-window models (Phase 2).

- **Phase 4 adds a new DB column** — migration required for both backends. The
  column defaults to the compiled-in fractions, so existing installations see no
  behavioral change until an operator explicitly changes the sliders.

- **No override mechanism.** There is no profile flag or admin bypass for context
  limits. The single control is `context_window_tokens` in the provider config.
  Increasing it gives all consumers proportionally more room.

- **v1 / `brassclaw_engine` is excluded entirely** — F6 and F7 are noted for
  historical completeness but not implemented.

---

## Files Changed per Phase (summary)

| Phase | Key files |
|-------|-----------|
| 0 | `brassclaw_agent_loop/src/context_budget.rs` (new), `brassclaw_llm/src/registry.rs`, `providers.json`, `brassclaw_llm/src/host.rs`, `brassclaw_agent_loop/src/executor/prompt.rs` |
| 1 | `brassclaw_turns/src/run_profile/skill_context.rs`, `brassclaw_loop_support/src/skill_context.rs` |
| 2 | `brassclaw_skills/src/selector.rs`, `brassclaw_agent_loop/src/strategies/context.rs`, `brassclaw_agent_loop/src/executor/prompt.rs`, `brassclaw_agent_loop/src/executor/canonical.rs` |
| 3 | `brassclaw_turns/src/run_profile/host.rs`, `brassclaw_turns/src/run_profile/skill_context.rs`, `brassclaw_reborn_composition/src/runtime_input.rs` |
| 4 | DB migration, `brassclaw_reborn_webui_ingress/`, `brassclaw_webui_v2_static/`, `brassclaw_agent_loop/src/context_budget.rs` |

---

## Resolved Design Questions

| # | Question | Answer |
|---|----------|--------|
| 1 | Token counting accuracy | 4 chars ≈ 1 token heuristic is sufficient. Slices are soft advisory defaults; only the total window is a hard limit. No need for per-provider tokenizer. |
| 2 | Multi-model context window | Not applicable for failover (not planned). When the model-role system arrives, `TurnContextBudget` is built from the **active role's** model window. `ProviderDefinition.context_window_tokens` schema is forward-compatible with per-model overrides. |
| 3 | v1 / brassclaw_engine scope | v1 is dead. F6 and F7 are excluded. No changes to `brassclaw_engine`. |
| 4 | Config precedence / override | No override flag needed. Slices are soft — any consumer can exceed its starting hint as long as the aggregate stays under the window. To give everything more room, increase `context_window_tokens` in the provider config. That is the single lever. |
