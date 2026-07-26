# Subplan: Step 6.7 — fetch_for_turn / intent-driven retrieval

## Status: ✅ IMPLEMENTED

`fetch_for_turn` default method added to `RetrievalSource` trait.
`FetchForTurnResult` enum added (`Components` + `Disambiguation` variants).
`PostgresSource::fetch_for_turn` override implemented with `resolve_intent`
→ match/disambiguation/no-match fallback. `handle_assemble_prior_knowledge`
uses `fetch_for_turn`. All items 1–7 complete.

## Goal (historical)
Replace the "load all docs" UNION ALL path in `PostgresSource::fetch_for_consumer`
with an intent-driven two-stage lookup:
1. `resolve_intent(pool, scope, query)` → component_id / disambiguation / no-match
2. If match: fetch the specific component(s) by ID
3. If no-match / fallback: fall through to existing UNION ALL (keyword path)

## What's already done
- `resolve_intent(pool, scope, query)` exists in `intent_system.rs` (feature `skills-db`)
- `PostgresSource::fetch_for_consumer` does the UNION ALL query (already correct)
- `handle_assemble_prior_knowledge` uses `fetch_for_consumer` on the Phase 5 path
- `IntentResolution::{Match, Disambiguation, NoMatch, DbLessFallback}` all defined

## What needs to be added

### 1. `fetch_for_turn` method on `RetrievalSource` trait (with default impl)
Add `fetch_for_turn` to the `RetrievalSource` trait with a default impl that
calls `fetch_for_consumer` (so `RamSource` gets it for free):

```rust
async fn fetch_for_turn(
    &self,
    scope: &ComponentScope,
    query: &str,
    token_budget: usize,
    sender_class_code: &str,
) -> Result<FetchForTurnResult, RetrievalSourceError> {
    let items = self.fetch_for_consumer(scope, query, token_budget, sender_class_code).await?;
    Ok(FetchForTurnResult::Components(items))
}
```

### 2. `FetchForTurnResult` enum
```rust
pub enum FetchForTurnResult {
    /// Specific component(s) fetched by ID after intent match.
    Components(Vec<ComponentItem>),
    /// Multiple candidates — disambiguation needed.
    Disambiguation(Vec<IntentCandidate>),
}
```

### 3. `PostgresSource::fetch_for_turn` override
Override `fetch_for_turn` in `PostgresSource` to use `resolve_intent`:

```rust
// a. Resolve intent
match resolve_intent(pool, intent_scope, query).await? {
    IntentResolution::Match { component_id, component_class_code } => {
        // b. Increment score atomically (PERF-03, SEC-05)
        let _ = increment_score(pool, intent_scope, component_id).await;
        // c. Fetch the specific component by ID
        let items = fetch_by_id(pool, scope, component_id, component_class_code).await?;
        return Ok(FetchForTurnResult::Components(items));
    }
    IntentResolution::Disambiguation { candidates } => {
        return Ok(FetchForTurnResult::Disambiguation(candidates));
    }
    IntentResolution::NoMatch | IntentResolution::DbLessFallback => {
        // Fall through to UNION ALL keyword path.
    }
}
// d. Fallback: existing UNION ALL via fetch_for_consumer
let items = self.fetch_for_consumer(scope, query, token_budget, sender_class_code).await?;
Ok(FetchForTurnResult::Components(items))
```

### 4. `handle_assemble_prior_knowledge` uses `fetch_for_turn`
Update the Phase 5 path in `orchestrator.rs` to call `fetch_for_turn` instead
of `fetch_for_consumer`, and handle the `Disambiguation` variant by returning
a structured disambiguation result to Python.

### 5. Wire `IntentScope` from `ComponentScope`
Add a helper to convert `ComponentScope` → `IntentScope`.

### 6. `fetch_by_id` helper in `PostgresSource`
Fetch a single component by `(id, class_code)` from the right table.
Use the same `COALESCE(prior_knowledge_content, content)` pattern.
Add `validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)` guard
(SEC-01 — always enforce even on ID fetch).

### 7. "AI before User" silent path
When `sender_class_code == "00"` (action/tool context), skip writing new
`reborn_intent_inputs` rows. The `resolve_intent` function already handles this
by not inserting new rows — it only increments existing scores. So no code
change is needed beyond not inserting when the AI is the caller.

Actually: "AI before User" means the AI's response is injected before the user
sees the disambiguation message. This is a `reborn_user_preferences` key
`ai_before_user`. When ON, the intent no-match path silently invokes AI
completion before showing disambiguation. This requires WebUI plumbing (Step 8.2)
and is explicitly out of scope for Step 6.7 in the non-WebUI Rust path.
For Step 6.7: the Rust path simply calls the UNION ALL fallback on no-match —
the "AI before User" preference check is deferred to Step 8.2.

## Files to touch
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  - Add `FetchForTurnResult` enum
  - Add `fetch_for_turn` default method to `RetrievalSource` trait
  - Add `PostgresSource::fetch_for_turn` override (intent-driven)
  - Add `fetch_component_by_id` helper
- `crates/brassclaw_engine/src/executor/orchestrator.rs`
  - Update `handle_assemble_prior_knowledge` to use `fetch_for_turn`
  - Handle `Disambiguation` variant → return disambiguation result to Python
- `checkup.md` — update Step 6.7
