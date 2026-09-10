# Phase V — Orchestrator MCP Server

**Status:** [x] DONE — commit `a72ced60` (full stack: Rust backend + WebUI v2 routes + frontend tab).
**Plan reference:** `saved_plan_to_v3.md` §Phase V (line 10109)
**Created:** 2025-07
**Author:** agent

---

## 0. Context and Goal

vLLM is running `cyankiwi/Ornith-1.5-9B-AWQ-INT4` with:

```
--enable-auto-tool-choice
--tool-call-parser qwen3_xml
--language-model-only
```

vLLM exposes an **OpenAI-compatible Chat Completions API** at `http://<host>:8000/v1`.  
The model uses the standard OpenAI `tool_calls` format — not raw MCP JSON-RPC directly.  
vLLM bridges between the model's `qwen3_xml` tool-call syntax and the OpenAI `tool_calls`
JSON format it sends back to the client (brassclaw).

**Phase V goal:** Make the Orchestrator act as an MCP server so the LLM can:
1. Discover available capabilities via `tools/list`
2. Call a capability via `tools/call`  
3. Receive the result and continue reasoning

On the **brassclaw side**, each exposed "MCP tool" is a projection of a Skill row from
the component tables. When the LLM calls the tool, brassclaw dispatches through the
existing `PgCompositionPort::compose()` → orchestrator pipeline — the same path as any
other skill invocation. The LLM and vLLM see a normal MCP connection. Brassclaw sees
a normal skill dispatch.

---

## 1. Architecture Decisions (All Resolved)

| # | Fork | Decision | Rationale |
|---|------|----------|-----------|
| 1 | Transport | **Real MCP JSON-RPC 2025-06-18 over HTTP** | vLLM connects to brassclaw as a real MCP server. The model uses standard OpenAI tool_calls format; vLLM handles the bridging. |
| 2 | Catalogue | **Skills where `consumer_tags @> '["02:orchestrator"]'`** | Existing tag convention used by `unified_store.rs` and all builtin seeds. Zero new machinery — operator adds/removes the tag to include/exclude. |
| 3 | Schema placement | **`tools/list` MCP-native discovery** | The LLM sends `tools/list` and gets back `{ name, description, inputSchema }` per skill. `variable_patterns` JSONB on `reborn_recipes` provides the slot schema. |
| 4 | Code location | **Module inside `brassclaw_reborn_composition`** | All deps already present: `brassclaw_engine`, `brassclaw_mcp`, `PgCompositionPort`. No new crate. |
| 5 | Gate coverage | **Verify-only** | All dispatches go through `PgCompositionPort::compose()` → existing `LeaseManager` + `PolicyEngine` + `gate_controller`. A test asserts the path fires. No new gates needed. |

---

## 2. What vLLM Actually Does (Wire Shape)

vLLM is already configured as the LLM provider in brassclaw via the OpenAI-compat path
(`brassclaw_llm/src/lib.rs` `ProviderProtocol::OpenAiCompletions`). When brassclaw sends
a Chat Completions request with a `tools` array, vLLM:

1. Passes the tool list + user message to the model
2. Model emits a `qwen3_xml` tool call
3. vLLM parses it → returns OpenAI `tool_calls` format to brassclaw
4. Brassclaw's `CapabilityStage` dispatches the call
5. Brassclaw returns the result as a `role: "tool"` message
6. vLLM passes this back to the model for the next turn

**Phase V does NOT change this flow.** It adds a separate MCP server endpoint that the
LLM operator can point a separate MCP client at — for use cases where the model is given
brassclaw's skill catalogue as MCP tools *instead of* or *in addition to* the current
prompt-injected skill bodies. The two paths are independent.

---

## 3. Components to Build

### V.1 — `OrchestratorMcpServer` trait in `brassclaw_reborn_composition`

New file: `crates/brassclaw_reborn_composition/src/orchestrator_mcp_server.rs`

```rust
/// An MCP server that projects orchestrator Skills as callable MCP tools.
/// 
/// Implements the MCP JSON-RPC 2025-06-18 protocol over HTTP (Streamable HTTP).
/// Exposes Skills tagged `consumer_tags @> '["02:orchestrator"]'` as tools.
pub struct OrchestratorMcpServer {
    composition_port: Arc<dyn ComponentPort>,
    scope: ComponentScope,
    config: OrchestratorMcpServerConfig,
}

pub struct OrchestratorMcpServerConfig {
    /// Max bytes returned per tool call result.
    pub max_output_bytes: usize,  // default 1 MB
}
```

**MCP endpoints handled:**
- `POST /mcp` — JSON-RPC dispatch (initialize, tools/list, tools/call)
- `GET /mcp` — SSE stream for server-sent notifications (empty in V.0)

### V.2 — `OrchestratorSkillProjection` — derive `tools/list` from tagged Skills

```rust
/// Fetch all Skills tagged `02:orchestrator` from the component tables and
/// project them into MCP tool descriptors.
async fn list_tools(
    composition_port: &dyn ComponentPort,
    scope: &ComponentScope,
) -> Result<Vec<McpDiscoveredTool>, OrchestratorMcpError>
```

**Projection rules:**
- `name` = Skill `name` field (slugified: spaces → `_`, lowercased)
- `description` = Skill `description` field (truncated to 1024 chars)
- `inputSchema` = JSON Schema derived from `variable_patterns` JSONB on the
  associated Recipe row (if present), else `{ "type": "object", "properties": {} }`

**`variable_patterns` → JSON Schema derivation:**
Each `variable_patterns` entry is `{ "name": "slot0", "type": "string", "description": "..." }`.
Projected as:
```json
{
  "type": "object",
  "properties": {
    "slot0": { "type": "string", "description": "..." }
  },
  "required": ["slot0"]
}
```

### V.3 — Tool dispatch — route `tools/call` into `PgCompositionPort`

```rust
/// Dispatch a single MCP tools/call through the orchestrator.
/// 
/// 1. Resolve skill name → UUID via `composition_port.resolve_component_by_name()`
/// 2. Look up associated Recipe via `composition_port.compose()`
/// 3. Execute via the existing orchestrator path
/// 4. Return result as MCP `{ content: [{ type: "text", text: "..." }] }`
async fn call_tool(
    composition_port: &dyn ComponentPort,
    scope: &ComponentScope,
    name: &str,
    arguments: serde_json::Value,
) -> Result<McpClientOutput, OrchestratorMcpError>
```

**Gate verification:** The existing `PgCompositionPort::compose()` threads through
`LeaseManager` + `PolicyEngine` + `gate_controller`. A unit test asserts that calling
`call_tool()` with a mock `ComponentPort` that returns `GateRequired` causes the
MCP response to carry `isError: true` with the gate reason — i.e. no bypass is possible.

### V.4 — Update K4 fallback bundle

File: `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`  
Constant: `PC_HOST_FALLBACK_PRIOR_KNOWLEDGE_CONTENT`

Update the catalogue lines to reference the real MCP server URL pattern:

```python
_lines.append("Deeper context is available over the orchestrator MCP server:")
_lines.append("  endpoint: http://localhost:<BRASSCLAW_PORT>/mcp")
_lines.append("  tools/list: returns all available orchestrator skills as callable tools")
```

The `<BRASSCLAW_PORT>` is not known statically — use a template comment explaining
the operator configures the MCP server URL in their vLLM session setup.

### V.5 — HTTP server wiring in `brassclaw_reborn_composition`

Add a `spawn_orchestrator_mcp_server(port, composition_port, scope)` async fn
that starts the Axum HTTP listener for the MCP endpoint. Wired into the composition
startup sequence alongside the existing WebUI v2 server.

---

## 4. MCP JSON-RPC Wire Format (2025-06-18)

### `initialize` request/response

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"initialize",
 "params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"vllm","version":"0.5.1"}}}

// Response
{"jsonrpc":"2.0","id":1,"result":{
  "protocolVersion":"2025-06-18",
  "capabilities":{"tools":{"listChanged":false}},
  "serverInfo":{"name":"brassclaw-orchestrator","version":"1.0.0"}
}}
```

### `tools/list` request/response

```json
// Request
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}

// Response
{"jsonrpc":"2.0","id":2,"result":{"tools":[
  {
    "name": "list_files",
    "description": "List files in a directory",
    "inputSchema": {
      "type": "object",
      "properties": {
        "slot0": {"type":"string","description":"directory path"}
      },
      "required": ["slot0"]
    }
  }
]}}
```

### `tools/call` request/response

```json
// Request
{"jsonrpc":"2.0","id":3,"method":"tools/call",
 "params":{"name":"list_files","arguments":{"slot0":"/tmp"}}}

// Response (success)
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"file1.txt\nfile2.rs"}],"isError":false}}

// Response (error / gate)
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"Gate required: approval needed"}],"isError":true}}
```

---

## 5. File Map

| File | Change |
|------|--------|
| `crates/brassclaw_reborn_composition/src/orchestrator_mcp_server.rs` | **NEW** — MCP server impl |
| `crates/brassclaw_reborn_composition/src/orchestrator_mcp_server/projection.rs` | **NEW** — `list_tools()` |
| `crates/brassclaw_reborn_composition/src/orchestrator_mcp_server/dispatch.rs` | **NEW** — `call_tool()` |
| `crates/brassclaw_reborn_composition/src/lib.rs` | add `pub mod orchestrator_mcp_server` |
| `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` | update `PC_HOST_FALLBACK_PRIOR_KNOWLEDGE_CONTENT` |
| `crates/brassclaw_reborn_composition/Cargo.toml` | add `axum`, `tower` if not already present |

No new migrations. No new DB tables. No new Rust tools. No new execution primitives.

---

## 6. Implementation Steps (Strict Sequential Order)

- [x] Write this subplan
- [ ] **V.1** — Scaffold `orchestrator_mcp_server.rs` with `OrchestratorMcpServer` struct, `OrchestratorMcpServerConfig`, `OrchestratorMcpError` (thiserror), and the `POST /mcp` Axum handler skeleton (initialize / tools/list / tools/call dispatch)
- [ ] **V.2** — Implement `list_tools()`: query `composition_port.list_skills()` filtered to `consumer_tags @> '["02:orchestrator"]'`, project to `McpDiscoveredTool`; unit test with stub port
- [ ] **V.3** — Implement `call_tool()`: resolve → compose → execute → MCP result; unit test gate-blocks dispatch; unit test successful dispatch
- [ ] **V.4** — Update K4 `PC_HOST_FALLBACK_PRIOR_KNOWLEDGE_CONTENT` in `builtin_bootstrap.rs`
- [ ] **V.5** — Wire `spawn_orchestrator_mcp_server` into composition startup; clippy clean; commit + push
- [ ] Mark Phase V as `[x] Done` in `saved_plan_to_v3.md`

---

## 7. Out of Scope (Phase V.0)

- SSE server-sent notifications (`GET /mcp` stream) — return 200 empty stream for now
- Per-tool auth/credential injection — existing session auth covers this
- `tools/list` pagination (`cursor`) — not needed for the skill catalogue size
- Dynamic catalogue invalidation (`listChanged` notification) — deferred
