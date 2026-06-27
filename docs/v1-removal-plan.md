# v1 WASM/Script Extension Removal Plan

**9 phases · ~15 commits · compile-checked after every phase · deploy after final phase**

Removes all v1 WASM and Script extension infrastructure from BrassClaw, including 5 whole crates,
5 first-party asset directories, the runtime adapters wired into `brassclaw_host_runtime`, and
ultimately the `RuntimeKind::Wasm/Script` variants from `brassclaw_host_api`.

---

## What Gets Removed

| Crate / File | ~Lines | Action | Phase |
|---|---|---|---|
| `brassclaw_wasm` | 1 410 | DELETE entire crate | 6 |
| `brassclaw_wasm_sandbox_core` | 357 | DELETE entire crate | 6 |
| `brassclaw_wasm_limiter` | 144 | DELETE entire crate | 6 |
| `brassclaw_wasm_product_adapters` | 3 416 | DELETE entire crate | 5 |
| `brassclaw_scripts` | 805 | DELETE entire crate | 7 |
| `ExtensionRuntime::Wasm/Script` variants | — | Remove from `brassclaw_extensions` | 8 |
| `ExtensionRuntimeV2::Wasm/Script` variants | — | Remove from `brassclaw_extensions` | 8 |
| `RuntimeKind::Wasm/Script` variants | — | Remove from `brassclaw_host_api` | 9 |
| `LifecycleExtensionRuntimeKind::WasmTool/Script` | — | Remove from `brassclaw_product_workflow` | 1 |
| JS: `wasm_channel` in settings/extensions UI | — | 3 JS files | 2 |
| github WASM assets in `brassclaw_first_party_extensions` | — | DELETE assets dir | 3 |
| google-docs/drive/sheets/slides assets | — | DELETE asset dirs | 3 |
| `WasmRuntimeAdapter`, `ScriptRuntimeAdapter` in `brassclaw_host_runtime` | — | Remove from services layer | 6 |
| `brassclaw_host_runtime::wasm_credentials` | — | DELETE file | 6 |
| `brassclaw_hooks` — `brassclaw_wasm_limiter` dep | — | Remove dep + usage | 6 |
| `brassclaw_reborn` — `brassclaw_scripts` dep | — | Remove dep + usage | 7 |

---

## Dependency Graph (simplified)

```
brassclaw_wasm_limiter  ←  brassclaw_wasm, brassclaw_hooks
brassclaw_wasm_sandbox_core  ←  brassclaw_wasm_product_adapters
brassclaw_wasm  ←  brassclaw_host_runtime, brassclaw_hooks, brassclaw_reborn_composition
brassclaw_wasm_product_adapters  ←  brassclaw_reborn_composition, brassclaw_telegram_v2_adapter
brassclaw_scripts  ←  brassclaw_host_runtime, brassclaw_reborn
```

---

## Phase 1 — Product-layer lifecycle enum cleanup

**Scope:** `brassclaw_product_workflow`, `brassclaw_reborn_composition` · 1 commit · zero blast radius

These are pure metadata variants that only generated the `"wasm_tool"` wire string for the
registry UI. No dispatch path touches them.

- **1.1** `brassclaw_product_workflow/src/lifecycle.rs` — remove `WasmTool` and `Script` variants
  from `LifecycleExtensionRuntimeKind`; remove their arms from `wire_kind()`.
- **1.2** `reborn_services/extension_onboarding.rs` — two test fixtures use `WasmTool` as dummy
  kind; change to `FirstParty`.
- **1.3** `reborn_services/extensions.rs` — test fixture at line ~406 uses `WasmTool`; change to
  `FirstParty`.
- **1.4** `brassclaw_reborn_composition/src/available_extensions.rs` — remove `Wasm => WasmTool`
  and `Script => Script` arms from `runtime_kind()`; remove `ExtensionRuntime::Wasm { module }`
  block in `load_filesystem_packages()`.
- **1.5** `extension_lifecycle_command.rs` test fixture line ~236 — change `WasmTool` to
  `FirstParty`.
- ✓ `cargo check -p brassclaw_product_workflow -p brassclaw_reborn_composition`
- Commit: `chore(lifecycle): remove WasmTool/Script from LifecycleExtensionRuntimeKind`

---

## Phase 2 — Frontend: remove remaining wasm_channel dead code

**Scope:** 3 JS files · 1 commit · no Rust changes

- **2.1** `settings/hooks/useChannels.js` lines 26 & 28 — remove `e.kind === "wasm_channel"` and
  `e.kind === "channel"` branches (channels are now always empty).
- **2.2** `extensions/components/extension-card.js` lines 144 & 154 — remove the two
  `wasm_channel` conditional branches.
- **2.3** `extensions/lib/extension-actions.js` line 15 — remove the
  `if (ext.kind === "wasm_channel")` branch.
- Commit: `ui: remove wasm_channel dead code from extensions UI`

---

## Phase 3 — Delete WASM asset directories

**Scope:** 5 asset dirs · 1 commit · no Rust changes needed (includes removed from catalog already)

```bash
rm -rf crates/brassclaw_first_party_extensions/assets/github/
rm -rf crates/brassclaw_first_party_extensions/assets/google-docs/
rm -rf crates/brassclaw_first_party_extensions/assets/google-drive/
rm -rf crates/brassclaw_first_party_extensions/assets/google-sheets/
rm -rf crates/brassclaw_first_party_extensions/assets/google-slides/
```

- ✓ `cargo check -p brassclaw_reborn_composition` — must be clean (no `include_bytes!` pointing there)
- Commit: `chore: delete WASM extension asset directories (github, google-docs/drive/sheets/slides)`

---

## Phase 4 — Remove gsuite WASM dispatch from brassclaw_reborn_composition

**Scope:** `brassclaw_reborn_composition/src/gsuite.rs`, `extension_lifecycle_capabilities.rs` · 1 commit

> **First:** audit whether gmail and google-calendar share the same gsuite handler path.
> If so, split them to a non-WASM first-party handler before removing.

- **4.1** Audit `register_bundled_gsuite_first_party_handlers` and `gsuite_package_specs()` to
  confirm gmail/calendar are first-party only, not WASM.
- **4.2** Remove WASM dispatch from `gsuite.rs` — the `wasm/google_*_tool.wasm` dispatch path.
- **4.3** `extension_lifecycle_capabilities.rs` line ~350 — remove
  `missing_runtime_backends.contains(&RuntimeKind::Wasm)` health check.
- ✓ `cargo check -p brassclaw_reborn_composition`
- Commit: `refactor(gsuite): remove WASM dispatch path; keep first-party gmail/calendar handlers`

---

## Phase 5 — Delete brassclaw_wasm_product_adapters crate

**Scope:** optional dep in 2 crates · 1 commit

- **5.1** Remove `brassclaw_wasm_product_adapters` optional dep from
  `brassclaw_reborn_composition/Cargo.toml` and its feature flag entry.
- **5.2** Remove dep from `brassclaw_telegram_v2_adapter/Cargo.toml` and any
  `#[cfg(test)]` usage referencing it.
- **5.3** Remove all `#[cfg(feature = "…wasm_product_adapters")]` guarded code from
  `brassclaw_reborn_composition/src/factory.rs` and any other files.
- **5.4** Remove crate from workspace `Cargo.toml` members list and global dep declaration.
- **5.5** `rm -rf crates/brassclaw_wasm_product_adapters/`
- ✓ `cargo check` (workspace)
- Commit: `chore: delete brassclaw_wasm_product_adapters crate`

---

## Phase 6 — Remove WASM runtime from brassclaw_host_runtime + delete wasm crates

**Scope:** `brassclaw_host_runtime` (5+ files), `brassclaw_hooks` · 2 commits · largest phase

> ⚠ Work file-by-file, compile-check after each sub-step.

- **6.1** `services/runtime_adapters.rs` — delete `WasmRuntimeAdapter` struct, its impl,
  `ExtensionRuntime::Wasm { module }` arm at line ~516, `RuntimeKind::Wasm` dispatch error arm
  at line ~791. Remove `brassclaw_wasm::*` imports.
- **6.2** `services/production_services.rs` — remove `ProductionWiringComponent::WasmRuntime`
  and its wiring block (lines ~156–182).
- **6.3** `services.rs` — remove `wasm_runtime: Option<…>` field, builder methods,
  `backends.push(RuntimeKind::Wasm)`, rank entry (lines ~155, 162, 332, 358, 428, 616–617, 775).
- **6.4** `services/builder.rs` — remove Wasm capability filter at line ~938.
- **6.5** `wasm_credentials.rs` — DELETE file; remove its `mod` declaration.
- **6.6** `surface.rs` — remove `RuntimeKind::Wasm` from surface list (line 19) and its
  string arm (line 435).
- **6.7** `obligations.rs` — replace 4 `RuntimeKind::Wasm` test fixtures (lines ~2365, 2446,
  2532, 2606) with `RuntimeKind::Mcp`.
- **6.8** Remove `brassclaw_wasm` and `brassclaw_wasm_limiter` from
  `brassclaw_host_runtime/Cargo.toml`.
- **6.9** `brassclaw_hooks` — remove `brassclaw_wasm_limiter` dep from `Cargo.toml`; remove
  `WasmResourceLimiter` usage from `capability_port.rs` middleware.
- ✓ `cargo check -p brassclaw_host_runtime -p brassclaw_hooks`
- Commit: `refactor(host_runtime): remove WASM runtime adapter and wiring`
- **6.10** `rm -rf crates/brassclaw_wasm/ crates/brassclaw_wasm_sandbox_core/ crates/brassclaw_wasm_limiter/`
- Remove all 3 from workspace `Cargo.toml` members and global dep declarations.
- ✓ `cargo check` (workspace)
- Commit: `chore: delete brassclaw_wasm, brassclaw_wasm_sandbox_core, brassclaw_wasm_limiter crates`

---

## Phase 7 — Remove Script runtime + delete brassclaw_scripts

**Scope:** `brassclaw_host_runtime`, `brassclaw_reborn` · 2 commits

- **7.1** `services/runtime_adapters.rs` — delete `ScriptRuntimeAdapter`, `RuntimeKind::Script`
  dispatch error arm at line ~790. Remove `brassclaw_scripts` imports.
- **7.2** `services/production_services.rs` — remove `ProductionWiringComponent::ScriptRuntime`
  and its wiring block (lines ~156–168).
- **7.3** `services.rs` — remove `script_runtime` field, registration, backend push, rank entry
  (lines ~159, 329, 358, 401–405, 622–623, 777).
- **7.4** `planner.rs` line ~130 — remove `RuntimeKind::Script` from `needs_process`; condition
  simplifies to `descriptor.runtime == RuntimeKind::Mcp`.
- **7.5** `services/process_executor.rs` — replace `RuntimeKind::Script` test fixture usages
  (lines ~161, 210, 233, 282, 312) with `RuntimeKind::Mcp`.
- **7.6** `brassclaw_reborn/src/milestone_events.rs` — replace `RuntimeKind::Script` test
  fixtures (lines ~534, 555, 581) with `RuntimeKind::Mcp`.
- **7.7** Remove `brassclaw_scripts` dep from `brassclaw_host_runtime/Cargo.toml` and
  `brassclaw_reborn/Cargo.toml`.
- ✓ `cargo check -p brassclaw_host_runtime -p brassclaw_reborn`
- Commit: `refactor(host_runtime): remove Script runtime adapter and wiring`
- **7.8** `rm -rf crates/brassclaw_scripts/`
- Remove from workspace `Cargo.toml` members and global dep.
- ✓ `cargo check` (workspace)
- Commit: `chore: delete brassclaw_scripts crate`

---

## Phase 8 — Remove Wasm/Script from ExtensionRuntime in brassclaw_extensions

**Scope:** `brassclaw_extensions/src/lib.rs`, `v2.rs`, test contracts · 1 commit

By this phase, no live code produces or consumes `ExtensionRuntime::Wasm/Script`.

- **8.1** `brassclaw_extensions/src/lib.rs` — remove `Wasm` and `Script` variants from
  `ExtensionRuntime`; remove `RuntimeKind` mapping arms and `from_v2()` branches.
- **8.2** `brassclaw_extensions/src/v2.rs` — remove `Wasm` and `Script` from
  `ExtensionRuntimeV2` and the TOML parse enum. Make parser return an error:
  `"wasm and script runtimes are no longer supported"`.
- **8.3** Update test contracts in `brassclaw_extensions/tests/` — replace WASM/Script
  `ExtensionRuntime` construction with an assertion of the new parse error.
- **8.4** `available_extensions.rs` — delete the now-unreachable `if let ExtensionRuntime::Wasm`
  block in `load_filesystem_packages()` (line ~1051).
- ✓ `cargo check && cargo test -p brassclaw_extensions`
- Commit: `refactor(extensions): remove Wasm/Script variants from ExtensionRuntime`

---

## Phase 9 — Remove RuntimeKind::Wasm/Script from brassclaw_host_api

**Scope:** `brassclaw_host_api` + exhaustiveness fixes across ~10 crates · 1 commit · final phase

> ⚠ `RuntimeKind` is in `brassclaw_host_api`, the lowest-level crate. Do this last.
> Every crate holding a `CapabilityDescriptor` is affected. Compiler surfaces all stragglers.

- **9.1** `brassclaw_host_api/src/runtime.rs` — remove `Wasm` and `Script` from `RuntimeKind`.
- **9.2** Fix all match exhaustiveness errors — expected in:
  - `brassclaw_host_api/src/dispatch.rs` — remove `DispatchError::Wasm/Script` variants
  - `brassclaw_loop_support/capability_info.rs` — remove string arms `"wasm"` / `"script"`
  - `brassclaw_events/runtime_event.rs` — remove `TrustedRuntimeKindWire::Wasm/Script` variants
  - `brassclaw_capabilities/error.rs` — update match arms
  - `brassclaw_host_runtime/surface.rs` — already cleaned in phase 6
  - `brassclaw_loop_support/capability_surface_filter.rs` — remove wasm match arm
- **9.3** `brassclaw_events/src/runtime_event.rs` — remove `TrustedRuntimeKindWire::Wasm/Script`.
  Add a comment that these variants are no longer produced (historical log compat note).
- **9.4** `brassclaw_host_api/src/dispatch.rs` — remove `DispatchError::Wasm` and
  `DispatchError::Script` variants.
- **9.5** Update test fixtures across all affected crates — replace `RuntimeKind::Wasm/Script`
  with `RuntimeKind::Mcp` or `RuntimeKind::FirstParty` as appropriate:
  - `brassclaw_authorization/tests/`
  - `brassclaw_approvals/tests/`
  - `brassclaw_dispatcher/tests/`
  - `brassclaw_capabilities/tests/`
  - `brassclaw_host_runtime/tests/`
  - `brassclaw_loop_support/tests/`
  - `brassclaw_hooks/`
  - `brassclaw_reborn/tests/`
- ✓ `cargo test` (full workspace) — all tests must pass
- Commit: `refactor(host_api): remove RuntimeKind::Wasm/Script — v1 extension runtime fully removed`

---

## Post-Removal

- Bump `brassclaw_reborn_cli` to `0.31.0` (semver minor — breaking change to extension manifest format)
- `cargo update -p brassclaw_reborn_cli`
- Push + tag `v0.31.0`
- Wait for CI (~17 min)
- Deploy to testmachine

---

## Total Estimated Deletions

| Category | Approx. lines removed |
|---|---|
| Deleted Rust crates (5) | ~6 600 |
| Deleted asset dirs (5) | WASM binaries + ~300 schema/prompt files |
| Host runtime / extensions / events edits | ~800 |
| Test fixture updates | ~150 changed lines |
| JS files | ~20 lines removed |
| **Total Rust source deleted** | **~7 400 lines** |
