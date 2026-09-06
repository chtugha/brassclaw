# Subplan — Phase V: Orchestrator MCP Server

> Parent: `./saved_plan_to_v3.md` (new phase V, sequenced after Phase U). Tied
> to the Kohai K4 no-Prefix fallback (the `assemble-prior-knowledge` fallback
> recipe's catalogue references this server). **Status: [ ] Pending —
> design-only subplan; no implementation until the forks below are resolved.**

## Goal

Make the **Orchestrator (Monty) itself act as an MCP server** so the provider
LLM can pull deeper information / invoke capabilities **through the
orchestrator** instead of having the full skill+tool documentation baked into
every prompt. This is the "future MCP-server-functionality" referenced in the
Phase K.2 marker (`saved_plan_to_v3.md` line ~7557): the orchestrator defines
LLM-prompt tools that call predefined Orchestrator Skills to perform Rust tool
calls.

### The token-cost insight (user lock)

The local LLM is slow (~150 tokens/s), so prompt size dominates latency and
cost. Today the no-Prefix / non-match fallback (`basic_mode.py
::_non_match_answer`) sends the LLM only `chat_history + user_query +
prefix_placeholder` — no capability catalogue. The LLM therefore cannot reach
the orchestrator's tools mid-turn except by emitting text.

Phase V changes that: the orchestrator exposes a **minimal** MCP tool catalogue
to the LLM. Each exposed tool is a thin wrapper over an existing
**Skill → ToolSkill → Tool** path. The LLM sees only the tool's intent + argument
shape (the Skill's `intent_examples` / signature), **not** the full body —
because execution + tool usage are already defined inside the
skill→toolskill→tool architecture. "A question the LLM asks the orchestrator is
embedded in the orchestrator and does not need any deeper explanation."

So the MCP surface is a **projection** of the seeded Skill/ToolSkill/Tool graph
(Phase L's 378 components), not a parallel description.

## Architecture

```
provider LLM  ──MCP──▶  Orchestrator (Monty, in-VM)  ──skill→toolskill→tool──▶  Rust Executioner
                              │
                              └─ resolves the called MCP tool name → a Skill
                                 → compose_orchestrator(recipe, step_link, input)
                                 → host.run_program(composed python)
                                 → returns the result to the LLM as the MCP response
```

- The MCP connection **always goes over the orchestrator** (the orchestrator is
  the only thing the LLM talks to for capabilities). No direct LLM→Rust path.
- What the LLM can access over MCP **is defined by the orchestrator's skills**
  (the seeded Skill rows, class 1/2). An MCP tool = a Skill projected to a
  `{name, description, input_schema}` triple.
- A call from the LLM is resolved inside the orchestrator: the MCP tool name
  maps to a Skill → its Recipe → `host.compose_orchestrator` +
  `host.run_program` (the exact path `basic_mode.py::_compose_and_run` already
  uses). No new execution primitive.

## Relation to existing crates

- **`brassclaw_mcp`** is the **client** lane (calls *external* third-party MCP
  servers over host-mediated HTTP/SSE). Phase V is the **server** side and is
  distinct — it does not call out; it *is* called. The dropped Phase K.2
  `mcp_translation.rs` (external-MCP→component translator) is also distinct and
  stays dropped.
- **Composition / execution reuse:** Phase V adds **no new Rust tool** and **no
  new execution path**. It reuses `PgCompositionPort::compose` (C.4.5 / C.6
  4d-3) + `host.run_program` + the seeded Recipes. The new code is the MCP
  **framing + tool-catalogue projection + tool-call dispatch table**.
- **Kohai integration:** the MCP server is offered to the LLM **by Kohai** when
  it forwards the prompt to the provider (the provider's MCP client config
  points at the orchestrator's MCP endpoint). Kohai already owns the
  provider-facing prefix swap (K.1 / K2); the MCP endpoint advertisement is a
  natural addition to the Kohai call envelope.

## Grounding (verified live source, 2026-09-06)

- `basic_mode.py::_compose_and_run` (`crates/brassclaw_engine/orchestrator/basic_mode.py:104`)
  — `host.compose_orchestrator(component_id, step_link, user_input)` →
  `host.run_program(program)` is the exact reuse path for an MCP tool call.
- `host.resolve_component_by_name(name, class_code)` — already used at
  `basic_mode.py:124,146` to look up recipes by name; an MCP tool name→Skill
  resolution uses the same lookup.
- `PgCompositionPort` (composition, wired into `PersistentMontyDriver` at
  `runtime.rs:2643` per C.6 4d-3 / `ef99cf18`) — the compose backing.
- Seeded component graph (Phase L, 378 components): the Skill (class 1/2) +
  ToolSkill (class 13) + Tool (class 0) + Recipe (class 14) + PythonCode
  (class 22) rows in `builtin_bootstrap.rs` are the catalogue source.
- `brassclaw_mcp` crate exists (client lane) — Phase V server code should live
  in a new module/crate to avoid mixing client + server concerns (fork below).

## Forks (decisions needed — user owns these)

1. **Transport / LLM-facing shape.** (a) A real MCP JSON-RPC endpoint (stdio or
   host-mediated local socket) the provider MCP client connects to; (b) an
   **in-VM tool-call protocol** — the LLM emits a structured tool-call in its
   answer, Kohai extracts it, the orchestrator dispatches (no separate
   transport; reuses the existing answer channel). (b) is the lower-friction
   fit for an in-VM orchestrator but is "MCP-shaped", not wire-MCP. **Which?**
2. **Tool-catalogue derivation.** Project (a) every seeded Skill, (b) only
   Domain Skills (class 2), (c) only a curated allowlist tagged for MCP
   exposure. (c) is the safest (least prompt bloat, least leakage) but needs a
   tagging convention. **Which?**
3. **Catalogue placement in the prompt.** The K4 fallback recipe already emits
   a minimal preamble + a catalogue of deeper-info categories. Should the
   **full per-tool MCP schema** be (a) injected into the prompt too (defeats
   the token-cost goal), or (b) only the **category list** is in the prompt and
   the per-tool schema is discovered by the LLM via an MCP `tools/list`-style
   call? (b) is the intent. **Confirm.**
4. **Server code location.** New crate `brassclaw_orchestrator_mcp` (server
   lane, mirrors `brassclaw_mcp` client lane) vs a module inside
   `brassclaw_engine`/composition. **Which?**
5. **Auth / trust.** The MCP server executes Orchestrator Skills (which call
   Rust tools). The provider LLM is the caller. Confirm the existing
   approval/lease/policy gates (`LeaseManager`/`PolicyEngine`/`GateController`
   already on `drive_to_yield`) gate MCP-driven tool calls identically to
   orchestrator-driven ones — no bypass. (Verify-only; likely already true.)

## Sub-slices (sketch — refine once forks resolve)

- **V.1** — MCP tool-catalogue projection: Skill → `{name, description,
  input_schema}` + the name→Skill resolution table. Pure data; unit-testable.
- **V.2** — MCP tool-call dispatch: receive a tool call → resolve Skill →
  compose_orchestrator + run_program → return result. Reuses `_compose_and_run`
  shape.
- **V.3** — Kohai advertisement: the provider call envelope carries the MCP
  endpoint/tool list so the LLM can call back.
- **V.4** — K4 fallback recipe wiring: the `assemble-prior-knowledge` fallback
  text references the live MCP catalogue (today it references the *forthcoming*
  one — Phase V makes it real).
- **V.5** — both configs clippy-clean + tests + commit + push.

## Out of scope (explicit)

- The dropped Phase K.2 external-MCP translator (`mcp_translation.rs`) — stays
  dropped.
- Any new Rust tool / new execution primitive — Phase V reuses the existing
  skill→toolskill→tool pipeline.
- Sempai idle self-optimization.
- Changing the approval/lease/policy gate semantics (verify-only).
