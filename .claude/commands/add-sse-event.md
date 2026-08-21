---
description: Scaffold a new SSE/WebSocket event end-to-end (Rust engine event to Reborn WebUI frontend)
allowed-tools: Read, Edit, Write, Glob, Grep, Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*)
argument-hint: <event_name> [description]
model: opus
---

Add a new event called `$ARGUMENTS` to the BrassClaw Reborn WebUI. Events flow from the engine event log through projection to the SSE stream. Follow each step exactly.

## Architecture

```
brassclaw_engine::EventKind (engine event)
  → crates/brassclaw_reborn_event_store/ (persisted event)
    → crates/brassclaw_event_projections/ (projection → AppEvent)
      → brassclaw_reborn_composition/src/projection/ (SSE broadcast)
        → crates/brassclaw_webui_v2_static/js/ (frontend listener)
```

All events MUST project from a source log per `.claude/rules/gateway-events.md`. Never broadcast directly from a handler.

## Step 1: Add the engine `EventKind` variant

**File**: `crates/brassclaw_engine/src/` (find the `EventKind` enum)

Add a new variant. Use the event name in PascalCase. Include typed fields — never a generic `String` payload.

```rust
pub enum EventKind {
    // existing variants...
    <EventName> {
        // typed fields
    },
}
```

## Step 2: Add the projection arm

**File**: `crates/brassclaw_event_projections/src/` (find the projection function for `EventKind`)

Add a match arm that converts the new `EventKind` variant to an `AppEvent`. The SSE event name should be `snake_case`.

## Step 3: Add the `AppEvent` variant (if needed)

**File**: `crates/brassclaw_reborn_webui_ingress/src/` or wherever `AppEvent` is defined.

If the event carries structured data, add a serializable variant:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    // existing variants...
    <EventName>(<EventName>Data),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct <EventName>Data {
    // typed fields
}
```

## Step 4: Emit the engine event at the right point

Find the appropriate location in the engine or host runtime where this event should be emitted. Use the existing event emission pattern — never broadcast SSE directly.

```rust
// In the appropriate engine path:
event_tx.send(EventKind::<EventName> { /* fields */ }).await?;
```

## Step 5: Add frontend handler

**File**: `crates/brassclaw_webui_v2_static/js/`

Find the SSE connection setup and add a new `addEventListener` for the snake_case event name:

```js
sse.addEventListener('<event_name>', (e) => {
    const data = JSON.parse(e.data);
    handle<EventName>(data);
});

function handle<EventName>(data) {
    // update the relevant UI section
}
```

## Step 6: Add CSS if needed

If the event needs custom UI (cards, badges, etc.), add styles to the appropriate surface CSS file in `crates/brassclaw_webui_v2_static/styles/`.

## Step 7: Quality gate

```bash
cargo fmt
cargo clippy --all --benches --tests --examples --all-features -- -D warnings
cargo test -p brassclaw_event_projections
```

Verify the `// projection-exempt:` rule is NOT needed — the event flows through the projection layer, not a direct broadcast.

## Checklist

Before finishing, verify:
- [ ] `EventKind` variant added with typed fields
- [ ] Projection arm added (EventKind → AppEvent)
- [ ] `AppEvent` variant added (if needed)
- [ ] Engine event emitted at correct location
- [ ] Frontend `addEventListener` added
- [ ] Frontend handler function created
- [ ] CSS styles added (if needed)
- [ ] `cargo fmt` clean
- [ ] `cargo clippy` clean with `-D warnings`
- [ ] No direct `broadcast` call added outside the projection dispatcher
