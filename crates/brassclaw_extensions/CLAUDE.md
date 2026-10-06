# brassclaw_extensions guardrails

## Recipe architecture routing

Follow [the binding Recipe authoring contract](../../recipe.md) and root AGENTS.md for
v3 component work. Recipes instruct the orchestrator using reusable Tools,
ToolSkills, Skills and small PythonCode components. One referenced component
per Recipe step; internal PythonCode composition is allowed. IBS pins approved
immutable versions in BuildInstruction at task start, including nested includes;
Recipes reference stable UUIDs without versions. Preserve typed inputs/results
and task state across steps and waits. Replacement versions do not alter active
tasks; current global Tool policy still applies at every dispatch. Older scoped,
source-substitution or fresh-step descriptions below are implementation/legacy
notes, not permission to extend those paths as the v3 target. These requirements
remain subject to the documented runtime implementation gaps.

- Own extension manifest parsing, typed package metadata, capability descriptors, runtime declarations, and the in-memory extension registry.
- Keep this crate declarative. Do not execute tools, resolve authorization, perform network I/O, read secrets, spawn processes, or inspect WASM/script/MCP payloads here.
- Depend only on neutral substrate crates such as `brassclaw_host_api`; runtime, host composition, authorization, approvals, networking, and persistence live in their owning crates.
- Preserve manifest validation as fail-closed and stable: unknown/invalid capability ids, provider mismatches, malformed paths, duplicate capabilities, or unsupported runtime shapes should not be papered over.
- Keep package roots virtual-path based. Do not introduce raw host paths or product-specific workspace assumptions.
- Registry lookups should remain deterministic and side-effect free; callers own trust, visibility filtering, and execution policy.
