# 17 — WebUI v2 & the Prefix Tab

> **Subsystem:** The browser surface of the agent — the React single-page
> app (WebUI v2), the Rust route layer that backs it, the host-owned
> ingress that binds the listener, and the **Prefix Tab**, the new v3
> settings surface that lists the vLLM prefix-cache entries (today only
> the base prompt; more in the future) and gives each a
> generate/regenerate button that assembles the prefix and ships it to
> the LLM for compilation. This doc is the UI + prefix-compilation
> companion to `09-sempai-kohai.md` and `10-prefix-base-prompt.md`.
> **Grounded in:** `crates/brassclaw_webui_v2/AGENTS.md` + `CLAUDE.md`
> (route table, boundary rules, streaming model), `crates/brassclaw_webui_v2/src/lib.rs`
> (router exports), `src/descriptors.rs` (`get_interceptor_config_descriptor:915`,
> `list_prefixes_descriptor:943`, `regenerate_prefix_descriptor:957`; patterns
> `/api/webchat/v2/prefixes:78` + `…/prefixes/{name}/regenerate:79`),
> `src/handlers.rs` (`get_interceptor_config:1340`, `update_interceptor_config:1351`,
> `list_prefixes:1366`, `regenerate_prefix:1387`), `crates/brassclaw_webui_v2_static/static/js/pages/settings/`
> (`lib/settings-schema.js` (`prefix` tab:17), `lib/settings-api.js`
> (`fetchPrefixes:176`, `regeneratePrefix:179`), `hooks/usePrefixes.js`,
> `components/interceptor-tab.js`, `components/validation-queue-tab.js`,
> `components/skills-tab.js`), `static/js/app/routes.js` (`prefix` route:51),
> `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
> (`do_assemble_bundle:193`, `get_system_bundle:334`, `snapshot:346`,
> `list_prefix_entries:397`, `regenerate_prefix:442`, `KEY_PERSONA:47`),
> `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs`
> (`PgBasicPromptStore::{new,get_for_scope,store,mark_stale,compute_fingerprint,
> minimal_base_prompt_fallback}` + crate `get_system_bundle:285`, migration
> **V063** `reborn_basic_prompt_store`), `crates/brassclaw_reborn_webui_ingress/AGENTS.md`,
> `saved_plan_to_v3.md` §0.13 (KV-Cache / LMCache-Aware Design), and the user's
> Task item 8 (Prefix Tab under settings; generate/regenerate each prefix; the
> existing base-prompt implementation shifted into the Prefix Tab — **shipped**).

## 1. Purpose

The WebUI is the only human surface on the agent. It does three jobs:

1. **Chat** — `POST /api/webchat/v2/threads/{id}/messages` drives a turn
   through `RebornServicesApi`; the browser consumes the result over SSE
   (`stream_events`) or WebSocket (`stream_events_ws`). This is the
   message-flow of `MESSAGE_FLOW_AND_PLAN_AUDIT.md` §2 — orchestrator →
   intent → recipe or LLM — surfaced as a chat thread.
2. **Operator configuration** — the settings page (17 tabs) tunes the
   agent: inference/LLM providers, tools, skills, actions, orchestrator,
   scaffold, Monty VM, the validation queue, reliability, the
   Sempai-Kohai interceptor, safety, tokens, users, language.
3. **Component authoring & review** — authoring skills/tools/recipes/
   PythonCode/Actions (the v3 component catalog of `15-component-catalog.md`)
   and reviewing them through the validation queue (`14-validation-queue.md`).

The **Prefix Tab** is the v3 settings tab (item 8) that consolidates
**prefix compilation** — the act of assembling a long, stable prompt
prefix and shipping it to the LLM once so its KV cache holds it, after
which every turn only sends a small patch. Today there is exactly one
prefix — the **base prompt** — and its assemble+regenerate flow is
**shipped** as a dedicated Prefix Tab (`prefix` route in the settings
schema, `usePrefixes` hook, `fetchPrefixes`/`regeneratePrefix` SPA API)
that lists prefixes (one now, more later) and gives each a
generate/regenerate button. The Sempai-Kohai system substitutes the real
prefix content for the `base-prompt` placeholder line at the very end of
prompt creation (`09-sempai-kohai.md`, `10-prefix-base-prompt.md`).

## 2. Location

The WebUI is split across three crates, mirroring the three-layer model
(Products own UX; the Kernel owns authority):

| Crate | Layer | Owns |
|-------|-------|------|
| `crates/brassclaw_webui_v2/` | Product (route layer) | Rust route descriptors + axum handlers. The `IngressRouteDescriptor` set (`webui_v2_routes()`) is the canonical contract host composition mounts; handlers dispatch only to `RebornServicesApi`. |
| `crates/brassclaw_webui_v2_static/` | Product (SPA) | The React single-page app: `static/index.html`, `static/js/` (app, components, design-system, hooks, i18n, lib, pages). Built by `build.rs` and served as static assets. |
| `crates/brassclaw_reborn_webui_ingress/` | Host (ingress) | Binds `tokio::net::TcpListener`, drives `axum::serve` with the composed `Router`, and provides `WebuiAuthenticator` impls (env-bearer first; OIDC/DB follow-ups). Constant-time token comparison. |

Inside the SPA, the settings surface is:

- `static/js/pages/settings/settings-page.js` — the tab router. `tabContent`
  maps each tab id to a component (`InferenceTab`, `ToolsTab`, `SkillsTab`,
  `ActionsTab`, `OrchestratorTab`, `ScaffoldTab`, `MontyVmTab`,
  `ValidationQueueTab`, `ReliabilityTab`, `InterceptorTab`, `SafetyPanel`,
  `TokensTab`, `UsersTab`, `LanguageTab`, …). The `tab` URL param defaults
  to `inference`; an unknown or admin-gated tab redirects.
- `static/js/pages/settings/lib/settings-schema.js` — `SETTINGS_TABS`, the
  ordered list of 17 tabs (`{ id, labelKey, icon }`). **There is no
  `prefix` tab today** — confirmed by reading the file: the list ends at
  `language`.
- `static/js/app/routes.js` — `SETTINGS_SUB_ROUTES`, the sidebar
  navigation. Tabs whose `lib/*-api.js` are still stubs are commented out
  (hidden) until their v2 endpoints land; the unhide rule is documented
  in the file header. The Interceptor tab *is* unhidden (its endpoints are
  real).
- `static/js/pages/settings/lib/settings-api.js` — the API client. Each
  function maps to one v2 endpoint (`apiFetch("/api/webchat/v2/...")`) or
  is an explicit `TODO: requires v2 ... endpoint` stub. This file is the
  single source of truth for which settings surfaces are real vs stubbed.
- `static/js/pages/settings/components/interceptor-tab.js` — **the existing
  base-prompt compile UI** (StatusCard + PersonaCard + ControlCard with
  Reassemble + Pre-warm buttons). This is what the Prefix Tab absorbs.

On the Rust side, the prefix-compile routes today are:

- `crates/brassclaw_webui_v2/src/descriptors.rs` —
  `WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR` / `_PREWARM_INTERCEPTOR` and
  patterns `/api/webchat/v2/interceptor/reassemble` / `.../prewarm`
  (`reassemble_interceptor_descriptor:938`, `prewarm_interceptor_descriptor:953`).
  Both are `mutation_policy(NoBody, mutation_rate_limit(), AuditTraceClass::UserAction, AllowedEffectPath::ProductWorkflow)` — a 1/min per-caller rate limit is declared at the descriptor *and* enforced in the service.
- `crates/brassclaw_webui_v2/src/handlers.rs` — `get_interceptor_config:1338`,
  `update_interceptor_config:1349`, `reassemble_interceptor:1365`,
  `prewarm_interceptor:1381`. Each handler is a thin pass-through to
  `state.services().<method>(caller)`.
- `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` —
  the backend: `do_reassemble:204`, `reassemble_base_prompt:356`,
  `prewarm:395`, `build_snapshot:146`, keys `KEY_BASE_PROMPT:34` /
  `KEY_BASE_PROMPT_ASSEMBLED_AT:35` / `KEY_PREWARM_LAST_AT:37`.

## 3. Data model / route model

### 3.1 The descriptor-driven route contract

Every v2 route is an `IngressRouteDescriptor` returned by
`webui_v2_routes()` (`descriptors.rs`). A descriptor declares: route id,
method, path pattern, auth scheme, `BodyLimitPolicy`, rate-limit policy,
streaming mode (None / SSE / WebSocket), `AuditTraceClass`, and the
`AllowedEffectPath` (`ProductWorkflow` for mutations, `ProjectionOnly` for
reads). Host composition consumes this set and mounts each handler after
running **its own** bearer/CORS/body-limit/rate-limit middleware
(`16-kernel-composition.md` §5). The route layer enforces none of those
itself — by boundary, handlers consume only `RebornServicesApi` and may
not touch the dispatcher, `HostRuntime`, run-state, DB stores, or
capability hosts (`webui_v2/AGENTS.md` "Do Not Move In Here").

All HTTP errors travel through `WebUiV2HttpError` (redacted vocabulary) —
never hand-built `StatusCode` returns (`webui_v2/AGENTS.md`). The
descriptor contract is locked by `tests/webui_v2_descriptors_contract.rs`
(count/methods/patterns/auth/rate/SSE); the handler contract by
`tests/webui_v2_handlers_contract.rs`, which drives a real axum router
against a stub `RebornServicesApi` ("Test Through the Caller").

### 3.2 The settings tabs (today)

`SETTINGS_TABS` (`settings-schema.js`) lists 17 tabs. The chat-relevant
operator tabs and their backend reality (per `settings-api.js`):

| Tab id | Component | Backend | Status |
|--------|-----------|---------|--------|
| `inference` | `InferenceTab` | `/api/webchat/v2/llm/*` (providers, active, test-connection, list-models, NEAR AI/Codex login) | **live** |
| `tools` | `ToolsTab` | `/api/webchat/v2/tools`, `.../tools/{id}/permission` | **live** |
| `skills` | `SkillsTab` | `/api/webchat/v2/skills`, `.../install`, `.../{name}` (DELETE) | **live** (install/remove; list is the filesystem skill bundle, not the v3 DB catalog) |
| `actions` | `ActionsTab` | `/api/settings/actions` | Phase 6 (v1 settings endpoint) |
| `orchestrator` | `OrchestratorTab` | `/api/settings/orchestrators` | Phase 6 |
| `scaffold` | `ScaffoldTab` | `/api/settings/scaffolds` | Phase 6 |
| `monty-vm` | `MontyVmTab` | `/api/settings/monty-vm`, `.../restart`, `.../status` | Phase 6 |
| `validation-queue` | `ValidationQueueTab` | `/api/webchat/v2/validation-queue`, `.../count`, `/components/{class}/{id}/validate|reject` | **live** |
| `reliability` | `ReliabilityTab` | (settings) | Phase 6 |
| `interceptor` | `InterceptorTab` | `/api/webchat/v2/interceptor/config|reassemble|prewarm` | **live** ← holds the base-prompt compile UI today |
| `safety` | `SafetyPanel` | `/api/webchat/v2/safety/sensitive-paths|workspace-rules|blocked-paths` | **live** |
| `tokens` | `TokensTab` | `/api/webchat/v2/providers/{id}/tokens` | live (per-provider; global removed) |
| `users` | `UsersTab` | (stub: `TODO: requires v2 users endpoint`) | stub |
| `language` | `LanguageTab` | (client-side i18n) | live |

Sidebar visibility (`SETTINGS_SUB_ROUTES` in `routes.js`) unhides only
the tabs whose api libs call real endpoints; stubs stay hidden until
their contract lands.

### 3.3 The shipped base-prompt compile path (Prefix Tab)

> **Symbol note.** This section was first grounded against an older layout
> where the compile flow lived under `interceptor/reassemble|prewarm` routes
> (`do_reassemble`, `reassemble_base_prompt`, `prewarm`, `build_snapshot`). That
> path has since been **renamed + shifted into dedicated prefix routes** backed
> by `PgBasicPromptStore` (V063): `do_reassemble`→`do_assemble_bundle:193`,
> `reassemble_base_prompt`→`get_system_bundle:334`, `prewarm`→`regenerate_prefix:442`,
> `build_snapshot`→`snapshot:346`. The bundle format and SEC-01 validated-gate
> logic below are unchanged; only the names/routes/storage moved.

The Interceptor tab (`interceptor-tab.js`) currently owns three cards:

- **StatusCard** — `mode` (routing vs rerouting), `sempai_connected` badge,
  `base_prompt_assembled_at`, `base_prompt_size_chars`,
  `components_since_rebuild` (a "stale" hint), `prewarm_last_at`.
- **PersonaCard** — textarea editing the Sempai persona text (Part B),
  saved via `update_interceptor_config`.
- **ControlCard** — two buttons: **Reassemble** (assembles the base prompt
  from validated components) and **Pre-warm** (ships the assembled base
  prompt to the Sempai LLM to warm its KV cache). Pre-warm is disabled
  until `base_prompt_assembled_at` is set.

Backend (`interceptor_config_service.rs`):

- `do_reassemble:204` — for each table in `COMPONENT_TABLES`
  (`interceptor_config_service.rs:47`), `SELECT prompt_uid, name, content
  WHERE validation_status='validated' AND NOT ('05:validator' = ANY(consumer_tags))
  ORDER BY prompt_uid ASC LIMIT 1000`. Rows are sorted by
  `(class_code, prompt_uid)` and rendered as
  `## {class_code}:{prompt_uid}  {label}  "{name}"\n\n{content}`, then a
  literal **Sempai Response Schema** JSON block is appended so the Sempai
  knows the expected `SempaiReviewOutcome` shape even with no persona. The
  result is the base-prompt bundle string.
- `reassemble_base_prompt:356` — rate-limit check, `do_reassemble()`, then
  store the bundle under `brassclaw_config` key `interceptor.sempai_base_prompt`
  and the timestamp under `interceptor.sempai_base_prompt_assembled_at`.
- `prewarm:395` — rate-limit check, load the stored base prompt (error
  `BasePromptNotAssembled` if absent/empty), require a `sempai_gateway`,
  build a `HostManagedModelRequest` with a single `System` message whose
  content is the base prompt, and `gateway.stream_model(request)` it to
  the Sempai model (`ModelProfileId "sempai_model"`). On success, store
  `interceptor.sempai_prewarm_last_at`. This is the literal "send the
  prefix to the LLM for compilation" action.
- `build_snapshot:146` — assembles `InterceptorConfigSnapshot` (mode,
  sempai_connected, persona, base_prompt_assembled_at,
  base_prompt_size_chars, components_since_rebuild, prewarm_last_at).

**The bundle is stored in `reborn_basic_prompt_store` (V063)** via
`PgBasicPromptStore` — per `(tenant,user,agent,project)` scope, `fingerprint`
+ `bundle_json` JSONB + `is_stale` + `assembled_at`. The prior
`brassclaw_config` key-value string layout was retired with the V063 shift.

### 3.4 The Prefix Tab (shipped)

The dedicated store + the tab (user item 8) are both **shipped**:

**Migration V063** `reborn_basic_prompt_store`:

```sql
CREATE TABLE reborn_basic_prompt_store (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     TEXT NOT NULL,
    user_id       TEXT,
    agent_id      TEXT,
    project_id    TEXT,
    fingerprint   TEXT NOT NULL,   -- SHA-256 of bundle content
    bundle_json   JSONB NOT NULL,
    is_stale      BOOLEAN NOT NULL DEFAULT false,
    assembled_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, user_id, agent_id, project_id)
);
```

`PgBasicPromptStore` facade (`pg_basic_prompt_store.rs`): `get_for_scope:108`,
`store:151`, `mark_stale:208`, `compute_fingerprint:257`,
`minimal_base_prompt_fallback:307`, + crate-level `get_system_bundle:285`
(per-turn read). Any component `validated` transition → `mark_stale(scope)`.

The **Prefix Tab** (item 8) is the shipped UI over this store + the
assemble/regenerate actions, generalized from one prefix to a list:

- A list of prefixes. Today: one row, `base-prompt`. The schema is
  designed for more ("more will be added in the future" — item 8).
- Each row shows: name, fingerprint, `assembled_at`, `is_stale`,
  size, and last-compiled timestamp.
- A **Generate / Regenerate** button per prefix that, on click,
  (1) assembles that prefix's bundle from validated components
  (`do_assemble_bundle:193`) and (2) ships it to the LLM for compilation
  (`regenerate_prefix:442`) — one button per prefix, moved out of the
  Interceptor tab.
- The base-prompt compile endpoints are the prefix-named routes
  `GET /api/webchat/v2/prefixes` (list, `list_prefixes:1366`) +
  `POST /api/webchat/v2/prefixes/{name}/regenerate` (`regenerate_prefix:1387`),
  backed by `PgBasicPromptStore`. The Interceptor tab keeps the
  mode/persona/status surface but no longer owns the compile buttons.

### 3.5 The base-prompt placeholder substitution (§0.13)

The prefix is consumed at prompt-creation time, not at compile time. Per
`saved_plan_to_v3.md:1487-1500` (§0.13 KV-Cache / LMCache-Aware Design):

- The base prompt is a **pre-assembled `InstructionBundle`** stored in
  `reborn_basic_prompt_store` (V056). Manual trigger only. **Stale when
  any component passes Gate 2** (Q2 approval).
- The turn prompt carries a single `base-prompt` placeholder line while
  it is being composed. The Sempai-Kohai system replaces that placeholder
  with the real base-prompt content **at the very end of prompt creation**
  (`09-sempai-kohai.md`, `10-prefix-base-prompt.md`). If the base prompt
  was not precompiled (and so is not in the KV cache), the Sempai-Kohai
  system emits a **short minimal-context prompt-part** with only the most
  necessary information instead — because the LLM can only compute ~200
  tokens/s of *new* tokens, while prefix tokens are cached and free.
- `BuildInstruction` patch rules: the per-turn patch **must NOT repeat**
  content already in the stored base prompt. `basic_prompt_section_refs`
  carries navigation hints (pointers, not content), e.g.
  `→ see §ls-skill in basic-prompt` — the LLM already has the body from
  the KV cache. Target patch size **< 4k tokens** (fast new-token
  computation). Orchestrator patch = PRIORITY 2 (instruction snippets);
  Memory = PRIORITY 3 (memory snippets); Rust context is delivered
  directly by `RecipeStage`, not in the bundle at all.

This is why the Prefix Tab is a compile-time (operator) action, not a
per-turn action: the per-turn work is just the small patch + the
placeholder substitution.

### 3.6 SKILL.md export (item 5.1) — not yet implemented

The user's item 5.1 specifies that Classic Claude-style skills are
**DB-stored parts** (name, description, body, tool_name, param_schema,
activation criteria) with **no actual `SKILL.md` file**, but a `SKILL.md`
can be **exported via the WebUI on demand**. A grep for `SKILL.md` /
`skill.*export` across `crates/` finds only the v1 filesystem skill-bundle
import/export paths (`skill_import.rs`, `bundled_skills.rs`,
`skill_bundle_context_source.rs`) — **no v2 export endpoint exists
today**. This is a v3 addition: a `GET /api/webchat/v2/skills/{id}/export`
(or similar) route that renders the DB-stored parts into the Anthropic
SKILL.md format on demand. It is unimplemented and not yet on a numbered
phase — it is part of the same component-authoring UI work as the Prefix
Tab and the validation queue.

## 4. Behavior

### 4.1 A turn from the browser (chat)

1. Browser `POST /api/webchat/v2/threads/{id}/messages` (descriptor
   `webui.v2.send_message`, `AllowedEffectPath::TurnCoordinator`). Host
   composition has already converted `?token=` (SSE/WS) or `Authorization:
   Bearer` into a `WebUiAuthenticatedCaller` `Extension`.
2. Handler `send_message` calls `state.services().send_message(caller, ...)`
   on `RebornServicesApi`, which routes into the turn coordinator → the
   agent loop → orchestrator → intent → recipe/LLM
   (`MESSAGE_FLOW_AND_PLAN_AUDIT.md` §2, `12-agent-loop.md`,
   `13-orchestrator-default-py.md`).
3. The browser does **not** block on the POST. It opens
   `GET /api/webchat/v2/threads/{id}/events` (SSE) or `.../ws` (WebSocket)
   and drains `ProductOutboundEnvelope`s rendered as `WebChatV2EventFrame`s.
   The frame's projection cursor is the SSE `id`; reconnect resumes via
   `Last-Event-ID`. Both transports share one `(tenant,user)` `SseCapacity`
   pool (default 3 streams), and every stream is closed after a 5-minute
   max lifetime (`webui_v2/CLAUDE.md` "SSE resource caps").
4. `capability_activity` / `capability_display_preview` SSE frames carry
   only sanitized tool-activity DTOs (invocation_id, capability_id,
   status, bounded summaries ≤2 KiB / previews ≤16 KiB) — never raw
   args/results/paths. Full output stays behind the scoped `result_ref`
   fetch path.

### 4.2 Compiling a prefix (operator, the Prefix Tab target)

Today (Interceptor tab):

1. Operator opens Settings → Interceptor. `useInterceptor` calls
   `fetchInterceptorConfig` → `GET /api/webchat/v2/interceptor/config` →
   `build_snapshot`. StatusCard shows whether a base prompt is assembled.
2. Operator clicks **Reassemble**. `handleReassemble` →
   `POST .../interceptor/reassemble` → `reassemble_base_prompt` →
   `do_reassemble` (query validated components, render bundle) → store
   under `interceptor.sempai_base_prompt` + `_assembled_at`. The button
   is rate-limited to 1/min.
3. Operator clicks **Pre-warm** (enabled only once assembled).
   `handlePrewarm` → `POST .../interceptor/prewarm` → `prewarm` → load
   stored base prompt → `HostManagedModelRequest{ System: base_prompt }` →
   `gateway.stream_model` to the Sempai → store `_prewarm_last_at`.
4. The Sempai LLM now has the base prompt in its KV cache. Subsequent
   turns send only the small patch + the `base-prompt` placeholder (which
   Sempai-Kohai replaces, or falls back to a minimal-context part if the
   prefix was never compiled).

v3 target (Prefix Tab): steps 2+3 become one **Generate/Regenerate**
button per prefix, backed by `PgBasicPromptStore` (per-scope, versioned,
fingerprinted, `is_stale`), and the routes become prefix-named. The
"stale when any component passes Gate 2" invariant (§0.13) is what makes
the regenerate button meaningful: after Q2 approvals change the validated
catalog, the prefix is stale and the operator regenerates.

### 4.3 Authoring & reviewing a component (operator)

- **Authoring**: the actions/orchestrator/scaffold/monty-vm tabs hit
  `/api/settings/*` (Phase 6 v1 endpoints); tools/skills/validation-queue
  hit real `/api/webchat/v2/*`. A new skill/tool/recipe/PythonCode save
  creates a row at `validation_status='pending'` and (per Phase A.5 /
  `14-validation-queue.md`) enqueues it to `reborn_validation_queue`
  (state 1, Q1 queue).
- **Reviewing**: the Validation Queue tab (`validation-queue-tab.js`)
  lists queue rows; **Validate** (Q2 approve → graduation) and
  **Reject** call `/api/webchat/v2/components/{class}/{id}/validate|reject`.
  For LLM-audited classes (10 Orchestrator, 50 Scaffold) the Validate
  button is disabled until the backend LLM audit returns `clean` — the
  frontend mirrors the backend guard (the backend returns 403 if
  bypassed). Q2 approval drives the three side effects of §0.15
  (`14-validation-queue.md`): queue row deleted → `last_graduation_at`
  bumped; `validation_status='validated'`; SplitResult memo-cache evicted
  on next hit. **A graduation also marks the base prompt `is_stale`** —
  the Phase K.1 wire (`on any component validated → mark_stale(scope)`),
  which is what makes the Prefix Tab's regenerate button light up.

## 5. Relations

- **`09-sempai-kohai.md`** — the Sempai-Kohai interceptor is the runtime
  consumer of the compiled prefix: it substitutes the `base-prompt`
  placeholder (or emits the minimal-context fallback) at the end of
  prompt creation. The Prefix Tab compiles *what* the interceptor
  substitutes.
- **`10-prefix-base-prompt.md`** — the prefix-caching mechanism, the
  `reborn_basic_prompt_store` (V056) store, and the `do_reassemble`
  assembly. This doc is the UI surface over that subsystem.
- **`14-validation-queue.md`** — Q2 graduation marks the prefix stale
  (Phase K.1 `mark_stale`), which is the signal the Prefix Tab surfaces as
  "regenerate".
- **`15-component-catalog.md`** — `do_reassemble` reads exactly the
  validated catalog (`WHERE validation_status='validated' AND NOT
  '05:validator'`), ordered by `(class_code, prompt_uid)`. The prefix is
  the validated catalog reassembled.
- **`16-kernel-composition.md`** — host composition runs the
  bearer/CORS/body-limit/rate-limit middleware the descriptor declares;
  the route layer enforces none of it. The Interceptor/prewarm
  `AllowedEffectPath::ProductWorkflow` and 1/min rate limit are the
  kernel-level gates on prefix compilation.
- **`01-architecture-overview.md`** — the WebUI is the Product layer;
  the Prefix Tab is a Product-surface control over a Loop/Kernel
  subsystem (prefix caching).

## 6. Status — shipped vs. pending

| Aspect | Shipped | Pending |
|--------|---------|---------|
| Prefix Tab UI | dedicated **Prefix Tab** (`prefix` route, `usePrefixes` hook, `fetchPrefixes`/`regeneratePrefix` SPA API) listing prefixes (one now, more later), each with Generate/Regenerate | more prefixes beyond `base-prompt` |
| Base-prompt compile UI location | **shifted** to the Prefix Tab; Interceptor tab keeps mode/persona/status | — |
| Base-prompt storage | `reborn_basic_prompt_store` (V063): per `(tenant,user,agent,project)` scope, `fingerprint`, `bundle_json` JSONB, `is_stale`, `assembled_at` | — |
| Staleness signal | `is_stale` column, set by `mark_stale(scope)` on any component `validated` transition | — |
| Compile action | one Generate/Regenerate button per prefix (`do_assemble_bundle` + `regenerate_prefix` ships to LLM) | — |
| Compile routes | `GET /api/webchat/v2/prefixes` + `POST /api/webchat/v2/prefixes/{name}/regenerate` | — |
| Placeholder substitution | `base-prompt` placeholder line + Sempai-Kohai replacement + `minimal_base_prompt_fallback` (per-turn `get_system_bundle`) | — |
| SKILL.md export | — | on-demand `SKILL.md` export from DB-stored skill parts (item 5.1) — no endpoint yet |
| Settings tabs | 18 (`SETTINGS_TABS` incl. `prefix`) | — |

**What is shipped:** the Interceptor tab (mode/persona/status), the Prefix
Tab UI + `prefixes` routes, `do_assemble_bundle`/`regenerate_prefix`/
`get_system_bundle`, `reborn_basic_prompt_store` (V063) +
`PgBasicPromptStore`, `mark_stale`-on-graduation, and the `base-prompt`
placeholder substitution. **What is pending:** the SKILL.md export endpoint
(item 5.1) and additional prefixes beyond `base-prompt`.

## 7. LLM-summary (machine-convertible)

- The WebUI v2 is a React SPA (`brassclaw_webui_v2_static`) + a Rust route
  layer (`brassclaw_webui_v2`) + a host ingress
  (`brassclaw_reborn_webui_ingress`). Routes are descriptor-driven;
  handlers dispatch only to `RebornServicesApi`; the host runs
  bearer/CORS/body/rate-limit middleware; all errors are redacted via
  `WebUiV2HttpError`.
- The settings page has 18 tabs (`SETTINGS_TABS`) incl. `prefix`; the
  sidebar unhides only those with real endpoints.
- The **Prefix Tab** (item 8) is shipped: `GET /api/webchat/v2/prefixes`
  (`list_prefixes`) lists prefix-cache entries; `POST
  /api/webchat/v2/prefixes/{name}/regenerate` (`regenerate_prefix`)
  assembles the bundle from validated components (`do_assemble_bundle`) +
  ships it to the Sempai LLM as a System message to warm its KV cache.
  Backed by `reborn_basic_prompt_store` (V063: per-scope, fingerprinted,
  `is_stale`, `bundle_json`) + `PgBasicPromptStore`
  (`get_for_scope`/`store`/`mark_stale`/`compute_fingerprint`/
  `minimal_base_prompt_fallback`). Any component Q2-graduation →
  `mark_stale(scope)` → the regenerate button lights up. 1/min rate-limited.
- At turn time the prompt carries a `base-prompt` placeholder line; the
  Sempai-Kohai system substitutes the real prefix content at the very end
  of prompt creation (§0.13) via per-turn `get_system_bundle`, or emits a
  short minimal-context fallback (`minimal_base_prompt_fallback`) if the
  prefix was never compiled (the LLM computes only ~200 new tokens/s;
  prefix tokens are cached). The per-turn patch must not repeat base-prompt
  content; `basic_prompt_section_refs` are pointers; patch target < 4k
  tokens; orchestrator = PRIORITY 2, memory = PRIORITY 3, Rust context
  delivered by `RecipeStage` (not in the bundle).
- **SKILL.md export** (item 5.1): Classic skills are DB-stored parts with
  no `SKILL.md` file; export-on-demand is pending (no endpoint yet).
- Open work: add the SKILL.md export endpoint; additional prefixes beyond
  `base-prompt`.
