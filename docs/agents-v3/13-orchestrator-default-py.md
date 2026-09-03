# 13 — Python Orchestrator (Monty)

> **Subsystem (f2):** the **Python Orchestrator** — the brain of the
> Orchestrator/Executioner split. The Orchestrator (Monty VM, Python) is the
> **sole sequencing authority**: it resolves intent, composes the matched recipe
> into a predefined program, iterates that program's `steplist`, consults the
> carried `skills` array for exact tool usage, and runs each step's
> `executable_code` via `host.run_program`. It assembles every LLM prompt. The
> Rust **Executioner** is a library it drives — precompiled tools invoked as
> `host.<tool>(...)` — with no sequencing of its own.
> **Grounded in:** `crates/brassclaw_engine/src/executor/orchestrator.rs`
> (`execute_orchestrator` + the `host.*` dispatch arms), `crates/brassclaw_engine/
> src/executor/composition_port.rs` (`CompositionPort` trait),
> `crates/brassclaw_engine/src/memory/composition.rs` (`ComposedProgram`/
> `compose_program`), `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`
> (`PgCompositionPort`), `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs`
> (the builtin `host.*` seed), `builtin_stuff_v3.md` Step 27, `saved_plan_to_v3.md`
> Step C.

## 1. Purpose

The Orchestrator is **Monty** — a Python VM program. It does not contain a fixed
script that "performs the recipe's steps one by one" (the retired `default.py` /
`__execute_action__` Model-A step-machine did that, and is gone). Instead, each
turn the Orchestrator **composes** the matched recipe into a predefined structure
and **runs** that structure:

```
host.resolve_intent(scope, user_text)
  └─ Match {component_id, step_link, ...}
       └─ host.compose_orchestrator(component_id, step_link, user_input)
            → {ok, program:{skills[], steplist[], rust_directives[],
               variables{}, assembled_program, tier}}
                 └─ for step in program["steplist"]:
                        consult program["skills"]   # exact tool usage
                        host.run_program(step["executable_code"])
```

The **composer never runs anything** and **never bakes a single program string**.
It returns parts; Monty iterates the `steplist` and runs each step's
`executable_code` through `host.run_program` (Monty 0.0.16 has no
`exec`/`eval`/`compile` builtin — a host callable is the only way for Monty to
run a dynamic code string). Tools are invoked inside that code as
`host.<tool>(...)` and executed by the Rust Executioner.

## 2. Location

- **Engine host-call dispatch:** `crates/brassclaw_engine/src/executor/orchestrator.rs`
  - `execute_orchestrator` — the engine entry that runs the Orchestrator Monty
    VM program and dispatches its `host.*` MethodCalls.
  - The `host.*` handler functions (one per builtin host call; see §3).
- **Composition port (engine side):** `crates/brassclaw_engine/src/executor/composition_port.rs`
  — the `CompositionPort` trait + `CompositionPortError`. The `host.compose_orchestrator`
  handler thin-calls `CompositionPort::compose`.
- **Composition core:** `crates/brassclaw_engine/src/memory/composition.rs` —
  `ComposedProgram`, `ComposedStep`, `RustDirective`, `SkillRef`,
  `ComponentResolver`, `compose_program` (pure; binds `{{vars.NAME}}`).
- **Composition impl (the IBS):** `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`
  — `PgCompositionPort` (owns `Arc<PgPool>`, runs the recipe SELECT → variant
  match by `step_link` → `build_instruction` → resolve includes/tools →
  `compose_program` pipeline).
- **Builtin seed:** `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs`
  — seeds the `host.*` toolskills/skills/python-code/recipes idempotently at boot.
- **Plan:** `saved_plan_to_v3.md` Step C (C.1 retirement, C.2 seed, C.3 cdylib,
  C.4 security, C.4.5 common syntax + composition system); `builtin_stuff_v3.md`
  Step 27 (the `host.*` surface).

## 3. The `host.*` call surface

The Orchestrator talks to the engine exclusively through `host.*` MethodCalls
(empty-attrs `host` Dataclass → `CallAttr` → `MethodCall` → `FunctionCall{
function_name: "<tool>", method_call: true, args[0] = self }`). The builtin
surface (seeded C.2):

| Host call | Kind | Purpose |
|---|---|---|
| `host.resolve_intent(scope, user_text)` | read | 4-class intent classifier → `{status, component_id, step_link, component_name, …}` (Match/NoMatch/Disambiguate). f1. |
| `host.compose_orchestrator(component_id, step_link, user_input)` | read | Compose the matched recipe + variant → `{ok, program:{skills, steplist, rust_directives, variables, assembled_program, tier}}`. The IBS. |
| `host.run_program(code_string)` | exec | Run a dynamic Python code string in the Monty VM (nested `execute_code`). The per-step run mechanism. |
| `host.fetch_component(component_id)` | read | Fetch a component by id (raw content). |
| `host.resolve_component_by_name(name)` | read | Resolve a well-known name → component id. |
| `host.validate_component(component_id)` | read | Run the Q1 validator on a component. f3. |
| `host.assemble_prior_knowledge(scope, …)` | read | Tier-0/Tier-1 prior-knowledge assembly (`PkrAssemblyResult`). |
| `host.non_match_llm_answer(scope, user_text, …)` | read/llm | Non-Matching-Mode: build the prompt + get the LLM answer. |
| `host.post_reply(scope, text)` | write | The single end-of-turn emit (both modes). |
| `host.save_history(scope, …)` | write | Persist the turn transcript. |
| `host.check_signals()` | read | Cooperative stop/inject signal check. |
| `host.kohai_complete(…)` | write | Mark a Kohai self-improvement cycle done. f7. |

Tools (class 0) are *also* `host.<tool>(...)` calls — the same dispatch path;
the Executioner runs them (builtins directly, cdylib tools via the C.3
`DynamicToolLoader`/`DynamicToolPort`).

## 4. Matching-Mode flow

1. **Resolve intent** — `host.resolve_intent` classifies the user text. A
   `Match` carries the recipe `component_id` + the matched variant `step_link`
   (+ `component_name`). `NoMatch` → §5. `Disambiguate` → the disambiguation UX.
2. **Compose** — `host.compose_orchestrator(component_id, step_link, user_input)`
   thin-calls `PgCompositionPort::compose`, which:
   - SELECTs the recipe (class 21) row by `component_id` + scope.
   - Matches the variant by `step_link` (`NoVariantMatch` → degrade).
   - Runs the IBS `build_instruction` → `BuildInstruction`.
   - Captures `{{vars.NAME}}` slots from `user_input`.
   - Resolves included component UUIDs (PythonCode → `executable_code`,
     Skill/ToolSkill → `skills`, Tool → `rust_directives`) via a
     `ComponentResolver`.
   - Returns the predefined `ComposedProgram`.
3. **Iterate + run** — Monty iterates `program.steplist`: for each step it
   consults `program.skills` for the exact tool usage (so the steplist need not
   repeat that detail), then runs `step.executable_code` via `host.run_program`.
   Tier 0 (no LLM) replies directly; Tier 1 (LLM-guided) has the Orchestrator
   assemble the prompt → InterceptorStage (Sempai review) → ModelStage.

`rust_directives` (cdylib load directives) are **carried** in the program for the
Executioner's `DynamicToolLoader` and are **not executed by the Orchestrator**.
Applying them (dlopen) is a C.5/C.6 driver concern.

## 5. Non-Matching-Mode flow

When `host.resolve_intent` returns `NoMatch`, the Orchestrator builds an LLM
prompt (head + body) and obtains the answer:

- **body** = chat message + history + selected memories/components.
- **head** = the pre-compiled `base-prompt` (in the vLLM KV-cache). Composition
  inserts a single `base-prompt` line; the Sempai-Kohai system substitutes the
  real pre-compiled content at the very end of prompt creation, just before
  tokenizing (f6).
- `host.assemble_prior_knowledge` + `host.non_match_llm_answer` carry the
  retrieval + LLM-call work; `host.post_reply` emits the answer.

## 6. Skills as a first-class array

Skills are **not** assembled into a static prefix by the Orchestrator alone. The
composed `program.skills` is a first-class array Monty **consults while stepping**:
each skill carries the exact usage of one or more tools, so a `steplist` step
need only name the approach and its `executable_code` — the skill supplies the
tool-call detail. This keeps recipes small and tool usage consistent. (See
`05-skills-system.md` for the skill kinds; `09-sempai-kohai.md` for skill
authoring.)

## 7. The Executioner boundary

The Rust Executioner runs **only** what the Orchestrator calls:
- builtins (the `host.*` surface above + first-party tools),
- cdylib tools loaded per `rust_directives` (C.3 `DynamicToolLoader`).

**Rust does not sequence steps, pick recipes, or assemble prompts.** The
orchestrator-command-driven rule: every Executioner action originates from an
Orchestrator `host.*` call. Exceptions (a host call that may proceed without an
explicit orchestrator command) are limited to safety-critical enforcement
(stop-signal honoring, budget hard-stops, sandbox refusal) and are enumerated in
`06-tools-system.md` (f5).

## 8. Activation status

The engine Monty VM `execute_orchestrator` host-call path — and therefore
`host.compose_orchestrator` / `PgCompositionPort` — is **constructed and
unit-tested but inert in production until the C.5/C.6 driver** wires
`PgCompositionPort` into `ThreadManager` and applies `rust_directives` via the
`DynamicToolLoader`. The live production Tier-0/Tier-1 path today runs through
the **turns `PgOrchestratorLookup` bridge** (wraps the engine
`TierZeroOrchestrator::run_tier_zero`). `#![allow(dead_code)]` covers the
inert window on the engine-side port plumbing. C.5/C.6 is what activates this
path as the primary driver.

## 9. Retired — Model A (`default.py`)

The prior engine driver was `default.py` (Model A): a fixed, self-modifiable
Python `run_loop(context, goal, actions, state, config)` that owned turn
sequencing, prior-knowledge assembly, the LLM call (`__llm_complete__`), and
code/action execution (`__execute_code_step__`, `__execute_action__`,
`__execute_actions_parallel__`). It is **retired** (C.1):

- `__execute_action__` / `__execute_actions_parallel__` match arms + handlers
  + helpers deleted; the `__execute_actions_parallel__` meta-primitive retired
  (Python cannot do true parallel dispatch).
- The entire `loop_engine::tests` and `runtime::manager::tests` mods deleted
  (Model-A test retirement, pulled forward into C.1).
- `default.py` itself is gone; the Orchestrator is now the composed-program
  runner described in §1–§8.

The no-LLM pattern Model A used for class-16 Actions
(`execute_action_procedure`, returning directly from `run_loop` without an LLM
call) is **generalised** by Tier 0: any recipe whose variant has
`llm_call_required == false` composes to a `steplist` the Orchestrator runs
without an LLM call.

## 10. Relations

- **Intent** (`02-intent-system.md`, f1) — `host.resolve_intent` is the entry;
  the Match carries `component_id` + `step_link`.
- **IBS / composition** (`04-ibs.md`) — the IBS **is** the composition system;
  `host.compose_orchestrator` thin-calls it (F4).
- **Skills** (`05-skills-system.md`, f4/f7) — carried as `program.skills`,
  consulted while stepping.
- **Tools / Executioner** (`06-tools-system.md`, f5) — `host.<tool>(...)` +
  cdylib `rust_directives`; the orchestrator-command-driven rule.
- **PythonCode** (`07-pythoncode-system.md`) — each `steplist` step's
  `executable_code` is composed PythonCode (placeholders bound by the composer).
- **Validation** (`14-validation-queue.md`, f3/f8) — `host.validate_component`;
  Q1 gates new components.
- **Prefix / base-prompt** (`10-prefix-base-prompt.md`, f6) — the `base-prompt`
  placeholder substitution (Non-Matching-Mode head).
- **Sempai-Kohai** (`09-sempai-kohai.md`, f7) — `host.kohai_complete`; the
  InterceptorStage review on the Tier-1 path.
- **Agent loop** (`12-agent-loop.md`) — the stage pipeline that hosts the
  Orchestrator on the turns path (the `PgOrchestratorLookup` bridge today; the
  engine VM host-call path after C.5/C.6).
