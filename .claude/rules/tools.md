---
paths:
  - "crates/brassclaw_host_runtime/src/first_party_tools/**"
  - "crates/brassclaw_host_runtime/src/services/**"
  - "crates/brassclaw_dispatcher/**"
  - "crates/brassclaw_mcp/**"
---
# Tool Architecture

**Keep tool-specific logic out of the agent loop.** The agent loop (`crates/brassclaw_agent_loop/`) provides generic infrastructure; tools are self-contained capabilities registered and dispatched through `crates/brassclaw_dispatcher/`.

First-party tools live under `crates/brassclaw_host_runtime/src/first_party_tools/`. MCP servers integrate via `crates/brassclaw_mcp/`. See `crates/AGENTS.md` for the full location table.

## Tool Implementation Pattern

First-party tools implement a capability-handler trait and are registered in `crates/brassclaw_host_runtime/src/first_party.rs`. They receive a structured request and return a structured response — no raw JSON parsing in the handler body.

```rust
// First-party tool handler pattern (crates/brassclaw_host_runtime/src/first_party_tools/<name>.rs)
pub async fn handle_<name>(
    request: <Name>Request,
    services: &InvocationServices,
) -> Result<<Name>Response, HostRuntimeError> {
    // ... do work ...
    Ok(<Name>Response { ... })
}
```

## Everything Goes Through the Dispatcher

**All capability invocations from agent turns — shell commands, file reads, DB tool calls, MCP tool calls, process spawns — must go through `crates/brassclaw_dispatcher/`.** Never call `HostRuntimeServices` methods directly from the agent loop or from composition handlers.

This is the core dispatch invariant. The reasons are concrete:

1. **Audit trail.** Every dispatched call creates an event record linked to the turn, so all tool executions are observable in the event log.
2. **Safety pipeline parity.** The dispatcher runs the same pipeline for all tools: parameter validation, capability lease check, policy enforcement, output sanitization.
3. **Approval gates.** The dispatcher is the only correct place to check whether a capability invocation requires user approval before executing.

### Forbidden pattern

```rust
// DO NOT call host runtime capabilities directly from a turn handler or composition adapter:
services.shell.execute(cmd).await?;  // BYPASSES dispatcher, lease checks, approval gate
```

### When direct access IS allowed

| Layer | Why exempt |
|---|---|
| `crates/brassclaw_host_runtime/src/first_party_tools/` | These ARE the tool implementations — the leaves |
| `crates/brassclaw_host_runtime/src/services/process_executor/` | Low-level subprocess primitives — invoked only by first-party tools |
| Background system tasks (retention sweep, trigger poller) | Not agent-loop turns; emit their own events |
| Pure read endpoints that aggregate from multiple sources | Read aggregation queries, not actions |
