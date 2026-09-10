# Phase P Step 3 — `component_db` Rust Tool Subplan

> **Created:** Phase P Step 3 pre-flight research
> **Status:** [ ] In progress
> **Parent plan:** `docs/agents-v3/subplan_phase_p_steps_2_to_11.md`
> **Master plan ref:** `saved_plan_to_v3.md` Phase P §8 step 3

---

## Goal

Implement the one generic `component_db` Rust Tool (`builtin.component_db`) and
its single ToolSkill row (`ts-component-db`). This is the only kernel-boundary
DB tool in the doc-sync mechanism. Everything else (Skills, Recipes, Action,
PythonCode) is a pure v3 DB component.

---

## Design decisions (from DOC_CONVERSION_MECHANISM_DESIGN.md §4.0.1)

**Ops:** `op ∈ {read_hash, read_row, upsert, mark_stale}`

Plus two ops added during implementation to handle Monty VM constraints
(no stdlib, no `hashlib`, no multi-line string injection):

- `compute_hash` — SHA-256 of input text in Rust, returned as hex string
- `extract_section` — extract a named `## N. title` section from markdown in Rust

**Why in Rust:** The Monty VM has no `hashlib` and cannot safely inject large
markdown text via `{{vars.slotN}}` substitution. Moving these to Rust eliminates
the constraint entirely and keeps the orchestrator layer clean.

**Final op list:** `{read_hash, read_row, upsert, mark_stale, compute_hash, extract_section}`

---

## Dependency approach

`BuiltinFirstPartyTools` currently carries two states:
- `coding_state: CodingCapabilityState`
- `memory_state: memory::MemoryCapabilityState`

**Pattern:** Add `component_db_state: component_db::ComponentDbState` where
`ComponentDbState` carries an `Option<Arc<dyn ComponentDbBackend>>`.

`ComponentDbBackend` is a trait defined in `crates/brassclaw_host_runtime/src/first_party_tools/component_db.rs`
(no separate crate needed — `brassclaw_host_runtime` has the `postgres` feature
already and is already a dep of `brassclaw_reborn_composition`).

The concrete impl `PgComponentDbBackend` lives in `brassclaw_reborn_composition`
and implements `ComponentDbBackend`. Injected at wiring time in `factory.rs`
via a new builder method on `BuiltinFirstPartyTools`.

Without injection (e.g. in tests not wiring the DB), all ops return
`Err(ComponentDbNotWired)`.

---

## Files to create/modify

### New file: `crates/brassclaw_host_runtime/src/first_party_tools/component_db.rs`

```
pub const COMPONENT_DB_CAPABILITY_ID: &str = "builtin.component_db";

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError>

pub trait ComponentDbBackend: Send + Sync {
    fn read_hash<'a>(&'a self, scope: &'a ComponentDbScope, name: &'a str)
        -> BoxFuture<'a, Result<Option<String>, ComponentDbError>>;
    fn read_row<'a>(&'a self, scope: &'a ComponentDbScope, name: &'a str)
        -> BoxFuture<'a, Result<Option<ComponentDbRow>, ComponentDbError>>;
    fn upsert<'a>(&'a self, scope: &'a ComponentDbScope, row: ComponentDbUpsert)
        -> BoxFuture<'a, Result<ComponentDbUpsertResult, ComponentDbError>>;
    fn mark_stale<'a>(&'a self, scope: &'a ComponentDbScope)
        -> BoxFuture<'a, Result<(), ComponentDbError>>;
    fn compute_hash(&self, text: &str) -> String;
    fn extract_section(&self, markdown: &str, title: &str) -> Option<String>;
}

pub struct ComponentDbState {
    backend: Option<Arc<dyn ComponentDbBackend>>,
}

pub(super) async fn dispatch(
    state: &ComponentDbState,
    input: &Value,
) -> Result<Value, FirstPartyCapabilityError>
```

`compute_hash` uses `sha2::Sha256` (already in `brassclaw_host_runtime/Cargo.toml`: `sha2 = "0.10"`).

`extract_section` is pure Rust string scanning: find `## ` + title heading,
collect until the next `## ` heading.

### Modified: `crates/brassclaw_host_runtime/src/first_party_tools/mod.rs`

- Add `mod component_db;`
- Add `pub use component_db::{COMPONENT_DB_CAPABILITY_ID, ComponentDbBackend, ComponentDbState};`
- Add `component_db_state: component_db::ComponentDbState` to `BuiltinFirstPartyTools`
- Add `pub fn with_component_db(mut self, backend: Arc<dyn ComponentDbBackend>) -> Self`
- Add `COMPONENT_DB_CAPABILITY_ID => component_db::dispatch(&self.component_db_state, &request.input).await?`
  to the dispatch match
- Add `component_db::manifest()?` to `builtin_first_party_package()`

### New file: `crates/brassclaw_reborn_composition/src/pg_component_db_backend.rs`

```rust
pub struct PgComponentDbBackend {
    pool: Arc<PgPool>,
    basic_prompt_store: PgBasicPromptStore,
    tenant_id: String,
}

impl ComponentDbBackend for PgComponentDbBackend { ... }
```

### Modified: `crates/brassclaw_reborn_composition/src/factory.rs`

Wire `PgComponentDbBackend` into `BuiltinFirstPartyTools` when the postgres
feature is active:

```rust
let tools = BuiltinFirstPartyTools::default()
    .with_memory_embedding_provider(...)
    .with_chat_memory_writer(...)
    .with_component_db(Arc::new(PgComponentDbBackend::new(pool, basic_prompt_store, tenant_id)));
```

### Modified: `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`

Seed the Tool row (`builtin.component_db`) and ToolSkill row (`ts-component-db`)
in the Pass 15 `seed_doc_sync_group`.

### Modified: `crates/brassclaw_reborn_composition/Cargo.toml`

No new deps needed (already depends on `brassclaw_host_runtime`).

---

## Steps within Step 3

3.1. Write `component_db.rs` with trait + state + manifest + dispatch  
3.2. Wire into `BuiltinFirstPartyTools` dispatch + manifest  
3.3. Write `pg_component_db_backend.rs` in `brassclaw_reborn_composition`  
3.4. Wire into `factory.rs`  
3.5. Seed Tool + ToolSkill rows in `builtin_bootstrap.rs`  
3.6. Clippy + tests

---

## Tests

- Unit test: `compute_hash("hello")` returns correct SHA-256 hex
- Unit test: `extract_section(md, "LLM-summary")` extracts correct section
- Unit test: dispatch with `op="compute_hash"` returns hash string
- Unit test: dispatch without wired backend returns error (not panic)
