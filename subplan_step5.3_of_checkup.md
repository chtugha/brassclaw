# Subplan: Step 5.3 — Retire recipe_store.rs + recipe_library.rs ✅ COMPLETE

> **Parent:** `checkup.md` Step 5.3 + PG-4 GAP (DocType::Recipe / DocType::ToolSkill routing)
> **Goal:** Route `DocType::Recipe` / ToolSkill lookups through Postgres-native stores,
> retire the MemoryDoc-backed implementations, and wire both agent-loop and WebUI surfaces.

---

## Background

Two parallel adapter chains exist:

| Surface | Old (MemoryDoc-backed) | New (PG-native) | Status |
|---------|------------------------|-----------------|--------|
| **Agent loop lookup** | `RecipeLibrary` → `RecipeMatcher` → `Store` | `PgRecipeLibrary` → `reborn_recipes` SQL | PgRecipeLibrary ✅ ready |
| **WebUI CRUD / validation queue** | `StoreBackedRecipeStore` → `Store` (engine) | `PgRecipeStore` (low-level) | ⚠️ no trait impl yet |

The old stores will be deleted **only after** the new ones are fully wired and tested.

---

## Step 1 — Implement `RecipeStore` trait on `PgRecipeStore` ❌

Location: `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`

Implement all 13 methods of `brassclaw_product_workflow::RecipeStore`:

| Method | Approach |
|--------|----------|
| `list_recipes` | SELECT from `reborn_recipes` scoped to user/project |
| `list_tool_skills` | Query from `reborn_tool_skills` (V037); return empty vec when table absent |
| `get_recipe` | Fetch by id from `reborn_recipes` → `RecipeDetail` |
| `get_tool_skill` | Fetch by id from `reborn_tool_skills`; return None when table absent |
| `list_validation_queue` | Filter by `queue_code`/`validation_status`; honour `consumer_tags` exclusion |
| `count_by_status` | COUNT(*) grouped by `validation_status` |
| `update_recipe_validation_status` | Call existing `update_validation_status()` + `pop_validator_tag()` |
| `update_skill_validation_status` | Update `reborn_tool_skills` (no-op until V037) |
| `update_component_validation_status` | Dispatch by `class_code` |
| `re_review_component` | Set `Rejected → Pending`, pop `05:validator` tag |
| `delete_component` | Soft-delete via `garbage_collected = true` / hard delete |
| `get_component_audit_status` | Read `llm_audit_status` / `llm_audit_findings` from the row |
| `record_outcome` | Call existing `record_outcome()` helper |

ToolSkill-specific methods return `Ok(empty/None)` with a `tracing::debug!` until V037 lands.

---

## Step 2 — Wire `PgRecipeStore` in `webui.rs` (replace StoreBackedRecipeStore) ❌

Location: `crates/brassclaw_reborn_composition/src/webui.rs` lines ~211–216

```rust
// Replace:
let recipe_store = crate::recipe_store::StoreBackedRecipeStore::open(Arc::clone(&dyn_store));
api = api.with_recipe_store(Arc::new(recipe_store) as Arc<dyn RecipeStore>);

// With (postgres path):
#[cfg(feature = "postgres")]
if let Some(pool) = services.pg_pool.as_ref() {
    let recipe_store = crate::pg_recipe_store::PgRecipeStore::new(Arc::clone(pool), tenant_id);
    api = api.with_recipe_store(Arc::new(recipe_store) as Arc<dyn RecipeStore>);
}
// Non-postgres: old path retained as fallback (PG-8 cleanup)
```

---

## Step 3 — Wire `PgRecipeLibrary` in `runtime.rs` (replace RecipeLibrary) ❌

Location: `crates/brassclaw_reborn_composition/src/runtime.rs` lines ~1960–1966

```rust
// Replace Store-backed RecipeLibrary with PgRecipeLibrary:
let recipe_lookup = services.pg_pool.as_ref().map(|pool| {
    Arc::new(crate::pg_recipe_store::PgRecipeLibrary::local_dev(Arc::clone(pool)))
        as Arc<dyn RecipeLookup>
});
```

---

## Step 4 — Delete `recipe_store.rs` and retire `recipe_library.rs` ❌

1. Delete `crates/brassclaw_reborn_composition/src/recipe_store.rs` (2,188 lines)
2. In `recipe_library.rs`: delete `RecipeLibrary` struct + `RecipeLookup` impl; keep
   `DisabledRecipeLookup` (used by test compositions)
3. Update `lib.rs` mod declarations to remove the deleted modules

---

## Step 5 — Update `PgMemoryDocStore` comment + clippy ❌

Remove the GAP note from checkup.md PG-4 for the recipe routing once the above is wired.
Run `cargo clippy -p brassclaw_reborn_composition --features postgres,root-llm-provider --all-targets -- -D warnings`.

---

## Step 6 — Mark checkup steps as IMPLEMENTED ❌

- PG-4 GAP (recipe routing) → resolved
- Step 5.3 → IMPLEMENTED (new stores wired, old stores retired)
- Commit + push to `origin/main`
