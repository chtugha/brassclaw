# Agent Map — brassclaw_product_workflow

## Recipe architecture routing

Follow [the binding Recipe authoring contract](../../recipe.md) and root AGENTS.md for
v3 component work. Recipes instruct the orchestrator using reusable Tools,
ToolSkills, Skills and small PythonCode components. One referenced component
per Recipe step; internal PythonCode composition is allowed. IBS pins approved
immutable versions in BuildInstruction at task start, including nested includes;
Recipes reference stable UUIDs without versions. Preserve typed inputs/results
and task state across steps and waits. Replacement versions do not alter active
tasks; current global Tool policy still applies at every dispatch. Older scoped,
source-substitution or fresh-step descriptions below are implementation/legacy
notes, not permission to extend those paths as the v3 target. These requirements
remain subject to the documented runtime implementation gaps.

## Start Here

- Read `CLAUDE.md` first; it is the crate-local guardrail file.
- Read `Cargo.toml` for actual dependencies and feature shape.
- Use these local contracts as the source of truth before changing behavior:
- `tests/product_workflow_contract.rs`
- `tests/approval_interaction_contract.rs`
- `tests/inbound_turn_contract.rs`
- `tests/webui_inbound_contract.rs`
- `tests/reborn_services_contract.rs`

## What This Crate Owns

- Product-facing Reborn workflow orchestration between product adapters and host-layer services.
- Binding resolution, inbound message staging, turn submission, idempotency, busy/deferred handling, gate routing, and product-safe acknowledgements.
- The WebUI-facing Reborn facade over thread, turn, and projection ports.
- Crate-local public API, tests, and fakes needed to prove that ownership.

## Do Not Move In Here

- Dispatcher, extensions, host runtime, MCP, WASM, scripts, network, engine, or gateway dependencies.
- Product adapter transport/rendering logic, host runtime execution, capability dispatch, or storage backend details.
- Raw secrets, raw host paths, backend error details, or unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_product_workflow`
- Lint check: `cargo clippy -p brassclaw_product_workflow --all-targets -- -D warnings`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture reborn_crate_dependency_boundaries_hold`

## Agent Notes

- Keep product adapters thin; adapter-specific code should not reimplement workflow ownership from this crate.
- User-message acceptance must persist canonical thread content through `brassclaw_threads::SessionThreadService` before turn submission.
- Do not return a successful product acknowledgement unless the inbound action has a durable terminal ledger outcome.
- Prefer caller-level tests when helpers gate ledger settlement, thread mutation, turn submission, gate resolution, projection access, or other side effects.
