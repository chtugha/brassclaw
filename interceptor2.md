# interceptor2.md — Interceptor Activation Plan

> **Scope:** Wire the interceptor so it actually works. The infrastructure
> (types, store, DB migration slot, pipeline stage, `ProviderRole::Sempai`,
> `set_active`, `sempai_swappable` hot-swap slot) already exists. Five
> specific wiring gaps prevent it from functioning.

---

## The Exact Data Flow (What Actually Happens)

```
PromptStage
  └── build_prompt_bundle()
        └── InstructionBundleBuilder.build()
              Priority 1  identity refs            ← STABLE (cached by Sempai)
              Priority 2  instruction snippets      ← STABLE (skills, system)
              Priority 3  memory snippets           ← STABLE (semi-stable context)
              Priority 4  safety context            ← STABLE (policy)
              Priority 5  capability surface        ← STABLE (tools/extensions)
              ── KV-cache boundary ──────────────────────────────────────
              Priority 6  thread history            ← VOLATILE (per-turn tail)
              Priority 7  inline nudges             ← VOLATILE (per-turn tail)
              └── Vec<LoopModelMessage> (opaque content_refs)
                      │
                      ▼
InterceptorStage           ← sits here, between PromptStage and ModelStage
  └── host.on_prompt_assembled(snapshot)
        saves ForensicPacket, returns Option<packet_id>
                │
                ▼
ModelStage
  └── host.stream_model(messages)
        └── ThreadBackedLoopModelPort::stream_model()
              └── resolve_model_messages()
                    Dereferences each LoopMessageRef → raw text
                    (reads InstructionMaterializationStore + thread service)
                    Produces Vec<HostManagedModelMessage> (role + content string)
                          │
                          ▼
                    LlmProviderModelGateway::stream_model()
                          → CompletionRequest → HTTP to Kohai
```

**KV-cache design constraint:** The stable base (priorities 1–5) must be placed
first in the prompt so it forms a prefix that Sempai can cache. New skills, tools,
or extensions are prepended to their section (priority 2 or 5), not appended,
so the prefix grows upward and older stable content stays at the bottom.
The volatile tail (thread history + inline nudges, priorities 6–7) follows.
This ordering must not be inverted.

**The Sempai is an OpenAI-compatible HTTP provider** — structurally identical to
Kohai, configured through the same `brassclaw_llm::LlmProvider` abstraction,
wired through the same `SwappableLlmProvider` / `LlmProviderModelGateway` pattern.
"Sempai connected" means `ProviderRole::Sempai` has been configured; the
`SharedInterceptorMode` flag reflects this state at runtime.

---

## What Already Exists and Where

| What | Where | State |
|---|---|---|
| `ForensicPacket`, `CapturedPrompt`, `SempaiReviewOutcome` | `crates/brassclaw_interceptor/src/packet.rs` | ✅ |
| `SharedInterceptorMode` (`Routing`/`Rerouting` AtomicBool) | `crates/brassclaw_interceptor/src/mode.rs` | ✅ Never wired |
| `PgInterceptorStore` code | `crates/brassclaw_interceptor/src/pg_store.rs` | ✅ code only |
| `brassclaw_forensic_packets` DB table | `migrations/` | ❌ Missing — V26 is `root_filesystem_entries`; no V34 yet (V33 is the current highest) |
| `LoopInterceptorPort` trait (`on_prompt_assembled`, `on_kohai_response`) | `crates/brassclaw_turns/src/run_profile/host.rs:2103` | ✅ |
| `InterceptorStage` between `PromptStage` and `ModelStage` | `crates/brassclaw_agent_loop/src/executor/interceptor.rs` | ✅ |
| `RebornLoopDriverHost` saves every packet, always Routing | `crates/brassclaw_reborn/src/loop_driver_host.rs:1800` | ✅ Routing only |
| `interceptor_store` wired from `DefaultPlannedRuntimeParts` into factory | `crates/brassclaw_reborn/src/runtime.rs:557` | ✅ |
| `interceptor_store: None` in composition | `crates/brassclaw_reborn_composition/src/runtime.rs:1974` | ❌ Always `None` |
| `ProviderRole::Sempai`, `set_active(Sempai)` writes `llm.sempai.*` to DB | `crates/brassclaw_reborn_composition/src/llm_config_service.rs:898` | ✅ DB write only |
| `sempai_swappable: Option<Arc<SwappableLlmProvider>>` in `RebornLlmReloadParts` | `crates/brassclaw_reborn_composition/src/runtime.rs:2494` | ✅ Scaffolded, always `None`, dead |
| `LlmProviderModelGateway` — calls `brassclaw_llm::LlmProvider::complete()` | `crates/brassclaw_reborn/src/model_gateway.rs:350` | ✅ Kohai only |
| `ThreadBackedLoopModelPort::resolve_model_messages` — materializes refs → text | `crates/brassclaw_loop_support/src/lib.rs:1088` | ✅ Kohai path only |
| `InstructionBundleBuilder.build()` — 7-priority assembly order | `crates/brassclaw_turns/src/run_profile/instruction_bundle.rs:202` | ✅ |
| `InstructionStoreBackedHookSink` — materialization pattern | `crates/brassclaw_reborn/src/loop_driver_host.rs:125` | ✅ |
| `InterceptorConfig` / `SempaiConfig` settings structures | anywhere | ❌ Does not exist yet |

---

## The Five Wiring Gaps

**Gap 0 — `brassclaw_forensic_packets` table does not exist.**
`pg_store.rs` references `brassclaw_forensic_packets` throughout, and its module
doc says "V026" — but `migrations/V26__root_filesystem_entries.sql` creates an
unrelated table. The highest existing migration is V33 (`provider_role`). No migration
creates `brassclaw_forensic_packets`. Without the table every `save()` call fails
(silently — it is `debug!`-logged at `loop_driver_host.rs:1851` and treated
non-fatal), and no packets are persisted.

**Gap 1 — `PgInterceptorStore` never instantiated.**
`DefaultPlannedRuntimeParts.interceptor_store` exists and is wired into the
factory at line 557 of `brassclaw_reborn/src/runtime.rs`, but the composition
passes `None` at line 1974 of `brassclaw_reborn_composition/src/runtime.rs`.

**Gap 2 — `sempai_swappable` never allocated.**
`RebornLlmReloadParts.sempai_swappable` is `None` at all times. The comment
says "Phase 8 wires this". `set_active(Sempai)` writes config to DB but never
builds a live provider or gateway.

**Gap 3 — `SharedInterceptorMode` never created or read.**
The flag exists but no code creates a shared instance or reads it anywhere.

**Gap 4 — `on_prompt_assembled` cannot return adjusted messages.**
Return type is `Option<String>`. To reroute to Sempai, the interceptor must be able
to return replacement messages (adjusted resolved text from Sempai) so `ModelStage`
forwards those to Kohai instead of the originals. The current `Option<String>`
return makes this architecturally impossible.

---

## Step 0 — Create the `brassclaw_forensic_packets` migration

**Prerequisite for all other steps. Without this the store compiles but every
`save()` silently fails.**

File: `migrations/V34__forensic_packets.sql` *(new)*

> **Migration number:** The current highest migration is **V33** (`provider_role`).
> This interceptor plan takes **V34** for `forensic_packets`. The phased redesign
> plan (`plan.md`) also proposed V34 for `reborn_skills` — that plan's migration
> sequence must shift by one: V34 → V35 for `reborn_skills`, V35 → V36 for
> `reborn_intent_inputs`, V36 → V37 for `reborn_actions`, and so on for all
> subsequent migrations in that plan. The interceptor ships first as a standalone
> change; `forensic_packets` must exist before any agent turn runs regardless of
> when the redesign phases land.

The exact schema is derived from the INSERT, SELECT, UPDATE, and `link_chat_record`
statements in [`pg_store.rs`](crates/brassclaw_interceptor/src/pg_store.rs).

```sql
-- V34: Sempai–Kohai forensic packet store.
--
-- Captures one ForensicPacket per agent-loop turn: the assembled prompt
-- (segments + token accounting), optional Sempai review outcome, and the
-- Kohai response with actual token usage.

CREATE TABLE IF NOT EXISTS brassclaw_forensic_packets (
    id                                TEXT         NOT NULL,
    tenant_id                         TEXT         NOT NULL,
    run_id                            TEXT         NOT NULL,
    iteration                         INTEGER      NOT NULL,
    status                            TEXT         NOT NULL
                                                     CHECK (status IN
                                                       ('awaiting_kohai',
                                                        'complete',
                                                        'sempai_reviewed')),
    captured_at                       TIMESTAMPTZ  NOT NULL,
    completed_at                      TIMESTAMPTZ,

    -- Assembled prompt: CapturedPrompt serialised as JSONB.
    prompt                            JSONB        NOT NULL,

    -- Kohai response text and token usage (NULL until ModelStage completes).
    kohai_response                    TEXT,
    kohai_input_tokens                INTEGER,
    kohai_output_tokens               INTEGER,
    kohai_cache_read_input_tokens     INTEGER,
    kohai_cache_creation_input_tokens INTEGER,

    -- Sempai review outcome (NULL in routing state).
    sempai_review                     JSONB,

    -- Retroactive join to chat-memory records (written post-turn by
    -- PgChatMemoryRecordStore via link_chat_record()).
    chat_record_id                    TEXT,

    updated_at                        TIMESTAMPTZ  NOT NULL DEFAULT now(),

    PRIMARY KEY (id),
    UNIQUE (tenant_id, run_id, iteration)
);

CREATE INDEX IF NOT EXISTS idx_forensic_packets_tenant_captured
    ON brassclaw_forensic_packets (tenant_id, captured_at DESC);

CREATE INDEX IF NOT EXISTS idx_forensic_packets_run
    ON brassclaw_forensic_packets (tenant_id, run_id, iteration);
```

Also fix the stale reference in the `pg_store.rs` module doc:

```
Before:  //! Persists `ForensicPacket`s to `brassclaw_forensic_packets` (V026).
After:   //! Persists `ForensicPacket`s to `brassclaw_forensic_packets` (V034).
```

**Gate:** `cargo test -p brassclaw_interceptor`. Migration validated at
integration-test time by embedded Postgres applying all migrations in order.

---

## Step 1 — Fix the trait to carry adjusted messages

**The only breaking change. The rest is additive.**

### 1a. Add `InterceptorResult` in `brassclaw_turns`

File: `crates/brassclaw_turns/src/run_profile/host.rs`

```rust
/// Outcome of `on_prompt_assembled`. `adjusted_messages`, when `Some`,
/// replaces the prompt list that `ModelStage` forwards to the Kohai provider.
/// The messages are resolved (role, text) pairs — the host resolves refs from
/// the InstructionMaterializationStore before returning so ModelStage receives
/// plain text that bypasses ref-resolution entirely.
pub struct InterceptorResult {
    pub packet_id: String,
    pub adjusted_messages: Option<Vec<(String, String)>>,
}
```

Change `on_prompt_assembled` return from `Option<String>` to
`Option<InterceptorResult>`. `NoInterceptor` default impl returns `None`.

`adjusted_messages` is `Vec<(role, content_text)>` — **resolved text**, not refs.
The host has access to the `InstructionMaterializationStore` at the time
`on_prompt_assembled` is called (it is held on `RebornLoopDriverHost`), so it
can resolve the opaque content refs in the snapshot before calling Sempai.
The adjusted text comes back from Sempai as plain strings and does not need to
travel through ref-resolution again.

> **Type note — `(String, String)` vs `HostManagedModelMessage`:** The tuple
> form is used here to avoid introducing a new crate dependency from
> `brassclaw_turns` into `brassclaw_loop_support` solely for this struct. The
> conversion from `Vec<(String, String)>` to `Vec<HostManagedModelMessage>`
> happens in Step 1c (inside `canonical.rs`, which already depends on
> `brassclaw_loop_support`). If at implementation time `brassclaw_turns`
> already depends on `brassclaw_loop_support` (check `Cargo.toml`), prefer
> using `HostManagedModelMessage` directly in `InterceptorResult` and
> `InterceptorPromptOutput` to eliminate the conversion step entirely.

### 1b. Update `InterceptorStage`

File: `crates/brassclaw_agent_loop/src/executor/interceptor.rs`

`InterceptorPromptOutput.messages` is currently `Vec<LoopModelMessage>` (refs).
Add a second field:

```rust
pub(super) struct InterceptorPromptOutput {
    pub(super) state: LoopExecutionState,
    pub(super) messages: Vec<brassclaw_turns::run_profile::LoopModelMessage>,
    /// Sempai-adjusted messages as resolved (role, text) pairs.
    /// `Some` only in rerouting state; `None` means forward `messages` unchanged.
    pub(super) adjusted_messages: Option<Vec<(String, String)>>,
    pub(super) packet_id: InterceptorPacketId,
}
```

When `on_prompt_assembled` returns `Some(result)`:
- extract `packet_id` from `result.packet_id`
- set `adjusted_messages = result.adjusted_messages`

### 1c. Thread adjusted messages through `canonical.rs` into `ModelStage`

File: `crates/brassclaw_agent_loop/src/executor/canonical.rs`, around line 131

When `interceptor_out.adjusted_messages` is `Some(pairs)`, convert
`Vec<(String, String)>` → `Vec<HostManagedModelMessage>` and pass them
to `ModelStage` directly, bypassing `resolve_model_messages`. When `None`,
forward `interceptor_out.messages` (refs) unchanged — the existing Kohai path.

This requires `ModelInput` to accept either refs or pre-resolved messages.
The minimal change: add `resolved_messages: Option<Vec<HostManagedModelMessage>>`
to `ModelInput`; in `ThreadBackedLoopModelPort::stream_model`, if
`resolved_messages` is `Some`, skip `resolve_model_messages` and use them
directly.

### 1d. Update all `LoopInterceptorPort` stubs (mechanical)

Six files return the old type (no behaviour change — just the return type):
- `crates/brassclaw_agent_loop/src/executor/tests/support.rs`
- `crates/brassclaw_agent_loop/src/test_support/mod.rs`
- `crates/brassclaw_turns/tests/agent_loop_host_contract.rs`
- `crates/brassclaw_reborn/src/planned_driver.rs`
- `crates/brassclaw_reborn/tests/planned_driver_e2e.rs`
- `crates/brassclaw_reborn/src/turn_runner/tests/mod.rs`

**Gate:** `cargo clippy --all -- -D warnings` clean; existing tests pass.

---

## Step 2 — Wire `PgInterceptorStore`, allocate `sempai_swappable`, create `SharedInterceptorMode`

These three changes are all in the composition and are independent of each other.

### 2a. Wire `PgInterceptorStore`

File: `crates/brassclaw_reborn_composition/src/runtime.rs`, line 1974

```rust
// Before:
interceptor_store: None,

// After (#[cfg(feature = "postgres")]):
interceptor_store: services.pg_pool.as_ref().map(|pool| {
    Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
        Arc::clone(pool),
        validated_identity.tenant_id.as_str(),
    )) as Arc<dyn brassclaw_interceptor::InterceptorStore>
}),
```

### 2b. Allocate `sempai_swappable` in `wrap_swappable_gateway`

File: `crates/brassclaw_reborn_composition/src/runtime.rs`, inside
`wrap_swappable_gateway` (line 2538)

Add alongside the existing Kohai swappable:

```rust
let sempai_inner: Arc<dyn LlmProvider> = Arc::new(PlaceholderLlmProvider);
let sempai_swappable = Arc::new(SwappableLlmProvider::new(sempai_inner));
```

Replace `sempai_swappable: None` in `RebornLlmReloadParts` with
`sempai_swappable: Some(Arc::clone(&sempai_swappable))`. Remove `#[allow(dead_code)]`.

### 2c. Create `SharedInterceptorMode` and thread it through

In the same inner composition function (after the model gateway is built):

```rust
#[cfg(feature = "root-llm-provider")]
let interceptor_mode = brassclaw_interceptor::SharedInterceptorMode::new();
```

Add `interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>` to
`DefaultPlannedRuntimeParts`. Wire through `build_default_planned_runtime` in
`crates/brassclaw_reborn/src/runtime.rs` (after line 557):

```rust
if let Some(mode) = parts.interceptor_mode {
    host_factory = host_factory.with_interceptor_mode(mode);
}
```

Add `interceptor_mode: SharedInterceptorMode` field and `with_interceptor_mode`
builder to `RebornLoopDriverHostFactory`. Thread it into `RebornLoopDriverHost`.

Carry `interceptor_mode: Option<SharedInterceptorMode>` on `RebornRuntime`
(cfg-gated), exposed via an accessor so Step 3 can reach it.

> **Feature gate requirement:** The store (Step 2a) is gated
> `#[cfg(feature = "postgres")]` while the mode flag (Step 2c) and live-swap
> (Step 3) are gated `#[cfg(feature = "root-llm-provider")]`. The **rerouting
> path requires both features simultaneously** — without `postgres` no packets
> are saved; without `root-llm-provider` the mode flag never flips to
> `Rerouting`. Gate the rerouting branch entry in `on_prompt_assembled` (Step
> 4b) with `#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]`
> so it only compiles when both are present. The `postgres`-only path (store
> wired, mode always `Routing`) is a valid forensic-logging-only mode and
> compiles cleanly on its own. The `root-llm-provider`-only path without
> `postgres` must not compile the rerouting branch.

**Gate:** `cargo clippy -p brassclaw_reborn -p brassclaw_reborn_composition -- -D warnings` clean.

---

## Step 3 — Wire `set_active(Sempai)` to the live swap + mode flag

The existing `set_active(Sempai)` path (line 898) writes to DB only. Extend it
to immediately swap the running provider and flip the interceptor mode.

### 3a. Add `sempai_swappable` and `interceptor_mode` to `RebornLlmConfigService`

File: `crates/brassclaw_reborn_composition/src/llm_config_service.rs`
(`set_active` at line 874, `ProviderRole::Sempai` arm at line 898)

```rust
#[cfg(feature = "root-llm-provider")]
sempai_swappable: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
#[cfg(feature = "root-llm-provider")]
interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>,
```

Add `with_sempai_swappable` and `with_interceptor_mode` builder methods.
Wire from the WebUI facade composition path using the `sempai_swappable()` and
`interceptor_mode()` accessors on `RebornRuntime`.

### 3b. Extend the `ProviderRole::Sempai` match arm

```rust
ProviderRole::Sempai => {
    // Existing: DB write.
    #[cfg(feature = "postgres")]
    self.save_role_to_db("llm.sempai.provider_id", ..., "llm.sempai.model", ...).await;

    // New: live swap + mode flip.
    // Both features are required: `postgres` saves packets; `root-llm-provider`
    // drives the live swap + mode flag. See Step 2c gate note.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    if let Some(swappable) = &self.sempai_swappable {
        let new_provider: Arc<dyn brassclaw_llm::LlmProvider> = if id.is_empty() {
            Arc::new(PlaceholderLlmProvider)   // clearing the role
        } else {
            match self.build_sempai_provider(&id, request.model.as_deref()).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!(%e, "sempai provider build failed; mode stays Routing");
                    return self.build_snapshot().await;
                }
            }
        };
        swappable.swap(new_provider);
        if let Some(mode) = &self.interceptor_mode {
            if id.is_empty() { mode.set_routing(); } else { mode.set_rerouting(); }
        }
    }
}
```

### 3c. `build_sempai_provider` private method

Reuses `brassclaw_llm::build_static_provider_chain` exactly as
`RebornLlmReloadAdapter::reload()` does for Kohai. Reads the stored API key
from `self.keys.read(&provider_id)`. Returns `Result<Arc<dyn LlmProvider>, String>`.

No streaming session manager — the Sempai audit call is a plain
`CompletionRequest` (single completion, not a streaming session).

**Gate:** `cargo clippy -p brassclaw_reborn_composition -- -D warnings` clean;
`cargo test -p brassclaw_reborn_composition` passes.

---

## Step 4 — Build Sempai gateway, implement the rerouting branch, and the KV-cache base-context pre-warm

### 4a. Build a `LlmProviderModelGateway` for the Sempai

In composition, wrap `sempai_swappable` in its own `LlmProviderModelGateway`:

```rust
#[cfg(feature = "root-llm-provider")]
let sempai_gateway: Option<Arc<dyn HostManagedModelGateway>> = {
    let swappable = Arc::clone(&sempai_swappable);
    let policy = LlmModelProfilePolicy::new()
        .allow_model_profile(ModelProfileId::new("sempai_model")?, None);
    Some(Arc::new(LlmProviderModelGateway::new(swappable, policy))
        as Arc<dyn HostManagedModelGateway>)
};
```

Pass through `DefaultPlannedRuntimeParts.sempai_gateway`
(`Option<Arc<dyn HostManagedModelGateway>>`) → `build_default_planned_runtime`
→ `host_factory.with_sempai_gateway(...)`.

Add `sempai_gateway` field and `with_sempai_gateway` builder to
`RebornLoopDriverHostFactory`. Thread it into the built `RebornLoopDriverHost`.

### 4b. Rerouting branch in `on_prompt_assembled`

File: `crates/brassclaw_reborn/src/loop_driver_host.rs`, line 1801

The host already holds `self.interceptor_store` and `self.instruction_materialization_store`
(created per-build at line 1499). After Step 2c it also holds `self.interceptor_mode`.
After Step 4a it also holds `self.sempai_gateway`.

**Routing path** (mode == Routing OR sempai_gateway is None):
```
→ save ForensicPacket (existing code, unchanged)
→ return Some(InterceptorResult { packet_id, adjusted_messages: None })
  Kohai receives original messages unchanged.
```

**Rerouting path** (mode == Rerouting AND sempai_gateway is Some):
*(Compiled only when `#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]` — see Step 2c gate note.)*

```
1. Resolve the snapshot's message refs to text using
   self.instruction_materialization_store + run_context.
   The snapshot already carries the content_ref strings; look each one up
   in the materialization store (Priority::Materialization path), then in the
   thread context window for durable transcript refs.
   This produces Vec<(role, content_text)> — the real prompt text.

2. Build the Sempai audit prompt (§4c).

3. Call sempai_gateway.stream_model(audit_request).await.
   On error: tracing::debug!(...); fall back → return Some(..., adjusted_messages: None).

4. Parse response text as SempaiReviewOutcome JSON.
   On parse failure: tracing::debug!(...); fall back → return Some(..., adjusted_messages: None).

5. Update ForensicPacket: sempai_review = Some(outcome), status = SempaiReviewed. Save.

6. return Some(InterceptorResult {
       packet_id,
       adjusted_messages: Some(outcome.adjusted_messages),
   })
```

The adjusted `Vec<(role, text)>` travels through `InterceptorStage` →
`canonical.rs` → `ModelInput.resolved_messages` → into Kohai directly,
bypassing `resolve_model_messages` (Step 1c above).

### 4c. Sempai prompt structure — three parts

File: `crates/brassclaw_engine/prompts/sempai_audit.md` *(new, persona default text only)*

The Sempai prompt is assembled at intercept time from three ordered parts.
Part A is the largest and changes only when the operator explicitly triggers a
rebuild. Part B is editable per deployment. Part C changes every turn.

---

#### Part A — Static base prompt (full DB snapshot, KV-cache prefix)

This is the bulk of the Sempai prompt. It is assembled by querying every
`Validated` component from every component table — in the same order used for
Kohai prompt assembly in `spec.md §3.7`: `(class_code asc, prompt_uid asc)`.

Filter: `validation_status = Validated` AND `consumer_tags[] NOT CONTAINS
'05:validator'` — exactly the `fetch_for_consumer` gate from `spec.md §3.9`.
The full content of every passing row is included: tool param schemas,
skill bodies, extension payloads, orchestrator Python code, scaffold sections,
the `SempaiReviewOutcome` JSON schema.

**Class code coverage is additive.** The initial implementation queries the
tables that exist at interceptor-ship time (see §4e `reassemble_base_prompt()`).
As Phase 5 tables are created (class codes 12–20: Spec, ToolSkill, Plan,
Summary, Docu, Lesson, Issue, Note, Actions), each new table is added to the
`UNION ALL` query in `reassemble_base_prompt()`. The Sempai prompt grows to
cover them automatically once the operator clicks "Reassemble Basic Prompt"
after each Phase 5 migration.

> **Note on Actions (class 16):** `reborn_actions` rows contain deterministic
> execution templates. Although Actions are LLM-free at runtime, their schemas
> and trigger conditions must be visible to the Sempai for audit completeness.
> Include them in Part A.
>
> **Note on Orchestrator (class 10):** Orchestrator Python/config is excluded
> from the *Kohai* prompt by the `spec.md §3.7` assembly path (it is not
> injected into normal turns). It is **included** in Part A because the Sempai
> needs the orchestrator source to audit agent decision-making. This is
> intentionally different from the Kohai case.

**This part is never rebuilt automatically.** It is rebuilt only when the
operator clicks **"Reassemble Basic Prompt"** in the interceptor config tab
(§4e). Automatic rebuilds on every component change would generate a new byte
sequence and destroy the provider's KV-cache prefix, defeating the entire
design. After clicking Reassemble, the operator should follow up with
**"Pre-warm Sempai KV-cache"** (§4d) to push the new prefix to the provider.

**Storage:** The assembled Part A string is stored as a single value in
`brassclaw_config` under key `interceptor.sempai_base_prompt`. Its assembly
timestamp is stored under `interceptor.sempai_base_prompt_assembled_at`.
The runtime reads both values once at startup; no per-turn DB query.

---

#### Part B — Persona / role definition (per-deployment, editable in WebUI)

A text block stored in `brassclaw_config` under key `interceptor.sempai_persona`.
Contains the Sempai's role definition, purpose in this deployment, and what it
may decide. The default text is loaded from
`crates/brassclaw_engine/prompts/sempai_audit.md` via `include_str!()`.

Editable from the interceptor config tab (§4e). Editing this does not
invalidate Part A or require a KV-cache re-warm — it changes only the
per-session system context.

---

#### Part C — Per-turn stripped Kohai prompt + component manifest

The only part that varies turn-to-turn. It is assembled from the resolved
Kohai messages at intercept time with the following transformation:

**Stripped:** All messages whose `content_ref` resolves to an
`InstructionBundleBuilder` priority 1–5 message (identity, instruction
snippets, memory, safety context, capability surface) are removed. Their
content is already in Part A; sending it again would duplicate tokens and
push the provider's per-turn input past the stable prefix.

**Replaced with a component manifest:** A structured list of every component
that was in the Kohai prompt's stable base. Each line:
```
{class_code}:{prompt_uid}  {type}  "{name}"
```
This manifest is also stored in the `ForensicPacket.prompt` JSONB under a
`component_manifest` key, linked to the packet id. The Sempai reads the
manifest to know exactly which components were active, without processing
their full text again (already in its KV cache).

**What remains after stripping:** the volatile tail — thread history messages
(priority 6) and inline nudges (priority 7) from
`InstructionBundleBuilder.build()`.

> **Phase 1.5 note (User-at-N-1 change):** Phase 1.5 (not Phase 5) resurrects
> `build_step_context` and restructures priority 6 into the User-at-N-1 path —
> prior-knowledge (volatile memories) moves from the System message into a User
> message injected at position N-1. This is an earlier change than originally
> noted here. The stripping boundary (priorities 1–5 = stable, 6–7 = volatile)
> remains the same conceptually, but the specific messages in priority 6 change
> shape when Phase 1.5 lands. **After Phase 1.5 ships**, verify that the Part C
> stripping logic correctly identifies and strips the new stable-tier injections
> and preserves only the per-turn volatile tail. Add this as a follow-up task
> in the Phase 1.5 completion checklist.

**Full layout sent to Sempai:**
```
[SYSTEM — Part A, from brassclaw_config key interceptor.sempai_base_prompt]
  All Validated DB components ordered by (class_code asc, prompt_uid asc):
    Tools (00) · Skills (01–03) · Extensions (04–09) · Orchestrator (10) · Scaffolds (50)
    · Phase-5 tables when present: (12 Spec · 13 ToolSkill · 14 Plan · 15 Summary
      · 16 Actions · 17 Docu · 18 Lesson · 19 Issue · 20 Note)
    (class code 11 is reserved — never emitted)
  SempaiReviewOutcome JSON schema

[SYSTEM — Part B, from brassclaw_config key interceptor.sempai_persona]
  Sempai role definition and task description

[USER — Part C, per-turn]
  --- Component manifest (stable-base portion, stripped from Kohai prompt) ---
  00:0001  tool       "bash_exec"
  01:0003  skill      "deploy-workflow"
  ...
  ---
  --- Kohai volatile tail (thread history + inline nudges) ---
  system: {content}
  user: {content}
  assistant: {content}
  ...
  ---
  iteration: N  message_count: N  token_budget_remaining: N
  bundle_fingerprint: <sha256>

[USER — request]
  Respond with a SempaiReviewOutcome JSON object.
```

---

#### Sempai response structure and Kohai prompt recomposition

The Sempai returns a `SempaiReviewOutcome` JSON. The existing struct in
[`packet.rs`](crates/brassclaw_interceptor/src/packet.rs) needs these changes:

```rust
pub struct SempaiReviewOutcome {
    /// The volatile tail messages (thread history + inline nudges), as
    /// adjusted by Sempai. May be identical to the input if no changes.
    pub adjusted_volatile_messages: Vec<(String, String)>,

    /// Optional bridge messages to insert BETWEEN the adjusted volatile
    /// tail and the recomposed stable base — used when Sempai detects a
    /// missing or supplementary component. Empty in the common case.
    pub bridge_messages: Vec<(String, String)>,

    /// Sempai's analysis summary.
    pub composition_summary: String,

    /// New components proposed for the validation queue.
    ///
    /// At interceptor ship time this field is **stored in the ForensicPacket JSONB
    /// but not acted upon** — the Q1 auto-validation queue and `05:validator`
    /// lifecycle are introduced in plan.md Phase 3. A follow-up task in Phase 3
    /// must wire this field: read `proposed_recipe_updates` from persisted packets
    /// and submit each entry to Q1 via `ComponentValidator`. Until Phase 3 ships,
    /// the field is always deserialized and written to `sempai_review` JSONB so no
    /// data is lost, but no validation or queue insertion happens.
    pub proposed_recipe_updates: Vec<serde_json::Value>,

    /// Optional agent-settings adjustments proposed by Sempai.
    /// Stored in the ForensicPacket JSONB; not acted upon in this release.
    /// A follow-up task should define which settings are safe to apply automatically.
    pub settings_adjustments: Vec<serde_json::Value>,
}
```

**Recomposition in `on_prompt_assembled` (rerouting path), after receiving
`SempaiReviewOutcome`:**

1. Take the stable-base messages the host saved before stripping (the
   materialized priority 1–5 messages from `InstructionBundleBuilder.build()`).
2. Append `outcome.bridge_messages` (if any).
3. Append `outcome.adjusted_volatile_messages`.

This produces the complete adjusted Kohai prompt. `ModelStage` forwards it
to the Kohai provider, which sees its normal stable prefix (KV-cache hit)
followed by the Sempai-adjusted volatile tail.

The adjusted messages travel as `InterceptorResult.adjusted_messages:
Some(Vec<(String, String)>)` through `InterceptorStage` → `canonical.rs` →
`ModelInput.resolved_messages` → Kohai directly (bypassing
`resolve_model_messages` — Step 1c).

### 4d. KV-cache pre-warm (manual button, synchronous HTTP call)

The pre-warm is a **manual operator action** in the interceptor config tab.
There is no automatic background task, no scheduler, no `mpsc` channel.

**Button:** **"Pre-warm Sempai KV-cache"** in the interceptor config tab (§4e),
next to "Reassemble Basic Prompt".

**New HTTP endpoint:**
```
POST /api/interceptor/prewarm    → PREWARM_SEMPAI descriptor
```

Handler (synchronous — button shows a spinner until response arrives):
1. Read `interceptor.sempai_base_prompt` from config store.
2. If empty: return `400 Bad Request` ("Basic prompt not yet assembled —
   click Reassemble Basic Prompt first").
3. Build a `CompletionRequest` with Part A as the sole system message and a
   minimal user message (e.g., `"Ready."`). Call `sempai_gateway.stream_model(...)`.
   Discard the response text.
4. Write `interceptor.sempai_prewarm_last_at = now()` to config store.
5. Return `200 OK` with the timestamp.

Rate-limit: 1 request per minute per caller (per `WebUiAuthenticatedCaller`
identity — not a global limit, so one operator's usage does not block others).
Part A can be hundreds of kilobytes; this protects the Sempai provider quota.
On exceeding the limit the handler returns `429 Too Many Requests` with body:
```json
{ "error": "rate_limited", "retry_after_seconds": 60 }
```
The WebUI spinner must handle `429` explicitly: show "Please wait 60 seconds
before re-warming" rather than a generic error. Body-limit: minimal (no
request body needed).

**Gate for §4d:** depends on §4a (sempai_gateway on host) and §4e (config
store keys `interceptor.sempai_base_prompt` + `interceptor.sempai_prewarm_last_at`).

### 4e. Interceptor config — DB-backed settings + WebUI tab

No `InterceptorConfig` structure exists anywhere. This adds it end-to-end,
following the identical pattern as `LlmConfigService` / `SafetyConfigStore`.

#### Port trait — `brassclaw_product_workflow`

File: `crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs` *(new)*

```rust
#[async_trait]
pub trait InterceptorConfigService: Send + Sync {
    async fn snapshot(&self, caller: WebUiAuthenticatedCaller)
        -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;
    async fn update(&self, caller: WebUiAuthenticatedCaller, request: UpdateInterceptorConfigRequest)
        -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;
    async fn reassemble_base_prompt(&self, caller: WebUiAuthenticatedCaller)
        -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;
    async fn prewarm(&self, caller: WebUiAuthenticatedCaller)
        -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;
}

pub struct InterceptorConfigSnapshot {
    pub sempai_connected: bool,
    pub mode: String,                              // "routing" | "rerouting"
    pub base_prompt_assembled_at: Option<DateTime<Utc>>,
    pub base_prompt_size_chars: Option<usize>,     // display only
    pub persona: String,                           // current persona text
    pub prewarm_last_at: Option<DateTime<Utc>>,
}

pub struct UpdateInterceptorConfigRequest {
    pub persona: Option<String>,
}
```

Add `Option<Arc<dyn InterceptorConfigService>>` field + default no-op bodies to
`RebornServicesApi` (same pattern as all other optional services).

#### Store — `brassclaw_interceptor`

File: `crates/brassclaw_interceptor/src/config_store.rs` *(new)*

Trait `InterceptorConfigStore` with `load()` and `save()`. Backed by the
existing `brassclaw_config` Postgres table — no new migration needed.

Config keys written/read:
- `interceptor.sempai_base_prompt` — the assembled Part A string
- `interceptor.sempai_base_prompt_assembled_at` — ISO 8601
- `interceptor.sempai_persona` — the editable Part B text
- `interceptor.sempai_prewarm_last_at` — ISO 8601

#### Impl — `brassclaw_reborn_composition`

File: `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` *(new)*

`RebornInterceptorConfigService` holds:
- `Arc<PgPool>` — reads/writes `brassclaw_config` keys AND queries component
  tables directly (see below)
- `Arc<SharedInterceptorMode>` — reads current mode for snapshot
- `Arc<dyn HostManagedModelGateway>` (sempai_gateway) — used by `prewarm()`

> **No `Arc<dyn RetrievalSource>`.** The `RetrievalSource` trait is introduced
> in Phase 5. The interceptor ships before Phase 5. `reassemble_base_prompt()`
> must issue its own SQL directly via `Arc<PgPool>`. Use a single `UNION ALL`
> query (or sequential queries) against the tables that exist at ship time.
> When Phase 5 adds new tables, extend the query with additional `UNION ALL`
> branches — a mechanical two-line change per new class code.

> **Prerequisite — component tables must exist.** `reassemble_base_prompt()`
> requires at least plan.md Phase 1 (migration V35, `reborn_skills`) through
> Phase 4 (migrations V39–V40, `reborn_extensions_unified` + `reborn_recipes`)
> to have been applied. If called before those migrations have run, the handler
> must detect missing tables and return `400 Bad Request` with
> `{ "error": "component_tables_missing", "message": "Run Phase 1–4 migrations
> before assembling the base prompt." }`. Detection: query
> `information_schema.tables WHERE table_name = 'reborn_skills'` before
> executing the `UNION ALL`. If absent, return early with the 400 response.
> The WebUI "Reassemble Basic Prompt" button must display this error message
> verbatim so the operator understands what to do next.

`reassemble_base_prompt()` — direct SQL, no trait dependency:
1. Check that `reborn_skills` exists in `information_schema.tables`. If not,
   return `400` as described above (all component tables are created in the
   same migration window; if the first is absent the rest are too).
2. Run a `UNION ALL` SQL query across all component tables that exist at
   ship time, filtering `validation_status = 'Validated'` and excluding rows
   where `consumer_tags @> ARRAY['05:validator']`. Order by
   `(class_code ASC, prompt_uid ASC)`. Tables at initial ship (created by
   plan.md Phases 1–4): `reborn_tools` (00), `reborn_skills` (01–03),
   `reborn_extensions_unified` (04–09), `reborn_orchestrators` (10),
   `reborn_scaffolds` (50).
   Each branch selects: `class_code`, `prompt_uid`, `component_type`,
   `display_name`, `content` (or equivalent column per table schema).
   **Note:** class code 11 is reserved; no `reborn_scaffolds (11)` table
   exists. Scaffold is class 50 (`reborn_scaffolds`).
3. Serialize each row into Part A: one block per row with its class/uid header
   and full content text.
4. Append the `SempaiReviewOutcome` JSON schema as a literal `include_str!()`
   block so the Sempai knows what shape to respond with.
5. Write assembled string to `brassclaw_config` key
   `interceptor.sempai_base_prompt` and timestamp to
   `interceptor.sempai_base_prompt_assembled_at = now()`.
6. Return updated snapshot.

**Phase 5 extension:** when `reborn_actions` (16), `reborn_specs` (12), etc.
are created, add each as a new `UNION ALL` branch in this query. No trait
changes needed — it is plain SQL.

`prewarm()`: delegates to the `POST /api/interceptor/prewarm` handler logic
described in §4d.

#### HTTP endpoints — `brassclaw_webui_v2`

Four routes in `descriptors.rs` + router:

```
GET  /api/interceptor/config          → GET_INTERCEPTOR_CONFIG descriptor
POST /api/interceptor/config          → UPDATE_INTERCEPTOR_CONFIG descriptor
POST /api/interceptor/reassemble      → REASSEMBLE_SEMPAI_BASE descriptor
POST /api/interceptor/prewarm         → PREWARM_SEMPAI descriptor
```

All four endpoints require the existing `WebUiAuthenticatedCaller` bearer token
— the same auth as all other Settings endpoints. No additional operator-role
gate is required. `reassemble` and `prewarm` are potentially long-running —
set `streaming: None` (synchronous) with a generous timeout (recommend 120s
for `reassemble`, 60s for `prewarm`) but tight rate-limit (1/min per caller —
`429 Too Many Requests` with `retry_after_seconds` as in §4d). The same
rate-limit applies to `reassemble` to prevent repeated expensive `UNION ALL`
queries.

#### WebUI interceptor config tab

File: `crates/brassclaw_webui_v2_static/pages/settings/interceptor/` *(new)*

Displays on load (`GET /api/interceptor/config`):
- Sempai status: connected / disconnected, mode (routing / rerouting)
- Base prompt: last assembled timestamp + char count. Button: **"Reassemble
  Basic Prompt"** — calls `POST /api/interceptor/reassemble`, shows spinner,
  refreshes snapshot on completion.
- KV-cache: last pre-warm timestamp. Button: **"Pre-warm Sempai KV-cache"** —
  calls `POST /api/interceptor/prewarm`, shows spinner, refreshes on completion.
- Persona: editable textarea bound to `snapshot.persona`. Save button calls
  `POST /api/interceptor/config` with `{ persona: "..." }`.

No build step — `node --check` on the JS file for syntax validation.

**Gate for §4e:**
```bash
cargo clippy -p brassclaw_product_workflow -p brassclaw_webui_v2 \
             -p brassclaw_reborn_composition -p brassclaw_interceptor -- -D warnings
cargo test -p brassclaw_product_workflow -p brassclaw_reborn_composition
```
Update `tests/webui_v2_descriptors_contract.rs` with the four new descriptors.

### 4f. `on_kohai_response` — no change needed

The existing implementation reads the packet by id and calls
`packet.with_kohai_response(...)` regardless of routing/rerouting mode.

**Final gate (all of Step 4):**
- `cargo clippy --all -- -D warnings` clean.
- `cargo test -p brassclaw_agent_loop -p brassclaw_reborn -p brassclaw_interceptor -p brassclaw_reborn_composition`
- Integration test: configure Sempai mock → mode flips → full turn → Kohai
  receives Sempai-adjusted messages → packet `status = sempai_reviewed` →
  component manifest present in `ForensicPacket.prompt`.
- Integration test: Sempai error → Kohai receives original messages →
  packet `status = complete`.
- Integration test: `set_active(Sempai, "")` clears → mode flips to Routing.
- Integration test: `POST /api/interceptor/reassemble` → Part A written to
  `brassclaw_config`; subsequent `GET /api/interceptor/config` shows new
  timestamp + char count.
- Integration test: `POST /api/interceptor/prewarm` with empty base prompt →
  `400`; with assembled prompt → `200` + last-prewarm timestamp updated.

---

## File Change Summary

| File | Change |
|---|---|
| `migrations/V34__forensic_packets.sql` *(new)* | Create `brassclaw_forensic_packets` table |
| `crates/brassclaw_interceptor/src/pg_store.rs` | Fix module doc: V026 → V034 |
| `crates/brassclaw_interceptor/src/packet.rs` | Replace `adjusted_messages` with `adjusted_volatile_messages` + `bridge_messages` in `SempaiReviewOutcome` |
| `crates/brassclaw_interceptor/src/config_store.rs` *(new)* | `InterceptorConfigStore` trait + Pg impl; 4 config keys |
| `crates/brassclaw_turns/src/run_profile/host.rs` | Add `InterceptorResult`; change `on_prompt_assembled` return type |
| `crates/brassclaw_agent_loop/src/executor/interceptor.rs` | Add `adjusted_messages` field to `InterceptorPromptOutput`; extract from `InterceptorResult` |
| `crates/brassclaw_agent_loop/src/executor/canonical.rs` | Thread `adjusted_messages` → `ModelInput` |
| `crates/brassclaw_loop_support/src/lib.rs` | Add `resolved_messages` fast path in `stream_model`; skip `resolve_model_messages` when pre-resolved |
| 6 test stub files | Mechanical return-type update (`Option<String>` → `Option<InterceptorResult>`) |
| `crates/brassclaw_reborn/src/runtime.rs` | Add `interceptor_mode` + `sempai_gateway` to `DefaultPlannedRuntimeParts`; wire both |
| `crates/brassclaw_reborn/src/loop_driver_host.rs` | Add `interceptor_mode` + `sempai_gateway` fields + builders; rerouting branch in `on_prompt_assembled`; strip stable-base messages + build component manifest |
| `crates/brassclaw_reborn_composition/src/runtime.rs` | Wire `PgInterceptorStore`; allocate `sempai_swappable`; create `SharedInterceptorMode`; build Sempai gateway |
| `crates/brassclaw_reborn_composition/src/llm_config_service.rs` | `set_active(Sempai)` live-swap + mode flip; `build_sempai_provider` |
| `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` *(new)* | `RebornInterceptorConfigService` impl; `reassemble_base_prompt()` via direct SQL (no `RetrievalSource`) + `prewarm()` |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Wire `RebornInterceptorConfigService` into WebUI facade (`webui.rs` confirmed present in the crate) |
| `crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs` *(new)* | Port trait + DTOs; 4 methods |
| `crates/brassclaw_webui_v2/src/descriptors.rs` | Add 4 interceptor descriptors |
| `crates/brassclaw_webui_v2/src/router.rs` | Mount 4 interceptor routes |
| `crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs` | Add 4 new descriptors |
| `crates/brassclaw_webui_v2_static/pages/settings/interceptor/` *(new)* | Settings tab (2 action buttons + persona editor) |
| `crates/brassclaw_engine/prompts/sempai_audit.md` *(new)* | Default persona text (Part B) |

---

## Execution Order

```
Step 0 (migration — prerequisite, no Rust changes)
  │
  ▼
Step 1 (API — only breaking change: InterceptorResult return type;
              SempaiReviewOutcome struct update)
  │            Step 1 must be fully resolved (clippy clean) before starting
  │            Step 2 or Step 3 — both depend on the new `InterceptorResult`
  │            return type and `SharedInterceptorMode` type introduced here.
  │
  ├──► Step 2 (PgStore + swappable + mode flag — pure composition, 3 independent edits)
  │            Requires: Step 1 complete.
  │
  └──► Step 3 (set_active live-swap — extends config service)
               Requires: Step 1 complete + Step 2 complete
               (`sempai_swappable` must be allocated before being threaded
               into `RebornLlmConfigService`).
                    │
                    ▼
              Step 4a–c (Sempai gateway + rerouting branch + 3-part prompt assembly)
              Requires: Step 2 (mode flag in host) + Step 3 (swappable allocated)
                    │
              Step 4e can run in parallel with 4a–c
              (interceptor config service + WebUI tab; only blocks 4d)
                    │
                    ▼
              Step 4d (pre-warm endpoint)
              Requires: 4a (gateway wired to host) + 4e (config store with base prompt key)
```

---

## Validation

```bash
# Step 0
cargo test -p brassclaw_interceptor

# Steps 1–3
cargo clippy --all -- -D warnings
cargo test -p brassclaw_agent_loop -p brassclaw_turns -p brassclaw_reborn \
           -p brassclaw_reborn_composition

# Step 4 (full integration)
cargo test --features integration -p brassclaw_reborn -p brassclaw_reborn_composition \
           -p brassclaw_interceptor

# Step 4e (WebUI contract)
cargo test -p brassclaw_webui_v2
node --check crates/brassclaw_webui_v2_static/pages/settings/interceptor/index.js
```
