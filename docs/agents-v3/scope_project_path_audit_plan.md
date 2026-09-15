# Scope, Project, and Path Security Design — Simplification Plan

## What this plan is and is not

This plan addresses the **implementation complexity** of the scope/path security
design — duplicated code, unnecessary error propagation, and fragile boilerplate.
It does **not** re-architect the underlying security model. The security concepts
themselves (scope hierarchy, three-layer path types, MountView ACL) are load-bearing
and correctly implemented; removing them would break isolation.

Two changes are proposed. Both are net-negative in lines. Neither changes runtime
behavior or security properties.

---

## What Is Actually Causing Pain (Full Picture)

### Tier 1 — Fixed by this plan

These are implementation duplications that can be eliminated right now with zero risk:

| Problem | Location | Impact |
|---------|----------|--------|
| `within_tenant_scope()` reimplemented in 5 separate modules | `brassclaw_authorization`, `brassclaw_processes`, `brassclaw_run_state`, `brassclaw_secrets`, `brassclaw_reborn_composition` | Any subtle difference between the five versions is a latent isolation bug |
| `ScopedPath::new(format!(...)).map_err(...)` on structurally-guaranteed-valid paths | All five modules + their callers | ~10 function signatures carry a `Result` whose error branch is unreachable; `?` operators obscure which paths are fallible and which are not |

### Tier 2 — Architectural tensions (known, not fixed here)

These are real sources of complexity that would require separate, larger changes:

**A. Dual-write fallback in authorization** (`crates/brassclaw_authorization/src/lib.rs:490–655`)  
Every lease write attempts to write with an indexed `tenant_id` projection, then falls
back to a plain-bytes write when the backend returns `Unsupported`. This exists because
`LocalFilesystem` (dev) and `InMemoryBackend` (tests) don't support CAS versioning or
indexed projections. Two code paths per write, two failure modes to reason about.
The code comments are clear and honest about the trade-off; the fallback is not a bug,
but it is complexity.

**B. Index+scan fallback in authorization** (`crates/brassclaw_authorization/src/lib.rs:670–678`)  
Lease enumeration tries the index file first, then falls back to a full directory scan
if the index is absent. Correct behavior, but two code paths for every list operation.

**C. `PER_USER_ALIASES` hardcoded list** (`crates/brassclaw_reborn_composition/src/lib.rs:517–532`)  
14 alias strings are hardcoded in composition and must stay in sync with all consumer
stores. Adding a new store requires updating this list. No compile-time enforcement.

**D. `ResourceScope::system()` used inside request handlers**  
`invocation_services.rs` constructs `ResourceScope::system()` inside per-tenant
capability dispatch handlers for event logging and process output. The system scope
routes to a global path, not the tenant-specific path. This is intentional (those
records are process-global), but it means the tenant context of the originating
request is not carried into those writes.

None of these are addressed by this plan. Each would require a targeted design
document and coordinated change across multiple crates.

---

## What Is Genuinely Doing Security Work (Keep, Don't Touch)

| Component | File | What it does |
|-----------|------|--------------|
| `ResourceScope` | `crates/brassclaw_host_api/src/resource.rs:28` | Isolation key for leases, secrets, sandbox, output |
| `ScopedPath` / `VirtualPath` / `HostPath` | `crates/brassclaw_host_api/src/path.rs` | Three-layer path type system: extensions cannot address host paths or internal virtual roots |
| `MountView` + `resolve_with_permission()` | `crates/brassclaw_filesystem/src/scoped.rs:377` | Per-invocation ACL on every filesystem op, fail-closed, no bypass paths |
| `RebornSandboxScopeKey` | `crates/brassclaw_host_runtime/src/sandbox_process/scope_key.rs` | Container/workspace isolation via SHA-256 digest |
| `same_scope_owner()` | `crates/brassclaw_authorization/src/lib.rs:1503` | Exact-scope lease matching |
| `within_tenant_scope()` | `crates/brassclaw_authorization/src/lib.rs:1563` | Encodes agent/project/thread hierarchy in FS paths |
| `is_sensitive_path()` | `crates/brassclaw_safety/src/sensitive_paths.rs:66` | Blocks credential file reads |
| Approval `ActionPattern::PathPrefix` | `crates/brassclaw_host_api/src/approval.rs:161` | Path-pattern scoped delegated authority |
| `ResourceAccount::cascade()` | `crates/brassclaw_resources/src/lib.rs` | Hierarchical resource budget enforcement |

---

## Change 1 — Centralize the within-tenant path segment

### What exists

The logic of building a path segment from optional agent/project/thread fields of a
`ResourceScope` is hand-coded in **five** production source files. Each maintains its
own conditional string-building loop independently:

| File | Function | Lines | Differs from auth version? |
|------|----------|-------|---------------------------|
| `crates/brassclaw_authorization/src/lib.rs:1563` | `within_tenant_scope()` | 17 | Reference implementation. Falls back to `"scope"` sentinel |
| `crates/brassclaw_processes/src/filesystem_store.rs:657` | `scope_owner_root_string()` | 16 | Prepends `/processes`. No `"scope"` fallback |
| `crates/brassclaw_run_state/src/lib.rs:1046` | `scope_owner_alias_string()` | 16 | Takes prefix param. No `"scope"` fallback |
| `crates/brassclaw_secrets/src/filesystem_store.rs:987` | `secret_owner_alias()` | 12 | Emits agent+project only (no thread). No `"scope"` fallback |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs:72` | `product_auth_base_root()` | 9 | Emits agent+project only. Appends `/product-auth` suffix |

The divergence in the "fallback" behaviour between the auth version and the others is
not a correctness bug today — the non-auth callers never call the function with a scope
where all three fields are `None`. But it is a latent trap: if a caller is ever added
with a fully-unscoped `ResourceScope`, processes and secrets would produce an empty
path component, corrupting the alias-relative path silently.

**Cannot be centralized — different input type:**

- `crates/brassclaw_reborn_composition/src/runtime.rs:1384` — takes `&TurnScope`, builds a `VirtualPath`; incompatible types in both dimensions
- `crates/brassclaw_threads/src/filesystem_service.rs:1758` — takes `&ThreadScope`; emits `owner_user_id` instead of `thread_id`
- `crates/brassclaw_loop_support/src/filesystem_checkpoint_state.rs:214` — takes `&TurnScope`; `thread_id` is non-optional

**Test mirror to update alongside production:**

- `crates/brassclaw_processes/tests/process_store_contract.rs:2700` — `alias_relative_owner_root()` is an explicit mirror of `scope_owner_root_string`, documented to catch drift

### What to do

Add `ResourceScope::within_tenant_segment() -> String` to `brassclaw_host_api/src/resource.rs`.
Returns the agent+project+thread chain for whichever axes are present, or `"scope"` when
all are absent — identical to `within_tenant_scope()` in authorization today.

The four callers that emit only agent+project (secrets, product_auth) are unaffected:
`thread_id` is `None` in all their call sites, so `within_tenant_segment()` produces the
same output as their current code.

**Net: +12 lines added (method), −70 lines deleted (five implementations). −58 lines.**

**The method:**

```rust
/// Build the within-tenant path segment for filesystem path isolation.
/// Returns `agents/{id}/projects/{id}/threads/{id}` for whichever axes
/// are present, or `"scope"` when all are absent so the caller's
/// alias-relative path is always a non-empty directory component.
pub fn within_tenant_segment(&self) -> String {
    let mut segments = Vec::new();
    if let Some(id) = &self.agent_id   { segments.push(format!("agents/{id}"));   }
    if let Some(id) = &self.project_id { segments.push(format!("projects/{id}")); }
    if let Some(id) = &self.thread_id  { segments.push(format!("threads/{id}"));  }
    if segments.is_empty() { "scope".to_string() } else { segments.join("/") }
}
```

**What each caller becomes:**

```rust
// authorization: within_tenant_scope() deleted; 3 call sites become:
scope.within_tenant_segment()

// processes: scope_owner_root_string() body collapses to one line:
fn scope_owner_root_string(scope: &ResourceScope) -> String {
    format!("{}/{}", PROCESSES_PREFIX, scope.within_tenant_segment())
}

// run_state: scope_owner_alias_string() body collapses to one line:
fn scope_owner_alias_string(prefix: &'static str, scope: &ResourceScope) -> String {
    format!("{}/{}", prefix, scope.within_tenant_segment())
}

// secrets: secret_owner_alias() body collapses to one line:
// (thread_id is never set in secrets callers — within_tenant_segment()
// produces the same agent/project-only string as the current code)
fn secret_owner_alias(scope: &ResourceScope) -> String {
    format!("/secrets/{}", scope.within_tenant_segment())
}

// product_auth_durable: product_auth_base_root() body collapses to one line:
fn product_auth_base_root(resource: &ResourceScope) -> String {
    format!("/secrets/{}/product-auth", resource.within_tenant_segment())
}
```

**Do not touch `RebornSandboxScopeKey::from_scope()`.**
Its cascade is project OR thread OR invocation (exclusive, not all-present) — a stable
isolation digest, not a path. Add a comment there documenting this divergence.

**Files to change:**
- `crates/brassclaw_host_api/src/resource.rs` — add `within_tenant_segment()`
- `crates/brassclaw_authorization/src/lib.rs` — delete `within_tenant_scope()`, inline at 3 call sites
- `crates/brassclaw_processes/src/filesystem_store.rs` — collapse `scope_owner_root_string()` body
- `crates/brassclaw_processes/tests/process_store_contract.rs` — update `alias_relative_owner_root()` mirror
- `crates/brassclaw_run_state/src/lib.rs` — collapse `scope_owner_alias_string()` body
- `crates/brassclaw_secrets/src/filesystem_store.rs` — collapse `secret_owner_alias()` body
- `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs` — collapse `product_auth_base_root()` body
- `crates/brassclaw_host_runtime/src/sandbox_process/scope_key.rs` — add one comment

---

## Change 2 — Remove spurious `Result` from scope-derived path helpers

### What exists

Every internal path helper that assembles a `ScopedPath` from already-validated
`AgentId` / `ProjectId` / `ThreadId` / `InvocationId` values calls
`ScopedPath::new(...).map_err(...)` and returns `Result<ScopedPath, SomeError>`.
The error branch is structurally unreachable — these ID types are validated at
construction time and cannot contain `/`, `..`, or control characters.

The unreachable `Result` propagates through callers via `?`, making 10+ function
signatures look fallible when they are not. This matters because **genuine fallibility
(filesystem I/O, deserialization, CAS failure) is buried in the same noise**, reducing
the signal value of `?` throughout the authorization and store code.

Affected groups:

**Authorization — three path-builder functions (lines 1532–1557):**
```rust
fn lease_path(scope, lease_id)   -> Result<ScopedPath, CapabilityLeaseError>
fn lease_index_path(scope)        -> Result<ScopedPath, CapabilityLeaseError>
fn lease_owner_prefix(scope)      -> Result<ScopedPath, CapabilityLeaseError>
```
Called from: lines 470, 505, 520, 603, 623, 633, 684, 764.
All 8 call sites use `?` solely to propagate the unreachable path-construction error.

**Processes — `scoped_path()` wrapper (line 674) serving scope-derived builders:**
```
process_record_path, process_records_root, process_result_path, process_output_path
```

**Run-state — `scoped_path()` wrapper (line 1063) serving scope-derived builders:**
```
run_record_path, run_records_root, approval_record_path, approval_records_root
```

**Secrets — `scoped_path_secret()` / `scoped_path_broker()` wrappers (lines 1000–1005)
serving scope-derived builders:**
```
secret_path, lease_root, secret_owner_root, credential_session_root,
credential_account_root, credential_session_path
```

**product_auth_durable — `scoped_path()` wrapper (line 98) serving scope-derived builders:**
```
flow_path, flow_root, surface_sessions_root, interaction_path, account_path, account_root
```

**`join_scoped()` helpers in all crates receive a leaf from `list_dir` results — that
leaf is NOT a validated ID and must keep `ScopedPath::new()`.**

### What to do

Add `pub ScopedPath::from_trusted(value: String) -> Self` to
`crates/brassclaw_host_api/src/path.rs`. Must be `pub` because callers are in
separate crates. A `debug_assert!` guards misuse in debug builds:

```rust
/// Construct from a value proven correct by construction — path assembled
/// from already-validated ID types that cannot contain separators, `..`,
/// or control characters. Never call with user-supplied strings.
pub fn from_trusted(value: String) -> Self {
    debug_assert!(
        ScopedPath::new(value.clone()).is_ok(),
        "ScopedPath::from_trusted called with invalid path: {value}"
    );
    Self(value)
}
```

With this, all scope-derived path builders become infallible:
- The three authorization helpers drop their `Result` return type and `.map_err()`.
- Their 8 call sites drop the `?` operator for those specific calls.
- The scope-derived `scoped_path()` call sites in processes, run_state, secrets, and
  product_auth use `ScopedPath::from_trusted(...)` directly.
- The `scoped_path()` / `scoped_path_secret()` / `scoped_path_broker()` wrappers
  remain for `join_scoped()` — those paths come from `list_dir` and need validation.

**Net: +8 lines added (method + assert), −30 lines deleted. −22 lines.**

**Files to change:**
- `crates/brassclaw_host_api/src/path.rs` — add `ScopedPath::from_trusted()`
- `crates/brassclaw_authorization/src/lib.rs` — drop `Result` from `lease_path`, `lease_index_path`, `lease_owner_prefix`; remove `?` at 8 call sites
- `crates/brassclaw_processes/src/filesystem_store.rs` — use `from_trusted` for scope-derived paths
- `crates/brassclaw_run_state/src/lib.rs` — same
- `crates/brassclaw_secrets/src/filesystem_store.rs` — same
- `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs` — same

---

## What Is NOT Changed and Why

| Thing | Why It Stays |
|-------|-------------|
| `project_id: Option<ProjectId>` / `thread_id: Option<ThreadId>` on `ResourceScope` | Actively used for sandbox isolation, secret keying, lease paths, resource budgets. `None` is the correct "unscoped" state. |
| Three-layer path types | Each layer has a distinct security role. Removing any exposes a boundary. |
| `MountView` + `GrantConstraints.mounts` | Per-invocation ACL, fail-closed, no bypass paths exist. |
| `PER_USER_ALIASES` hardcoded list | Real architectural debt (Tier 2 above), but requires a coordinated cross-crate change with a dedicated design doc. Out of scope here. |
| Dual-write fallback in authorization | Intentional design for byte-only backends; documented clearly. Fixing it requires backend capability parity. Out of scope. |
| Index+scan fallback in authorization | Same. |
| `SYSTEM_RESERVED_ID` + `ResourceScope::system()` | Clean sentinel; `TenantId::new` rejects `\x1f`. |
| `RebornSandboxScopeKey` project-vs-thread cascade | Intentionally different from `within_tenant_segment()`. |
| `TurnScope::to_resource_scope()` mutation pattern | Style issue only — zero net lines removed. |

---

## Change Summary

| Change | Net lines | Files |
|--------|-----------|-------|
| Add `within_tenant_segment()`, delete/collapse five implementations | **−58** | `brassclaw_host_api/src/resource.rs`, `brassclaw_authorization/src/lib.rs`, `brassclaw_processes/src/filesystem_store.rs`, `brassclaw_run_state/src/lib.rs`, `brassclaw_secrets/src/filesystem_store.rs`, `brassclaw_reborn_composition/src/product_auth_durable/paths.rs` |
| Add `ScopedPath::from_trusted()`, simplify scope-derived path helpers | **−22** | `brassclaw_host_api/src/path.rs`, `brassclaw_authorization/src/lib.rs`, `brassclaw_processes/src/filesystem_store.rs`, `brassclaw_run_state/src/lib.rs`, `brassclaw_secrets/src/filesystem_store.rs`, `brassclaw_reborn_composition/src/product_auth_durable/paths.rs` |
| Comment on `RebornSandboxScopeKey::from_scope()` | +3 | `brassclaw_host_runtime/src/sandbox_process/scope_key.rs` |
| **Total** | **−77 lines** | 8 source files + 1 test file |

Zero behavior change. Zero security regression.

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/brassclaw_host_api/src/resource.rs` | Add `within_tenant_segment()` |
| `crates/brassclaw_host_api/src/path.rs` | Add `ScopedPath::from_trusted()` |
| `crates/brassclaw_authorization/src/lib.rs` | Delete `within_tenant_scope()`; inline 3 call sites; drop `Result` from 3 helpers; remove `?` at 8 call sites |
| `crates/brassclaw_processes/src/filesystem_store.rs` | Collapse `scope_owner_root_string()`; use `from_trusted` for scope-derived paths |
| `crates/brassclaw_processes/tests/process_store_contract.rs` | Update `alias_relative_owner_root()` mirror |
| `crates/brassclaw_run_state/src/lib.rs` | Collapse `scope_owner_alias_string()`; use `from_trusted` for scope-derived paths |
| `crates/brassclaw_secrets/src/filesystem_store.rs` | Collapse `secret_owner_alias()`; use `from_trusted` for scope-derived paths |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs` | Collapse `product_auth_base_root()`; use `from_trusted` for scope-derived paths |
| `crates/brassclaw_host_runtime/src/sandbox_process/scope_key.rs` | Add one comment |

**Intentionally unchanged:**
- `crates/brassclaw_reborn_composition/src/runtime.rs` — `&TurnScope` input, `VirtualPath` output; incompatible
- `crates/brassclaw_threads/src/filesystem_service.rs` — `&ThreadScope` input; `owner_user_id` field instead of `thread_id`
- `crates/brassclaw_loop_support/src/filesystem_checkpoint_state.rs` — `&TurnScope`; `thread_id` non-optional
- `crates/brassclaw_turns/src/scope.rs` — style issue only, zero net lines
- `crates/brassclaw_host_api/src/ids.rs` — no changes needed

---

## Verification

```bash
df -h /Users/ollama/brassclaw-target  # mandatory space check

CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy \
  -p brassclaw_host_api \
  -p brassclaw_authorization \
  -p brassclaw_processes \
  -p brassclaw_run_state \
  -p brassclaw_secrets \
  -p brassclaw_reborn_composition \
  -p brassclaw_host_runtime \
  --all-targets -- -D warnings

CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test \
  -p brassclaw_host_api \
  -p brassclaw_authorization \
  -p brassclaw_processes \
  -p brassclaw_run_state \
  -p brassclaw_secrets \
  -p brassclaw_reborn_composition \
  -p brassclaw_loop_support

# Must pass without change:
# lease_store_hides_leases_across_project_scope
# lease_authorizer_hides_leases_across_tenant_scope
# scope_key_isolates_tenants_with_same_user_and_project
# scope_key_uses_thread_when_project_is_absent
# scope_key_covers_agent_project_and_invocation_fallbacks
```
