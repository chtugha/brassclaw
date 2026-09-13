# Subplan: Restore deleted runtime test helpers (Step 6 of skill-context removal)

## Status: DONE

## Problem

During the previous sessions' cleanup of `HostSkillContextSource`-related code, several
**live** test helpers were accidentally deleted from `runtime.rs` along with the genuinely
dead ones. The test module now fails to compile.

## What was incorrectly deleted

From `runtime.rs` test module (all present in commit `312d07b1`):

1. `#[derive(Debug, Default)] struct ToolCallingGateway { ... }` — 3 fields
2. `#[derive(Debug, Default)] struct WorkspaceListingGateway { ... }` — 2 fields
3. `#[derive(Debug)] struct AllowingTriggerFireAccessChecker;` 
4. `impl HostManagedModelGateway for RecordingGateway { ... }` — stream_model
5. `impl TriggerFireAccessChecker for AllowingTriggerFireAccessChecker { ... }`
6. `impl HostManagedModelGateway for ToolCallingGateway { ... }` — full with stream_model_with_capabilities
7. `impl HostManagedModelGateway for WorkspaceListingGateway { ... }` — full with stream_model_with_capabilities

Also the `std::sync::atomic::{AtomicUsize, Ordering}` import in the test module was
removed — `FailingSkillContextSource` was the only user of `AtomicUsize/Ordering` and
that struct is also gone (its `impl HostSkillContextSource` is correctly deleted), but
the imports themselves will be needed by no remaining code.

Additionally, `LoopCapabilityPort`, `VisibleCapabilityRequest`, `ProviderToolCall`,
`SkillVisibility` were removed from the `brassclaw_turns::run_profile` import —
these ARE used by `ToolCallingGateway` and `WorkspaceListingGateway`.
`TriggerFireAccessCheck`, `TriggerFireAccessChecker`, `TriggerFireAccessDecision`,
`TriggerFireAccessError` were removed from `crate::runtime_input` — needed by
`AllowingTriggerFireAccessChecker`.
`AcceptedMessageRef` was in `brassclaw_turns` — still used by tests.

## What was CORRECTLY deleted (stay deleted)

- `struct FailingSkillContextSource` — only used for `HostSkillContextSource` tests
- `struct StaticSkillContextSource` — only used for `HostSkillContextSource` tests
- `impl HostSkillContextSource for StaticSkillContextSource`
- `impl HostSkillContextSource for FailingSkillContextSource`
- `HostSkillContextBuildError`, `HostSkillContextCandidate`, `HostSkillContextSource` from imports
- `SkillVisibility` — only used in dead `HostSkillContextSource` tests (check carefully)

## Steps

### Step 1: Restore imports
- Restore `LoopCapabilityPort`, `VisibleCapabilityRequest`, `ProviderToolCall` to
  `brassclaw_turns::run_profile` import
- Restore `TriggerFireAccessCheck`, `TriggerFireAccessChecker`, `TriggerFireAccessDecision`,
  `TriggerFireAccessError` to `crate::runtime_input` import
- Restore `AcceptedMessageRef` to `brassclaw_turns` import (if still used)
- `async_trait::async_trait` must be restored for the impl blocks

### Step 2: Restore struct definitions
Insert after `RecordingGateway` struct:
- `#[derive(Debug, Default)] struct ToolCallingGateway { calls, stream_model_calls, requests }`
- `#[derive(Debug, Default)] struct WorkspaceListingGateway { calls, requests }`
- `#[derive(Debug)] struct AllowingTriggerFireAccessChecker;`

### Step 3: Restore impl blocks
Insert after the struct definitions:
- `impl TriggerFireAccessChecker for AllowingTriggerFireAccessChecker`
- `impl HostManagedModelGateway for RecordingGateway`
- `impl HostManagedModelGateway for ToolCallingGateway` (includes stream_model_with_capabilities)
- `impl HostManagedModelGateway for WorkspaceListingGateway` (includes stream_model_with_capabilities)

### Step 4: Verify `SkillVisibility` usage
Check if `SkillVisibility` is still used anywhere in the test module after the skill-context
test removal. If not, leave it removed from imports.

### Step 5: Run clippy to verify clean build
`cargo clippy -p brassclaw_reborn_composition --tests -- -D warnings`
