---
description: Trace a data flow or bug through the BrassClaw codebase end-to-end
allowed-tools: Read, Glob, Grep, Bash(cargo test:*)
argument-hint: <symptom or feature name>
model: sonnet
---

Trace the flow of `$ARGUMENTS` through the BrassClaw codebase. Your job is to map every file and function involved, identify where data transforms or could break, and report the full chain.

## Architecture Reference

BrassClaw Reborn is organized in three layers: **Products** (surface/UX), **Loops** (agent behavior), and **Kernel** (authority/security). See `AGENTS.md` for the full mental model.

The main data flow paths are:

### Turn Flow (user input to agent response)

```
brassclaw_reborn_webui_ingress (HTTP/WS ingress)
  → TrustedInboundTurnRequest / UntrustedInboundTurnRequest (brassclaw_turns)
    → brassclaw_turns/src/coordinator.rs — admission + scoping
      → brassclaw_turns/src/runner.rs — turn runner
        → brassclaw_turns/src/run_profile/ — run profile (host, driver)
          → brassclaw_agent_loop — agent loop driver
            → brassclaw_engine — engine executor (Python orchestrator or direct)
              → brassclaw_host_runtime — tool dispatch, process execution
                → brassclaw_llm — LLM provider routing
          → brassclaw_turns/src/response.rs — response emitted
```

### Event Flow (engine events to WebUI)

```
brassclaw_engine::EventKind (event emitted during execution)
  → crates/brassclaw_reborn_event_store/ (persisted to DB)
    → crates/brassclaw_event_projections/ (projected to AppEvent)
      → brassclaw_reborn_composition/src/projection/ (SSE broadcast)
        → brassclaw_webui_v2_static/js/ (frontend EventSource)
```

### Tool Dispatch Flow

```
Engine / orchestrator (default.py) calls __execute_action__(name, params)
  → brassclaw_agent_loop executor dispatch
    → brassclaw_dispatcher (capability lookup, lease check, policy)
      → brassclaw_host_runtime first-party tool handler
        or brassclaw_mcp MCP server call
      → safety scan (brassclaw_safety)
    → result returned to engine turn
```

## Tracing Instructions

1. **Read** each file in the relevant flow path, focusing on the functions that handle the data.
2. **Identify transforms**: Where does the data change shape?
3. **Identify failure points**: Where could the data be lost, malformed, or misrouted?
4. **Report the chain**: List every file:line involved, what happens at each step, and where the issue (if any) is.

## Key Files Quick Reference

| Area | Crate/File | Key Items |
|------|-----------|-----------|
| Turn admission | `crates/brassclaw_turns/src/admission.rs` | `TurnAdmissionPolicy` |
| Turn runner | `crates/brassclaw_turns/src/runner.rs` | `run_turn` |
| Run profile host | `crates/brassclaw_turns/src/run_profile/host.rs` | `AgentLoopDriverHost` trait, all port traits |
| Agent loop driver | `crates/brassclaw_agent_loop/src/executor.rs` | `execute_turn` |
| Engine executor | `crates/brassclaw_engine/src/executor/loop_engine.rs` | `execute_orchestrator` |
| Python orchestrator | `crates/brassclaw_engine/src/executor/orchestrator.rs` | `execute_orchestrator` (pub) |
| First-party tools | `crates/brassclaw_host_runtime/src/first_party.rs` | tool dispatch |
| Tool dispatch | `crates/brassclaw_dispatcher/` | capability dispatch |
| LLM routing | `crates/brassclaw_llm/src/` | `LlmProvider` trait, decorators |
| Safety | `crates/brassclaw_safety/src/` | `sanitizer.rs`, `leak_detector.rs` |
| Memory/retrieval | `crates/brassclaw_engine/src/memory/` | `retrieval_source.rs`, `fetch_for_turn` |
| Intent system | `crates/brassclaw_engine/src/memory/intent_system.rs` | `resolve_intent` |
| Composition wiring | `crates/brassclaw_reborn_composition/src/runtime.rs` | `build_reborn_runtime` |
| WebUI ingress | `crates/brassclaw_reborn_webui_ingress/src/` | auth, session, turn submission |

## Output Format

Report your findings as:

1. **Flow path**: The specific chain of files and functions involved
2. **Data transforms**: How the data changes at each step
3. **Findings**: Any bugs, missing data, or suspicious patterns
4. **Recommendation**: What to fix or investigate further
