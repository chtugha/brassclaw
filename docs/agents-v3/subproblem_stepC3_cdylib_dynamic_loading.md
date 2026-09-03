# Step C.3 — Two Tool Systems: cdylib dynamic loading (subplan)

Parent: `./saved_plan_to_v3.md` Step C.3. Sibling subplans:
`./subplan_problem_stepC2_builtin_seed_of_saved_plan_to_v3.md` (DONE).

## Architecture context (locked, do not relitigate)

- **Orchestrator** = Monty/Python, the one long-persisting main process per user input. It runs
  Recipes (lists of steps), calls Tools **by name** through the `host` namespace, and assembles every
  LLM prompt. It never talks to the LLM directly — that goes through Kohai (`host.kohai_complete`).
- **Executioner** = Rust. It holds the **Two Tool Systems** and executes a Tool on call. It does no
  sequencing, no planning.
  1. **Built-in Tools** — precompiled into the Rust binary. These are the 8 `host.*` tools seeded in
     C.2 (`resolve_intent`, `compose_orchestrator`, `post_reply`, `fetch_component`,
     `resolve_component_by_name`, `validate_component`, `check_signals`, `kohai_complete`) plus the
     reused builtins (`memory_write`, `http`, `skill_list`, `regex_match`, `time`). Dispatched by the
     **static `match call.function_name`** in `orchestrator.rs:756-802` (the C.1 arms).
  2. **Dynamic Tools** — kohai/sempai-minted Tools+ToolSkills that are **NOT** in the binary. They
     ship as **separate `cdylib` crates**, compiled out-of-band, and are **dlopen'd at runtime on
     demand** by a Recipe and **unloaded at main-process task end**. This is what C.3 builds.

### Why this makes composition simpler

A Recipe's `rust_steps` only ever name a tool + tool_skill. If the tool is built-in, the static match
handles it. If the tool is dynamic, the SAME `host.<tool>(...)` call routes through a **dispatch
fallthrough** to the `DynamicToolLoader`. No recipe has to know which system a tool lives in — the
namespace is uniform. New capabilities minted by the kohai→sempai validation loop become cdylib
artifacts + a `reborn_tools` row carrying `cdylib_artifact_path`; the Orchestrator just calls them.

## Locked design forks (user-locked 2026-09-03)

- **F1 = A — in-process `libloading` dlopen.** A Q1 component cannot be run at all (it is stuck in
  the validation queue and never reachable by Rust or the Orchestrator), so every *runnable* cdylib
  tool is already **Q2+ validated/trusted**. There is no "less than Q2+" executable tier, so a
  sidecar/sandboxed-subprocess branch would be dead logic. Pure in-process dlopen it is. (Accepted
  cost: a cdylib segfault takes down the main process — but Q2+ trust + Matching-Mode security-off
  make this the intended tradeoff. The existing `sandbox_process` / `brassclaw_process_sandbox`
  docker backend is NOT used for cdylib tools.)
- **F2 = A — JSON `extern "C"` ABI.** Every dynamic cdylib Tool exports a single stable symbol:
  `extern "C" fn brassclaw_tool_invoke(payload: *const c_char, payload_len: usize,
  out: *mut *mut c_char, out_len: *mut usize) -> i32` plus
  `extern "C" fn brassclaw_tool_drop_out(buf: *mut c_char, len: usize)`. Request and response are
  JSON serialized as UTF-8 `c_char` buffers. Stable across rustc versions and language-agnostic (a
  future non-Rust cdylib could honor the same ABI). Serialization overhead is accepted.
- **F3 = A — new `cdylib_artifact_path` column on `reborn_tools`.** Nullable `TEXT`. `NULL` for the
  precompiled built-in tools (they live in the binary); the cdylib filesystem path for
  kohai/sempai-minted dynamic tools. Added by migration **V067**. The `DynamicToolLoader` reads this
  path to dlopen. No filesystem blob storage in DB — just the path.
- **F4 = B — dedicated `DynamicToolLoader` service in `brassclaw_host_runtime`.** It owns the actual
  dlopen / symbol-bind / invoke / unload mechanics and the per-task loaded-tool map. The
  **composition-mechanism** (the `host.compose_orchestrator` handler rewrite, DEFERRED to C.5/C.6) is
  what *produces* the load directives: on intent match it fetches the matched component, splits its
  instructions into a PYTHON part (the Monty program) + a RUST part, injects the RUST part into the
  executioner, and that RUST part **carries the cdylib-tool load directives** (tool_name →
  artifact_path). The executioner hands those directives to the `DynamicToolLoader`, which dlopens +
  binds them into the `host` namespace on demand and unloads them at main-process task end.

## Scope decision (surfaced for correction before slice 1)

**C.3 delivers the cdylib load/unload PRIMITIVES only:**

1. V067 migration + the `cdylib_artifact_path` field on `NewPgTool` + store read/write.
2. `brassclaw_host_api::cdylib_abi` — the JSON `extern "C"` ABI signature + request/response serde
   types + the `CdylibToolInvoke` fn-pointer type + a host-side invoke helper.
3. `DynamicToolLoader` service in `brassclaw_host_runtime` — `load` / `invoke` / `unload` /
   `unload_all` over `libloading` + the directive-acceptance API (`Vec<CdylibLoadDirective>`).
4. Unit tests with a **fixture cdylib compiled in a tempdir by `rustc --crate-type cdylib`** (no
   separate fixture crate, no target-dir artifact hunt) — load, JSON round-trip, unload, `unload_all`.
5. **Executioner dispatch fallthrough** — when a `host.<name>(...)` call is not in the static match,
   consult the `DynamicToolLoader`'s loaded map and dispatch via `invoke`, else `NotFound`. The bridge
   from C.1's static match to dynamic tools. Unit-tested with the fixture.

**DEFERRED to C.5/C.6:** the `host.compose_orchestrator` HANDLER REWRITE — the composition-mechanism
that fetches the matched component + splits instructions into python/rust parts + injects the rust
part carrying the cdylib load directives. The loader has no production caller until that rewrite
lands; the loader is fully unit-testable in isolation meanwhile (load a fixture cdylib, call it,
unload). `#[allow(dead_code)]` on the loader's directive-acceptance API is acceptable for the gap.

## Slices (one commit each, both configs clippy green, push to main)

- **Slice 1 — V067 migration only.** `V067__reborn_tools_cdylib_artifact_path.sql`: `ALTER TABLE
  reborn_tools ADD COLUMN IF NOT EXISTS cdylib_artifact_path TEXT;` (+ a partial index
  `reborn_tools_cdylib_path_idx ... WHERE cdylib_artifact_path IS NOT NULL` for the
  composition-mechanism's "which tools to dlopen" lookup). The column defaults to NULL; all current
  built-in seed rows stay NULL (built-ins live in the binary). The store WRITE/READ of the path lands
  with the dynamic-tool AUTHORING surface (a later step) — NOT in C.3, because the seed only ever
  inserts built-ins (NULL) and the loader takes the path via a `CdylibLoadDirective` the
  composition-mechanism (C.5/C.6) builds. Migrations are auto-discovered by
  `refinery::embed_migrations!("migrations")` (compile-time embed, version-ordered) — no registry to
  touch. Verify: `cargo check -p brassclaw_pg` (confirms the embed accepts the new filename); the SQL
  itself runs at runtime (testcontainer validation is CI-only — Docker unavailable locally).

- **Slice 2 — `brassclaw_host_api::cdylib_abi` ABI module.** New module in
  `crates/brassclaw_host_api/src/cdylib_abi.rs` (re-exported from lib.rs). Defines:
  - `pub const CDYLIB_TOOL_INVOKE_SYMBOL: &str = "brassclaw_tool_invoke";`
  - `pub const CDYLIB_TOOL_DROP_OUT_SYMBOL: &str = "brassclaw_tool_drop_out";`
  - `pub type CdylibToolInvoke = unsafe extern "C" fn(*const c_char, usize, *mut *mut c_char, *mut usize) -> i32;`
  - `pub type CdylibToolDropOut = unsafe extern "C" fn(*mut c_char, usize);`
  - `CdylibRequest { tool: String, args: Value }` / `CdylibResponse { ok: bool, result: Option<Value>, error: Option<String> }` (serde_json).
  - A host-side `invoke_via_fn(invoke_fn, drop_fn, req) -> Result<Value>` helper that serializes the
    request, calls the fn, reads `out`/`out_len`, deserializes the response, frees the out buffer via
    `drop_fn`, and maps non-zero return codes to an error. No `unwrap`/`expect` — all fallible paths
    return `Result`. Unit-test the (de)serialization + the helper against an in-process mock fn (no
    dlopen) — proves the ABI buffer protocol.

- **Slice 3 — `DynamicToolLoader` service in `brassclaw_host_runtime`.** New module
  `crates/brassclaw_host_runtime/src/dynamic_tool_loader.rs`. Holds:
  - `struct LoadedTool { library: libloading::Library, invoke_fn: CdylibToolInvoke, drop_fn: CdylibToolDropOut }`
  - `struct DynamicToolLoader { loaded: HashMap<String, LoadedTool> }` (behind a `Mutex` or
    `&mut`-style single-owner — the executioner is single-threaded per turn, so a `RefCell`/`&mut` is
    fine; pick the simplest that clippy accepts).
  - `CdylibLoadDirective { tool_name: String, artifact_path: PathBuf }`.
  - `fn load(&mut self, directive: CdylibLoadDirective) -> Result<()>` — `unsafe` dlopen via
    `libloading::Library::new`, `get` both symbols, store. Errors: file-not-found / missing symbol →
    a `thiserror` enum in the crate's `error.rs` (`DynamicToolLoaderError`).
  - `fn load_directives(&mut self, directives: Vec<CdylibLoadDirective>) -> Result<()>` — the
    directive-acceptance API (the "rust part" hand-off point; `#[allow(dead_code)]` until C.5/C.6).
  - `fn invoke(&self, tool_name: &str, args: Value) -> Result<Value>` — look up, build a
    `CdylibRequest`, call the ABI helper, return the `result`.
  - `fn unload(&mut self, tool_name: &str) -> Result<()>` and `fn unload_all(&mut self)` (task-end).
  - `fn is_loaded(&self, tool_name: &str) -> bool` (used by the dispatch fallthrough).
  Add `libloading` to `brassclaw_host_runtime`'s Cargo deps. No `unwrap`/`expect`; `thiserror` errors.

- **Slice 4 — fixture cdylib + loader round-trip unit tests.** In
  `dynamic_tool_loader.rs`'s `#[cfg(test)] mod tests`: write a tiny `.rs` source to a tempdir that
  exports `brassclaw_tool_invoke` + `brassclaw_tool_drop_out` matching the ABI (hardcoded signature —
  the fixture does NOT depend on the ABI crate, keeping it self-contained) and implements an echo
  (read the JSON request, add a `"echoed": true` field, return it). Compile it with
  `std::process::Command::new("rustc").args(["--edition","2021","--crate-type","cdylib","-o",
  <out.dylib>, <src.rs>])`. Skip the test if `rustc` is not on PATH or compile fails. Then: construct
  a `DynamicToolLoader`, `load` a directive pointing at the fixture, `invoke` with args, assert the
  echoed response, `unload`, assert `is_loaded` false, and an `unload_all` test. This is the only
  automated proof that dlopen + the ABI + the loader mechanics actually work end-to-end.

- **Slice 5 — executioner dispatch fallthrough + test.** In
  `crates/brassclaw_engine/src/executor/orchestrator.rs`, the `host` dispatch match (the C.1 block at
  `orchestrator.rs:756-814`) currently ends with `other => ExtFunctionResult::NotFound(other)`.
  Insert a **dynamic-tool branch BEFORE the `NotFound` fallthrough**: if the bare name is in the
  `DynamicToolLoader`'s loaded map, build a `CdylibRequest` from `args[1..]` + `kwargs`, call
  `loader.invoke`, return the JSON result as a `MontyObject` (or an error result). The loader is
  threaded into `execute_orchestrator` as a new `&mut DynamicToolLoader` parameter (owned by the
  per-turn executioner state; `#[allow(dead_code)]` on the param until C.5/C.6 wires real directives).
  Unit-test: seed the loader with the slice-4 fixture (compiled in-test), run a tiny Monty program
  `host.fixture_echo(x=2)`, assert the echoed result comes back through the dispatch.

## Before-finishing gates (every slice)

- `df -h /Users/ollama/brassclaw-target` — `cargo clean -p <crate>` first if Avail < 15GB or
  Capacity > 90%. `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build/test/clippy/check.
- `cargo clippy -p <crate> --all-targets -- -D warnings` (the `-D warnings` AFTER `--`).
- `cargo test -p <crate> --lib` where a lib exists.
- Both configs where relevant: default (`postgres`,`root-llm-provider`) + `--features skills-db`.
- Mark the slice complete in `saved_plan_to_v3.md` + append a mindmap line + commit + push to main.
