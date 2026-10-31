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

Inspect existing production callers and test constructors before choosing a test
context; do not assume a `for_testing()` API exists. Reuse adequate existing
caller regressions; otherwise add focused coverage through actual dispatch for
the declared operation, applicable invalid inputs, denials and effect/failure
handling. Helper-only success does not establish registration or kernel policy.

### A4: Quality gate

Follow [the development policy](../../docs/development-policy.md) and
[testing rules](../rules/testing.md). Review the connected handler/registration/
binding/caller diff, then choose actual targets/features and affected-package
linting for the changed behavior. Real backend/security/lifecycle claims need
the actual production path and relevant failures. Component/seed changes retain
their behavior, package/integrity, qualification and activation requirements;
a successful compile or metadata row is insufficient.

Respect central Cargo ownership, source freezes and the configured canonical
target. Reuse still-applicable evidence rather than stacking equivalent builds.
Report skipped/zero-case/unavailable checks as unverified; fix causes before
repeating failed/invalidated checks and stop when required checks pass. Ordinary
prose needs links/consistency review and `git diff --check`, not Cargo.

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

Verify discovery and invocation through the actual supported BrassClaw consumer,
including relevant transport/error/cleanup cases; an inspector-only test does not
prove product attachment. Reuse applicable evidence and choose the smallest
sufficient real check under the same policy. Read `LOCAL_TEST_ENV.md` when present
before remote tests/provider setup. Report an unavailable path as unverified.

---

## Checklist

Before finishing, verify:
- [ ] Tool type chosen (first-party or MCP) and confirmed with user
- [ ] For first-party: handler file created with request/response structs
- [ ] For first-party: registered in `first_party.rs` dispatch + schema list
- [ ] For first-party: meaningful dispatch/caller regression evidence established
- [ ] For MCP: server implements `tools/list` and `tools/call` correctly
- [ ] Required behavior, affected-package linting and applicable backend/security/lifecycle checks completed
- [ ] Reused evidence and any unverified checks reported with their actual scope
