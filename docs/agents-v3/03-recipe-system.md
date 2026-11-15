# 03 — Recipe system

## 1. Purpose and governing contract

A Recipe (class 21) selects, orders and connects reusable component usages to
fulfil a task. It declares variants, typed inputs/results, bindings, failure
behavior and completion. Design the Recipe first and reuse existing Skills,
PythonCode, ToolSkills and primitives. A specialized Rust workflow Tool is not a
substitute for reusable steps. [recipe.md](../../recipe.md) is the authoring
contract; this guide is a source map checked on 2026-11-15.

## 2. Location — verified sources

| Source | Role |
| --- | --- |
| [types/recipe.rs](../../crates/brassclaw_engine/src/types/recipe.rs) | `Recipe`, `RecipeVariant` and legacy `RecipeStep` types. |
| [instruction_builder.rs](../../crates/brassclaw_engine/src/memory/instruction_builder.rs) | `StepDescriptionEntry`, `build_instruction` and `build_ordered_instruction`. |
| [retained_instruction.rs](../../crates/brassclaw_engine/src/memory/retained_instruction.rs) | `compile_retained_recipe`, `compile_matched_retained_recipe`, `WorkflowClass` and `RetainedRecipeInstruction`. |
| [retained_inputs.rs](../../crates/brassclaw_engine/src/memory/retained_inputs.rs) | `prepare_retained_inputs` and typed data bindings. |
| [retained_source.rs](../../crates/brassclaw_engine/src/executor/retained_source.rs) | `InspectedRetainedProgram` and invocation/export preparation. |
| [pg_recipe_store.rs](../../crates/brassclaw_reborn_composition/src/pg_recipe_store.rs) | `PgRecipeStore` and existing mutable Recipe projections / review submission integration. |
| [component_deployment.rs](../../crates/brassclaw_reborn_composition/src/component_deployment.rs) | Deployment preparation and exact proposal provenance checks. |
| [authored_monty_catalogue.rs](../../crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs) | Prepares and verifies candidate retained catalogue selections, usage evidence and implementation availability. |

The [component deployment guide](../reborn/component-deployment.md) and
[implementation index](../implementation-status.md) establish the supported
publication route and attributed acceptance. No table row alone is activation.

## 3. Persisted and retained model

The authored step schema is `StepDescriptionEntry.steps` with `stepnumber`,
`knowledge`, `type`, `include` and applicable binding/dependency fields. IBS reads
the structured array, not the YAML display or prose goal. Use `knowledge:
"rust"` for a ToolSkill binding and the immediately following `knowledge:
"orchestrator"` step for its associated class-22 executable. A component step
references exactly one stable UUID. `text` is annotation; `snippet` cannot be
assembled before supported promotion. There is no persisted `llm` step type.
See the [complete persisted example](../../recipe.md#7-emit-the-actual-persisted-ibs-schema).

A variant identifies its `variant_key`, human explanation, exact `step_link`,
intent examples and variable patterns. Retained Recipe documents additionally
carry per-variant `input_layouts` and supported `invocations`; these are not extra
fields on the legacy SQL INSERT constructor. Their placement and exact shapes
are described in [04-ibs.md](04-ibs.md).

Recipe references carry stable UUIDs, not authored version numbers. At task start
the catalogue/IBS path selects exact immutable revisions/checksums, variant/link,
order/layout, dependencies, usage approvals and actual Tool implementations.
`RetainedRecipeInstruction::retained_selection` retains those component references
and workflow metadata. The instruction itself is ephemeral, not another persisted
instruction table. Selection records do not create approval or permission.

## 4. Ordinary behavior

1. Capture one coherent active approved catalogue for matching and assembly.
   Match the exact Recipe variant and intent expression; reject absent/ambiguous
   membership. Private workflows use their trusted admission contract instead.
2. Compile the ordered instruction from immutable selected Recipe bytes. Resolve
   dependencies and check data edges, Tool bindings, exports and applicable
   approval/implementation evidence before allowing effects.
3. Bind task values as typed data. Per-step inputs may use task inputs, constants,
   supported consumer defaults or earlier result fields. Defaults apply only to
   missing values; null and invalid values are not missing.
4. Load selected definitions dependency-first without effects. Monty invokes
   pinned exports, validates/retains results and decides the next selected step.
   The current ordinary retained adapter allows one bound Tool call per executable
   step and stop-only failure. Independent calls need separate binding/execution
   pairs; internal code composition cannot bypass that boundary.
5. Complete through the qualified reply/history owner. Do not publish another
   reply or repeat a completed Tool because a downstream operation failed.

Tier 0 is qualified deterministic execution with no LLM calls. Tier 1 contains
its declared reasoning/composition work; shell and subagent Recipes always belong
there. Legacy maturity/Wilson fields are not retained workflow approval or a way
to classify arbitrary work as deterministic. Only true No-Match enters Tier 2.
Errors, disambiguation and begun failures are separate outcomes.

Publicly matched Recipes require at least ten genuine intent examples and
positive/negative coverage for every public variant, including exact command
syntax, separator/escaping and malformed input boundaries. Private workflows
qualify trusted typed admission rather than fabricating public commands. MCP
publication separately requires a qualified command Recipe/variant.

## 5. Review, successors and compatibility

The [own-functional approval successor](../../skills.md#own-functional-approval-and-successor-compatibility)
is a binding target, not an already general publication API. A new/changed Recipe
validates its own supported variants, effects, data handoffs, failures and
completion. It does not reapprove unchanged qualified components. Skill/code
functions have their own joint review subject; independent pure steps need their
own applicable evidence.

Existing exact authored association profiles and complete-Recipe deployment keep
their actual supported meaning until connected successor cutover. Controlled S
bootstrap separately requires noncircular integrity/Q1/behavior qualification;
`source='system'` alone supplies none of it. Installed authored edits require
human Q2. Preserve operator overrides and historical approved bytes.

Compatible A successors preserve required consumer contracts without new approvals
for unchanged consumers. New tasks select the coherent active successor graph;
running, child, waiting and resumed tasks keep their exact original selection and
artifacts. Unsupported newest combinations fail before effects; do not downgrade
silently or rematch after failure.

## 6. Legacy paths and remaining work

The mutable SQL projection, legacy `RecipeStep { skill, tool, params, description }`,
Wilson scoring and legacy `compose_program` are distinct from immutable retained
workflow selection. Legacy source substitution and prompt-hint execution are not
the v3 authoring interface. Do not activate the old `ExecutionLoop` bridge as a
replacement for ordinary global execution.

General independent pure approval/publication, broader own-functional successor
readers, full-library migration, qualified dynamic occurrences and durable recovery
remain with existing owners. V35-03/04 consume pure approval; V35-13/14 own remaining
occurrence support; V35-05/06 qualify reference workflows using retained support.
Use current owner evidence rather than old Phase N migration-number predictions.
Class-50 Scaffold is historical storage, not a new validator or protected root;
future-use flows use normal Recipes while preserving old UUIDs/pins.

## 7. LLM-relevant summary

A Recipe assembles reusable usages and pure exports with typed data and exact
retained identities. ToolSkill binding executes nothing; Monty invokes the
selected class-22 export and the kernel checks every real dispatch. Tier 0 is
model-free; Tier 1 has explicit model work. Only No-Match enters Tier 2. Preserve
original selections/results and effect certainty across waits, cancellation and
successors; approval, activation and live Tool permission remain distinct.
