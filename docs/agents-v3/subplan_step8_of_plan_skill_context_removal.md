# Subplan: Steps 8–11 — Delete VFS-based skill execution path

> **Status:** [x] COMPLETE — Steps 8–11 of `plan_skill_context_removal.md`
> **Parent plan:** `.bob/plans/plan_skill_context_removal.md`
> **Blocked unblocked:** Steps 1–7 done. The blocking dependency was
> `SkillExecutionAdapter` / `execute_skill_message` / `bundle_source`. This subplan
> resolves that dependency by removing the VFS-based skill execution path.

---

## Scope summary

The VFS-based skill execution path consists of:
1. `SkillExecutionAdapter<S>` — in `brassclaw_first_party_extension_ports/src/execution.rs`
2. `SkillExecutionPlan<S>` + `SkillBundleAssetReader<S>` — same crate
3. `bundle_source: Arc<S>` field on `SelectableSkillContextSource<S>` — `activation.rs`
4. All methods that call `bundle_source`: `load_activation_descriptors`, `load_activation_candidates`,
   `select_activation_plan`, `activate_skills_for_run`, `resolve_activation_plan_with_candidates`
5. `FilesystemSkillBundleSource<F>` — `brassclaw_loop_support/src/filesystem_skill_bundle_source.rs`
6. `SkillBundleSource` trait — `brassclaw_loop_support/src/skill_bundle_source.rs`
7. `execute_skill_message` / `read_skill_execution_asset` on `RebornRuntime`
8. `LocalDevSkillContextSource`, `local_dev_filesystem_skill_context_source`, type aliases
9. `runtime/skills.rs` — `RebornSkillExecutionResult/Plan/Activation/Asset/Mode` types
10. `FirstPartySkillsExtension.bundle_source` + `selectable_skill_runtime` / `execution_adapter`
11. `FirstPartySelectableSkillsRuntime.execution_adapter`
12. `SkillActivationMessageTextResolver<S>` generic parameter (non-generic after step 8)

## What STAYS

- `SelectableSkillContextSource` struct itself — stripped of `bundle_source`/generic
- `record_user_message`, `peek_message_text`, `clear_accepted_message` — LIVE in webui wiring
- `SkillActivationPlan` / `SkillActivationSelection` / `SkillActivationRequest` — used by tests
- `SkillActivationSelectorConfig` / `SkillActivationSelectionMode` — needed
- `SkillActivationObserver` / `SkillActivationObservedEvent` — type definitions stay
- `SkillActivationSelectionError` — stay
- `SkillSourceKind`, `SkillBundleId` — check if used by `brassclaw_skills/management.rs`

---

## Step-by-step (one commit per step, check build before next)

### Step 8a — Gut `SelectableSkillContextSource`: remove generic + bundle_source + all VFS methods

**File:** `crates/brassclaw_first_party_extension_ports/src/activation.rs`

Remove from struct:
- `bundle_source: Arc<S>` field
- `config: SkillActivationSelectorConfig` field (only used by VFS methods)
- `setup_marker_source` field (only used by `satisfied_setup_markers` → VFS path)
- `activation_observer` field (only fires in test-only `selected_candidates`)
- `activation_cache`, `active_plans_by_run`, `plans_by_run` fields (VFS path only)

Remove generic `<S: SkillBundleSource + ?Sized>` → struct becomes plain.

Remove methods:
- `new(bundle_source, config)` → replace with `new()` (no params needed)
- `with_setup_marker_source` → DELETE
- `record_user_message_for_execution` → DELETE (only called by SkillExecutionAdapter)
- `bundle_source()` → DELETE
- `set_activation_observer` → DELETE
- `take_activation_plan_for_run` → DELETE
- `select_activation_plan` → DELETE
- `activate_skills_for_run` → DELETE
- `take_message_for_run` (test-only) → KEEP (only touches messages_by_run)
- `selected_candidates` (test-only) → DELETE
- `resolve_activation_plan` / `resolve_activation_plan_with_candidates` → DELETE
- `load_activation_descriptors`, `load_activation_candidate_set*`, `load_named_*` → DELETE
- `load_activation_candidates`, `satisfied_setup_markers`, `activation_candidate_from_skill_md` → DELETE
- `active_plan`, `merge_active_plan` → DELETE

Delete private types/structs:
- `ActivationCandidate`, `ActivationCandidateSet`, `CachedActivationCandidate`
- `ActivationCandidateCacheKey`, `CapturedSkillActivationPlan`, `ActivePlanCache`

Delete private fns:
- `active_plan_key`, `activation_plan_for_candidates`, `candidates_with_unsatisfied_setup_markers`
- `candidate_for_loaded_skill`, `select_skill_activations`, `select_named_skill_activations`
- `loaded_skill_from_candidate`, `validate_descriptor_policy_metadata`
- `skill_bundle_source_error_to_selection_error`, `context_candidates_for_plan`
- `reserve_skill_budget`, `descriptor_context_ordering_key`, `content_hash`
- `lowercased`, `feedback_skill_name` → DELETE (only used by select_* fns)

Keep:
- `SkillActivationMessageKey`, `SkillActivationMessage` (minus `capture_plan` field)
- `record_message` method (used by `record_user_message` only, remove `capture_plan` logic)
- `record_user_message`, `peek_message_text`, `clear_accepted_message`
- `extract_explicit_skill_names`, `normalize_dollar_skill_mentions`, `is_skill_mention_boundary`
  → Check: these are helpers for VFS selection. If no caller remains after deletion → DELETE.

Actually `extract_explicit_skill_names` and `normalize_dollar_skill_mentions` are called only from
`select_skill_activations` (being deleted). DELETE them too.

Remove imports:
- `SkillBundleDescriptor`, `SkillBundleId`, `SkillBundleSource`, `SkillBundleSourceError`, `sort_skill_bundle_descriptors`
- `LoadedSkill`, `SkillSelectionOptions`, `SkillSource`, `extract_skill_mentions`, `parse_skill_md`,
  `prefilter_skills_with_options`, `skill_token_cost`, `validate_skill_name`
- `SkillVisibility`
- `futures::{StreamExt, TryStreamExt, stream}`
- `async_trait`
- `std::path::PathBuf`
- `std::hash::{Hash, Hasher}` (if `content_hash` removed)
- `std::collections::{HashMap, HashSet, VecDeque}` → keep `HashMap` (messages_by_run), keep `HashSet` if referenced

Tests: DELETE all tests that use `StaticSkillBundleSource`, `activate_skills_for_run`,
`select_activation_plan`, `selected_candidates`. Keep only tests for `record_user_message`/
`peek_message_text`/`clear_accepted_message` (these only touch `messages_by_run`).

### Step 8b — Delete `execution.rs` + `assets.rs` + update `lib.rs`

Delete files:
- `crates/brassclaw_first_party_extension_ports/src/execution.rs`
- `crates/brassclaw_first_party_extension_ports/src/assets.rs`

Update `crates/brassclaw_first_party_extension_ports/src/lib.rs`:
- Remove `mod execution;`, `mod assets;`
- Remove `pub use assets::{SkillBundleAsset, SkillBundleAssetReadError, SkillBundleAssetReader};`
- Remove `pub use execution::{SkillExecutionAdapter, SkillExecutionAdapterError, SkillExecutionPlan};`

### Step 8c — Simplify `skills.rs` in `brassclaw_first_party_extension_ports`

`FirstPartySkillsExtension` no longer needs `FilesystemSkillBundleSource`.
After step 8a, `SelectableSkillContextSource::new()` takes no bundle source.

New `FirstPartySkillsExtension`:
```rust
pub struct FirstPartySkillsExtension {
    default_activation_source: Arc<SelectableSkillContextSource>,
}

impl FirstPartySkillsExtension {
    pub fn new() -> Self {
        let default_activation_source = Arc::new(SelectableSkillContextSource::new());
        Self { default_activation_source }
    }

    pub fn activation_source(&self) -> Arc<SelectableSkillContextSource> {
        Arc::clone(&self.default_activation_source)
    }
}
```

Remove entirely:
- `FirstPartySkillsExtensionHandles` — no longer needed
- `FirstPartySkillsExtensionError` — check if still used elsewhere
- `FirstPartySelectableSkillsRuntime` — only had `activation_source` + `execution_adapter`
- All filesystem-related imports and logic

Update `lib.rs`:
- Remove `pub use skills::{FirstPartySelectableSkillsRuntime, FirstPartySkillsExtensionHandles};`
- Keep `pub use skills::FirstPartySkillsExtension;`

Check if `FirstPartySkillsExtensionError` is used outside this crate:

### Step 9 — Delete `filesystem_skill_bundle_source.rs`

Delete `crates/brassclaw_loop_support/src/filesystem_skill_bundle_source.rs`.
Update `crates/brassclaw_loop_support/src/lib.rs`:
- Remove `mod filesystem_skill_bundle_source;`
- Remove `pub use filesystem_skill_bundle_source::{FilesystemSkillBundleRoot, FilesystemSkillBundleSource};`

### Step 10 — Handle `SkillBundleSource` trait + `skill_bundle_source.rs`

First check: which symbols from `skill_bundle_source.rs` are still used after steps 8-9?
- `SkillSourceKind` — used in `brassclaw_skills/src/management.rs`
- `SkillBundleId` — used in `brassclaw_skills/src/management.rs`  
- `SkillBundleDescriptor`, `SkillBundleProvenance`, `SkillFilePath`, `sort_skill_bundle_descriptors`,
  `SkillBundleSource`, `SkillBundleSourceError` → only used by deleted files

Resolution:
- Keep `SkillSourceKind` + `SkillBundleId` — move to `brassclaw_skills` or keep in minimal form
- Delete the rest of `skill_bundle_source.rs`

OR: check if `brassclaw_skills` already defines `SkillSourceKind` internally. If so, just remove
the re-export in loop_support and update management.rs to import from `brassclaw_skills` directly.

### Step 11 — Remove execute_skill_message + RebornSkill* from RebornRuntime

**`runtime/skills.rs`** — DELETE FILE. Remove `mod skills;` from runtime.rs.

**`runtime.rs` changes:**
- Remove `skill_execution_adapter: Option<Arc<LocalDevSkillExecutionAdapter>>` field
- Change `skill_activation_source` field type to `Option<Arc<SelectableSkillContextSource>>`
- Remove `execute_skill_message`, `read_skill_execution_asset`, `skill_execution_plan_for_run` methods
- Remove `LocalDevSkillContextSource` struct + `local_dev_filesystem_skill_context_source` fn
- Remove `local_dev_selector_config` fn + test
- Remove type aliases `LocalDevSelectableSkillContextSource`, `LocalDevSkillExecutionAdapter`
- Simplify `build_reborn_runtime`: replace the VFS block with just:
  ```rust
  let skill_activation_source: Option<Arc<SelectableSkillContextSource>> =
      if local_runtime_ref.is_some() {
          Some(Arc::new(SelectableSkillContextSource::new()))
      } else {
          None
      };
  ```
- Remove `capture_skill_execution_plan` branch from `send_user_message_internal`
  (keep only the `skill_activation_source.record_user_message` branch)
- Remove test `execute_skill_message_returns_plan_and_reads_active_bundle_assets` + related tests

**`lib.rs`:**
- Remove `RebornSkillActivation, RebornSkillActivationMode, RebornSkillAsset, RebornSkillBundle,`
  `RebornSkillExecutionPlan, RebornSkillExecutionResult, RebornSkillSourceKind` from pub use

**`retrieval_lookup_impl.rs`:**
- Remove generic `<S: SkillBundleSource + ?Sized>` from `SkillActivationMessageTextResolver<S>`
- Change field to `source: Arc<SelectableSkillContextSource>`
- Remove `MockBundleSource` and its `SkillBundleSource` impl from tests
- Simplify `mock_selectable()` to `Arc::new(SelectableSkillContextSource::new())`

---

## Post-completion: update plans

After all steps pass build + clippy + tests:
- Update `.bob/plans/plan_skill_context_removal.md` — mark Steps 8–11 [DONE]
- Update `saved_plan_to_v3.md` Phase P.1 — add "Step C (VFS layer cleanup) complete"
