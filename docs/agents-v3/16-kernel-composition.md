# 16 — Kernel / Authority & Composition / Wiring

> **Subsystem:** The two foundational layers of the three-layer model.
> The **Kernel** owns authority — trust decisions, secret resolution, safety
> policy, sandboxing, capability grants, and session identity — and its
> boundaries are *non-negotiable* from product or loop code. **Composition**
> (`brassclaw_reborn_composition`) is the production wiring root: the one
> place that constructs the substrate handles (`HostRuntime`,
> `TurnCoordinator`, the PG pool, the LLM catalog, the interceptor) and
> exposes them behind a small facade (`RebornServices` / `RebornRuntime`),
> resolves the runtime profile, and assembles the WebUI security
> middleware. This doc is the authority-and-wiring companion to
> `01-architecture-overview.md`.
> **Grounded in:** `AGENTS.md` (three-layer model, Subagent/Loop Rules,
> Security Invariants, Database Rules, Environment Variables),
> `crates/brassclaw_reborn_composition/CLAUDE.md` + `AGENTS.md`
> (`factory.rs`, `runtime.rs`, `webui.rs`, `local_runtime_profile.rs`),
  the kernel-crate `AGENTS.md` files (`brassclaw_safety`, `brassclaw_trust`,
  `brassclaw_secrets`, `brassclaw_capabilities`, `brassclaw_runtime_policy`),
  `Goals_pre_v3_review.md` (Goal 1/2 verdict, lines 428-440).

## 1. Purpose

The three-layer model (Products / Loops / Kernel) is the routing map of
the whole codebase. This doc covers the bottom two layers:

- **Kernel — owns authority.** It controls trust decisions, secret
  resolution, safety policy enforcement, sandboxing, capability grants,
  and session identity. *Kernel boundaries are not negotiable from
  product or loop code.* Any change touching listeners, routes, auth,
  secrets, sandboxing, approvals, or outbound HTTP is reviewed with a
  security mindset; none may weaken bearer-token auth, webhook auth,
  CORS/origin checks, body limits, rate limits, allowlists, or
  secret-handling guarantees.
- **Composition — owns wiring.** `brassclaw_reborn_composition` is the
  *single* facade-shaped production composition root. It builds the
  substrate handles, exposes only `HostRuntime` / `TurnCoordinator` /
  product-auth / WebUI / readiness, keeps lower substrate handles
  private to factories, and fails closed on local-only or missing
  required handles in production/migration-dry-run profiles.

The user's higher goals land here concretely:
- **"No more installation (runtime) profiles — one full installation
  path"** (Goal 1): the old `RebornCompositionProfile` enum is gone;
  `BRASSCLAW_RUNTIME_PROFILE` survives only as a *capability-policy*
  selector that tunes the security resolver, never the storage backend.
- **"Postgres is mandatory"** (Goal 2): composition always wires the PG
  pool; the in-memory/filesystem production fallbacks were removed.

## 2. Location

### Kernel (authority) crates

| Crate | Owns | Does NOT own |
|-------|------|--------------|
| `brassclaw_safety` | prompt-injection detection, input validation, sanitization, safety-policy evaluation, sensitive-path helpers, manual credential detection, secret-leak scanning (the **Q1 injection scan**) | sandbox execution, credential storage, network allowlists, tool dispatch, agent loops, UI; never logs/returns raw secrets in findings |
| `brassclaw_trust` | host-controlled trust evaluation: `EffectiveTrustClass`/`TrustDecision`/`AuthorityCeiling`/`HostTrustAssignment` (privileged `FirstParty`/`System` variants are *crate-internal* to construct), `TrustPolicy`/`HostTrustPolicy`, layered sources (`AdminConfig`/`BundledRegistry`), synchronous fail-closed invalidation (`InvalidationBus`/`TrustChange`) | treating trust as a grant/bypass, package execution, extension storage, capability dispatch |
| `brassclaw_secrets` | scoped secret storage + credential brokering: `SecretStore` trait, `SecretLease`/one-shot consumption, `CredentialAccount`/`CredentialSession`, `SecretsCrypto` + AAD constructors, filesystem-backed stores | raw secret material in errors/events/debug/snapshots/docs or provider HTTP beyond mediated handoff |
| `brassclaw_capabilities` | the single caller-facing `CapabilityHost` authority path: invoke/resume/spawn requests, the **obligation seam** (`CapabilityObligationHandler`), capability-profile conformance evaluation | parallel dispatch paths, dispatch *before* authorization/obligations/approval gates |
| `brassclaw_authorization` / `brassclaw_auth` | bearer-token + OAuth + product-auth authorization | — |
| `brassclaw_runtime_policy` | runtime-profile resolver + runtime selection policy (`EffectiveRuntimePolicy`, `RuntimeProfile`, `DeploymentMode`) | runtime process startup, action dispatch |
| `brassclaw_process_sandbox` / `brassclaw_host_runtime` | sandboxing + host-runtime shell access (sandboxed subprocess path via `services/process_executor` + `sandbox_process/`; the v1 `services/script_runtime` lane was removed in Phase 4) | — |
| `brassclaw_reborn_identity` | session identity | — |
| `brassclaw_outbound` | outbound HTTP (treats external services as untrusted) | — |
| `brassclaw_approvals` | approval gates | — |

### Composition crate (`brassclaw_reborn_composition`)

- `factory.rs` — `RebornServices` / `build_reborn_services`,
  `build_pg_runtime_stores`, `RebornBuildInput`/`RebornBuildError`, the
  LLM catalog resolvers (`llm_catalog`). Holds the `pg_pool` threaded from
  the production build path.
- `runtime.rs` — `RebornRuntime` / `build_reborn_runtime`
  (`:204`), `ConversationId`, `AssistantReply`, `RebornRuntimeError`. The
  conversation-level facade; its struct fields include `turn_coordinator`,
  `thread_service`, `thread_scope`, `worker_handle`, `trigger_poller`,
  `projection_services`, `approval_interaction_service`,
  `auth_interaction_service`, `plan_library`, `llm_reload`, and the
  Sempai on/off `interceptor_mode: SharedInterceptorMode` (cfg-gated to
  `postgres` + `root-llm-provider`).
- `webui.rs` — `RebornWebuiBundle`, `build_webui_services`,
  `webui_v2_app` (the composed axum `Router` + security middleware stack).
- `local_runtime_profile.rs` — `local_runtime_build_input`,
  `local_dev_runtime_policy`, `local_dev_yolo_runtime_policy`.
- `profile.rs` / `readiness.rs` — production/migration-dry-run profile
  validation for required handles.
- `interceptor_config_service.rs` — the Sempai base-prompt assembler
  (`do_assemble_bundle` / `SystemBundleSource::get_system_bundle`, see
  `10-prefix-base-prompt.md`).

## 3. Kernel Authority Model

The kernel's job is to answer *may this happen?* before any loop or
product code can make it happen. Every authority decision is host-controlled
and fail-closed.

- **Trust** (`brassclaw_trust`): the trust vocabulary (`EffectiveTrustClass`,
  `TrustDecision`, `AuthorityCeiling`) decides how much authority a caller/
  extension is granted. Privileged variants (`FirstParty`, `System`) are
  *crate-internal* to construct — product/loop code cannot mint them.
  `InvalidationBus` propagates `TrustChange` synchronously so a revoked
  trust assignment takes effect before the next capability call (fail-closed
  invalidation, no polling lag).
- **Secrets** (`brassclaw_secrets`): `SecretStore` brokers scoped secret
  material with one-shot `SecretLease` consumption (a leased secret can be
  used once, then it is gone — no replay). `SecretsCrypto` + AAD binds each
  ciphertext to its scope/record kind. Raw secret material never appears in
  errors, events, snapshots, logs, or docs.
- **Safety** (`brassclaw_safety`): prompt-injection detection and the
  safety policy gate run on untrusted input *before* storage or LLM
  injection. This is the technical backstop behind the validation queue's
  Q1 scan (see `14-validation-queue.md`). Bounded, linear-time pattern
  matching — no backtracking regex on untrusted input.
- **Capabilities** (`brassclaw_capabilities`): `CapabilityHost` is the
  *single* caller-facing authority path. Dispatch runs *after*
  authorization + obligation + approval gates — never before, never on a
  parallel path. The obligation seam (`CapabilityObligationHandler`)
  tracks that a capability was actually attempted/fulfilled.
- **Identity / sandbox / outbound** (`brassclaw_reborn_identity`,
  `brassclaw_process_sandbox`, `brassclaw_outbound`): session identity is
  kernel-owned; sandboxing isolates process execution; outbound HTTP treats
  Docker containers and external services as untrusted.

**Non-negotiable boundary rules** (from `AGENTS.md`):

- Product adapters, product workflow, first-party capabilities, and
  host-runtime handlers must use **untrusted inbound requests** and must
  not mint `TrustedInboundTurnRequest` or call trusted trigger submitter
  factories. Host-trusted trigger ingress is sealed by trigger-worker-owned
  request minting + private conversation-owned trusted inbound construction.
- Session, thread, and turn state matters. Submission parsing happens
  *before* normal chat handling.
- Skills are selected deterministically. Tool approval and auth flows are
  special paths and must not be mixed into normal chat history.
- Persistent memory is the workspace system, not just transcript storage.
- Subagent spawn creates and wires child runs only — it must not implement
  a second agent loop. Child planning, execution, capability calls,
  checkpointing, gates, retries, and completion must go through the
  existing loop runner/driver/executor path.

## 4. Composition / Wiring

Composition is where the substrate handles are built and where the runtime
profile is resolved. It owns *only top-level composition*; lower substrate
handles stay private to factories.

### Factory + runtime

`build_reborn_services` (`factory.rs:531`) is the production entry: it
takes `RebornBuildInput`, builds the PG-backed stores
(`build_pg_runtime_stores` — the `pg_pool` is threaded from the production
build path so every store is Postgres-backed), and returns `RebornServices`
exposing `HostRuntime`, `TurnCoordinator`, `secret_store()`, `pg_pool()`,
and readiness. `build_reborn_runtime` lifts that into the
conversation-level `RebornRuntime` facade (`new_conversation`, send/turn
drive, trigger poller, projection).

Two profile-validation guarantees:
- **Production and migration-dry-run profiles fail closed** on local-only
  or missing required handles.
- **Substrate handles are private** except via
  `#[cfg(any(test, feature = "test-support"))]` accessors that ship zero
  bytes in production binaries (each must name the production call site it
  mirrors).

### Runtime profile (Goal 1 resolution)

The old **`RebornCompositionProfile`** enum — the *installation* profile
that selected the storage backend — is **removed**. Setting
`BRASSCLAW_REBORN_PROFILE` (the old composition-profile name) is a **hard
startup error**.

The retained **`BRASSCLAW_RUNTIME_PROFILE`** is a *per-invocation
capability policy*, **not** an installation profile:

| Value | Meaning |
|-------|---------|
| `local_dev` (default) | local development, restricted host access |
| `local_safe` | local, locked-down capability set |
| `local_yolo` | trusted single-user local dev with inherited host env access (`confirm_host_access = true`) |
| `hosted_safe` | non-local hosted deployment (requires `BRASSCLAW_PG_URL`) |

It is resolved by `brassclaw_runtime_policy` into an
`EffectiveRuntimePolicy` (`local_runtime_profile.rs`:
`local_runtime_build_input` defaults to `RuntimeProfile::LocalDev`;
`local_dev_yolo_runtime_policy` for yolo). It controls the **security
resolver only** — which capabilities are granted, how sandboxing is
applied. It does **not** affect which storage backend is used: **Postgres
is always used** regardless of profile (Goal 2). `BRASSCLAW_PG_URL` is
optional for single-host local deployments (embedded Postgres on port
5434) and required for all non-local profiles.

### WebUI security middleware (`webui_v2_app`)

Composition owns the WebUI v2 HTTP gateway security stack. Inbound order:

1. Static security headers (`X-Content-Type-Options: nosniff`,
   `X-Frame-Options: DENY`, CSP).
2. `CorsLayer` — allow-origin from config; **empty list fails closed**
   (no echoing attacker-supplied origin).
3. `CatchPanicLayer` — panic boundary.
4. Outer `RequestBodyLimitLayer` (14 MiB default) — defense in depth for
   unmatched paths.
5. Descriptor-driven per-route body limit — reads each route's
   `BodyLimitPolicy` (exhaustive `match`, so a new variant fails the build
   rather than silently disabling enforcement) and enforces it *before*
   auth runs.
6. WS same-origin enforcement — inline on descriptors with a non-
   `NotApplicable` `WebSocketOriginPolicy` (the browser cannot pre-flight
   WS upgrades); mismatch → `403` before the upgrade.
7. Bearer auth + `?token=` shim — `Authorization: Bearer` on every route;
   `?token=` honored *only* on the `EventSource` GET (browsers cannot set
   headers there); mutations stay bearer-only.
8. Descriptor-driven per-route rate limit — sliding window; authenticated
   routes use `PerCaller`, the public OAuth callback uses `PerIp` backed
   by `ConnectInfo<SocketAddr>` (never `X-Forwarded-For`/`X-Real-IP`).
   Composition fails closed if a future descriptor declares an unsupported
   scope.

This stack is the composition-layer expression of the kernel's
"do not weaken auth/CORS/body-limits/rate-limits/origin" invariant.

## 5. Relations

- **Kernel → Composition:** composition *uses* kernel authority ports
  (`SecretStore`, `CapabilityHost`, trust policy, safety scan) but never
  re-implements them. Product auth composition must use `brassclaw_auth`
  trait-shaped ports — never V1 OAuth routes, pending maps,
  `ExtensionManager`, or route-local raw HTTP clients.
- **Composition → Loops:** `RebornRuntime` hands `TurnCoordinator` +
  `HostRuntime` to the loop driver. The Sempai on/off `interceptor_mode`
  flag (flipped by the settings service when an operator connects a Sempai
  provider) is consumed by `RebornLoopDriverHost` on every turn to decide
  routing vs rerouting (see `09-sempai-kohai.md`).
- **Composition → Retrieval/prefix:** composition wires
  `PostgresSource` (Phase E.0 — shipped, the active production retrieval
  backend via `PgRetrievalLookup`) and `PgBasicPromptStore` (V063 — shipped);
  `interceptor_config_service::do_assemble_bundle` /
  `SystemBundleSource::get_system_bundle` builds the Sempai base prompt from
  `COMPONENT_TABLES` (see `10`/`11`). `RamSource` (`PgMemoryDocStore`,
  keyword-over-Postgres) is engine-internal and dormant in production —
  deleted in Phase K.3.
- **Kernel → Validation queue:** `brassclaw_safety` is the Q1 injection
  scan; `gate1_pass` visibility (`pub(crate)` in composition) enforces
  the state-2 write invariant (see `14-validation-queue.md`).
- **Kernel → Triggers:** host-trusted trigger ingress is sealed by
  trigger-worker-owned request minting + conversation-owned trusted
  inbound construction; product adapters use untrusted inbound only.

## 6. Status — shipped vs. pending

| Aspect | Shipped | Pending |
|--------|---------|---------|
| Installation profiles | `RebornCompositionProfile` removed; `BRASSCLAW_REBORN_PROFILE` hard-error; `BRASSCLAW_RUNTIME_PROFILE` retained as capability policy only (Goal 1 **done**) | — |
| Storage backend | Postgres always; silent in-memory production fallbacks removed; filesystem fallback annihilated | — |
| Retrieval backend | `PostgresSource` wired (Phase E.0) — the active production backend via `PgRetrievalLookup` | `RamSource` (`PgMemoryDocStore`, keyword-over-Postgres) dormant in prod — deleted Phase K.3 |
| Base-prompt store | `PgBasicPromptStore` (V063) + `do_assemble_bundle`/`get_system_bundle` | — |
| Validation queue | `reborn_validation_queue` (V051) + Q1 scan (`brassclaw_safety` + `component_validator`) | — |
| Loop host ports | `LoopRetrievalPort` + `LoopOrchestratorPort` host impls (H.0) — 15 ports now | — |
| Composition wiring | `factory`/`runtime`/`webui` facades + `PgRetrievalLookup`/`PgOrchestratorLookup` | — |
| WebUI middleware | full security stack live (headers/CORS/body/WS-origin/bearer/rate); Prefix Tab routes (`/api/webchat/v2/prefixes`) run under the same stack | — |
| Kernel boundaries | non-negotiable, unchanged | — |

**The kernel is not a v3 migration target.** v3 adds subsystems (validation
queue, prefix/base-prompt store, recipe/IBS, intent upgrade) *inside* the
existing authority model — every new component enters the Q1 queue, every new
prefix is a DB row, every new retrieval path goes through the SEC-01
`validation_status = 'validated'` gate. The composition layer gains wiring for
these subsystems but the facade shape and the security middleware stack are
already production-grade.

## 7. LLM Summary (machine-convertible)

The Kernel owns authority and its boundaries are non-negotiable from
product/loop code: `brassclaw_trust` (host-controlled trust decisions;
privileged `FirstParty`/`System` variants are crate-internal; fail-closed
synchronous `InvalidationBus`), `brassclaw_secrets` (scoped secret storage
with one-shot `SecretLease` consumption; `SecretsCrypto`+AAD; raw material
never in errors/logs/docs), `brassclaw_safety` (prompt-injection detection +
safety policy — the Q1 scan — run before storage/LLM injection; bounded
linear-time matching), `brassclaw_capabilities` (the single
`CapabilityHost` authority path; dispatch only after
authorization+obligation+approval gates; no parallel paths),
`brassclaw_runtime_policy` (profile resolver), `brassclaw_process_sandbox`/
`brassclaw_host_runtime` (sandbox + sandboxed subprocess path), `brassclaw_reborn_identity`
(session identity), `brassclaw_outbound` (untrusted external HTTP),
`brassclaw_approvals` (gates). Boundary rules: product adapters/first-party
capabilities/host-runtime handlers use untrusted inbound requests and
cannot mint `TrustedInboundTurnRequest`; submission parsing precedes chat;
skills selected deterministically; tool approval/auth are special paths
not mixed into chat history; subagent spawn wires child runs only (no
second loop). Composition (`brassclaw_reborn_composition`) is the single
facade-shaped wiring root: `build_reborn_services`/`RebornServices`
(factory.rs, builds PG-backed stores, exposes `HostRuntime`/
`TurnCoordinator`/`secret_store`/`pg_pool`/readiness), `RebornRuntime`
(runtime.rs, conversation-level facade, holds turn_coordinator/
thread_service/plan_library/llm_reload/Sempai `interceptor_mode`),
`webui_v2_app` (the axum Router + security middleware: static headers,
fail-closed CORS, CatchPanic, descriptor-driven body limits enforced
before auth, WS same-origin, bearer+`?token=`-shim auth, descriptor-driven
rate limits). Runtime profile: the old `RebornCompositionProfile`
installation profile is removed (`BRASSCLAW_REBORN_PROFILE` is a hard
startup error — Goal 1 fully accomplished); the retained
`BRASSCLAW_RUNTIME_PROFILE` (`local_dev`/`local_safe`/`local_yolo`/
`hosted_safe`) is a per-invocation *capability policy* that tunes the
security resolver only and does NOT select the storage backend — Postgres
is always used (Goal 2 partially accomplished: fallbacks removed,
`PgMemoryDocStore` live; `RamSource`→`PostgresSource` swap deferred to v3
Phase E.0/K.3). The kernel is not a v3 migration target; v3 subsystems are
added inside the existing authority model (every component enters Q1,
every prefix is a DB row, every retrieval path keeps the
`validation_status='validated'` SEC-01 gate). **Status:** kernel authority
crates and composition facades are live and production-grade; v3 adds
wiring (PostgresSource E.0, PgBasicPromptStore K.1, ValidationQueueStore/
q1_orchestrator A.5, LoopRetrievalPort/LoopOrchestratorPort H.0) but no
boundary changes.
