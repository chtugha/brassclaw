# BrassClaw Reborn live vertical slice

**Date:** 2026-04-25 (Revised 2026-07-14: Phase 4 WASM/Script removal)
**Status:** Runnable V1 demo (extended to MCP-only after Phase 4)
**Crates:** `brassclaw_filesystem`, `brassclaw_extensions`, `brassclaw_resources`, `brassclaw_events`, `brassclaw_dispatcher`, `brassclaw_host_runtime`, `brassclaw_mcp`, `brassclaw_process_sandbox`

---

## 1. Purpose

This slice proves the first Reborn host path is runnable:

```text
LocalFilesystem mounted at /system/extensions
-> ExtensionDiscovery reads manifests
-> ExtensionRegistry registers capabilities
-> RuntimeDispatcher receives already-authorized dispatch requests
-> RuntimeDispatcher routes dispatch by RuntimeKind through registered adapters
-> dispatcher example adapters execute JSON echo capabilities
-> HostRuntimeServices examples wrap configured first-party tool backends for end-to-end capability/process demos
-> InMemoryResourceGovernor reserves and reconciles all invocations
-> JsonlEventSink records requested/selected/succeeded events under tenant/user/agent-scoped /engine event paths
-> JSON outputs are returned through one dispatch path
```

It is intentionally not a product agent loop, gateway, TUI, secret flow, or full event bus. The current event slice is dispatcher-level observability only, and the MCP slice is an adapter contract rather than a full MCP protocol/server lifecycle implementation.

> **Phase-4 update.** The WASM and Script runtime kinds are gone. The
> demo no longer echoes through `echo-wasm` / `echo-script` adapters;
> it covers MCP and first-party lanes. Shell-style work uses
> `brassclaw_process_sandbox` and is composed via agent recipes
> rather than runtime adapters.

---

## 2. Run it

```bash
cargo run -p brassclaw_dispatcher --example reborn_echo
```

Expected output shape (post-Phase-4):

```text
reborn_dispatcher_adapter_slice=ok
discovered_extensions=N
dispatch=echo-mcp.say runtime=mcp output={"message":"hello mcp"} reservation_status=Reconciled
dispatch=echo-first-party.say runtime=first_party output={"message":"hello first-party"} reservation_status=Reconciled
durable_event_path=VirtualPath("/engine/tenants/tenant1/users/user1/agents/_none/events/runtime/reborn-demo.jsonl")
events=6
event[0]=dispatch_requested capability=echo-mcp.say runtime=none error=none
event[1]=runtime_selected capability=echo-mcp.say runtime=mcp error=none
event[2]=dispatch_succeeded capability=echo-mcp.say runtime=mcp error=none
event[3]=dispatch_requested capability=echo-first-party.say runtime=none error=none
event[4]=runtime_selected capability=echo-first-party.say runtime=first_party error=none
event[5]=dispatch_succeeded capability=echo-first-party.say runtime=first_party error=none
```

The default dispatcher example uses in-crate echo adapters so `brassclaw_dispatcher` can demonstrate routing, resource lifecycle, and event emission without depending on concrete MCP runtime crates. Real runtime wiring now lives in `brassclaw_host_runtime`, whose examples use `HostRuntimeServices` to adapt configured runtimes into dispatcher adapters and then drive capability/process workflows.

---

## 3. What this validates

The integration test `crates/brassclaw_dispatcher/tests/vertical_slice_contract.rs` validates:

- extension manifests are read from `LocalFilesystem` via `/system/extensions`
- extension discovery returns MCP and first-party packages (no WASM/Script packages)
- dispatcher crate tests exercise already-authorized `CapabilityDispatchRequest` values directly
- higher-level caller workflow stays out of dispatcher crate dev surfaces
- MCP dispatch goes through `RuntimeDispatcher` and a registered runtime adapter
- FirstParty dispatch goes through `RuntimeDispatcher` and a registered runtime adapter
- all invocations reserve and reconcile resource usage
- all lanes emit dispatch requested/runtime selected/dispatch succeeded events
- event history is durably written through `RootFilesystem` at the scoped runtime event path
- all lanes return JSON output through the same normalized dispatch result type

---

## 4. Non-goals

This slice does not add:

- full realtime event bus fanout/reconnect
- durable transcript/job state
- approval resolution/resume
- scoped script filesystem mounts (replaced by capability-gated shell inside `brassclaw_process_sandbox`)
- artifact export
- secret injection
- network access for MCP servers
- full MCP protocol handshake/server lifecycle
- conversation or agent-loop behavior

Those are follow-on slices once this dispatch path is stable.
