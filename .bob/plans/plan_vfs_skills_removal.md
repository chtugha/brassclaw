# Plan: Remove VFS filesystem-skills path and SkillSource::System

**Created after:** Phase P.0 Step 5b commit `bfeb7728`.

## Problem statement

Two things need removing:

1. **`management::SkillSource::System` enum variant** — in
   `crates/brassclaw_skills/src/management.rs`. The disk-load path that populated
   it was removed in Phase P.1. The variant still exists and is used in one
   `lifecycle.rs` match arm that maps it to `LifecycleSkillSource::System`. Both
   the variant and the match arm are now dead code.

2. **VFS filesystem-skills activation path** — `FirstPartySkillsExtension` /
   `FirstPartySkillsExtensionHandles` / `SYSTEM_SKILLS_ROOT` / `USER_SKILLS_ROOT`
   in `crates/brassclaw_first_party_extension_ports/src/skills.rs`. Per the
   instruction: "there is no such thing as user-installed skills in the
   architecture anymore, as all is db-based — remove that whole functionality."

## Scope investigation

### Item 1 — `management::SkillSource::System`

- Defined in `crates/brassclaw_skills/src/management.rs:160`.
- Used **only** in `crates/brassclaw_reborn_composition/src/lifecycle.rs:527` in
  a match arm mapping it to `LifecycleSkillSource::System`.
- `LifecycleSkillSource::System` is defined in
  `crates/brassclaw_product_workflow/src/lifecycle.rs:423`.
- Nothing populates `SkillSource::System` any more — the only call sites that
  produce `SkillSummary` are `list_skill_root` / `read_skill_summary` in
  `management.rs`, which only use `SkillSource::User`.
- `Installed` is used in tests and via `SkillInstallSource::InstalledUrl` →
  `installed_skill_source` in `install_bundle.rs` — that variant stays.
- `LifecycleSkillSource::System` is serialized to JSON over the wire; removing it
  changes the wire type. We need to check whether any WebUI or test code
  deserializes `"system"` and handles it. The only producer is the match arm in
  `lifecycle.rs` which we are removing — so no running code ever sets it.
- `LifecycleSkillSource` has two variants `System` and `User`; after this change
  it has only `User`. We should keep `User` and remove `System`.

**Fix:** Remove `System` variant from `management::SkillSource`,
`LifecycleSkillSource`, and the match arm in `lifecycle.rs`. Update `as_str`.
Also remove `LifecycleSkillSource::System` from `product_workflow`.

### Item 2 — VFS filesystem-skills activation path

This is a larger change. The full path is:

```
FirstPartySkillsExtensionHandles
  → FilesystemSkillBundleRoot (system/user/tenant_shared roots)
  → FilesystemSkillBundleSource<F>         (reads SKILL.md from VFS)
  → SkillBundleContextSource               (wraps bundle source)
  → SelectableSkillContextSource<S>        (skill activation/selection)
  → HostSkillContextSource                 (trait used by loop)
  → FirstPartySkillsExtension              (top-level wirer)
```

The entry point in composition is:
`crates/brassclaw_reborn_composition/src/runtime.rs:3221-3242`
→ calls `FirstPartySkillsExtension::new` + `selectable_skill_runtime_with_setup_markers`
→ stores result as `LocalDevSelectableSkillContextSource`
→ threaded into `local_dev_wires_skill_context_source` → loop
→ also used by `retrieval_lookup_impl.rs` (SkillActivationMessageTextResolver)

The VFS install/remove/list calls go through `lifecycle.rs` → `management.rs` and
are surfaced at `POST /api/webchat/v2/skills/install` + `DELETE /api/webchat/v2/skills/{name}`.
The `GET /api/webchat/v2/skills` endpoint lists from `reborn_skills` DB path.

**What still genuinely uses VFS skills at runtime:**
- `FilesystemSkillBundleSource` reads `SKILL.md` from the VFS during turn setup
  (skill activation / `use skill <name>`).
- `SelectableSkillContextSource` selects which skills to inject per-turn.
- `SkillActivationMessageTextResolver` resolves the message text of skill
  activation messages.

**What the DB path covers:**
- `fetch_llm_skills_as_json` / `fetch_monty_skills_as_json` — skills injected
  into the LLM system prompt per-turn (via `pg_composition_port.rs::list_skills`).
- `reborn_skills` table — the authoritative skills store (v3 architecture).

**The gap:** The DB path handles `__list_skills__` (skills in the LLM prompt) but
the VFS path handles *skill activation* (the `use skill X` command that selects
a skill for a turn). These are different mechanisms. However, per the instruction
the VFS path should be removed entirely.

**Impact of removal:**
- `runtime.rs`: `local_dev_runtime_wires_skill_context_source` becomes a no-op /
  returns `None`; `skill_activation_source` field removed.
- `retrieval_lookup_impl.rs`: `SkillActivationMessageTextResolver` stub removed or
  replaced with a DB-backed equivalent.
- `lifecycle.rs` install/remove operations: the VFS write is no longer performed.
  `install_skill` + `remove_skill` management methods either become stubs or are
  removed entirely.
- `brassclaw_skills::management` module: `install_skill` / `remove_skill` /
  `list_skills` functions are now only used by `lifecycle.rs`. Once that caller is
  removed, the management module becomes dead code (except `SkillSummary` and the
  remaining types which may be used elsewhere).
- `first_party_extension_ports/src/skills.rs`: remove entirely.
- `FilesystemSkillBundleSource` / `FilesystemSkillBundleRoot`: still used by many
  tests in `brassclaw_loop_support` and `brassclaw_first_party_extension_ports`.
  These test utilities stay (they test the mechanics in isolation).
  
**Key constraint:** `FilesystemSkillBundleSource`, `SkillBundleContextSource`,
`SelectableSkillContextSource`, `HostSkillContextSource` are general-purpose
abstractions that may be used outside the VFS-skills context. They should NOT be
deleted — only the concrete wiring in composition (`runtime.rs`) and the
`FirstPartySkillsExtension` / `FirstPartySkillsExtensionHandles` wrappers that
configure the VFS roots should be removed.

## Steps

### Step 1 — Remove `SkillSource::System` from management layer

Files:
- `crates/brassclaw_skills/src/management.rs` — remove `System` variant + its
  `as_str` arm.
- `crates/brassclaw_product_workflow/src/lifecycle.rs` — remove
  `LifecycleSkillSource::System` variant; simplify `as_str`.
- `crates/brassclaw_reborn_composition/src/lifecycle.rs` — simplify the
  `skill_summary_to_lifecycle` match to map all remaining sources to
  `LifecycleSkillSource::User`.

### Step 2 — Remove `FirstPartySkillsExtensionHandles` / `FirstPartySkillsExtension`

File: `crates/brassclaw_first_party_extension_ports/src/skills.rs`
- Delete the file entirely (all contents).
- Remove `mod skills;` / `pub use skills::...` from `src/lib.rs`.
- Remove the imports in `src/activation.rs` that reference `SkillSourceKind::System`
  mapping to `SkillSource::Bundled`.

### Step 3 — Remove VFS skill context source wiring from runtime.rs

File: `crates/brassclaw_reborn_composition/src/runtime.rs`
- Remove `skill_activation_source: Option<Arc<LocalDevSelectableSkillContextSource>>`
  field from the runtime state struct.
- Remove `local_dev_runtime_wires_skill_context_source` function.
- Remove the `LocalDevSelectableSkillContextSource` / `LocalDevSkillExecutionAdapter`
  type aliases.
- Everywhere the skill activation source was threaded into the loop, pass `None`
  or remove the parameter.

### Step 4 — Remove VFS skill context source from retrieval_lookup_impl.rs

File: `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs`
- Remove `SkillActivationMessageTextResolver` (wraps `SelectableSkillContextSource`).
- Remove its constructor + wire site in `runtime.rs`.

### Step 5 — Remove VFS skill install/remove from lifecycle.rs (composition)

File: `crates/brassclaw_reborn_composition/src/lifecycle.rs`
- `RebornLocalSkillManager` struct: remove `filesystem` + `skill_management_mounts`
  fields; remove `install`, `install_with_source_url`, `remove` methods.
- `list` + `search` methods now have nothing to read from the VFS; they could
  return empty (since the DB path handles listing) — but the whole `RebornLocalSkillManager`
  is only used in the skills lifecycle service, so inspect whether it can be
  removed entirely.
- `list_skills` in `RebornServicesImpl` (the composition impl of `RebornServices`):
  currently calls `self.skill_manager.list()` then maps to `LifecycleSkillSummary`.
  With no VFS skills this is always empty. The DB-backed list is a separate path.
  Make `list_skills` return an empty `RebornListSkillsResponse` with a doc comment
  noting the DB path is `fetch_llm_skills_as_json`.
- Remove all imports that are now unused.

### Step 6 — Stub or remove the WebUI skills install/remove endpoints

`POST /api/webchat/v2/skills/install` calls `services.install_skill(...)` which
calls `RebornLocalSkillManager::install_with_source_url`. Once that is removed,
the handler either returns a `501 Not Implemented` or is removed from the router.
Per the v3 architecture: skills come from DB, not from user paste/URL installs.
Return `501` with a message explaining that skill installation is no longer
supported via file upload; use the component DB instead.

`DELETE /api/webchat/v2/skills/{name}` similarly: return `501`.

The handler in `crates/brassclaw_webui_v2/src/handlers.rs` for both routes:
replace body with `Err(WebUiV2HttpError::not_implemented(...))` or equivalent.

### Step 7 — Clean up management.rs dead code

After Step 5, `install_skill`, `remove_skill`, and `list_skills` in
`brassclaw_skills::management` may become unreferenced. Check and remove if so.
`SkillSummary`, `SkillSource` (remaining `User`/`Installed`), `SkillInstallResult`,
`SkillRemoveResult`, `SkillInstallRequest`, `SkillRemoveRequest`, `SkillSearchRequest`,
`SkillSearchResult` are all used by the management module's tests and possibly
external callers — check each.

### Step 8 — Update lib.rs exports and imports

Remove all `pub use` / `use` that reference the removed items from:
- `crates/brassclaw_first_party_extension_ports/src/lib.rs`
- `crates/brassclaw_skills/src/lib.rs`
- `crates/brassclaw_reborn_composition/src/lib.rs`

### Step 9 — Build, clippy, tests

```
df -h /Users/ollama/brassclaw-target
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo build -p brassclaw_reborn_composition -p brassclaw_skills -p brassclaw_product_workflow -p brassclaw_first_party_extension_ports
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy -p brassclaw_skills -p brassclaw_product_workflow -p brassclaw_reborn_composition -p brassclaw_first_party_extension_ports --all-targets -- -D warnings
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_skills -p brassclaw_product_workflow -p brassclaw_reborn_composition -p brassclaw_first_party_extension_ports
```

### Step 10 — Commit and push

One commit covering all removals with a clear message.

## Status

- [ ] Step 1 — Remove SkillSource::System from management layer
- [ ] Step 2 — Remove FirstPartySkillsExtensionHandles / FirstPartySkillsExtension
- [ ] Step 3 — Remove VFS skill context source wiring from runtime.rs
- [ ] Step 4 — Remove SkillActivationMessageTextResolver from retrieval_lookup_impl.rs
- [ ] Step 5 — Remove VFS skill install/remove from lifecycle.rs
- [ ] Step 6 — Stub install/remove endpoints with 501
- [ ] Step 7 — Clean up management.rs dead code
- [ ] Step 8 — Update lib.rs exports and imports
- [ ] Step 9 — Build, clippy, tests
- [ ] Step 10 — Commit and push
