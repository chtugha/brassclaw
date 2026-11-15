# 07 — PythonCode components

## 1. Purpose

PythonCode (class 22) provides reusable executable definitions for a Skill's one
Tool usage or for pure logic. Recipes supply sequencing and typed input/result
wiring. A Skill combines usage prose with explicitly associated preloadable code;
pure code needs no artificial Skill or ToolSkill. Follow
[skills.md](../../skills.md) and [recipe.md](../../recipe.md). This source map was
checked on 2026-11-15.

## 2. Location — verified sources

| Source | Symbols / role |
| --- | --- |
| [pg_python_code_store.rs](../../crates/brassclaw_reborn_composition/src/pg_python_code_store.rs) | `PgPythonCodeStore` and the existing save/review-submission surface. |
| [retained_preload.rs](../../crates/brassclaw_engine/src/executor/retained_preload.rs) | `PreloadDeclaration`, `ExportDeclaration` and invocation generation from `python-preload/2`. |
| [retained_source.rs](../../crates/brassclaw_engine/src/executor/retained_source.rs) | `InspectedRetainedProgram`, `InspectedPureSelection` and `InspectedPureComponent` expose exact inspected source/exports. |
| [retained_inputs.rs](../../crates/brassclaw_engine/src/memory/retained_inputs.rs) | Recursive input/result contracts and prepared typed bindings. |
| [retained_tools.rs](../../crates/brassclaw_engine/src/memory/retained_tools.rs) | Usage association and native binding preparation. |
| [retained_recipe.rs](../../crates/brassclaw_engine/src/executor/retained_recipe.rs) | `RetainedRecipeExecution` / `run_step` execute selected child work and retain actual Tool/result boundaries. |
| [pg_retained_q1.rs](../../crates/brassclaw_reborn_composition/src/pg_retained_q1.rs) | `StructuralReviewSet` and `BehavioralReviewSet` produce evidence for their supported retained adapter, not arbitrary caller success reports. |
| [q1_orchestrator.rs](../../crates/brassclaw_reborn_composition/src/q1_orchestrator.rs) | `Q1Outcome` distinguishes structural observation, passed, failed and deferred outcomes; the historical `run_q1_validation_retained` helper is gated by test + skills-db. |

## 3. Stored content and retained declarations

[V052](../../crates/brassclaw_pg/migrations/V052__reborn_python_code.sql) created
`reborn_python_code`, including `content`, component lifecycle/provenance metadata
and dependency registry. [V069](../../crates/brassclaw_pg/migrations/V069__reborn_python_code_includes.sql)
added `includes`. Those columns alone do not establish qualified recursive
assembly, approved dependencies or permission to execute current row bytes.
The queue tracks review work separately; `validation_status` is not live Tool policy.

The retained document declares input/result contracts and `python-preload/2`:
public exports with implementation symbols, parameters and mapping form; private
functions, constants, imports, code dependencies and an optional `default_export`.
These are checksummed revision-document records, not invented additions to legacy
INSERT constructors. The code/Skill association and Recipe invocation must agree.

A concrete packaged mapping export exists in
[global_bootstrap_components.rs](../../crates/brassclaw_skills/src/global_bootstrap_components.rs):

```python
def publish_turn_reply(inputs):
    return host.post_reply(answer=inputs['answer'])
```

Its declared input is a required string `answer`. The selected Recipe supplies
that value as data. Loading the function posts nothing; the actual invocation
uses the admitted task's reply adapter. Reusing these source bytes does not
approve a new Recipe or replace exact qualification/implementation evidence.

## 4. Preparation and execution

1. Select one coherent active approved graph and pin exact code/interface,
   dependencies, contracts, usage evidence and native artifacts.
2. Validate declared references and symbols. Load qualified definitions and
   immutable constants dependency-first with stable-UUID tie-breaking. Reject
   cycles, collisions, unsafe imports and effectful initialization/defaults.
3. Select a supported Recipe invocation or the code's declared default export.
   A missing/invalid export is an error. Do not execute prose, infer names from
   documentation or generate `eval`/`getattr` from task values.
4. Bind typed task/local inputs, applying defaults only to missing values.
   Monty invokes the retained export and captures its return. The native boundary
   validates actual arguments/results against the selected contracts.
5. Keep mutable data isolated per task/attempt/invocation. New activated code
   does not replace exports used by running, waiting or child execution.

A Tool-calling component needs its exact Skill association and preceding ToolSkill
binding. Ordinary retained execution accepts one bound Tool call per executable
step and stop-only failure. Independent calls stay separate even when hidden
inside helper code. Reconciliation/retry is not an implicit call inside a helper.

Pure code has zero host Tool calls across its selected closure. Its own interface,
source/dependency and behavioral approval remains distinct from merely appearing
as a helper in a qualified Tool usage. See
[the pure-component review contract](../reborn/pure-component-review.md#exact-remaining-cutover-contract).

## 5. Validation evidence and compatibility

Contained source inspection, contract parsing, observed execution, full functional
Q1, human Q2 and activation prove different things. A parse/body scan cannot prove
semantic agreement. The supported retained evidence producers preserve actual
source/subject/run/attempt/result custody; a caller-provided pass flag is not Q1.
The [validation guide](14-validation-queue.md) maps these owners and remaining gates.

There is no current engine `memory/component_validator.rs` at the historical path,
and the old names `validate_python_code_body` / `validate_python_code_placeholders`
are not a current class-22 production validator API. Their former substring-scan
and placeholder descriptions must not be presented as shipped qualification.
The test-gated Q1 runner is not enabled in production merely by seeding a Recipe.
Production discovery/selection and complete review require their actual owners.

The `snippet` enum and `UnpromotedSnippet` assembly guard exist in
[instruction_builder.rs](../../crates/brassclaw_engine/src/memory/instruction_builder.rs).
They do not prove a complete current WebUI promotion/review implementation.
Author new v3 code as qualified retained exports through supported authoring/review
paths; do not invent inline-execution permission or promise a historical Phase N
migration will supply it automatically.

## 6. Approval and remaining work

Installed authored changes require applicable Q1, observed behavior and human Q2.
The own-functional A successor reviews a Skill and implementing code jointly
with separate IDs; unchanged approved counterparts/dependencies are context.
Current exact authored readers keep their supported scope until connected cutover.
S bootstrap separately qualifies prerequisite Tool, ToolSkill and Skill/code with
noncircular integrity/behavior evidence and null human Q2. Labels do not select S.
See [05-skills-system.md](05-skills-system.md).

Typed retained inputs, preloading, selected exports, source inspection and exact
usage association support exist. General independent pure approval/publication,
broader successor readers, full-library migration, dynamic-occurrence qualification
and durable recovery remain with existing owners. A legacy `includes` column,
raw compose/run program or successful contained test does not complete those gates.
Use [the implementation index](../implementation-status.md) for scoped acceptance.

## 7. LLM-relevant summary

PythonCode exports implement one Tool usage or pure logic. Preload exact declared
definitions without effects; invoke selected exports with typed isolated inputs.
Associations, source inspection, behavior, approval and activation are separate.
The ordinary retained Tool runner is single-call/stop-only. Do not substitute
legacy source interpolation, prose execution or absent validator APIs for the
supported retained path, and never replay effects after downstream failure.
