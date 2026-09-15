# Subplan: Fix thread_id incorrectly included in secret/product-auth paths

## Problem

The parent plan (`scope_project_path_audit_plan.md`) assumed that `thread_id` is
always `None` in the secrets and product_auth path callers, so using
`within_tenant_segment()` (which includes thread_id when present) would be
behaviorally equivalent to the old hand-written agent+project-only code.

This assumption is **wrong**. Two tests prove it:

1. `brassclaw_secrets::filesystem_store::tests::filesystem_secret_store_aad_validates_cross_invocation_within_same_owner`
   — writes a secret with `thread_id = "thread-write"` and reads it with
   `thread_id = "thread-read"`. Expects them to share the same path (agent+project only).

2. `brassclaw_reborn_composition::product_auth_durable::tests::filesystem_runtime_account_selection_matches_new_thread_reusable_account`
   — creates an account with `thread_id = "thread-auth-1"` and queries it with
   `thread_id = "thread-auth-2"`. Expects cross-thread account sharing.

Both stores are intentionally **thread-agnostic** at the owner-path level:
- Secrets are per (agent, project), shared across invocations/threads.
- Product auth accounts are per (agent, project), shared across invocations/threads.

## Solution

Add a second method `ResourceScope::within_agent_project_segment() -> String` that
returns only the agent+project axes (no thread_id), or a fallback sentinel "scope"
when both are absent.

- `within_tenant_segment()` — includes agent+project+thread (used by authorization,
  processes, run_state)
- `within_agent_project_segment()` — includes agent+project only (used by secrets
  and product_auth)

## Files to Change

1. `crates/brassclaw_host_api/src/resource.rs` — add `within_agent_project_segment()`
2. `crates/brassclaw_secrets/src/filesystem_store.rs` — change `secret_owner_alias()` to
   call `scope.within_agent_project_segment()` instead of `scope.within_tenant_segment()`
3. `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs` — change
   `product_auth_base_root()` to call `resource.within_agent_project_segment()` instead

## The method

```rust
/// Build the within-tenant path segment for agent/project-scoped stores
/// (secrets, product auth) where records are shared across threads.
///
/// Returns `agents/{id}/projects/{id}` for whichever axes are present,
/// or `"scope"` when both are absent. Thread identity is intentionally
/// excluded — callers store records at the project granularity and
/// expect cross-thread reads to hit the same path.
pub fn within_agent_project_segment(&self) -> String {
    let mut segments = Vec::new();
    if let Some(id) = &self.agent_id {
        segments.push(format!("agents/{id}"));
    }
    if let Some(id) = &self.project_id {
        segments.push(format!("projects/{id}"));
    }
    if segments.is_empty() {
        "scope".to_string()
    } else {
        segments.join("/")
    }
}
```

## Verification

After fix, both tests must pass:
- `filesystem_secret_store_aad_validates_cross_invocation_within_same_owner`
- `filesystem_runtime_account_selection_matches_new_thread_reusable_account`
