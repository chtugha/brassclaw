---
description: Scaffold a new first-party tool or MCP integration with all boilerplate wired up
allowed-tools: Read, Edit, Write, Glob, Grep, Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Bash(ls:*), Bash(mkdir:*)
argument-hint: <tool_name> [description]
model: opus
---

Scaffold a new tool called `$ARGUMENTS` for the BrassClaw agent. First, determine the tool type and then follow the appropriate path.

## Step 0: Determine tool type

Ask the user which type of tool to create:

- **First-party tool** (recommended for core agent capabilities) — Compiled into the main binary. Lives in `crates/brassclaw_host_runtime/src/first_party_tools/<name>.rs`. This is the right choice for shell commands, file operations, DB operations, skill management, and anything that needs direct access to `InvocationServices`.
- **MCP server tool** — External MCP server, any language, plugged in via `crates/brassclaw_mcp/`. The right choice for third-party API integrations (Notion, GitHub, Slack, etc.).

If the description clearly implies an external service integration, default to MCP. If it's a core agent capability, default to first-party.

---

## Path A: First-Party Tool

### A1: Create the handler file

Create `crates/brassclaw_host_runtime/src/first_party_tools/<name>.rs`:

```rust
use crate::services::InvocationServices;
use brassclaw_host_api::capability::CapabilityRequest;

pub async fn handle_<name>(
    request: <Name>Request,
    services: &InvocationServices,
) -> Result<<Name>Response, crate::error::HostRuntimeError> {
    // implement the tool logic here
    todo!()
}

#[derive(Debug, serde::Deserialize)]
pub struct <Name>Request {
    // define request fields
}

#[derive(Debug, serde::Serialize)]
pub struct <Name>Response {
    // define response fields
}
```

### A2: Register in `first_party.rs`

Open `crates/brassclaw_host_runtime/src/first_party.rs` and add the new handler to the dispatch match and the tool schema list.

### A3: Add tests

Add a `#[cfg(test)] mod tests {}` block at the bottom of the handler file. Use `InvocationServices::for_testing()` or equivalent to construct a test context.

### A4: Quality gate

```bash
cargo fmt -p brassclaw_host_runtime
cargo clippy -p brassclaw_host_runtime --all-targets -- -D warnings
cargo test -p brassclaw_host_runtime
```

---

## Path B: MCP Server Tool

MCP servers are external processes registered via the MCP protocol. They can be written in any language and communicate over stdio or SSE.

### B1: Understand the MCP protocol

Read `crates/brassclaw_mcp/src/` to understand the client-side wiring. MCP servers advertise tools via `tools/list` and execute via `tools/call`.

### B2: Create the server

Create a new MCP server project (Rust, Python, Node.js, etc.) that:

1. Implements `tools/list` — returns the tool schema (JSON Schema `parameters`)
2. Implements `tools/call` — executes the tool and returns text or error
3. Communicates over stdio (standard MCP transport)

### B3: Register with BrassClaw

MCP servers are registered via the user-facing MCP configuration in the Reborn settings UI or CLI. No code changes are needed in the main repo to add a user-managed MCP server. For bundled/system MCP servers, add to `crates/brassclaw_reborn_composition/src/mcp.rs`.

### B4: Test

Run the MCP server locally and verify tool discovery and invocation via the MCP inspector or by wiring it into a local dev instance.

---

## Checklist

Before finishing, verify:
- [ ] Tool type chosen (first-party or MCP) and confirmed with user
- [ ] For first-party: handler file created with request/response structs
- [ ] For first-party: registered in `first_party.rs` dispatch + schema list
- [ ] For first-party: unit tests added
- [ ] For MCP: server implements `tools/list` and `tools/call` correctly
- [ ] `cargo fmt` clean
- [ ] `cargo clippy -p brassclaw_host_runtime -- -D warnings` clean (for first-party)
