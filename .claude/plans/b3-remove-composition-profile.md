# B-3: Remove RebornCompositionProfile

**Parent plan:** `unified-runtime-plan.md` Sprint B-3
**Date:** 2026-08-09
**Status:** ✅ COMPLETE — merged at `7b2527a3`

---

## Goal

Remove the `RebornCompositionProfile` 3-variant enum (`Disabled`, `LocalDev`, `LocalDevYolo`) from the composition/storage-selection layer. After this sprint:

- There is one code path in `build_reborn_services`: always PG-backed
- `RebornBuildInput` carries no `profile` field
- `BRASSCLAW_RUNTIME_PROFILE` continues to work for per-invocation `RuntimeProfile` (12-variant — NOT deleted)
- `BRASSCLAW_REBORN_PROFILE` emits a hard error at startup

The `RuntimeProfile` in `brassclaw_host_api::runtime_policy` is **not touched**.

---

## What Is Removed

| Symbol | Location | Action |
|--------|----------|--------|
| `RebornCompositionProfile` enum | `profile.rs` | Delete file |
| `RebornCompositionProfile` re-export | `lib.rs` | Remove |
| `profile` field on `RebornBuildInput` | `input.rs` | Remove |
| `match input.profile` in `build_reborn_services` | `factory.rs` | Replace with storage-based check |
| `local_dev_with_profile()` | `input.rs` | Remove |
| `profile()` accessor | `input.rs` | Remove |
| `profile` param from `postgres()` / `postgres_with_reborn_home()` | `input.rs` | Remove |
| Deprecated `postgres_with_resolved_secret_master_key()` | `input.rs` | Remove |
| `composition_profile_from_legacy_env` | `runtime/mod.rs` | Remove function + callers |
| `BRASSCLAW_REBORN_PROFILE` env var parse | `runtime/mod.rs` | Hard-error if set |
| `RebornCompositionProfile` from `serve.rs` | `serve.rs` | Remove import + usage |
| `RebornCompositionProfile` from `smoke.rs` | `smoke.rs` | Update tests |

## What Stays

| Symbol | Location | Reason |
|--------|----------|--------|
| `RuntimeProfile` (12-variant) | `brassclaw_host_api` | Per-invocation capability policy |
| `BRASSCLAW_RUNTIME_PROFILE` env var | `runtime/mod.rs` | Per-invocation RuntimeProfile |
| `RebornBuildInput::disabled()` | `input.rs` | Still used by tests (tied to `RebornStorageInput::Disabled`) |
| `build_local_dev()` | `factory.rs` | Stays (used by PG path to set up filesystem substrate) |
| `RebornServices::disabled()` | factory | Still reachable via `Disabled` storage input |

---

## Execution Steps (one at a time)

### 1. Delete `profile.rs`, fix `lib.rs` to remove re-export — compile to see all remaining errors
### 2. Fix `input.rs` — remove profile field, remove constructors that take profile
### 3. Fix `factory.rs` — replace match on profile with match on storage variant
### 4. Fix `runtime.rs` + `readiness.rs` + `local_runtime_profile.rs` + other composition internals
### 5. Fix CLI `runtime/mod.rs` — remove `composition_profile_from_legacy_env`, hard-error on BRASSCLAW_REBORN_PROFILE
### 6. Fix CLI `serve.rs`, `skills.rs` — remove profile from build inputs
### 7. Fix all test files that reference `RebornCompositionProfile`
### 8. Clippy -D warnings, full test run, commit

---

## Key Substitutions

- `RebornBuildInput::disabled("owner")` → keep (ties to `RebornStorageInput::Disabled`, no profile needed)
- `RebornBuildInput::local_dev_with_profile(profile, owner, root)` → `RebornBuildInput::local_dev(owner, root)`
- `RebornBuildInput::postgres(profile, owner, pool, url, key, home)` → `RebornBuildInput::postgres(owner, pool, url, key, home)`
- `RebornBuildInput::postgres_with_reborn_home(profile, owner, pool, url, home)` → `RebornBuildInput::postgres_with_reborn_home(owner, pool, url, home)`
- All `let _ = profile;` that exist for `LocalDevYolo` → the yolo check is via `runtime_policy.filesystem_backend` already
