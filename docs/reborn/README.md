# Reborn Harness Map

Reborn is BrassClaw's host/runtime integration work. This page is the agent-facing map for Reborn harness, validation, and local evidence.

This page is intentionally short. Use it for progressive disclosure: start here, then follow the smallest relevant repo-local source instead of loading every Reborn file into context.

## Current Reborn sources in this branch

The `reborn-integration` branch currently exposes Reborn structure primarily through implementation crates, crate-local agent docs, tests, and CI guardrails.

| Need | Start with |
| --- | --- |
| Standalone Reborn binary | `docs/reborn-binary.md` |
| ProductAdapter architecture (channels/surfaces/host adapters) | `crates/brassclaw_product_adapters/` |
| Native ProductAdapter contract | `crates/brassclaw_product_adapters/CLAUDE.md` |
| Extending the agent with Tools/Recipes/Skills | `docs/extensions/building-a-tool.md` |
| Extension runtime lanes (Mcp / FirstParty / System) | `crates/brassclaw_extensions/` |
| Process/subprocess isolation for tools that need it | `crates/brassclaw_process_sandbox/` |
| Proposed subagent spawn design | `docs/reborn/subagent-spawn/README.md` |
| Host API vocabulary | `crates/brassclaw_host_api/` |
| Host API local rules | `crates/brassclaw_host_api/CLAUDE.md` |
| Host/runtime composition and shared runtime HTTP egress | `crates/brassclaw_host_runtime/` |
| Architecture dependency guardrails | `crates/brassclaw_architecture/` |
| Reborn dependency-boundary tests | `crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs` |
| Events substrate | `crates/brassclaw_events/` |
| Event projection read models | `crates/brassclaw_event_projections/` |
| Standalone durable event/audit stores | `crates/brassclaw_reborn_event_store/` |
| Filesystem substrate | `crates/brassclaw_filesystem/` |
| Network policy and HTTP transport substrate | `crates/brassclaw_network/` |
| Secrets metadata and one-shot leases | `crates/brassclaw_secrets/` |
| Resource governor substrate | `crates/brassclaw_resources/` |
| Authorization substrate | `crates/brassclaw_authorization/` |
| Approval substrate | `crates/brassclaw_approvals/` |
| Run-state substrate | `crates/brassclaw_run_state/` |
| Extension runtime lanes (Mcp / FirstParty / System) | `crates/brassclaw_extensions/` |
| Process/subprocess isolation (incl. docker-image validator) | `crates/brassclaw_process_sandbox/` |
| MCP server adapters and host-mediated HTTP/fail-closed process policy | `crates/brassclaw_mcp/` |
| Replay fixtures | `tests/fixtures/llm_traces/README.md` |
| Replay workflow | `.github/workflows/replay-gate.yml` |
| E2E test harness | `tests/e2e/README.md` |
| Live/replay testing guide | `tests/support/LIVE_TESTING.md` |

## Future Reborn contract docs

When the Reborn contract-doc packet is present in this branch, agents should prefer these docs as the source of truth:

```text
docs/reborn/contracts/_contract-freeze-index.md
docs/reborn/contracts/host-api.md
docs/reborn/contracts/capability-access.md
docs/reborn/contracts/dispatcher.md
docs/reborn/contracts/events-projections.md
docs/reborn/contracts/triggers.md
docs/reborn/contracts/memory.md
docs/reborn/contracts/secrets.md
docs/reborn/contracts/network.md
docs/reborn/contracts/skills-extension.md
docs/reborn/contracts/migration-compatibility.md
```

Until then, use the crate-local `CLAUDE.md` files, public crate APIs, and architecture tests as the branch-local source of truth.

## Harness docs

| Harness area | Doc |
| --- | --- |
| Local per-worktree environment | `docs/reborn/harness/local-dev.md` |
| Replay and compatibility fixtures | `docs/reborn/harness/replay.md` |
| Logs, events, traces, debug bundles | `docs/reborn/harness/observability.md` |

## Existing harness assets

Reborn should reuse the existing BrassClaw harness where possible:

- `scripts/replay-snap.sh`
- `scripts/trace-coverage.sh`
- `tests/fixtures/llm_traces/README.md`
- `tests/support/LIVE_TESTING.md`
- `.github/workflows/replay-gate.yml`
- `.github/workflows/reborn-e2e.yml`
- `.github/workflows/live-canary.yml`
- `scripts/check-boundaries.sh`
- `scripts/check_gateway_boundaries.py`
- `scripts/check_no_panics.py`

## Harness principles

1. Humans steer with issues, docs, plans, compatibility manifests, and acceptance criteria.
2. Agents execute with isolated worktrees, deterministic fixtures, replay traces, E2E artifacts, and mechanical guardrails.
3. `AGENTS.md` remains a quick-start map, not the full architecture spec.
4. Reborn details should live in repo-local docs, crate-local `CLAUDE.md` files, tests, and scripts.
5. Architecture boundaries should be mechanically enforced where possible.
6. Product-surface compatibility should be proven through replay, E2E, and compatibility evidence before cutover.

## Golden boundaries

Preserve these Reborn boundaries unless the relevant contract or architecture test is deliberately changed:

1. `brassclaw_host_api` stays vocabulary/contract-only.
2. `brassclaw_architecture` stays test-only architecture enforcement.
3. Low-level substrate crates should not depend upward on product/runtime orchestration.
4. Product flows should not bypass authorization, approval, resource, network, secret, or event boundaries.
5. Secrets and credential material must not appear in user-facing errors, logs, events, snapshots, or debug bundles.
6. Persistence behavior that becomes production-facing must preserve PostgreSQL/libSQL parity unless explicitly scoped otherwise.
7. Caller-level tests are required when a helper gates a side effect.

## Related tracking issues

- Reborn substrate/cutover parent: #2987
- Reborn compatibility gate: #3020
- Reborn product-surface migration: #3031
- Reborn lifecycle UX realignment: `docs/reborn/2026-05-24-3288-lifecycle-ux-realignment.md`
