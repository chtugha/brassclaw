# Agent Map — brassclaw_extensions

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
- Use these Reborn contracts as the source of truth before changing behavior:
- `docs/reborn/contracts/extensions.md`
- `docs/reborn/contracts/kernel-boundary.md`
- `docs/reborn/contracts/capability-access.md`

## What This Crate Owns

- Declarative extension manifest, registry, lifecycle, and trust inputs (no execution, network, secrets, or WASM/script/MCP inspection), currently:
- Manifest discovery/validation and asset-path containment: `ExtensionError`, `ExtensionAssetPath` (`lib.rs`); the in-memory `ExtensionRegistry` (`registry`).
- Lifecycle: `ExtensionLifecycleEvent`, `ExtensionLifecycleEventSink`, `ExtensionLifecycleService` (`lifecycle`).
- The v2 manifest schema (`v2`): `ExtensionManifestV2`, `CapabilityDeclV2`, `ExtensionRuntimeV2`, `ManifestSource`, `CapabilityVisibility`, `ManifestV2Error`, and the schema-version/size constants.
- The host-API manifest contract projection (`v2`): `HostApiContractRegistry`, `HostApiManifestContract`, `HostApiRefV2`, `HostApiManifestProjection`; plus the capability-provider host-API contract (`host_api/capability_provider`).
- Crate-local public API, tests, and fixtures needed to prove that ownership.

## Do Not Move In Here

- direct authority grants or runtime-specific execution logic; use capabilities/authorization/trust and lane crates.
- Secrets, raw host paths, backend error details, and unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_extensions`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`
- If production persistence behavior changes, add/maintain PostgreSQL and libSQL parity tests.

## Agent Notes

- Keep edits inside this crate unless a contract explicitly requires a neighboring crate change.
- Prefer caller-level tests when a helper gates dispatch, persistence, network, secrets, approvals, resources, events, or process side effects.
- If the contract and code disagree, stop and treat the task as a contract-change request instead of silently changing ownership.
