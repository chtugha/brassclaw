# Subplan: factory.rs VFS filesystem field cleanup

**Status:** [-] In progress
**Parent:** `plan_skill_context_removal.md` / Step 8 of Phase P in `saved_plan_to_v3.md`
**Triggered by:** `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
  firing dead_code errors on `skill_filesystem` and `workspace_filesystem` fields in
  `RebornLocalRuntimeServices` after the VFS skill execution path was deleted.

## Problem

After removing `SkillExecutionAdapter` / `FilesystemSkillBundleSource`, two struct
fields in `RebornLocalRuntimeServices` (factory.rs) became dead:

1. **`skill_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>`** (line 340)
   — Was used by `FilesystemSkillBundleSource` / `SkillExecutionAdapter`. Now no
   production code reads this field. The construction `build_workspace_filesystems`
   still creates it with `skill_context_mount_view()`, which is also now dead.

2. **`workspace_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>`** (line 341)
   — Was used by the setup-markers path (now deleted). Only accessed from the test
   `local_dev_setup_marker_workspace_filesystem_is_read_only` which tests that the
   workspace FS has read-only policy. Since the setup-markers functionality is gone,
   this test and field are VFS-era dead code.

## Scope of changes

### Files
- `crates/brassclaw_reborn_composition/src/factory.rs`
- `crates/brassclaw_reborn_composition/src/local_dev_mounts.rs` (if `skill_context_mount_view` becomes unused)

### Changes

#### Step A — Remove `skill_filesystem` field and all construction
1. Remove field `skill_filesystem` from struct `RebornLocalRuntimeServices` (line 340)
2. Remove `skill_filesystem` from `RebornLocalDevStoreGraphInput` (line 487)
3. Remove from `build_local_dev_store_graph` destructure (line 1096) and struct literal (line 1158)
4. In `build_workspace_filesystems` (line 1618): remove the `skill_filesystem` construction
   (lines 1643-1648) and remove it from the return tuple. Update `LocalDevWorkspaceFilesystems`
   type alias.
5. Remove from line 789-814 where the return is destructured and passed to
   `RebornLocalDevStoreGraphInput`.
6. Remove from subagent local runtime copy (lines 2330-2331).
7. Check if `skill_context_mount_view` is still used anywhere else. If not, remove from
   `local_dev_mounts.rs` and the import at factory.rs line 66.

#### Step B — Remove `workspace_filesystem` field and dead test
1. Remove field `workspace_filesystem` from `RebornLocalRuntimeServices` (line 341)
2. Remove `workspace_filesystem` from `RebornLocalDevStoreGraphInput` (line 488)
3. Remove from `build_local_dev_store_graph` destructure (line 1097) and struct literal (line 1159)
4. In `build_workspace_filesystems`: remove the `workspace_filesystem` construction
   (lines 1649-1652) and remove it from the return tuple.
5. Remove from lines 789-814 where the return is destructured and passed.
6. Remove from subagent local runtime copy (line 2331).
7. Delete the test `local_dev_setup_marker_workspace_filesystem_is_read_only` (lines 2791-2834)
   — it tested VFS setup-markers behavior which is deleted.

#### Step C — Simplify `build_workspace_filesystems`
After removing both filesystem fields, `build_workspace_filesystems` should return only
`runtime_workspace_mounts: MountView` (or be inlined). Update callers accordingly.

## Verification
After each step: `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`
