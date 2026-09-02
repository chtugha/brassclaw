# Prefix Cache V3 — Implementation Plan (Revised v2, corrected architecture)

> **Supersedes:** all previous drafts of this file.
>
> **Key correction vs v1 draft:** the v1 draft wrongly removed `bundle_json` from the
> schema on the assumption that vLLM APC made it unnecessary. The user corrected this:
> both the **Kohai** (primary provider, runs on every turn) and the **Sempai** (optional
> reviewer, only when connected) need the bundle prepended to their prompts. The Sempai
> is not always connected, so the Kohai needs the bundle independently. Re-assembling
> 200k tokens from raw component rows on every turn is unacceptable — the bundle MUST
> be stored. `bundle_json` is restored.

---

## 0. Architecture clarification: two separate models, two separate prompts

```
Every turn:
┌───────────────────────────────────────────────────────────────────┐
│  KOHAI (primary provider — always present, handles user request)  │
│  prompt = [bundle: Part A] + [per-turn patch: <4k tokens]         │
│                           + [conversation history] + [user msg]   │
└───────────────────────────────────────────────────────────────────┘
         ↓ (only when Sempai is connected / rerouting mode)
┌───────────────────────────────────────────────────────────────────┐
│  SEMPAI (optional reviewer)                                        │
│  prompt = [bundle: Part A (different content)] + [persona: Part B]│
│         + [volatile tail: Kohai messages for review: Part C]       │
└───────────────────────────────────────────────────────────────────┘
```

- The **Kohai** call goes to the **provider** (vLLM / OpenAI-compat / any LLM),
  model profile `"default"` (or `"cheap_model"`). It runs on every single turn.
- The **Sempai** call goes to a separate gateway (`sempai_model` profile). It only
  runs when `interceptor_mode == Rerouting`.
- Both need the bundle — but they may eventually need different bundle content.
  Phase K.1 uses the same `bundle_json` for both; separate Sempai bundles are a
  future named-prefix extension.

**What was wrong in the v1 draft:**
- It assumed the bundle only went to the Sempai.
- It removed `bundle_json` (the stored text) from the schema, forcing a full
  re-assembly from Postgres on every LLM call — 200k tokens × many queries × every
  turn = unacceptable overhead.
- vLLM APC is a real benefit (byte-identical prefix token sequences hit the
  server-side KV-cache), but it does NOT eliminate the need for the client to send
  the same tokens. The client needs the bundle text to send it. Storing it is mandatory.

---

## 1. What `PgBasicPromptStore` IS — corrected

`PgBasicPromptStore` stores the **pre-assembled bundle text** plus metadata:

```
reborn_basic_prompt_store
  ├─ scope (tenant_id, user_id, agent_id, project_id)
  ├─ bundle_json       JSONB NOT NULL DEFAULT '""'   ← the bundle text, stored as JSON string
  ├─ fingerprint       TEXT  NOT NULL DEFAULT ''      ← sha256(bundle_text) for staleness check
  ├─ is_stale          BOOL  NOT NULL DEFAULT false
  ├─ assembled_at      TIMESTAMPTZ
  ├─ prewarm_last_at   TIMESTAMPTZ
  └─ updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
```

**`bundle_json`** stores the rendered bundle string as a JSONB string value (not an
object), matching the existing `do_reassemble` output type. This is exactly what
`saved_plan_to_v3.md §K.1.1` originally specified — the v1 draft wrongly removed it.

**Why store it:**
1. The Kohai calls the LLM on every turn. Re-assembling 200k tokens from raw
   component rows on every turn costs many Postgres round-trips and is unacceptable.
2. The Sempai also needs the bundle. If the Sempai is disconnected mid-session and
   then reconnected, it must still get the current bundle without a full re-assembly.
3. vLLM APC fires on byte-identical token sequences. Storing the bundle ensures the
   same text is sent every turn — any assembly divergence (different row order,
   different timestamp format) would break cache hits.

**The fingerprint** (`sha256(bundle_text)`) is used as a staleness check:
- If a new assembly produces the same fingerprint as the stored row, the DB write is
  skipped (no-op).
- The `is_stale` flag is set to `true` after any Q2 graduation; it signals the
  Prefix Tab to show the Regenerate button.

---

## 2. Multiple prefix caches

Phase K.1 has exactly one entry: `"base-prompt"`. Future named prefixes
(`"base-prompt:kohai"`, `"base-prompt:sempai"`) are additive rows — the `UNIQUE`
scope constraint already supports them.

---

## 3. Per-turn flow — how the bundle reaches each model

### 3.1 Kohai per-turn (always)

```rust
// In the Kohai prompt assembly path (loop_driver_host.rs or default.py orchestrator):
let system_content = get_kohai_system_content(scope, pool, store).await;
// Prepend as System message [0]; per-turn patch and history follow.
```

**Fast path:** `is_stale = false` → read `bundle_json` from DB (one cheap row fetch,
no content-column re-assembly) → return the stored text.

**Slow/cold path:** `is_stale = true` or no row → use fallback minimal prompt.
Operator must click Regenerate to restore the full bundle.

### 3.2 Sempai per-turn (only when rerouting)

Same `get_kohai_system_content` call, or a separate named-prefix read for the
Sempai-specific bundle (future). Phase K.1 uses the same bundle for both.

The Sempai prompt structure (from live code):
- `[System]` = bundle (Part A) — was previously `persona_text` (Part B) alone; Phase K.1 prepends the bundle
- `[System]` = persona (Part B) — kept as a second System message
- `[User]`   = volatile tail (Part C)

---

## 4. Database migration — V063

```sql
-- Thin per-scope prefix-cache metadata + stored bundle.
CREATE TABLE IF NOT EXISTS reborn_basic_prompt_store (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         TEXT        NOT NULL DEFAULT '',
    user_id           TEXT        NOT NULL DEFAULT '',
    agent_id          TEXT        NOT NULL DEFAULT '',
    project_id        TEXT        NOT NULL DEFAULT '',
    -- The assembled bundle stored as a JSONB string value (not an object).
    -- Empty string until the first assembly.
    bundle_json       JSONB       NOT NULL DEFAULT '""',
    -- sha256(bundle_text) — used to detect re-assembly producing identical output.
    fingerprint       TEXT        NOT NULL DEFAULT '',
    is_stale          BOOLEAN     NOT NULL DEFAULT false,
    assembled_at      TIMESTAMPTZ,
    prewarm_last_at   TIMESTAMPTZ,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT reborn_basic_prompt_store_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id)
);

CREATE INDEX IF NOT EXISTS reborn_basic_prompt_store_scope_idx
    ON reborn_basic_prompt_store (tenant_id, user_id, agent_id, project_id);

-- §0.23.7: component UUID reference on interceptor forensic packets.
ALTER TABLE brassclaw_forensic_packets
    ADD COLUMN IF NOT EXISTS component_uuid UUID;

-- §0.23.8: validation-improve settings on reborn_monty_vm_settings.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN IF NOT EXISTS validation_idle_threshold_minutes INT  NOT NULL DEFAULT 120,
    ADD COLUMN IF NOT EXISTS validation_improve_start_hour     INT  NOT NULL DEFAULT 15,
    ADD COLUMN IF NOT EXISTS validation_improve_enabled        BOOL NOT NULL DEFAULT true;
```

---

## 5. `PgBasicPromptStore` facade

```rust
pub(crate) struct BasicPromptEntry {
    pub id:             uuid::Uuid,
    pub bundle:         String,     // the rendered bundle text (from bundle_json JSONB)
    pub fingerprint:    String,     // sha256(bundle_text)
    pub is_stale:       bool,
    pub assembled_at:   Option<chrono::DateTime<chrono::Utc>>,
    pub prewarm_last_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at:     chrono::DateTime<chrono::Utc>,
}

impl PgBasicPromptStore {
    /// Get the stored row for a scope. Returns None if never assembled.
    pub async fn get_for_scope(user_id, project_id) -> Result<Option<BasicPromptEntry>>;

    /// Upsert after a successful assembly.
    /// - Stores bundle_json, fingerprint.
    /// - Sets is_stale = false, assembled_at = now().
    /// - If with_prewarm: also sets prewarm_last_at = now().
    pub async fn store(user_id, project_id, bundle: &str, with_prewarm: bool)
        -> Result<BasicPromptEntry>;

    /// Set is_stale = true. No-op if no row exists.
    pub async fn mark_stale(user_id, project_id) -> Result<()>;
}
```

**`store()`** computes `fingerprint = sha256(bundle)` itself before the upsert.
If the new fingerprint matches the existing row's fingerprint and `is_stale = false`,
it is still an upsert (updates `assembled_at` and optionally `prewarm_last_at`) but
does not rewrite the large `bundle_json` column unnecessarily (future optimization;
Phase K.1 always writes).

---

## 6. `do_assemble_bundle` (renamed from `do_reassemble`)

Unchanged SQL logic. Additions:
1. Compute `fingerprint = sha256(bundle_text)` from the assembled string.
2. Call `pg_basic_prompt_store.store(user_id, project_id, &bundle, false)`.
3. Return `(bundle: String, fingerprint: String)`.

**The bundle text IS written to DB** — this is the key correction vs v1.

---

## 7. `do_format_bundle` — conversion step

Pure-Rust formatter, same as before. Extracts the `buf.push_str` logic from
`do_reassemble` so it is testable and swappable.

---

## 8. `regenerate_prefix` — the operator-triggered assemble + prewarm

```
regenerate_prefix(caller, "base-prompt", user_id, project_id):
  1. Name guard: only "base-prompt" accepted.
  2. Rate-limit: 60s per caller.
  3. (bundle, fingerprint) = do_assemble_bundle()
      → stores bundle_json in PgBasicPromptStore (is_stale=false)
  4. Send bundle to gateway.stream_model("sempai_model", [System: bundle])
      → vLLM pre-warms KV blocks
  5. pg_basic_prompt_store.store(..., with_prewarm=true)
  6. Return PrefixRegenerateResponse { name, fingerprint, assembled_at, prewarm_last_at }
```

---

## 9. Per-turn System content retrieval

```rust
/// Returns the bundle to use as the System message prefix for this scope.
///
/// Fast path: non-stale row exists → return stored bundle_json (cheap single-row fetch).
/// Slow path: stale or no row → return minimal fallback; operator must click Regenerate.
pub async fn get_system_bundle(
    store: &PgBasicPromptStore,
    user_id: &str,
    project_id: &str,
) -> String {
    match store.get_for_scope(user_id, project_id).await {
        Ok(Some(entry)) if !entry.is_stale && !entry.bundle.is_empty() => entry.bundle,
        _ => minimal_base_prompt_fallback(),
    }
}
```

This replaces calling `do_assemble_bundle()` on every turn. The assembly only runs
during `regenerate_prefix` (operator action) — not per-turn.

---

## 10. Kohai injection (§K.1.5)

The per-turn Kohai prompt assembly prepends the bundle as `[System]` message [0].

In `run_sempai_review` (the Sempai path), add the bundle as an additional `[System]`
message before the persona (Part B). Both use `get_system_bundle(store, user_id, project_id)`.

The **Kohai injection** goes into:
- `crates/brassclaw_reborn/src/loop_driver_host.rs` — specifically the
  `LoopContextPort::load_loop_context` path (or the `handle_assemble_prior_knowledge`
  Python entry point that builds `working_messages`).
- The `basic_prompt_section_refs` field in `BuildInstruction` carries navigation hints
  for components already in the bundle, preventing duplication in the per-turn patch.

---

## 11. IBS `basic_prompt_section_refs` (§10 from v1 — unchanged)

The `basic_prompt_section_refs` field is populated when a recipe match has orchestrator
items whose UUIDs are already in the stored bundle. The per-turn patch omits those
bodies and adds navigation hints instead.

Phase K.1: this is not yet wired. The field stays `vec![]` — a Phase K.2 extension.

---

## 12. `mark_stale` wiring (§0.15 side-effect 4)

After Q2 graduation in `ValidationQueueStore::approve`:
```rust
if let Some(store) = &self.basic_prompt_store {
    if let Err(e) = store.mark_stale(&scope.user_id, &scope.project_id).await {
        debug!("mark_stale after Q2 graduation: {e}");
    }
}
```

---

## 13. Routes / DTOs / trait changes

Unchanged from what was already implemented in the previous coding steps:
- `GET /api/prefixes` → `PrefixListResponse`
- `POST /api/prefixes/:name/regenerate` → `PrefixRegenerateResponse`
- `InterceptorConfigService::list_prefix_entries` / `regenerate_prefix`
- `InterceptorConfigSnapshot` trimmed (no `base_prompt_assembled_at` etc.)

**Key DTO correction vs v1:**
- `PrefixEntry.component_fingerprint` → renamed to `fingerprint` (sha256 of bundle text,
  not of row metadata)
- `PrefixRegenerateResponse.component_fingerprint` → `fingerprint`

---

## 14. Implementation order

### Step 1 — Fix V063 migration: add `bundle_json` back ✅ DONE
### Step 2 — Fix `PgBasicPromptStore`: add `bundle` field, `store()` method ✅ DONE
### Step 3 — Fix `do_assemble_bundle`: call `store()` to persist bundle text ✅ DONE
### Step 4 — Add `get_system_bundle()` helper ✅ DONE
### Step 5 — Fix `regenerate_prefix`: use `store()` with `with_prewarm=true` ✅ DONE
### Step 6 — Wire Kohai injection: prepend bundle as System [0] on every turn ✅ DONE
### Step 7 — Wire Sempai injection: add bundle as System [0] in `run_sempai_review` ✅ DONE
### Step 8 — `mark_stale` wiring in validation_queue ✅ DONE
### Step 9 — Fix `PrefixEntry.fingerprint` field name in DTOs ✅ DONE
### Step 10 — WebUI Prefix Tab (§K.1.6) ✅ DONE

---

## 15. Tests

### Unit — `pg_basic_prompt_store.rs`
- `store(bundle, with_prewarm=false)` → `is_stale=false`, `prewarm_last_at=None`
- `store(bundle, with_prewarm=true)` → `prewarm_last_at=Some`
- `mark_stale` → `is_stale=true`; second `store()` → `is_stale=false`
- `mark_stale` with no row → `Ok(())`
- `get_for_scope` returns stored bundle text

### Unit — `get_system_bundle`
- Non-stale row → returns stored bundle text
- `is_stale=true` → returns fallback
- No row → returns fallback

### Unit — fingerprint
- Same text → identical fingerprint
- Different text → different fingerprint

### Integration
- `regenerate_prefix` → row in `reborn_basic_prompt_store`, `bundle_json` non-empty
- Q2 graduation → `mark_stale` → `is_stale=true`
- `regenerate_prefix` → `is_stale=false`, `prewarm_last_at` set

---

## 16. Alignment with `saved_plan_to_v3.md §0.13` and `§K.1`

| Plan clause | This plan | Status |
|---|---|---|
| `bundle_json JSONB NOT NULL` | §4 — restored to schema | ✅ DONE |
| `fingerprint = sha256(bundle_content)` | §5 — sha256 of bundle text | ✅ DONE |
| "Manual trigger only" | §8 `regenerate_prefix` | ✅ DONE |
| "Stale when any Q2 passes Gate 2" | §12 `mark_stale` | ✅ DONE |
| "Ships bundle to Sempai as System message" | §8 step 4 | ✅ DONE |
| "Kohai prompt has base-prompt bundle prepended" | §10 Kohai injection | ✅ DONE |
| "per-turn prompt carries single `base-prompt` placeholder" | §9 `get_system_bundle()` | ✅ DONE |
| "short minimal-context fallback when stale/absent" | §9 `minimal_base_prompt_fallback()` | ✅ DONE |
| `basic_prompt_section_refs` navigation hints | §11 — Phase K.2 | deferred (future) |
| Multiple named prefixes | §2 — additive rows | ✅ DONE |
| Routes / DTOs: `GET /api/prefixes`, `POST /api/prefixes/{name}/regenerate` | §13 | ✅ DONE |
| WebUI SPA: Prefix tab, `usePrefixes.js`, `prefix-tab.js` | §K.1.6 | ✅ DONE |
| i18n: `settings.prefix`, `prefix.*` keys in `en.js` | §K.1.6 | ✅ DONE |
| Axum route pattern: `{name}` syntax (not `:name`) | §13 | ✅ DONE |
