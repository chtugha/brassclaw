## Current Kohai/Sempai prefix contract — 2026-10-26

Follow the [shared selection/pair-design contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
and [Prefix implementation owner](../plans/prefix_v3_upgrade.md). A Recipe,
Orchestrator or role selects a specific prefix; without one, base-prompt is the
regular default, including normal No-Match, not a Tier-2-only restriction.
Unavailable explicit selections/conflicts block rather than silently becoming
base. Shared knowledge/composites require an explicit content contract.

Design for formats/limits belongs to the provider/model pair; role content rules
and finished role prefixes are distinct. Same-pair Kohai/Sempai use the same
accepted design as the generator. Generate the fixed prefix specifically for
that pair. Kohai first formats only the dynamic Orchestrator prompt for the final
target pair. A connected/enabled Sempai corrects that already formatted prompt,
using its own generated reviewer prefix/target-pair design, and returns the final
dynamic prompt. Kohai then inserts the intended finished prefix unchanged exactly
once before it, without reformatting the return, and checks the complete request.
Every manual limit editor links to the existing provider revision/CAS endpoint
and shared query. Historical snapshots are not current mutable settings.
Provider facts may be retrieved through supported APIs/documentation or supplied
manually with provenance; no provider host/template/service edits are an option.

The old store, placeholder, server-template and prewarm walkthrough below describes
historical implementation/evidence where it differs. It is not current serving
qualification. Generation and prefill are separate; cached tokens are not promised
free, permanently resident or reusable without current backend-specific evidence.
Runtime cutover/remaining acceptance stays open in the implementation index.

The [explicit handoff contract](../reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
distinguishes the original, Kohai-formatted and Sempai-returned dynamic prompts.
With Sempai enabled, Kohai inserts the final-target prefix before the returned
final value. Sempai's own reviewer prefix remains only in its separate model call.
Without review, Kohai inserts the prefix before its initially formatted value.

The [generation-dialog contract](../reborn/model-token-limits.md#generation-prompts-missing-selection-and-model-use)
requires OK/Cancel on missing/incompatible selected prefixes and after every
successful provider/model Use for base generation/regeneration. Use existing durable
jobs/readiness and preserve selection on Cancel after Use; no silent fallback,
provider call before readiness or automatic unreviewed activation.

## Binding Recipe architecture (v3)

Read [recipe.md](../../recipe.md) before authoring or changing components. This contract
supersedes older examples below where they conflict; it specifies the target,
not completed runtime or database functionality.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills explain one Tool usage and have associated executable PythonCode;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including nested dependencies. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls `host.<tool>(...)`. Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

The legacy composer still contains source-substitution and historical step-body
paths. Ordinary PostgreSQL/skills-db execution uses immutable retained selections,
typed inputs, checked single-component steps and preloaded exports on the global
Monty service. Full-library migration, general pure approval, dynamic occurrences
and durable recovery remain open; see the current implementation status index.

> **Subsystem:** Sempai-Kohai — prompt preparation, optional correction and final
> prefix insertion. Kohai first formats the dynamic Orchestrator prompt for the
> final provider/model pair. Connected/enabled Sempai corrects that formatted
> prompt using its own generated prefix and target-pair design, and returns the
> final dynamic prompt to Kohai. Kohai then prepends the intended finished prefix
> unchanged exactly once, without another formatting pass. Generator, Kohai and
> same-pair Sempai share the stored accepted design revision. This is the binding
> target; the source map and legacy structures below do not establish runtime
> conformance. See the [explicit request handoffs](../reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests).
> **Grounded in:** `crates/brassclaw_interceptor/` (`lib.rs`, `mode.rs`, `packet.rs`,
> `proposal_sink.rs`, `config_store.rs`, `pg_store.rs`, `store.rs`),
> `saved_plan_to_v3.md` §0.13 / §0.14 + the v3 flow (lines 1366-1367).

## 1. Purpose

The current architecture requires three responsibilities through the existing
Kohai/Sempai owners and Recipe orchestration:

1. **Retain provenance** — preserve the original Orchestrator prompt, Kohai's
   formatted dynamic prompt and any review request/result with their selected
   provider/model, accepted design revision and generated prefix identities.
2. **Optimize** — after Kohai formats the dynamic prompt for the final target,
   connected/enabled Sempai corrects it and returns the final dynamic prompt.
   Sempai's own request uses its own target-specific prefix/design. Kohai then
   inserts the intended finished final-target prefix unchanged exactly once,
   validates the complete request and sends it. Without Sempai, it inserts that
   prefix before its initially formatted dynamic prompt.
3. **Finalize** — the Sempai may also **propose** new Recipes/ToolSkills/intent-examples. These
   proposals are **not** written to production tables directly — they enter the **Q1 validation
   queue** (`pending`/`q1_auto`) via `SempaiProposalSink`, the same gate every authored
   component passes. This is the self-optimization loop (kohai+sempai → new components →
   validation → retrieval).

> **Historical implementation observation, not the current serving contract.** The user's description frames self-optimization
> as running "automatically in idle times." The **live** interceptor mechanism is **inline**: the
> proposals are produced *as part of* the pre-send Sempai review of a turn (the
> `SempaiReviewOutcome.proposed_*` fields) and enqueued to Q1 on the same turn. There is no
> separate idle-time sweep in the current design; the "idle-time continuous optimization" is the
> v3 direction (a background Sempai-driven refresh — see `DOC_CONVERSION_MECHANISM_DESIGN.md` and
> §6 below).

## 2. Historical implementation locations

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

## 3. Historical implementation data model

These types describe the compatibility implementation. In particular, the old
adjusted-message/bridge-message result below is not the new serving contract:
Sempai must return the final dynamic prompt after Kohai's initial formatting.
Its conversion and the exact prefix join remain with the
[Prefix implementation owner](../plans/prefix_v3_upgrade.md); do not introduce a
second formatter after review to adapt this old result.

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

<a id="4-behavior--flow"></a>

## 4. Historical implementation behavior / flow

The following compatibility walkthrough is historical. Its adjusted-message and
bridge-message result is not the required final dynamic-prompt return, and its
description of Kohai as execution authority does not define current ownership.
Monty/Recipes own sequencing; Kohai performs the formatting and prefix insertion
specified above. Migrate these consumers under the Prefix plan's handoff contract.

### 4.0 The Kohai's duties (f7) — with and without the Sempai

The **Kohai** is the working LLM — the model that actually answers the user and drives the
Orchestrator (Monty). The **Sempai** is an optional reviewing LLM. The interceptor sits between
prompt assembly and the Kohai call, so the Kohai's duties split by whether a Sempai is connected
(`InterceptorMode::Routing` vs `Rerouting`):

**Kohai's duties — without a Sempai (Routing):**
- Be the **sole execution authority**: iterate the composed `steplist`, consult the `skills`
  array for exact tool usage, and run each step's `executable_code` via `host.run_program`.
- Assemble every LLM prompt it needs (there is no Sempai to do it).
- Receive the prompt **unchanged** from the interceptor and produce the turn's answer.
- Its work is **memorized** every turn (the `ForensicPacket` capture happens regardless of
  Sempai), so the self-optimization loop still accumulates telemetry.

**Kohai's duties — with a Sempai (Rerouting):**
- All of the above, **except** the outgoing prompt is first **reviewed and adjusted** by the
  Sempai (volatile tail replaced, bridge messages injected; the stable KV-cached base Part A is
  kept). The Kohai runs on the **adjusted** prompt.
- The Kohai does **not** propose components — that is the Sempai's role. The Kohai's duty remains
  execution + answering; the Sempai's duty is review + proposal.

**Sempai's duties (only when connected):**
- **Review** the outgoing Kohai prompt (audit prompt = Kohai prompt + segment metadata + token
  accounting + recipe/skill/tool context) and return `SempaiReviewOutcome` (adjusted volatile
  messages + bridge messages + composition summary).
- **Propose** new/updated Recipes / ToolSkills / Skills / PythonCode / intent-examples — but
  **never write production tables**: proposals are drafts enqueued to Q1 (`SempaiProposalSink`).
  This is the f8 new-component-creation loop (§4.5).
- Optionally emit `settings_adjustments` (operator-confirmed).

The interceptor is **always running**; the mode only changes *what it does* with each packet
(capture+forward vs capture+review+adjust+propose+forward).

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

<a id="44-the-base-prompt-placeholder-substitution-shipped-v063"></a>

### 4.4 Historical V063 placeholder substitution (superseded target)

A single-line `base-prompt` placeholder is added to the prompt while it is composed; the
Sempai-Kohai system replaces that placeholder with the real (precompiled, KV-cached) base-prompt
**at the very end** of prompt creation. The durable store is `reborn_basic_prompt_store`
(**V063**, shipped) backed by `PgBasicPromptStore` (`mark_stale` on graduation — see §5); the
interceptor config keys (`interceptor.sempai_base_prompt`, `_assembled_at`, `_persona`,
`_prewarm_last_at`) hold the assembled bundle + pre-warm timestamp. See `10-prefix-base-prompt.md`
(f6) for the full Prefix-System. §0.13 constrains the **patch** the orchestrator adds on top of
the cached base: it must NOT repeat content already in the stored base-prompt;
`basic_prompt_section_refs` carry navigation hints (pointers, not content); target patch < 4k
tokens; orchestrator patch PRIORITY 2; memory PRIORITY 3; rust context is delivered directly by
the Executioner on `host.*` calls (not in the prompt bundle at all).

### 4.5 New-component-creation process (f8)

The Sempai is the **only** path by which the running system invents new components. The loop,
per component class:

1. **Observe** — during a rerouting review the Sempai sees the turn's composition plan +
   outcome and decides a new/revised component would help.
2. **Propose (draft)** — `SempaiReviewOutcome.proposed_recipe_updates` /
   `proposed_intent_examples` carry raw JSON drafts. The Sempai **cannot write production
   tables**; `SempaiProposalSink::submit_proposals` enqueues each draft to Q1
   (`validation_status='pending'`, `queue_code='q1_auto'`), best-effort (never aborts the Kohai
   call).
3. **Q1 (Gate 1)** — `run_q1_validation` runs the per-class structural + injection checks +
   the C.4.5.1–5 common-syntax placeholder-grammar gates. The **exact per-class Q1 criteria are
   in `14-validation-queue.md` §4b** (f8): Skill (1–3), Tool (0), Extension (4–9),
   Orchestrator/Scaffold (10/50), Action (16), Recipe (21), PythonCode (22),
   ExtensionCatalogue (23), Notes (15), ToolSkill (13), Memory (12/14/17–20). A Q1 failure
   stays state-1 with errors; the Sempai's draft is rejected at the gate.
4. **Q2 (human review)** — a reviewer approves (graduation: `validation_status='validated'`,
   queue row deleted, `PgBasicPromptStore::mark_stale(scope)` so the prefix reassembles) or
   rejects (state 3, permanent `counter`; at threshold → state 4 deletion candidate).
5. **Retrieve** — once validated, the component is retrievable and future turns may match it
   (recipes via `host.resolve_intent` → `host.compose_orchestrator`; skills/python-code via
   composition `include` UUIDs).

The Sempai never bypasses validation — every invented component is a draft through the same Q1/Q2
gate an authored component passes. Builtins (`source='system'`) skip Q2 but not Q1.

## 5. Relations

- **Agent Loop** (`12`): `InterceptorStage` sits between `PromptStage` and `ModelStage`; the
  interceptor is the stage's implementation.
- **Prefix / Base-Prompt** (`10`): the interceptor owns the base-prompt store + substitution
  (`reborn_basic_prompt_store` V063); §0.13 patch rules govern the orchestrator patch on top of
  the cached base.
- **Recipe / Skills / Tools / PythonCode** (`03`/`05`/`06`/`07`): the Sempai proposes
  new/updated ones; `PgSempaiProposalSink` inserts into the recipe store; proposals enter Q1.
- **Validation Queue** (`14`): the security boundary — Sempai proposals are drafts into Q1
  (`pending`/`q1_auto`), never direct production writes; Q1+Q2 graduation makes them trusted.
  The per-class Q1 criteria (f8) are in `14` §4b.
- **Intent System** (`02`): validated `proposed_intent_examples` are seeded into
  `reborn_intent_inputs` (V054 `step_link` resolution).
- **Orchestrator** (`13`): the composition plan the interceptor memorizes is the
  `ComposedProgram` (skills + steplist + rust_directives + routing signals) produced by
  `host.compose_orchestrator`.

<a id="6-status--shipped-vs-pending"></a>

## 6. Historical shipped/pending snapshot

**Shipped:**
- `brassclaw_interceptor` crate exists and is complete for the **routing** path (capture +
  forward unchanged + persist packet) and the **rerouting** path (Sempai audit →
  `SempaiReviewOutcome` → adjusted prompt + proposals → Q1).
- `SempaiReviewOutcome`, `SempaiProposalSink`, `PgSempaiProposalSink` (composition),
  `InterceptorConfigStore` (keys in `brassclaw_config`), `PgInterceptorStore`,
  `SharedInterceptorMode` (WebUI-toggleable) all exist.
- `reborn_basic_prompt_store` (**V063**) + `PgBasicPromptStore` (incl. `mark_stale`) are
  shipped; the `base-prompt` placeholder substitution wiring is in place. The interceptor config
  keys hold the assembled bundle + pre-warm timestamp.
- The proposals are produced **inline** during the rerouting review (not a separate idle-time
  sweep).

**Pending (v3 direction):**
- **Idle-time self-optimization:** the live inline proposal mechanism is the foundation; a
  background Sempai-driven refresh loop (re-deriving/optimizing components and the converted
  documentation — see `DOC_CONVERSION_MECHANISM_DESIGN.md`) extends it to true idle-time
  operation rather than turn-coupled review.
- **C.5/C.6 driver activation:** the engine Monty VM host-call path that the interceptor's
  `PromptStage`/`ModelStage` feed is inert in production until the driver wires the composition
  host-calls (active production path is the TURNS `PgOrchestratorLookup` bridge).

<a id="7-llm-relevant-summary"></a>

## 7. Current LLM-facing summary

This summary describes the binding target, not completed serving integration:

- Recipe, Orchestrator or role specifies a prefix; otherwise select `base-prompt`
  for the target provider/model pair. This applies beyond Tier 2; No-Match normally
  selects base. An unavailable explicit prefix or unresolved conflict blocks the
  affected call rather than substituting base.
- Formats/limit references belong to one saved accepted provider/model design.
  Same-pair Kohai/Sempai share that design; their knowledge/content rules and finished
  role prefixes are separate. Share source knowledge explicitly, not by compulsory
  insertion of the whole base prefix into every role.
- Orchestrator → Kohai formats only the dynamic prompt for its target pair →
  optional connected Sempai corrects it and returns the final dynamic prompt →
  Kohai inserts the intended finished prefix unchanged once, without reformatting
  the return → actual complete-request guard →
  send. Reviewer output cannot change fixed prefix/design/limits. No second runtime
  prefix render or provider server/template/service modification.
- All manually editable prefix/prompt limits link to the canonical provider record,
  endpoint and shared settings query. Historical immutable pins remain evidence;
  they are not another mutable settings source. Hard limits survive disabled budgets.
- Missing/incompatible selected prefix offers OK/Cancel for supported generation.
  Successful provider/model Use also asks to generate/regenerate base. Cancel after
  Use preserves the committed selection and exposes readiness. Existing durable
  jobs, validation, activation and pre-dispatch waits apply; no duplicate call or
  original-message replay and no unreviewed automatic activation.
- Retain full source/effect/proposal evidence and current Q1/behavior/human-Q2 or
  verified shipped-bootstrap contracts. Cache prefill is separate from generation;
  cache residency, token price, quality and performance need their own evidence.
- Older placeholder, minimal/stale fallback, server-template and fixed patch-cap
  descriptions in the historical walkthrough are not current architecture.
  Remaining runtime integration is owned by the [Prefix plan](../plans/prefix_v3_upgrade.md)
  and [implementation index](../implementation-status.md); follow the
  [shared contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).

## Open K3-2 consumer successor — prefix selection and construction requests

For new construction revisions, follow the one
[visible construction-selection successor](../reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection).
The earlier [K3-2 contract](../reborn/model-token-limits.md#open-k3-2-successor--retained-prefix-selection-and-prefix-free-construction-workflows)
and its [Exact §5.2](../plans/Exact_V3_definition_update_at_K3-2.md#52-bestehende-retained-prefixbindung-um-drei-auswahlzustände-erweitern)
retain their old-profile/accepted scope. Their mandatory prefix-free construction
and v3.5-only Maker clauses no longer prescribe the new workflow's context.

The submitted visible choice is captured before effect; required construction
children/reviews keep original lineage and their own executing-pair choice.
Ordinary independent chat/Sempai contracts remain. General Recipe Prefixless is
not enabled by absence, a new JSON tag or an invented bootstrap purpose. Native
reader/guard/response/UI support and the existing workflow qualification remain
open; a new target statement is no runtime/activation acceptance. Component and
Prefix-Q1/Q2 scopes, exact insertion and effect-safe retry/retention stay unchanged.
