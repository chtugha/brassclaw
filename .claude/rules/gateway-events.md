---
paths:
  - "crates/**"
---
# Gateway Events — Single Source of Truth

Every event reaching the SSE/WebSocket stream from the Reborn WebUI must come from a **typed source log**, or be on a small **transport-only allowlist**. Direct broadcast calls from tools, handlers, or extension managers are the root cause of UI state drift — the stream and the replayable source end up telling different stories.

## Why

When `AppEvent` has producers outside the projection layer, those
producers become a second source of truth. On SSE reconnect, replay
from the event log can't reconstruct them (they were never logged). On
tab focus, reconciliation against a GET endpoint can't confirm them
(no persisted state backs them).

## Source logs

Every event projects from exactly one of:

| Source log | Location | Typical variants |
|---|---|---|
| `brassclaw_engine::EventKind` | `crates/brassclaw_engine/src/` | Turn progression, tool execution, gates, leases, child threads, skills |
| Extension lifecycle events | `crates/brassclaw_reborn_composition/src/projection/` | `ExtensionStatus`, `OnboardingState` |
| Reborn event projections | `crates/brassclaw_reborn_event_store/` + `crates/brassclaw_event_projections/` | Session turns, event sourcing replay |

## Transport-only allowlist

A small number of event variants don't project from anything because they have no state backing them. These are documented exceptions, not a loophole for new state:

- `Heartbeat` — SSE keepalive, no payload, no state
- `StreamChunk` — LLM token streaming, pre-step-completion by design; adding to durable log would pollute it with token-level noise

New event variants that claim "transport-only" status require review sign-off and an entry in this table.

## The rule

**No call to an SSE/WS broadcast function is allowed outside:**

1. The projection dispatcher loop that consumes one of the source logs above, **or**
2. A line annotated with `// projection-exempt: <category>, <detail>`.

## Annotation format

```rust
state.sse.broadcast_for_user(user_id, event); // projection-exempt: channel-lifecycle, extension activation
```

The `<category>` must name either:

- A source log — `engine-event`, `extension-lifecycle`, `event-projection` — plus a short detail.
- A transport-only allowlist entry — `transport-only, heartbeat` or `transport-only, stream_chunk`.
- A scheduled migration — `migrate in #NNNN` where the issue tracks moving the emit into a source log.

An unnamed category (`// projection-exempt: legacy`) is not sufficient.
