# 01 — Architecture overview

## 1. Purpose and authority

BrassClaw uses Recipes and reusable PythonCode to sequence work on one global
Monty service. Rust supplies primitives, transport, persistence, authority,
execution fencing and effect custody. Product code wires these owners together.

This source map was checked on 2026-11-15. The component contracts are
[recipe.md](../../recipe.md), [skills.md](../../skills.md),
[tools.md](../../tools.md) and [toolskills.md](../../toolskills.md).
[Simplified v3](../../simplified_v3.md) governs the runtime/authorization target;
[path to v3.5](../../path_to_v3.5.md) assigns remaining work. The
[implementation index](../implementation-status.md) records scoped acceptance.
Source wiring does not prove installed uptake or full-plan completion.

## 2. Location — verified execution owners

| Source | Verified responsibility |
| --- | --- |
| [runtime.rs](../../crates/brassclaw_reborn_composition/src/runtime.rs) | Ordinary `postgres` + `skills-db` startup constructs the global owner, catalogue, task factory and `GlobalMontyDriver`; retains the owner for shutdown. |
| [global_monty_startup.rs](../../crates/brassclaw_reborn_composition/src/global_monty_startup.rs) | `start` prepares the installed selection and starts `GlobalMontyOwner`. |
| [global_monty_owner.rs](../../crates/brassclaw_reborn_composition/src/global_monty_owner.rs) | `GlobalMontyOwner` owns the instance service. |
| [global_monty_driver.rs](../../crates/brassclaw_reborn_composition/src/global_monty_driver.rs) | `GlobalMontyDriver` implements the turn-driver boundary and retains attempt/settlement ownership. |
| [turn_runner.rs](../../crates/brassclaw_reborn/src/turn_runner.rs) | `TurnRunnerWorker` dispatches through its configured `MontyTurnDriverPort`. |
| [global_mode.py](../../crates/brassclaw_engine/orchestrator/global_mode.py) | `_execute_task` routes tasks; `_execute_recipe` executes the selected flow and `_non_match` handles actual No-Match. |
| [global_recipe_ports.rs](../../crates/brassclaw_reborn_composition/src/global_recipe_ports.rs) | `MontyTaskCatalogue` and task-owned Recipe ports transport the selected graph and requested execution without becoming a Rust workflow loop. |

## 3. Responsibility model

| Layer | Responsibility |
| --- | --- |
| Products | UX, ingress and composition of services for the deployment. |
| Recipes / Monty | Planning and task sequencing, typed transformations, selected branches, explicit LLM work and business completion. |
| Kernel / native execution | Current Tool policy, external authentication, sandbox/network/secret/resource enforcement, stale-attempt fencing and effect custody. |
| Infrastructure | PostgreSQL, providers, interpreter hosting, transport and observability. |
| Component library | Recipes, one-Tool Skill usages, ToolSkill descriptors, PythonCode and ExtensionCatalogues. |

A Tool is a Rust primitive. A ToolSkill prepares a compatible binding without
dispatch or permission. A Skill is one Tool usage: prose plus an explicitly
associated preloadable function interface. PythonCode implements that usage or
pure logic. Recipes order reusable usages and connect their data. Each component
step references one stable UUID; pure logic needs no artificial Tool binding.

## 4. Ordinary execution and lifecycle

Under ordinary PostgreSQL/skills-db startup, one global Monty owner is already
wired before turn workers and trigger producers. A turn is a bounded task in
that service, not a new instance VM. Completion/cancellation ends the task; it
does not terminate the service. The binding lifecycle requires startup readiness,
bounded shutdown and supervised fatal recovery, never a parallel VM or silent
per-chat fallback. Remaining recovery/platform acceptance stays with Simplified
Phase 3a and the existing owners in the implementation index.

```text
Authenticated ingress / durable turn admission
  -> TurnRunnerWorker -> configured GlobalMontyDriver
  -> task-owned catalogue and host ports -> global Monty task
  -> approved catalogue match and retained Recipe/variant selection
       -> IBS / typed bindings / pinned preload exports
       -> Monty executes the selected flow
       -> actual host Tool call -> current kernel checks -> native result
       -> validated result handoff -> qualified reply/history completion
     OR actual No-Match -> explicit non-matching model path
```

Matching and assembly use one coherent active approved selection. Retain exact
Recipe/variant/link/order/layout, component and dependency revisions/checksums,
usage evidence and actual Tool implementations. Tasks, children and waits retain
that selection when successors activate. Runtime values travel as typed data.
Loading definitions performs no Tool usage. Every real dispatch checks current
policy and execution freshness independently of the pinned approval.

Tier 0 makes no LLM calls. Tier 1 executes its declared LLM work; it is not merely
unrestricted model execution with an injected hint. Shell and subagent Recipes
remain Tier 1. Only actual No-Match enters Tier 2. Database, matching, assembly
and begun Recipe failures remain errors, with no replay of completed effects.

## 5. Compatibility paths and related guides

The older `PlannedDriver` / canonical Rust stage pipeline and
`PgOrchestratorLookup` bridge remain source to inspect for their actual callers.
They do not describe the ordinary global Monty driver above. Likewise,
`ExecutionLoop` / `execute_orchestrator` and legacy `PgCompositionPort` compose/run
are not a future replacement to activate for retained execution. Their source
substitution and per-step bodies are migration inputs. See
[tools.md's legacy-versus-retained distinction](../../tools.md#d-legacy-composerun-operations-and-the-retained-production-path).

Use [intent routing](02-intent-system.md), [Recipes](03-recipe-system.md),
[IBS](04-ibs.md), [Skills](05-skills-system.md), [Tools](06-tools-system.md),
[PythonCode](07-pythoncode-system.md), [turn lifecycle](12-agent-loop.md) and
[validation](14-validation-queue.md) for the corresponding source maps.

## 6. Support boundaries and remaining work

Global startup, retained typed inputs/preload, exact usage selection and scoped
ordinary execution already have source support and attributed evidence in the
implementation index. General own-functional approval/publication, full library
migration, qualified dynamic occurrences and durable recovery are not established
by that support. Reuse the existing owners instead of rebuilding them.

V35-01/02 reconcile current source and guidance; V35-03/04 consume the existing
independent-pure approval owner; V35-13/14 own remaining qualified occurrence
support. The roadmap and current index, rather than old C.5/C.6 activation labels,
establish the exact remaining gates. No source map certifies deployment.

## 7. LLM-relevant summary

Recipes and pinned PythonCode exports own sequencing on the ordinary global
Monty service. Rust owns primitives, transport, integrity, authority, accounting
and effect custody. ToolSkill binding performs no operation and grants nothing.
Typed task data and results remain isolated; exact selections survive waits and
successor activation. Tier 0 is model-free; Tier 1 contains explicit model work;
only true No-Match enters Tier 2. Errors never authorize fallback or replay.

## Current prefix ownership and selection — 2026-10-26

Follow the [shared selection/pair-design contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
and [Prefix implementation owner](../plans/prefix_v3_upgrade.md). A Recipe,
Orchestrator or role selects a specific prefix; without one, base-prompt is the
regular default, including normal No-Match, not a Tier-2-only restriction.
Unavailable explicit selections/conflicts block rather than silently becoming
base. Shared knowledge/composites require an explicit content contract.

Design for formats/limits belongs to the provider/model pair; role content rules
and finished role prefixes are distinct. Same-pair Kohai/Sempai use the same
accepted design as the generator. Generate the fixed prefix specifically for
that pair. Kohai first formats only the dynamic Orchestrator prompt for the final
target pair. A connected/enabled Sempai corrects that already formatted prompt,
using its own generated reviewer prefix/target-pair design, and returns the final
dynamic prompt. Kohai then inserts the intended finished prefix unchanged exactly
once before it, without reformatting the return, and checks the complete request.
Every manual limit editor links to the existing provider revision/CAS endpoint
and shared query. Historical snapshots are not current mutable settings.
Provider facts may be retrieved through supported APIs/documentation or supplied
manually with provenance; no provider host/template/service edits are an option.

Historical store, placeholder, server-template and prewarm walkthroughs in older
subsystem guides do not establish current serving qualification. Generation and
prefill are separate; cache residency, pricing and reuse require their actual
backend evidence. Construction requests follow the
[visible selection successor](../reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection).
Runtime cutover/remaining acceptance stays with the Prefix owner and implementation
index.
