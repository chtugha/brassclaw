
> **Subsystem:** Sempai-Kohai — the prompt interceptor that **memorizes, optimizes, and
> finalizes** LLM prompts. The Kohai is the working LLM (always on); the Sempai is an optional
> reviewing LLM that, when connected, reviews/adjusts the outgoing prompt before it reaches the
> Kohai and proposes new skills/tools/recipes/python-code (queued for validation). The interceptor
> also performs the `base-prompt` placeholder substitution (Phase K.1) — see `10-prefix-base-prompt.md`.
> **Grounded in:** `crates/brassclaw_interceptor/` (`lib.rs`, `mode.rs`, `packet.rs`,
> `proposal_sink.rs`, `config_store.rs`, `pg_store.rs`, `store.rs`),
> `saved_plan_to_v3.md` §0.13 / §0.14 + the v3 flow (lines 1366-1367).

## 1. Purpose

The interceptor is the **single chokepoint between `PromptStage` and `ModelStage`** in the
agent-loop pipeline. It serves three goals from the user's task description:

1. **Memorize** — every turn's composition plan (`BuildInstruction` orchestrator_steps + routing
   signals — *not* the base-prompt content, §0.14) is captured as a `ForensicPacket` and
   persisted to the `InterceptorStore` (`PgInterceptorStore`). This happens **always**, with or
   without a Sempai — the Kohai half is always on.
2. **Optimize** — when a Sempai provider is connected, the interceptor constructs a rich audit
   prompt and asks the Sempai to **review and adjust the outgoing prompt before it ships to the
   Kohai** (pre-send review). The Sempai returns adjusted volatile messages + bridge messages +
   a composition summary.
3. **Finalize** — the Sempai may also **propose** new Recipes/ToolSkills/intent-examples. These
   proposals are **not** written to production tables directly — they enter the **Q1 validation
   queue** (`pending`/`q1_auto`) via `SempaiProposalSink`, the same gate every authored
   component passes. This is the self-optimization loop (kohai+sempai → new components →
   validation → retrieval).

> **"Idle-time" nuance vs. the live mechanism.** The user's description frames self-optimization
> as running "automatically in idle times." The **live** interceptor mechanism is **inline**: the
> proposals are produced *as part of* the pre-send Sempai review of a turn (the
> `SempaiReviewOutcome.proposed_*` fields) and enqueued to Q1 on the same turn. There is no
> separate idle-time sweep in the current design; the "idle-time continuous optimization" is the
> v3 direction (a background Sempai-driven refresh — see `DOC_CONVERSION_MECHANISM_DESIGN.md` and
> §6 below).

## 2. Location

- **Crate:** `crates/brassclaw_interceptor/` — `#![forbid(unsafe_code)]`.
  - `lib.rs` — crate doc (routing vs. rerouting state machine), re-exports.
  - `mode.rs` — `InterceptorMode { Routing, Rerouting }` + `SharedInterceptorMode` (atomic flag,
    flipped by the settings service when the operator activates/deactivates Sempai via the WebUI).
  - `packet.rs` — `ForensicPacket`, `PacketId`, `PacketStatus`, `CapturedPrompt`, `PromptSegment`,
    `TokenAccountingSnapshot`, `SempaiReviewOutcome`, `KohaiUsage`.
  - `proposal_sink.rs` — `SempaiProposalSink` trait + `NoopProposalSink` + `ProposalSubmitResult`.
  - `config_store.rs` — `InterceptorConfig` + `InterceptorConfigStore` (keys in `brassclaw_config`).
  - `store.rs` — `InterceptorStore` trait + `NoopInterceptorStore`.
  - `pg_store.rs` — `PgInterceptorStore` (Postgres-backed packet store).
- **Composition wiring:** `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
  (`InterceptorConfigService`) — `class_label` for the telemetry display; flips the
  `SharedInterceptorMode`; provides `PgSempaiProposalSink` (backed by `PgRecipeStore` insert).
- **Pipeline stage:** `brassclaw_agent_loop` `InterceptorStage` (between `PromptStage` and
  `ModelStage`).
- **Plan:** §0.13 (KV-cache / base-prompt patch rules), §0.14 (interceptor system), the v3
  flow (line 1366 `InterceptorStage — Sempai review of outgoing prompt`; 1367
  `ModelStage — LLM call (Kohai)`).

## 3. Data model

### `InterceptorMode` (`mode.rs`)

```rust
pub enum InterceptorMode {
    Routing,    // Sempai not connected: capture + forward unchanged
    Rerouting,  // Sempai connected: capture + review + adjust before forwarding
}
```

`SharedInterceptorMode` is an `Arc<AtomicBool>` (SeqCst). `new()` starts in **Routing** (no
Sempai). `set_rerouting()`/`set_routing()` flip it; the settings service does this when the
operator activates/deactivates the Sempai provider via the WebUI. The interceptor is **always
running** — the mode only changes *what it does* with each packet.

### `SempaiReviewOutcome` (`packet.rs`)

Returned by the Sempai during a rerouting review:

```rust
pub struct SempaiReviewOutcome {
    pub adjusted_volatile_messages: Vec<(String, String)>, // replace the volatile tail; Part A (stable base) kept unchanged
    pub bridge_messages:             Vec<(String, String)>, // injected between Part A and the adjusted tail
    pub composition_summary:         String,                // what it observed/adjusted/why (persisted with the packet)
    pub proposed_recipe_updates:     Vec<serde_json::Value>,// → Q1 validation queue (NOT production tables)
    pub proposed_intent_examples:    Vec<serde_json::Value>,// → Q1; once validated, seeded into reborn_intent_inputs (Q30)
    pub settings_adjustments:        Vec<serde_json::Value>,// → settings service (operator-confirmed application)
}
```

**The Sempai cannot write to production tables directly.** Every proposal is a *draft* that
enters Q1. This is the security boundary between "the model invents a component" and "the
component becomes trusted" (§0.18).

### `ForensicPacket` / `CapturedPrompt` / `PromptSegment`

- `ForensicPacket` — the telemetry record for one turn; created after `PromptStage`, completed
  after `ModelStage`. `PacketStatus`: `AwaitingKohai` → `Complete` (routing) or `SempaiReviewed`
  (rerouting).
- `CapturedPrompt` — the assembled prompt + structural breakdown.
- `PromptSegment` — one logical segment with its inclusion decision and provenance (e.g.
  `"skill:ibm_bob_people"`, `"recipe_hint:deploy-workflow"`; decision path
  `"recipe matched: wilson=0.82 tier=mature"`).

### Interceptor config keys (`brassclaw_config`, no new migration — `config_store.rs`)

| Key | Value |
|-----|-------|
| `interceptor.sempai_base_prompt` | assembled base prompt (Part A); `None` if never assembled |
| `interceptor.sempai_base_prompt_assembled_at` | ISO-8601 timestamp |
| `interceptor.sempai_persona` | Sempai persona text (Part B); falls back to the compiled-in default |
| `interceptor.sempai_prewarm_last_at` | ISO-8601 last successful pre-warm |

`InterceptorConfigStore`: `load`, `save_persona`, `save_base_prompt`, `save_prewarm_last_at`.

## 4. Behavior / flow

### 4.1 Routing state (no Sempai) — Kohai always-on

1. After `PromptStage`, capture the final prompt as a `ForensicPacket` (all segments + token
   accounting + capability surface) and save it to the `InterceptorStore`.
2. Forward the prompt to the **Kohai** provider **unchanged**.
3. Receive the Kohai response, attach it + actual token usage (`KohaiUsage`), set
   `status = Complete`, save again.

The composition plan is **memorized** every turn regardless of Sempai.

### 4.2 Rerouting state (Sempai connected) — pre-send review + optimize + finalize

Steps 1–3 as above, but between capture and forward:

4. Construct a rich **Sempai audit prompt**: the Kohai prompt + all segment metadata + token
   accounting + recipe/skill/tool context + orchestrator design information.
5. Send the audit prompt to the **Sempai** provider.
6. Receive `SempaiReviewOutcome`:
   - **Optimize the prompt:** `adjusted_volatile_messages` **replace the volatile tail**; the
     **stable base (Part A) is kept unchanged** (it is KV-cache-resident — see §0.13).
     `bridge_messages` are injected between Part A and the adjusted tail.
   - **Finalize → propose components:** `proposed_recipe_updates` +
     `proposed_intent_examples` are submitted to Q1 via `SempaiProposalSink::submit_proposals`
     (see 4.3). `settings_adjustments` go to the settings service for operator-confirmed
     application.
7. Forward the **adjusted** Kohai prompt (not the original) to the Kohai; attach the
   `SempaiReviewOutcome`; set `status = SempaiReviewed`; save.

### 4.3 The proposal sink → Q1 validation (`proposal_sink.rs`)

```rust
#[async_trait]
pub trait SempaiProposalSink: Send + Sync {
    async fn submit_proposals(
        &self,
        user_id: &str, project_id: &str,
        proposed_recipe_updates: &[serde_json::Value],
        proposed_intent_examples: &[serde_json::Value],
    ) -> Result<ProposalSubmitResult, InterceptorError>;
}
```

- Implementations route the raw JSON payloads into the **Q1 validation tables**
  (`validation_status='pending'`, `queue_code='q1_auto'`). Composition implements
  `PgSempaiProposalSink` (backed by `PgRecipeStore` insert); `NoopProposalSink` for builds with
  no store wired.
- **Best-effort:** failures are logged and counted but do **not** abort the interceptor pipeline
  — the Kohai call still proceeds with the adjusted messages even if proposal submission fails.
- **Shape requirements:** each recipe proposal needs `"name"` + `"steps"` (recipe) or
  `"tool_name"` (skill); missing/malformed entries are skipped+counted, remaining valid entries
  still submit. Intent examples need `"input"` + optional `"class"` (intent class 1–4, default 1);
  once validated they are seeded into `reborn_intent_inputs` (Q30 resolution).
- Returns `ProposalSubmitResult { recipe_updates_queued, intent_examples_queued }`.

This is the self-optimization loop: Sempai reviews a turn → proposes new
Recipes/ToolSkills/intent-examples → they enter Q1 → Q1+Q2 graduate them → they become
retrievable → future turns may match them. The Sempai never bypasses validation.

### 4.4 The `base-prompt` placeholder substitution (Phase K.1)

Per the user's task, a single line `base-prompt` is added to the prompt while it is composed;
the Sempai-Kohai system replaces that placeholder with the real (precompiled, KV-cached)
base-prompt **at the very end** of prompt creation. The interceptor config already stores the
assembled base prompt (`interceptor.sempai_base_prompt`) and the pre-warm timestamp. The full
`reborn_basic_prompt_store` (V055) + the substitution wiring is Phase K.1 — see
`10-prefix-base-prompt.md`. §0.13 constrains the **patch** the BuildInstruction adds on top of
the cached base: it must NOT repeat content already in the stored base-prompt;
`basic_prompt_section_refs` carry navigation hints (pointers, not content); target patch < 4k
tokens; orchestrator patch PRIORITY 2; memory PRIORITY 3; rust context delivered directly by
`RecipeStage` (not in the bundle at all).

## 5. Relations

- **Agent Loop** (`12`): `InterceptorStage` sits between `PromptStage` and `ModelStage`; the
  interceptor is the stage's implementation.
- **Prefix / Base-Prompt** (`10`): the interceptor owns the base-prompt store + substitution;
  §0.13 patch rules govern the BuildInstruction-on-top-of-cache.
- **Recipe / Skills / Tools / PythonCode** (`03`/`05`/`06`/`07`): the Sempai proposes
  new/updated ones; `PgSempaiProposalSink` inserts into the recipe store; proposals enter Q1.
- **Validation Queue** (`14`): the security boundary — Sempai proposals are drafts into Q1
  (`pending`/`q1_auto`), never direct production writes; Q1+Q2 graduation makes them trusted.
- **Intent System** (`02`): validated `proposed_intent_examples` are seeded into
  `reborn_intent_inputs` (Q30 resolution).
- **Orchestrator** (`13`): the composition plan the interceptor memorizes is the
  `BuildInstruction` orchestrator_steps + routing signals produced by the Python step-0 / IBS.

## 6. Status — today vs. v3

**Today:**
- `brassclaw_interceptor` crate exists and is complete for the **routing** path (capture +
  forward unchanged + persist packet) and the **rerouting** path (Sempai audit →
  `SempaiReviewOutcome` → adjusted prompt + proposals → Q1).
- `SempaiReviewOutcome`, `SempaiProposalSink`, `PgSempaiProposalSink` (composition),
  `InterceptorConfigStore` (keys in `brassclaw_config`), `PgInterceptorStore`,
  `SharedInterceptorMode` (WebUI-toggleable) all exist.
- The base-prompt **config keys** (`interceptor.sempai_base_prompt`, `_assembled_at`,
  `_persona`, `_prewarm_last_at`) exist, but the **`reborn_basic_prompt_store` table (V055) and
  the `base-prompt` placeholder substitution wiring are not implemented** (Phase K.1). The
  WebUI has a "Pre-warm Sempai KV-cache" button but the underlying store is a config-string,
  not the durable compiled-prefix store.
- The proposals are produced **inline** during the rerouting review (not a separate idle-time
  sweep).

**v3 plan adds:**
- **Phase K.1 (V055):** `reborn_basic_prompt_store` — the durable precompiled base-prompt store;
  the `base-prompt` placeholder substitution at the end of prompt creation (the
  sempai-kohai system replaces the single-line placeholder with the real KV-cached content);
  the WebUI **Prefix Tab** (list + generate/regenerate each prefix → compile to the LLM). The
  base-prompt shifts from the config-string to the dedicated store/table.
- **Idle-time self-optimization (v3 direction):** the live inline proposal mechanism is the
  foundation; a background Sempai-driven refresh loop (re-deriving/optimizing components and
  the converted documentation — see `DOC_CONVERSION_MECHANISM_DESIGN.md`) extends it to true
  idle-time operation rather than turn-coupled review.

## 7. LLM-relevant summary

The Sempai-Kohai interceptor (`brassclaw_interceptor`) sits between `PromptStage` and
`ModelStage` and **memorizes** every turn's composition plan as a `ForensicPacket` (always — the
Kohai half is always on). **Routing** (no Sempai): capture + forward the prompt unchanged to the
Kohai. **Rerouting** (Sempai connected, `SharedInterceptorMode` flipped via WebUI): build a rich
audit prompt → Sempai returns `SempaiReviewOutcome` (adjusted volatile messages replacing the
tail while the stable KV-cached base Part A is kept + bridge messages + composition summary) →
forward the **adjusted** prompt to the Kohai. The Sempai also **proposes** new
Recipes/ToolSkills/intent-examples (`proposed_recipe_updates`/`proposed_intent_examples`), but it
**cannot write production tables** — `SempaiProposalSink::submit_proposals` enqueues them to Q1
(`pending`/`q1_auto`, best-effort, never aborts the Kohai call); Q1+Q2 graduate them; validated
intent examples seed `reborn_intent_inputs` (Q30). Config keys live in `brassclaw_config`
(`sempai_base_prompt`, `_persona`, `_prewarm_last_at`). Today the routing + rerouting + proposal
paths exist; the **`base-prompt` placeholder substitution + `reborn_basic_prompt_store` (V055)
are Phase K.1** (the `base-prompt` line is replaced with the real KV-cached content at the end
of prompt creation; the WebUI Prefix Tab compiles/regenerates each prefix). The live
self-optimization is **inline** (proposals produced during the turn's review); true idle-time
operation is the v3 direction.
