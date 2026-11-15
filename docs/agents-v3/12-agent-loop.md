# 12 — Turn driver and global Monty lifecycle

## 1. Purpose

The ordinary PostgreSQL/skills-db turn driver delivers admitted work to the
instance's existing global Monty service. Recipes and PythonCode own task
sequencing. Rust owns admission, transport, claims, cancellation, accounting,
reply custody and native effects. A product must not implement another workflow
loop. This source map was checked on 2026-11-15; acceptance scope remains in the
[implementation index](../implementation-status.md).

## 2. Location — verified owners

| Source | Symbol / role |
| --- | --- |
| [runtime.rs](../../crates/brassclaw_reborn_composition/src/runtime.rs) | `build_reborn_runtime` constructs and retains the global owner, task factory, settings owner and driver under `postgres` + `skills-db`. |
| [turn_runner.rs](../../crates/brassclaw_reborn/src/turn_runner.rs) | `TurnRunnerWorker::invoke_driver` requires the configured Monty driver and passes a claimed task handoff to `drive_turn`. |
| [driver.rs](../../crates/brassclaw_turns/src/run_profile/driver.rs) | `MontyTurnDriverPort` is the driver contract. |
| [global_monty_driver.rs](../../crates/brassclaw_reborn_composition/src/global_monty_driver.rs) | `GlobalMontyDriver` owns task driving and settlement at that boundary. |
| [global_task_factory.rs](../../crates/brassclaw_reborn_composition/src/global_task_factory.rs) | `OwnedGlobalTaskFactory` prepares task-owned ports and selection. |
| [global_monty_startup.rs](../../crates/brassclaw_reborn_composition/src/global_monty_startup.rs) | `start` creates the instance service after preparing its required installed selection. |
| [global_monty_owner.rs](../../crates/brassclaw_reborn_composition/src/global_monty_owner.rs) | `GlobalMontyOwner` owns startup, shutdown request and join. |
| [global_mode.py](../../crates/brassclaw_engine/orchestrator/global_mode.py) | `_execute_task`, `_execute_recipe` and `_non_match` sequence the global task's work. |
| [live_monty_settings.rs](../../crates/brassclaw_reborn_composition/src/live_monty_settings.rs) | `MontySettingsOwner` owns live settings publication/reconciliation. |

## 3. Data and execution ownership

The worker constructs `MontyTaskHandoff::new_claimed` with the real claim and
host. The global driver supplies task-owned host ports. Retained execution keeps
exact Recipe/variant/link/layout, component/export/dependency and implementation
selections. Inputs/results are typed task data; they are not substitutions into
approved Python source. Claim tokens and credentials are native custody, not
model-visible Recipe inputs.

Monty asks for composition, receives a task-owned program reference and selected
steps/flow, then requests execution through the host. The retained adapter checks
membership and contracts; it does not choose business steps. Preloading defines
functions without effects. Every actual Tool call still checks live policy,
technical constraints, cancellation and attempt freshness.

## 4. Ordinary behavior

1. Startup prepares required services and the retained root/catalogue, starts the
   global owner and binds its task factory/driver before producers run.
2. `TurnRunnerWorker` claims durable work and creates the native task handoff.
   Its `invoke_driver` calls the configured `MontyTurnDriverPort::drive_turn`;
   a missing driver is an error, not an alternate execution route.
3. The task selects from its coherent active approved catalogue. A match retains
   its exact workflow and typed inputs. Disambiguation is handled explicitly.
   Only actual No-Match enters `_non_match`.
4. `_execute_recipe` checks the composition response and dispatch list before
   execution. Monty drives the selected flow and retains result handoffs.
5. Reply/history completion follows the admitted task's qualified ports. A failed
   output check, reply write or later step does not permit replay of an earlier
   completed Tool effect.
6. Driver failure/cancel keeps the original attempt and settlement evidence.
   A timeout or lost acknowledgement does not prove that effects were absent.

Tier 0 executes qualified deterministic work without model calls. Tier 1 runs
its explicit reasoning/composition calls; a low legacy Wilson score is not a
substitute workflow specification. Shell and subagent Recipes are always Tier 1.
Matching/database/composition failures and begun Recipe failures stay errors.

## 5. Lifecycle and resource contract

Exactly one global orchestrator starts for the instance. Completing a task does
not stop it or create another global VM. Idle orchestration awaits work without
LLM token consumption. Settings reconciliation is a separate infrastructure
responsibility. Fatal replacement must be supervised and fenced; in-memory VM
state is not durable crash recovery.

[Simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben)
defines the binding resource target:

- `max_duration_secs` defaults to **600 executing VM seconds per task**, including
  nested execution and continuations. Preserve consumed time. Idle, queue and
  external waits do not count; global service lifetime is not a task deadline.
- External call deadlines and ownership/cancellation deadlines have their own
  contracts. Do not use a second wall-clock copy of the task compute limit.
- Token budgets default to disabled. Accounting and technical model context/output
  limits remain; disabled budgets permit no hidden retrieval/history/task caps.
- Shared memory uses startup sizing by default with a **512 MiB minimum budget**;
  a budget is a ceiling, not reserved memory. Manual live settings and optional
  automatic adjustment follow the same desired/effective revision contract.
- A stored setting is not proof of uptake. Rust and the worker must acknowledge
  the same effective revision without resetting task consumption.

`RebornRuntime::shutdown` cancels workers, awaits their handle, closes private
intents, shuts down the settings owner and requests/joins the global owner,
collecting failures rather than skipping later cleanup. Whole-product drain,
store release, embedded-PostgreSQL ordering and fatal recovery must satisfy the
owning lifecycle acceptance; this source walkthrough does not certify every gate.

## 6. Compatibility and remaining work

The old canonical Rust stage pipeline, `PlannedDriver`, `RecipeStage` and
`TierZeroExecutionStage` describe compatibility source, not today's ordinary
Monty worker path. Runtime composition explicitly supplies `orchestrator_lookup:
None` for ordinary work and supplies `monty_driver`. Do not prescribe
`TierZeroStep::Degrade`, retrieval soft-failure or stash clearing/rematching as
retained recovery behavior. The older engine `ExecutionLoop` and raw compose/run
bridge are likewise not the future global driver to activate.

Use [the architecture map](01-architecture-overview.md), [IBS](04-ibs.md) and
[Tools](06-tools-system.md) for their boundaries. The existing Simplified and
v3.5 owners retain resource/platform, dynamic-occurrence qualification and durable
recovery work. General approval/publication and full-library migration remain
separate. Existing source support does not grant full-v3 or deployment acceptance.

## 7. LLM-relevant summary

Admitted turns use `GlobalMontyDriver` on the existing instance-wide service.
Monty/Recipes sequence pinned exports and typed results; Rust owns authority and
custody. One task's completion does not end the global service. Only true No-Match
uses Tier 2. Errors, cancellation and uncertain effects never authorize automatic
replay. Task compute, external deadlines, token-budget mode and shared memory are
independent live settings with explicit effective-state acknowledgement.

## Open K3-2 consumer successor — prefix selection and construction requests

For new construction revisions, follow the one
[visible construction-selection successor](../reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection).
The earlier [K3-2 contract](../reborn/model-token-limits.md#open-k3-2-successor--retained-prefix-selection-and-prefix-free-construction-workflows)
and its [Exact §5.2](../plans/Exact_V3_definition_update_at_K3-2.md#52-bestehende-retained-prefixbindung-um-drei-auswahlzustände-erweitern)
retain their old-profile/accepted scope. Their mandatory prefix-free construction
and v3.5-only Maker clauses no longer prescribe the new workflow's context.

The submitted visible choice is captured before effect; required construction
children/reviews keep original lineage and their own executing-pair choice.
Ordinary independent chat/Sempai contracts remain. General Recipe Prefixless is
not enabled by absence, a new JSON tag or an invented bootstrap purpose. Native
reader/guard/response/UI support and the existing workflow qualification remain
open; a new target statement is no runtime/activation acceptance. Component and
Prefix-Q1/Q2 scopes, exact insertion and effect-safe retry/retention stay unchanged.
