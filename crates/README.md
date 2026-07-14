# BrassClaw crates

This directory contains the Rust crates that split BrassClaw into smaller, reviewable boundaries. Most crates are Reborn system-service crates: they hold one slice of host authority, storage, policy, runtime composition, or product UI glue.

Use this page as a human map before opening individual crate docs or source files.

## Mental model

BrassClaw Reborn keeps authority narrow and explicit:

1. **Contracts describe authority**: `brassclaw_host_api` and adjacent contract crates define scoped identities, policies, requests, decisions, and DTOs.
2. Policy gates decide: authorization, trust, runtime policy, resources, approvals, secrets, safety, filesystem, network, hooks, and prompt-envelope crates each own one kind of decision, label, or side effect.
3. Capability hosts coordinate: capabilities, dispatcher, processes, scripts, MCP, WASM, shared WASM limiting, and host-runtime crates compose validated requests into sandboxed execution.
4. State is durable and replayable: events, event streams, run state, threads, conversations, memory, outbound, product workflow storage, traces, and event projections keep the host observable without leaking secrets.
5. Product surfaces adapt: agent loop, engine, loop support, gateway, WebChat v2, TUI, skills, first-party extensions, product adapters, and Reborn composition crates turn those lower-level boundaries into agent and user experiences.

A good rule of thumb: if a change adds new authority or persistence, put it in the crate that owns that boundary instead of threading it through a UI or runtime crate.

## Crate groups

### Core vocabulary and shared contracts

| Crate directory | Package | Human context |
| --- | --- | --- |
| `brassclaw_common` | `brassclaw_common` | Shared workspace types and utilities that are not authority-bearing enough to belong in `brassclaw_host_api`. Keep this small. |
| `brassclaw_host_api` | `brassclaw_host_api` | Canonical Reborn authority vocabulary: actors, scopes, policies, capability requests, decisions, obligations, and host-facing data contracts. Runtime behavior belongs elsewhere. |
| `brassclaw_prompt_envelope` | `brassclaw_prompt_envelope` | Leaf helper for wrapping trusted or untrusted prompt snippets with closed-vocabulary source/trust labels and rejecting instruction-hijack markers. |
| `brassclaw_runtime_policy` | `brassclaw_runtime_policy` | Resolves runtime profiles from host configuration and policy inputs. Use it when choosing what runtime shape a capability may use. |
| `brassclaw_architecture` | `brassclaw_architecture` | Workspace architecture contract tests. It has no production role; it fails builds when crate dependency boundaries drift. |

### Authority, safety, and policy gates

| Crate directory | Package | Human context |
| --- | --- | --- |
| `brassclaw_authorization` | `brassclaw_authorization` | Evaluates host API authority contracts before capability execution. It should not execute work, reserve resources, or prompt users. |
| `brassclaw_approvals` | `brassclaw_approvals` | Resolves durable approval requests and issues scoped authorization leases. It does not own prompting UI or runtime execution. |
| `brassclaw_trust` | `brassclaw_trust` | Host-controlled trust-class policy engine. Use it for decisions about how much trust a runtime, extension, or input receives. |
| `brassclaw_resources` | `brassclaw_resources` | Resource reservation governor. Owns budget/reservation mechanics, not runtime dispatch. |
| `brassclaw_auth` | `brassclaw_auth` | Product-facing Reborn auth setup contracts: auth-flow records, secure manual-token interactions, credential accounts, provider exchange, continuations, cleanup, and fakes. |
| `brassclaw_safety` | `brassclaw_safety` | Prompt-injection defense, input validation, secret-leak detection, and safety policy enforcement. |
| `brassclaw_secrets` | `brassclaw_secrets` | Tenant-scoped secret storage and leasing behind opaque `SecretHandle` values. It stores/leases material; other crates decide when leases are allowed and where to inject them. |
| `brassclaw_network` | `brassclaw_network` | Network policy and HTTP egress boundary. Resolves DNS, rejects disallowed/private targets when configured, and owns host-mediated outbound HTTP. |
| `brassclaw_filesystem` | `brassclaw_filesystem` | Scoped filesystem service. Use it for host-controlled path access, not direct runtime path handling. |
| `brassclaw_hooks` | `brassclaw_hooks` | Reborn loop hook framework. Owns trust-tiered hook contracts, predicates, ordering, dispatch, and failure policy; hooks cannot grant authority. |

### Capability execution and runtime lanes

| Crate directory | Package | Human context |
| --- | --- | --- |
| `brassclaw_capabilities` | `brassclaw_capabilities` | Caller-facing capability invocation host. Coordinates authorization, approvals, run-state transitions, and neutral runtime dispatch. |
| `brassclaw_dispatcher` | `brassclaw_dispatcher` | Composition-only runtime dispatch contracts. Wires validated extension descriptors to runtime lanes; it does not parse manifests or grant authority. |
| `brassclaw_processes` | `brassclaw_processes` | Host-tracked background process lifecycle. Owns lifecycle mechanics, not capability policy. |
| `brassclaw_mcp` | `brassclaw_mcp` | Adapts manifest-declared MCP tools into BrassClaw capabilities without granting ambient filesystem, secret, or network authority. |
| `brassclaw_extensions` | `brassclaw_extensions` | Extension manifest, lifecycle, and registration contracts. Owns install/activate/remove semantics; runtime crates consume validated descriptors from here. |
| `brassclaw_host_runtime` | `brassclaw_host_runtime` | Narrow facade upper Reborn services depend on. Provides `HostRuntime` plus production composition around capability hosting. Also hosts the in-kernel script lane (Docker-backed executor) so the script runtime is never a separate crate. |

### Durable state, eventing, and read models

| Crate directory | Package | Human context |
| --- | --- | --- |
| `brassclaw_events` | `brassclaw_events` | Redacted runtime/audit vocabulary plus durable append-log traits. Use it for observable history, not current state. |
| `brassclaw_reborn_event_store` | `brassclaw_reborn_event_store` | Concrete Reborn event/audit store backends and backend-profile validation. Depends on `brassclaw_events`; keeps storage adapters out of event vocabulary. |
| `brassclaw_event_projections` | `brassclaw_event_projections` | Product-facing read models over durable runtime and audit logs. Upper layers should consume these DTOs rather than parse event rows directly. |
| `brassclaw_event_streams` | `brassclaw_event_streams` | Transport-neutral projection stream manager: admission, bounded subscription buffers, replay/live updates, lag signals, and redaction validation. |
| `brassclaw_run_state` | `brassclaw_run_state` | Current lifecycle state for host-managed invocations. Events are history; run state answers “what is happening now?” |
| `brassclaw_threads` | `brassclaw_threads` | Canonical session thread and transcript service contracts. Use it for durable thread/transcript ownership. |
| `brassclaw_conversations` | `brassclaw_conversations` | Conversation binding and session-thread contracts that connect product conversation concepts to Reborn threads. |
| `brassclaw_memory` | `brassclaw_memory` | Memory document service adapters. This is for workspace/memory document semantics, not arbitrary transcript deletion. |
| `brassclaw_outbound` | `brassclaw_outbound` | Metadata-only outbound state: notification policy, projection subscription cursors, and delivery status. It does not own transport delivery or payload content. |
| `brassclaw_reborn_traces` | `brassclaw_reborn_traces` | Trace Commons / TraceDAO client surface: contribution pipeline, trace client, redaction helpers, and conversation-message compatibility type. |

### Product, agent loop, and user surfaces

| Crate directory | Package | Human context |
| --- | --- | --- |
| `brassclaw_reborn` | `brassclaw_reborn` | Standalone Reborn composition and adapters. This is the high-level Reborn composition crate. |
| `brassclaw_reborn_composition` | `brassclaw_reborn_composition` | Wiring layer that assembles Reborn services into the host runtime. Composition-only; no policy or persistence logic of its own. |
| `brassclaw_reborn_config` | `brassclaw_reborn_config` | Reborn boot-config boundary: typed configuration, profiles, and validation consumed before services start. |
| `brassclaw_reborn_cli` | `brassclaw_reborn_cli` | Reborn-first CLI surface (command modules, completion, shell entry points). Calls into composition; does not own host policy. |
| `brassclaw_reborn_webui_ingress` | `brassclaw_reborn_webui_ingress` | Host-owned listener binding, authenticator implementations, and serve loop for the Reborn WebChat v2 HTTP gateway. |
| `brassclaw_llm` | `brassclaw_llm` | LLM provider routing and abstraction used by Reborn product surfaces and the agent loop. |
| `brassclaw_agent_loop` | `brassclaw_agent_loop` | Agent-loop framework state, planner/executor, strategy/family contracts, and test support. |
| `brassclaw_loop_support` | `brassclaw_loop_support` | Adapts durable Reborn support boundaries into the narrow agent-loop host port. It should not own provider clients or runtime dispatchers. |
| `brassclaw_turns` | `brassclaw_turns` | Host-layer turn coordination contracts. Use it for turn lifecycle boundaries between loop/product code and host services. |
| `brassclaw_first_party_extensions` | `brassclaw_first_party_extensions` | Concrete first-party userland extension implementations behind scoped handles. |
| `brassclaw_first_party_extension_ports` | `brassclaw_first_party_extension_ports` | Loop-facing adapters for first-party extensions: skill activation/context/execution ports over loop-support and turn-run contracts. |
| `brassclaw_product_adapters` | `brassclaw_product_adapters` | Product-adapter contracts for mapping Reborn state and events into product-facing shapes. |
| `brassclaw_product_adapter_registry` | `brassclaw_product_adapter_registry` | ProductAdapter host-api projection and installation registry. |
| `brassclaw_product_workflow` | `brassclaw_product_workflow` | Product-facing workflow facade: inbound turn service, idempotency ledger, binding resolution. |
| `brassclaw_product_workflow_storage` | `brassclaw_product_workflow_storage` | Durable libSQL/PostgreSQL adapters for the product workflow idempotency ledger. |
| `brassclaw_engine` | `brassclaw_engine` | Unified thread-capability-CodeAct execution engine. It is closer to product/agent orchestration than low-level host policy. |
| `brassclaw_skills` | `brassclaw_skills` | Skill selection, scoring, and management. |
| `brassclaw_gateway` | `brassclaw_gateway` | Browser gateway frontend assets, layout configuration, and widget extension system. |
| `brassclaw_webui_v2` | `brassclaw_webui_v2` | Reborn WebChat v2 HTTP route surface and route descriptors. Off by default; enable with `webui-v2-beta`. |
| `brassclaw_tui` | `brassclaw_tui` | Modular Ratatui-based terminal UI. |
| `brassclaw_telegram_v2_adapter` | `brassclaw_telegram_v2_adapter` | Telegram v2 channel adapter for the Reborn product surface. Maps Telegram traffic into Reborn capability and turn contracts. |
| `brassclaw_silk_decoder` | `brassclaw_silk_decoder` | Standalone WeChat `audio/silk` decoder helper. Excluded from the default workspace build; needs `libclang` and a C toolchain. |

## Where to make common changes

- **New capability type or host API contract**: start in `brassclaw_host_api`, then update authorization/capability/runtime crates that consume it.
- **Authorization or approval behavior**: use `brassclaw_authorization` for policy decisions and `brassclaw_approvals` for approval lease resolution.
- **Secret storage or leasing**: use `brassclaw_secrets`; do not put SQL or crypto details in engine, gateway, or runtime lanes.
- **Network or filesystem access**: use `brassclaw_network` or `brassclaw_filesystem`; runtimes should ask host services instead of bypassing them.
- **WASM, MCP, or script execution**: use the corresponding runtime-lane crate plus `brassclaw_capabilities`/`brassclaw_dispatcher` for coordination.
- **Hook behavior or prompt snippet trust labeling**: use `brassclaw_hooks` for hook contracts/dispatch and `brassclaw_prompt_envelope` for model-facing snippet wrapping.
- **Extension lifecycle (install/activate/remove)**: use `brassclaw_extensions`; do not parse manifests or reimplement registration in runtime or UI crates.
- **Reborn composition or boot config**: use `brassclaw_reborn_composition` and `brassclaw_reborn_config`; keep `main.rs`/CLI entry points thin.
- **LLM provider routing**: use `brassclaw_llm`; do not wire provider clients directly into engine or gateway crates.
- **Channel adapters (e.g., Telegram)**: use the channel adapter crate (`brassclaw_telegram_v2_adapter`); keep authority in lower host crates.
- **Durable event history**: use `brassclaw_events` for contracts and `brassclaw_reborn_event_store` for backend adapters.
- **Current invocation state**: use `brassclaw_run_state`, not event logs.
- **User-visible read models and live projection streams**: prefer `brassclaw_event_projections`, `brassclaw_event_streams`, or `brassclaw_product_adapters` over parsing storage rows in UI code.
- **Product workflow persistence**: keep orchestration in `brassclaw_product_workflow` and durable ledger adapters in `brassclaw_product_workflow_storage`.
- **Agent loop/product orchestration**: use `brassclaw_agent_loop`, `brassclaw_loop_support`, `brassclaw_turns`, `brassclaw_engine`, or `brassclaw_reborn` depending on layer.
- **Web or terminal UI**: use `brassclaw_gateway`, `brassclaw_webui_v2`, `brassclaw_reborn_webui_ingress`, or `brassclaw_tui`; keep authority and persistence in lower crates.

## Boundary rules

- Keep crate-owned logic in the owning crate. Avoid reimplementing module-specific setup in `src/main.rs`, `src/app.rs`, gateway, or TUI code.
- Prefer extending existing traits and service boundaries over adding one-off integration paths.
- Do not give runtime lanes ambient access to secrets, filesystem, network, or process control. Route through host services.
- Treat `brassclaw_host_api` as the shared contract layer. It may define authority-bearing shapes; it should not perform side effects.
- Use `brassclaw_architecture` tests when dependency boundaries need to become enforceable.
- If behavior changes, check `../CLAUDE.md`, `../AGENTS.md`, and `../FEATURE_PARITY.md` for test/doc update expectations.

## Quick commands

From the repository root:

```bash
cargo fmt
cargo clippy --all --benches --tests --examples --all-features
cargo test
```

For targeted crate work, prefer the narrowest command first:

```bash
cargo test -p brassclaw_secrets --features libsql
cargo clippy -p brassclaw_network --tests -- -D warnings
```

Some crates are feature-gated or test backends conditionally. Read the crate-level docs and tests before assuming a command covers PostgreSQL, libSQL, WASM, or integration behavior.
