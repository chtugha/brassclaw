# Bridge Module

Adapter layer between the engine (`brassclaw_engine`) and the host crate's
persistence and workspace surfaces.

The v1 bridge modules (`auth_manager`, `router`, `effect_adapter_v2`,
`engine_actions`, `cost_guard_gate`, `sandbox`, `llm_adapter`,
`skill_migration`) were deleted as part of the v1 removal. See git history.

## Files

| File | Role |
|------|------|
| `store_adapter.rs` | Implements `Store` for the engine (threads, steps, events, memory docs). |
| `workspace_reader.rs` | Read-side adapter between the engine memory store and the workspace. |

## Exports

- `WorkspaceReaderAdapter` — workspace read surface for the engine.
- `EffectExecutor`, `ThreadExecutionContext` — re-exported from `brassclaw_engine`.
