# BrassClaw Crates Map

Instructions for AI coding assistants entering `crates/` on `main`.

This file is a routing map, not a full architecture spec. Pick the crate(s) that match the change, then read crate-local guidance before editing:

1. `crates/<crate>/AGENTS.md` when present.
2. `crates/<crate>/CLAUDE.md` if present.
3. `crates/<crate>/CONTRACT.md` or `README.md` if present.
4. Matching `docs/reborn/contracts/*.md` when behavior crosses crate boundaries.

Do **not** eagerly load every crate guide. Use this map to choose.

## Branch and Workspace

This map was refreshed from `main` after inspecting the workspace crate manifests, source layout, tests, and crate-local docs. Most crates have a crate-local `AGENTS.md`; when one is missing, load `CLAUDE.md`, `Cargo.toml`, and `src/lib.rs` instead.

Run crate work from repo root unless crate-local docs say otherwise.

```bash
cargo test -p <crate_name>
cargo clippy -p <crate_name> --all-targets --all-features -- -D warnings
cargo test -p brassclaw_architecture
scripts/check-boundaries.sh
scripts/reborn-e2e-rust.sh
```

Use targeted crate tests first. Add `brassclaw_architecture` when dependency edges or layer ownership change. Run Reborn e2e when turns, runtime lanes, host services, authorization, approvals, networking, secrets, product workflow, or capability dispatch change.

## Guidance Files

- `AGENTS.md` — crate-local agent entrypoint; read first.
- `CLAUDE.md` — crate guardrails/spec; read before changing behavior.
- `CONTRACT.md` — public cross-crate contract; update with semantic changes.
- `README.md` — helper/user/operator details.
- `docs/reborn/contracts/*.md` — Reborn source-of-truth contracts.
- `crates/brassclaw_architecture` — mechanical dependency-boundary enforcement.

Treat crate-local `AGENTS.md` as the first file to load when it exists. Current workspace crates without one include `brassclaw_hooks`, `brassclaw_prompt_envelope`, and `brassclaw_reborn_traces`.

## Dependency Mental Model

Keep lower layers neutral. Product and runtime composition flows downward through typed contracts, not concrete shortcuts.

```text
common / host_api / prompt_envelope
  -> filesystem / memory / events / event_projections / event_streams / extensions / trust / resources
  -> secrets / network / outbound / run_state / authorization / approvals / runtime_policy / hooks
  -> host_runtime (also hosts the script lane) / processes / dispatcher / runtime lanes (mcp, host_runtime::script_runtime)
  -> turns / threads / agent_loop / loop_support / capabilities
  -> reborn composition / product adapters / product workflow / product workflow storage / CLI
  -> engine / llm / gateway / webui_v2 / webui_ingress / tui / root product integration
```

Boundary rule: if you need an upstream crate in a low-level crate, stop and check `crates/brassclaw_architecture` plus matching Reborn contract.

## Crate Map

### Foundation and substrate

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_common` | `brassclaw_common/AGENTS.md`, `Cargo.toml` | Low-dependency shared types/utilities: app events, identity, trust-boundary helpers, paths, platform/env/timezone, attachment helpers. | Runtime orchestration, persistence, clients, policy, product domain logic. |
| `brassclaw_host_api` | `brassclaw_host_api/AGENTS.md`, `brassclaw_host_api/CLAUDE.md`, `docs/reborn/contracts/host-api.md` | Neutral authority vocabulary: IDs, scopes, paths, actions, decisions, resources, approvals, audit, HTTP, dispatch, runtime-policy, trust types. | Runtime execution, persistence, HTTP clients, product workflow, policy engines. |
| `brassclaw_prompt_envelope` | `Cargo.toml`, `src/lib.rs` | Leaf prompt-envelope helper: wraps model-visible snippets with closed-vocabulary source/trust labels, size limits, and instruction-hijack rejection. | Runtime orchestration, model routing, policy decisions, or free-form source labels. |
| `brassclaw_architecture` | `brassclaw_architecture/AGENTS.md`, `brassclaw_architecture/CLAUDE.md` | Workspace architecture tests, Reborn dependency boundaries, composition-boundary checks. | Production runtime code or production deps. |

### Files, memory, events, projections

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_filesystem` | `brassclaw_filesystem/AGENTS.md`, `brassclaw_filesystem/CLAUDE.md`, `docs/reborn/contracts/filesystem.md` | Root/scoped/composite filesystem, catalog, virtual path authority, backend containment, mount routing. | Memory-domain grammar, network/secrets/dispatcher/product workflow. |
| `brassclaw_memory` | `brassclaw_memory/AGENTS.md`, `brassclaw_memory/CLAUDE.md`, `docs/reborn/contracts/memory.md` | Memory docs, `/memory` paths, metadata/schema, chunking, embeddings, search, indexer hooks, memory filesystem adapter, backend contracts. | Generic mount/catalog logic or product workflow. |
| `brassclaw_events` | `brassclaw_events/AGENTS.md`, `brassclaw_events/CLAUDE.md`, `docs/reborn/contracts/events.md` | Typed redacted event/audit substrate, event envelopes, sinks/log traits, durable adapters. | SSE/WebSocket/product transport or projection policy. |
| `brassclaw_event_projections` | `brassclaw_event_projections/AGENTS.md`, `brassclaw_event_projections/CLAUDE.md`, `docs/reborn/contracts/events-projections.md` | Event projection model, cursor/visibility contracts, product-facing projection boundaries. | Canonical event storage or transport delivery. |
| `brassclaw_event_streams` | `brassclaw_event_streams/AGENTS.md`, `brassclaw_event_streams/CLAUDE.md`, `docs/reborn/contracts/events-projections.md` | Transport-neutral projection stream manager: admission, bounded subscription buffers, live/replay update delivery, lag/rebase signals, redaction validation. | Axum/SSE/WebSocket framing, product workflow submission, durable event-store adapters, raw runtime payloads. |
| `brassclaw_reborn_event_store` | `brassclaw_reborn_event_store/AGENTS.md`, `docs/reborn/contracts/events.md` | Reborn-owned durable event/audit store backends and fixtures. | Product projections, transport fanout, workflow policy. |
| `brassclaw_reborn_traces` | `Cargo.toml`, `src/lib.rs` | Trace Commons / TraceDAO client surface: contribution pipeline, trace client, redaction helpers, conversation-message compatibility, and trace preview re-exports. | Reborn CLI command behavior, LLM provider routing, unredacted trace submission. |

### Authority, policy, state

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_trust` | `brassclaw_trust/AGENTS.md`, `brassclaw_trust/CLAUDE.md`, `brassclaw_trust/CONTRACT.md` | Host-controlled trust classes, policy sources, requested-vs-effective trust, invalidation. | Authorization grants, runtime dispatch, product workflow. |
| `brassclaw_authorization` | `brassclaw_authorization/AGENTS.md`, `brassclaw_authorization/CLAUDE.md` | Grant matching, leases, dispatch/spawn authorization decisions, DB-backed auth state. | Execution, approvals, run-state persistence, prompting. |
| `brassclaw_approvals` | `brassclaw_approvals/AGENTS.md`, `brassclaw_approvals/CLAUDE.md` | Exact-invocation approval requests, leases, resume coordination, approval events. | Reusable broad approvals or dispatch before fingerprinted lease claim. |
| `brassclaw_run_state` | `brassclaw_run_state/AGENTS.md`, `brassclaw_run_state/CLAUDE.md` | Durable invocation state and approval request records. | Authorization policy, approval resolution, dispatch, runtime execution, process lifecycle. |
| `brassclaw_resources` | `brassclaw_resources/AGENTS.md`, `brassclaw_resources/CLAUDE.md` | Reservation, reconciliation, release, quota accounting. | Runtime dispatch, product workflow, hidden costed work without reservation. |
| `brassclaw_auth` | `brassclaw_auth/AGENTS.md`, `brassclaw_auth/CLAUDE.md`, `docs/reborn/contracts/auth-product.md` | Product-facing Reborn auth-flow, secure interaction, credential account, provider exchange, continuation, cleanup contracts and fakes. | V1 route handlers/pending maps, durable secret storage, raw provider HTTP, runtime injection, extension lifecycle mutation. |
| `brassclaw_runtime_policy` | `brassclaw_runtime_policy/AGENTS.md`, `brassclaw_runtime_policy/CLAUDE.md`, `docs/reborn/contracts/runtime-profiles.md` | Runtime profile resolver and runtime selection policy. | Runtime startup, action dispatch, product strategy outside selection. |
| `brassclaw_outbound` | `brassclaw_outbound/AGENTS.md`, `brassclaw_outbound/CLAUDE.md` | Metadata-only outbound egress policy, notification opt-in, projection subscription cursors, delivery attempt/status metadata. | Transport sends, concrete Slack/Telegram/Web payload validation, transcript/projection mutation. |
| `brassclaw_hooks` | `brassclaw_hooks/CLAUDE.md`, `Cargo.toml`, `src/lib.rs` | Reborn loop hook framework: trust-tiered hook contracts, sealed decision sinks, predicates, ordering, dispatch, telemetry, and failure policy. | Authority grants, runtime-policy bypasses, ambient secrets/network/filesystem handles, extension installation. |

### Host services and runtime lanes

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_secrets` | `brassclaw_secrets/AGENTS.md`, `brassclaw_secrets/CLAUDE.md` | Secret metadata, encrypted repositories, leases, one-shot consumption, legacy/db stores. | Raw secret exposure, provider HTTP, injection beyond mediated handoff. |
| `brassclaw_network` | `brassclaw_network/AGENTS.md`, `brassclaw_network/CLAUDE.md`, `docs/reborn/contracts/network.md` | Network policy boundary, URL targets, resolver, hardened transport, host/provider HTTP egress. | Runtime-lane behavior above boundary or manual credential injection. |
| `brassclaw_host_runtime` | `brassclaw_host_runtime/AGENTS.md`, `brassclaw_host_runtime/CLAUDE.md` | Host-side Reborn service composition: production services, obligations, HTTP egress, redaction, secrets/network/resource mediation. | Product workflow, runtime-specific request shapes, duplicate network/secret logic. |
| `brassclaw_processes` | `brassclaw_processes/AGENTS.md`, `brassclaw_processes/CLAUDE.md` | Process lifecycle, cancellation, stores, status/output helpers, `ProcessHost`, wrappers. | Authorization, approval policy, runtime lane internals beyond adapter contracts. |
| `brassclaw_dispatcher` | `brassclaw_dispatcher/AGENTS.md`, `brassclaw_dispatcher/CLAUDE.md` | Already-authorized runtime routing through `RuntimeAdapter`, redacted dispatch results, event dispatch contracts. | Authorization, approvals, run-state, concrete runtime deps, product workflow. |
| `brassclaw_mcp` | `brassclaw_mcp/AGENTS.md`, `brassclaw_mcp/CLAUDE.md` | MCP runtime lane, execution request/result types, JSON-RPC exchange, client abstraction, HTTP adapter, resource accounting. | Direct outbound networking, ad-hoc credential injection, product workflow. |

### Turns, threads, loops, engine

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_turns` | `brassclaw_turns/AGENTS.md`, `brassclaw_turns/CLAUDE.md` | Host-layer turn coordination: requests/responses, coordinator, runner, run profiles, loop exit, memory/context handoff, turn store. | Product adapter rendering, raw runtime lanes, UI behavior. |
| `brassclaw_threads` | `brassclaw_threads/AGENTS.md`, `brassclaw_threads/CLAUDE.md` | Canonical session thread/transcript service contracts, identifiers, tool-result references, db/in-memory stores. | Product delivery policy or model/provider behavior. |
| `brassclaw_conversations` | `brassclaw_conversations/AGENTS.md`, `brassclaw_conversations/CLAUDE.md` | Conversation binding, session thread contracts, inbound/state store, libSQL/Postgres conversation persistence. | Capability runtime internals or UI transport. |
| `brassclaw_agent_loop` | `brassclaw_agent_loop/AGENTS.md`, `brassclaw_agent_loop/CLAUDE.md` | Agent-loop framework state, planner/executor, strategy/family contracts, test support. | Product adapters, transport, concrete provider auth. |
| `brassclaw_loop_support` | `brassclaw_loop_support/AGENTS.md`, `brassclaw_loop_support/CLAUDE.md` | Loop host support services: capability/input ports, allow sets, input queue, identity/skill context, cancellation. | Owning core loop strategy or runtime lane execution. |
| `brassclaw_capabilities` | `brassclaw_capabilities/AGENTS.md`, `brassclaw_capabilities/CLAUDE.md` | Caller-facing `CapabilityHost` invoke/resume/spawn workflow, obligation seams, conformance helpers. | Process lifecycle APIs, direct concrete runtime dependencies. |
| `brassclaw_engine` | `brassclaw_engine/AGENTS.md`, `brassclaw_engine/CLAUDE.md`, `brassclaw_engine/MONTY.md` | Thread/capability/CodeAct engine: runtime manager, executor, gates, leases, memory retrieval, workspace mounts, traits/types. | Product transport, provider-specific auth, lower-layer host policy shortcuts. |

### Product, adapters, Reborn binary

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_reborn` | `brassclaw_reborn/AGENTS.md`, `brassclaw_reborn/CLAUDE.md` | Standalone Reborn composition/adapters: driver registry, home/profile/doctor support, runtime composition seams. | V1 root runtime imports unless explicitly bridged. |
| `brassclaw_reborn_config` | `brassclaw_reborn_config/AGENTS.md`, `Cargo.toml`, `src/lib.rs` | Boot configuration contracts for standalone Reborn binary. | Runtime execution or product adapter behavior. |
| `brassclaw_reborn_composition` | `brassclaw_reborn_composition/AGENTS.md`, `brassclaw_reborn_composition/CLAUDE.md` | Facade-shaped production composition root for Reborn. | Low-level policy internals that belong to service crates. |
| `brassclaw_first_party_extensions` | `brassclaw_first_party_extensions/AGENTS.md`, `Cargo.toml` | Concrete first-party userland extension implementations and deterministic tool behavior behind scoped handles. | Host runtime composition, loop-facing ports, ambient runtime authority, dispatcher/network/secrets handles. |
| `brassclaw_first_party_extension_ports` | `brassclaw_first_party_extension_ports/AGENTS.md`, `Cargo.toml` | Loop-facing adapters for first-party extensions: skill activation/context/execution ports over loop-support and turn-run contracts. | Concrete tool behavior, host runtime composition, product workflow, raw host authority. |
| `brassclaw_reborn_cli` | `brassclaw_reborn_cli/AGENTS.md` | Standalone Reborn CLI, command files, CLI context, shell completions, doctor/home/profile commands. | V1 runtime imports, root `brassclaw` deps, side effects in pure commands. |
| `brassclaw_product_adapters` | `brassclaw_product_adapters/AGENTS.md`, `brassclaw_product_adapters/CLAUDE.md` | Product-adapter contracts: adapter trait, auth, egress, identity, workflow, external/projection/inbound, redaction, fakes. | Host runtime internals or specific WASM runner implementation. |
| `brassclaw_product_adapter_registry` | `brassclaw_product_adapter_registry/AGENTS.md`, `brassclaw_product_adapter_registry/CLAUDE.md` | ProductAdapter host-api projection and installation registry. | Adapter execution or product workflow orchestration. |
| `brassclaw_product_workflow` | `brassclaw_product_workflow/AGENTS.md`, `brassclaw_product_workflow/CLAUDE.md` | Product-facing workflow facade: inbound turns, bindings, ledger, workflow/errors, Reborn service bridges. | Low-level runtime lane internals or direct provider-specific transports. |
| `brassclaw_product_workflow_storage` | `brassclaw_product_workflow_storage/AGENTS.md`, `Cargo.toml` | Durable libSQL/PostgreSQL adapters for the product workflow idempotency ledger. | Workflow orchestration, direct dispatch, or divergence between libSQL and PostgreSQL behavior. |
| `brassclaw_telegram_v2_adapter` | `brassclaw_telegram_v2_adapter/AGENTS.md`, `Cargo.toml`, `src/lib.rs` | Telegram ProductAdapter tracer bullet: payload parsing, rendering, adapter implementation. | Shared adapter contracts or registry semantics. |
| `brassclaw_reborn_webui_ingress` | `brassclaw_reborn_webui_ingress/AGENTS.md`, `Cargo.toml` | Host-owned listener binding, authenticator implementations, and serve loop for Reborn WebChat v2. | Product/API route semantics, transcript storage, v1 channel code, product adapter transport shims. |

### LLM, skills, safety, UI, helpers

| Crate | Load first | Owns / go here for | Avoid moving in |
| --- | --- | --- | --- |
| `brassclaw_llm` | `brassclaw_llm/AGENTS.md`, `brassclaw_llm/CLAUDE.md`, `brassclaw_llm/Cargo.toml` | Multi-provider LLM integration: provider trait, auth, registry, retry/failover/circuit breaker/cache, tool schemas, reasoning, tracing, transcription/vision. | Engine loop ownership or product workflow. |
| `brassclaw_skills` | `brassclaw_skills/AGENTS.md` | Skill catalog, parser, gating, selector/scoring, registry, validation, v2 skill types. | Agent-loop execution or UI command routing. |
| `brassclaw_safety` | `brassclaw_safety/AGENTS.md`, `crates/brassclaw_safety/fuzz/README.md` | Prompt-injection detection, validation, sanitization, safety policy, sensitive paths, credential detection, leak scanning, fuzz/benches. | Sandbox execution, credential storage/injection, network allowlists, dispatch, UI decisions. |
| `brassclaw_gateway` | `brassclaw_gateway/AGENTS.md` | Gateway frontend assets, layout config, bundle metadata, widget extension system. | Browser API/web channel runtime (`src/channels/web/`) or product workflow. |
| `brassclaw_webui_v2` | `brassclaw_webui_v2/AGENTS.md`, `brassclaw_webui_v2/CLAUDE.md` | Reborn WebChat v2 route descriptors, axum handlers, schemas, and redacted HTTP error shape behind `webui-v2-beta`. | Bearer validation, CSRF/origin/rate-limit middleware, direct runtime/DB access, unredacted responses. |
| `brassclaw_tui` | `brassclaw_tui/AGENTS.md`, `brassclaw_tui/CLAUDE.md` | Ratatui app, widgets, layout, render, theme, event/input loop, spinner. | Main crate channel bridge (`src/channels/tui.rs`) or backend workflow. |
| `brassclaw_silk_decoder` | `brassclaw_silk_decoder/AGENTS.md`, `brassclaw_silk_decoder/README.md`, `brassclaw_silk_decoder/Cargo.toml`, `brassclaw_silk_decoder/src/main.rs` | Excluded helper binary that decodes WeChat SILK v3 voice notes to WAV. | Main workspace build dependencies; keep libclang isolated. |

## Common Change Routes

- Host API shape: `brassclaw_host_api` -> matching `docs/reborn/contracts/*.md` -> affected service/runtime crates -> `brassclaw_architecture`.
- Storage and persistence: owning domain crate for schemas/queries; preserve libSQL/PostgreSQL parity where applicable. Product workflow ledger adapters live in `brassclaw_product_workflow_storage`; event/audit store backends live in `brassclaw_reborn_event_store`.
- Files/memory: `brassclaw_filesystem` for mount/path authority; `brassclaw_memory` for memory documents/search/chunking/indexing.
- Events/projections/outbound: `brassclaw_events` for canonical redacted events; `brassclaw_event_projections` for projection model; `brassclaw_event_streams` for transport-neutral live/replay streams; `brassclaw_outbound` for metadata-only delivery/subscription policy; adapters for concrete delivery.
- Trust/auth/approval: `brassclaw_trust` -> `brassclaw_authorization` -> `brassclaw_run_state`/`brassclaw_approvals` -> `brassclaw_capabilities` as needed.
- Hooks and prompt context: `brassclaw_hooks` for hook registration/dispatch/failure policy; `brassclaw_prompt_envelope` for model-visible untrusted or trust-labeled snippet wrapping.
- Runtime execution: lane crate (`mcp` or `script` execution via `host_runtime`'s script lane) first; `dispatcher` for routing; `host_runtime` for secrets/network/resources/redaction; `processes` for background lifecycle. The script lane lives inside `brassclaw_host_runtime` (no separate crate) to keep dispatcher composition private to the kernel.
- Turns/agent loop: `brassclaw_turns` for turn coordination; `brassclaw_agent_loop` for strategy/planner/executor contracts; `brassclaw_loop_support` for host support ports; `brassclaw_engine` for CodeAct/thread runtime.
- Product adapter flow: `brassclaw_product_adapters` contracts -> `brassclaw_product_adapter_registry` installation/projection -> `brassclaw_product_workflow` orchestration -> concrete adapter crate.
- Reborn binary/composition: `brassclaw_reborn_config` for boot config; `brassclaw_reborn_composition` for production wiring; `brassclaw_reborn_cli` for commands; `brassclaw_reborn` for standalone adapters/driver registry; `brassclaw_reborn_webui_ingress` for host-owned WebChat v2 listener lifecycle.
- Model/provider behavior: `brassclaw_llm`; do not leak provider auth/cache/retry concerns into engine or product workflow.
- UI presentation: `brassclaw_tui`, `brassclaw_gateway`, or `brassclaw_webui_v2`; backend API/web channel code remains under root `src/` unless the surface is the Reborn WebChat v2 route crate.

## Testing

Prefer narrow tests during iteration:

```bash
cargo test -p brassclaw_host_api
cargo test -p brassclaw_network network_policy_contract
cargo test -p brassclaw_outbound --all-features
cargo test -p brassclaw_product_workflow
```

Then expand by risk:

```bash
cargo test -p brassclaw_architecture
scripts/check-boundaries.sh
scripts/reborn-e2e-rust.sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Persistence behavior must support PostgreSQL and libSQL where applicable. If local Postgres is unavailable, follow crate-local skip flags only when docs/tests explicitly permit them.

## Guardrails

- Avoid `.unwrap()` / `.expect()` in production; use typed errors with context.
- Preserve tenant/user/agent/project/mission/thread scope on authority, state, memory, process, network, outbound, resource, and event records.
- Fail closed for auth, approvals, trust, filesystem containment, network policy, secret leases, runtime selection, and adapter identity.
- Do not expose raw secrets, backend paths, private URLs, transport internals, raw SQL/backend errors, or unredacted runtime/user content across public surfaces.
- Keep runtime crates untrusted: host-runtime mediates secrets/network/redaction/accounting.
- Keep declarative crates declarative: manifests, contracts, registries, and policy descriptions should not perform execution side effects.
- Use existing traits/ports/registries; avoid hardcoded cross-crate shortcuts.
- Test through caller when a helper gates dispatch, persistence, network, secrets, approvals, resources, events, process, adapter, or UI side effects.

## Docs / Parity Checklist

Behavior changes may require updates to:

- crate-local `AGENTS.md`, `CLAUDE.md`, `CONTRACT.md`, or `README.md`
- `docs/reborn/contracts/*.md`
- `FEATURE_PARITY.md`
- crate changelogs for packages that publish independently
- architecture boundary tests in `crates/brassclaw_architecture`
