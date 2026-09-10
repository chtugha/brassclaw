# Subplan: Wire SkillActivationObserver into PgRetrievalLookup (v3 activation projection)

**Status:** [x] COMPLETE
**Parent:** `subplan_step8_of_plan_skill_context_removal.md` / Step 8 of Phase P
**Triggered by:** dead_code errors on `LiveSkillActivationObserver`, `skill_activation_observer`,
  `skill_activation_id`, and `sanitize_bounded_model_visible_text` after the VFS-based
  observer wiring (`set_activation_observer`) was removed.

## Problem

`LiveSkillActivationObserver` is a complete, tested projection impl for the WebUI skill
activation panel. It converts `SkillActivationObservedEvent`s into live `ProductProjectionItem::
SkillActivation` items. The VFS-based `SkillExecutionAdapter` was the only production caller
of `observe_skill_activation`. That caller is now deleted.

In v3, skill activation happens when the intent system matches a recipe or component via
`PgRetrievalLookup::fetch_for_turn`. The observer must be called at that point.

## Scope

### Files
- `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs` — add observer field
- `crates/brassclaw_reborn_composition/src/runtime.rs` — wire observer when building PgRetrievalLookup
- `crates/brassclaw_reborn_composition/src/projection.rs` — keep `skill_activation_observer` public

### Change spec

#### Step A — Add observer to PgRetrievalLookup

In `retrieval_lookup_impl.rs`, add to `PgRetrievalLookup` (skills-db gated):
```rust
#[cfg(feature = "skills-db")]
pub(crate) struct PgRetrievalLookup {
    source: Arc<brassclaw_engine::memory::PostgresSource>,
    observer: Option<Arc<dyn brassclaw_first_party_extension_ports::SkillActivationObserver>>,
}
```

Add builder method:
```rust
pub(crate) fn with_skill_activation_observer(
    mut self,
    observer: Arc<dyn SkillActivationObserver>,
) -> Self {
    self.observer = Some(observer);
    self
}
```

In `fetch_for_turn`, after a successful `FetchForTurnResult::Components` or
`FetchForTurnResult::SplitResult`, fire the observer if `Some`:
```rust
if let Some(observer) = &self.observer {
    let activations = items.iter().map(|item| SkillActivationRequest {
        name: item.name.clone(),
        mode: SkillActivationMode::ActivationCriteria,
    }).collect();
    observer.observe_skill_activation(SkillActivationObservedEvent {
        run_context: context.clone(),
        activations,
        feedback: Vec::new(),
    });
}
```

Note: `LoopRunContext: Clone` — verify in `brassclaw_turns::run_profile`.

#### Step B — Wire observer in runtime.rs

In `build_reborn_runtime` (runtime.rs), after building `live_projection_publisher`,
when constructing `PgRetrievalLookup`:
```rust
let skill_activation_obs = projection_services
    .skill_activation_observer(Arc::clone(&live_projection_publisher));
let retrieval_lookup = PgRetrievalLookup::new(pg_source)
    .with_skill_activation_observer(skill_activation_obs);
```

#### Step C — Remove test-gate (not needed after Step B wires production)

After Steps A+B, the `skill_activation_observer` method in `projection.rs` will have
a production caller and the dead_code warnings will clear.

## Verification

`cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings`

## Intermediate state

Until this subplan is implemented, the `LiveSkillActivationObserver` machinery is
gated on `#[cfg(test)]` to keep clippy clean. See the TODO comments in
`projection.rs` and `live_progress.rs`.
