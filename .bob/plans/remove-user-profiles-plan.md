# Plan: Remove Boot Profile System — One Unified BrassClaw

## Problem Statement

BrassClaw currently has a `BRASSCLAW_REBORN_PROFILE` env var (4 values: `local-dev`,
`local-dev-yolo`, `production`, `migration-dry-run`) that causes the binary to behave
fundamentally differently depending on which "profile" is active: different storage
backends, different security policies, different feature availability. The user wants
ONE binary that installs and runs identically for everyone — no profile branching.

---

## Root Cause: One Enum Doing Two Jobs

The `RebornProfile` / `RebornCompositionProfile` enum conflates two orthogonal concerns:

| Job | What it controls | Right mechanism |
|-----|-----------------|-----------------|
| **Storage shape** | In-memory vs. libSQL vs. Postgres | Auto-detect from DB URL |
| **Security policy** | Approval gates, filesystem scope, isolation | `RuntimeProfile` (already exists) |
| **Migration mode** | Read-only schema validation run | CLI flag `--dry-run` |

These should be independent config knobs, not a single profile string.

---

## Architecture of the Two Systems

### Layer 1 — RebornProfile / RebornCompositionProfile (TO BE REMOVED)

```
BRASSCLAW_REBORN_PROFILE env var
        ↓
RebornProfile (4 variants)               crates/brassclaw_reborn_config/src/profile.rs
        ↓ composition_profile()          crates/brassclaw_reborn_cli/src/runtime/mod.rs:541
        ↓
RebornCompositionProfile (5 variants)    crates/brassclaw_reborn_composition/src/profile.rs
        ↓ match arm in build_reborn_services()
        ├── LocalDev | LocalDevYolo  → build_local_dev()      in-memory/libSQL storage
        └── Production | MigrationDryRun → build_production_shaped()  durable storage only
```

### Layer 2 — RuntimeProfile (TO BE KEPT ENTIRELY)

```
BRASSCLAW_RUNTIME_PROFILE env var (NEW) → RuntimeProfile (12 variants)
        ↓ brassclaw_runtime_policy::resolve()
        ↓ inputs: DeploymentMode, RuntimeProfile, yolo_disclosure_acknowledged, org_policy
        ↓
EffectiveRuntimePolicy          crates/brassclaw_host_api/src/runtime_policy.rs
  • ApprovalPolicy: AskAlways | AskWrites | AskDestructive | Minimal | OrgPolicy
  • FilesystemBackendKind: ScopedVirtual → HostWorkspace → HostWorkspaceAndHome
  • ProcessBackendKind: None → LocalHost → TenantSandbox → OrgDedicatedRunner
  • NetworkMode: Brokered | DirectLogged | Direct | Allowlist
  • SecretMode: BrokeredHandles | ScrubbedEnv | TenantBroker | InheritedEnv | OrgBroker
  • AuditMode: LocalMinimal | Standard | OrgPolicy
```

**The runtime policy resolver is pure, fail-closed, and security-critical. Nothing in it changes.**

---

## What the Target State Looks Like

```
BRASSCLAW_DB_URL (optional)      BRASSCLAW_RUNTIME_PROFILE (optional, default: local_dev)
         ↓                                        ↓
   URL scheme routing                resolver::resolve()
   ┌──── absent ──── in-memory / local libSQL file
   ├── libsql://...  → durable libSQL
   └── postgres://...→ durable Postgres          ↓  (yolo needs --confirm-host-access)
         ↓                               EffectiveRuntimePolicy
         └──────────────────────────────────────→ build_local_dev()  (single code path)
```

One binary. Storage is determined by whether you provide a DB URL.
Security policy is determined by which `RuntimeProfile` you select (default = `LocalDev` = AskDestructive).
Migration dry-run becomes `brassclaw-reborn migrate --dry-run`.

---

## What Is Preserved (Security Properties Unchanged)

All of the following remain byte-for-byte identical:

- `RuntimeProfile` enum and all 12 variants
- `brassclaw_runtime_policy::resolve()` — fail-closed resolver
- `yolo_disclosure_acknowledged` gate on the `ResolveRequest`
- `admin_approves_dedicated_yolo` gate for `EnterpriseYoloDedicated`
- `DeploymentMode` family gating (`Local*` only for `LocalSingleUser`, etc.)
- `validate_production_libsql_target()` — rejects non-TLS remote libSQL URLs
- `enforce_remote_ssl_mode()` — rejects `sslmode=disable` Postgres URLs
- `OrgPolicyConstraints::max_profile` ceiling narrowing
- The `--confirm-host-access` CLI flag that maps to `yolo_disclosure_acknowledged`

The production storage fail-closed guard **moves** from "profile == Production" to
"URL scheme is remote" — which is strictly stronger (you need a real DB URL to get
durable storage, not just an env var string).

---

## Files to Change

### Files to Delete

| File | Reason |
|------|--------|
| `crates/brassclaw_reborn_config/src/profile.rs` | RebornProfile enum and env-var parsing |

### Files to Modify

| File | Change |
|------|--------|
| `crates/brassclaw_reborn_config/src/lib.rs` | Remove `pub mod profile;` and re-exports of `RebornProfile`, `REBORN_PROFILE_ENV` |
| `crates/brassclaw_reborn_composition/src/profile.rs` | Collapse to `{ Disabled, Active }` (remove `LocalDevYolo`, `Production`, `MigrationDryRun` variants); remove `requires_production_shape()`; simplify `to_event_store_profile()` to always return `LocalDev` |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Remove `Production \| MigrationDryRun → build_production_shaped()` branch from `build_reborn_services()`; remove `build_production_shaped()` function |
| `crates/brassclaw_reborn_composition/src/input.rs` | Remove `profile: RebornCompositionProfile` param from `libsql()`, `postgres()`, `local_dev()` constructors; derive storage path entirely from the storage URL type |
| `crates/brassclaw_reborn_cli/src/runtime/mod.rs` | Remove `composition_profile()` (line ~541); remove `effective_profile()` (line ~655); add `db_url_from_env()` + `runtime_profile_from_env_or_config()`; construct `ResolveRequest` directly; add `BRASSCLAW_RUNTIME_PROFILE` env var reading |
| `crates/brassclaw_reborn_cli/src/commands/` | Add `migrate --dry-run` flag to migrate subcommand; remove any `profile list` subcommand or repurpose to show `RuntimeProfile` values |
| `crates/brassclaw_reborn_event_store/src/lib.rs` | Remove `RebornProfile::Production` branching from validation; enforce guards based on URL scheme instead |
| `crates/brassclaw_reborn_composition/src/local_runtime_profile.rs` | Remove `LocalDevYolo` → `RuntimeProfile::LocalYolo` branch (this mapping now comes from `BRASSCLAW_RUNTIME_PROFILE` env var directly) |
| `README.md` | Update profile documentation to describe the new `BRASSCLAW_DB_URL` + `BRASSCLAW_RUNTIME_PROFILE` knobs |
| `AGENTS.md` | Update the Key Environment Variables table — remove `BRASSCLAW_REBORN_PROFILE`, add `BRASSCLAW_DB_URL` and `BRASSCLAW_RUNTIME_PROFILE` |
| `crates/brassclaw_reborn_cli/tests/smoke.rs` | Update `profile_list_shows_supported_profiles_without_reborn_home` (lines 68–136); replace `skills_list_rejects_unsupported_profiles` (line ~332) with a test that uses `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` and expects `IncompatibleDeployment` error |

---

## Phased Execution

### Phase 1 — Add new knobs, keep old (no breakage)

Objective: New config knobs work; `BRASSCLAW_REBORN_PROFILE` still accepted. Zero test failures.

1. Add `BRASSCLAW_DB_URL` parsing in `crates/brassclaw_reborn_cli/src/runtime/mod.rs`.
   When set, derive `RebornStorageInput` from URL scheme. When absent, use existing `LocalDev` path.
   No profile enum changes.

2. Add `BRASSCLAW_RUNTIME_PROFILE` parsing in the same file. When set, call
   `brassclaw_runtime_policy::resolve()` directly; pass result to `services_input.with_runtime_policy()`.
   When absent, derive policy from existing `effective_profile()` shim.

3. Add `brassclaw-reborn migrate --dry-run` CLI flag. Calls `build_migration_input(dry_run: bool)`.
   No changes to main serve/run paths.

**Tests to add:**
- `BRASSCLAW_DB_URL=libsql://...` routes to durable libSQL storage
- `BRASSCLAW_RUNTIME_PROFILE=local_yolo` without `--confirm-host-access` returns clear error
- `BRASSCLAW_RUNTIME_PROFILE=local_yolo` with `--confirm-host-access` resolves `LocalYolo` policy

### Phase 2 — Deprecate old env var

Objective: `BRASSCLAW_REBORN_PROFILE` still accepted but emits a deprecation warning.

1. In `effective_profile()`: when `BRASSCLAW_REBORN_PROFILE` is present, print
   `eprintln!("WARNING: BRASSCLAW_REBORN_PROFILE is deprecated. Use BRASSCLAW_DB_URL and BRASSCLAW_RUNTIME_PROFILE.")`.
2. Translate old profile values to the new knobs in the deprecation shim:
   - `local-dev` → `BRASSCLAW_RUNTIME_PROFILE=local_dev` (already the default)
   - `local-dev-yolo` → `BRASSCLAW_RUNTIME_PROFILE=local_yolo`
   - `production` → storage derived from `BRASSCLAW_DB_URL`
   - `migration-dry-run` → error: "use `brassclaw-reborn migrate --dry-run` instead"
3. Repurpose `profile list` command to show available `RuntimeProfile` values and the new env vars.
4. Update `README.md` and `AGENTS.md`.

### Phase 3 — Remove old boot profile code (the actual removal)

Objective: `BRASSCLAW_REBORN_PROFILE` fully gone. Codebase is at target state.

Ordered steps (compiler guides you at each step):

1. **Delete** `crates/brassclaw_reborn_config/src/profile.rs`
2. **Remove** `pub mod profile;` from `crates/brassclaw_reborn_config/src/lib.rs`
3. **Remove** `RebornCompositionProfile::LocalDevYolo`, `Production`, `MigrationDryRun` from
   `crates/brassclaw_reborn_composition/src/profile.rs` — compiler will enumerate every match site
4. **Remove** the `Production | MigrationDryRun → build_production_shaped()` branch from
   `build_reborn_services()` in `factory.rs`
5. **Delete** `build_production_shaped()` function
6. **Remove** `profile: RebornCompositionProfile` parameters from `RebornBuildInput` constructors
7. **Remove** `composition_profile()` and `effective_profile()` from `mod.rs`
8. **Remove** `requires_production_shape()` from `RebornCompositionProfile`
9. **Update smoke tests** — remove/replace assertions about `BRASSCLAW_REBORN_PROFILE` and
   `migration-dry-run` (lines 68–136 and ~332 of `smoke.rs`)
10. **Run** `cargo clippy --all -- -D warnings` — must be zero warnings
11. **Run** `cargo test` — all tests must pass

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Production storage fail-closed guard bypassed | High | Guard moves to URL-scheme check; remote URL always → durable storage; no `BRASSCLAW_DB_URL` → in-memory/local only |
| Yolo disclosure gate silently lost | High | `resolver::resolve()` already enforces it; just ensure `yolo_disclosure_acknowledged` maps from `--confirm-host-access` at `ResolveRequest` construction |
| `RebornCompositionProfile` persisted in stored state | Medium | Profile is never serialized to DB; only parsed from startup string. Verify with `grep -r "composition_profile"` on data dirs before Phase 3 cut |
| 24+ call sites of `RebornBuildInput` break at once | Medium | Monorepo: compiler enumerates all sites in Phase 3 step 6; fix atomically |
| Smoke test `skills_list_rejects_unsupported_profiles` inverts | Low | Replace with equivalent test using `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` → `IncompatibleDeployment` error |
| `EnterpriseYoloDedicated` + `admin_approves_dedicated_yolo` path never tested after change | Low | Write a unit test for `ResolveRequest` with `EnterpriseYoloDedicated + admin_approves=false → error` |

---

## Verification / Testing Plan

After Phase 3 is complete, run these in order:

```bash
# 1. Lint — zero warnings required
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# 2. Unit tests for the composition crate
cargo test -p brassclaw_reborn_composition

# 3. Unit tests for the CLI / runtime module
cargo test -p brassclaw_reborn_cli

# 4. Runtime policy tests (all security gates)
cargo test -p brassclaw_runtime_policy

# 5. Config crate tests
cargo test -p brassclaw_reborn_config

# 6. Integration tests (requires PostgreSQL)
cargo test --features integration

# 7. Smoke test: default start uses LocalDev policy + local storage
BRASSCLAW_REBORN_HOME=/tmp/bc-test brassclaw-reborn repl --test-exit
# Expect: starts with AskDestructive policy, no profile env var needed

# 8. Smoke test: explicit Postgres URL works
BRASSCLAW_DB_URL=postgres://... brassclaw-reborn repl --test-exit
# Expect: starts with PostgreSQL backend, LocalDev policy

# 9. Smoke test: yolo without disclosure = clear error
BRASSCLAW_RUNTIME_PROFILE=local_yolo brassclaw-reborn repl --test-exit
# Expect: error "requires explicit disclosure acknowledgement"

# 10. Smoke test: yolo with disclosure works
BRASSCLAW_RUNTIME_PROFILE=local_yolo brassclaw-reborn repl --confirm-host-access --test-exit
# Expect: starts with Minimal approval policy

# 11. Smoke test: wrong deployment family = clear error
BRASSCLAW_RUNTIME_PROFILE=hosted_safe brassclaw-reborn repl --test-exit
# Expect: error "IncompatibleDeployment"

# 12. Smoke test: migration dry-run flag
brassclaw-reborn migrate --dry-run
# Expect: outputs what would be migrated, no DB writes

# 13. No mentions of BRASSCLAW_REBORN_PROFILE in compiled binary help output
brassclaw-reborn --help | grep -v "BRASSCLAW_REBORN_PROFILE"
# Expect: grep returns empty (old env var is gone from help text)
```

---

## Summary

The plan does not remove any security properties — all approval gates, isolation
boundaries, yolo disclosure requirements, and enterprise admin gates remain intact via
the unchanged `RuntimeProfile` resolver. It removes the `RebornProfile` boot-level
concept that was acting as a composite shorthand for two unrelated decisions (storage
selection + security policy) and replaces it with two focused, independent mechanisms:

1. **`BRASSCLAW_DB_URL`** — determines storage backend from URL scheme (absent = local, remote URL = durable)
2. **`BRASSCLAW_RUNTIME_PROFILE`** — determines security policy tier (default = `local_dev` = AskDestructive)
3. **`brassclaw-reborn migrate --dry-run`** — replaces the `migration-dry-run` profile

The result is one binary, one code path, no profile switching.
