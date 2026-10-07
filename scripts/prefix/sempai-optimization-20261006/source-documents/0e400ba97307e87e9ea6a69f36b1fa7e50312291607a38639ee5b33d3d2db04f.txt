# Agent Rules

## Binding Recipe architecture (v3)

Read all four ground-truth component guides before creating or changing components:

- [recipe.md](recipe.md): ordered workflows, variants, actual persisted IBS
  schema, typed input bindings, component assembly and task version selection.
- [skills.md](skills.md): one Tool usage with prose plus executable PythonCode,
  recursive contracts, exact-version association approval and retry rules.
- [tools.md](tools.md): Rust primitives, implementation identity/retention,
  live global policy, registration, technical constraints and crash recovery.
- [toolskills.md](toolskills.md): Rust-side IBS binding descriptors, reuse,
  parameter/adapter compatibility and the actual storage/binding limitations.

These four guides govern component definitions and authoring over summaries,
archive examples and older subsystem instructions. Recipe/Skill contracts remain
defined by recipe.md and skills.md; tools.md and toolskills.md specify their
primitive/binding support. The rules below are binding v3 targets, not proof of
completed runtime/store enforcement.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills explain one Tool usage and have associated executable PythonCode;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including Recipe/variant/step_link/input layout,
  nested dependencies and exact association approval references. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls host.<tool>(...). Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

Current code still has plain text substitution, fresh state in nested step
execution and incomplete immutable version manifests/binding preparation. The
new typed inputs interface and strict single-component validation require
implementation and production-path acceptance; do not claim these are shipped.

### Required authoring and startup checks

1. **Define the usage and workflow separately.** A Skill is one Tool usage:
   prose plus explicitly associated PythonCode. A Recipe orders usages and
   pure-logic components, maps inputs/results and defines completion. A Skill
   never hides a multi-Tool task. Creating a reusable Skill independently is
   allowed; verify it with a small workflow without requiring a permanent Recipe.
   A Tool supplies the primitive; a ToolSkill supplies IBS binding metadata.
   The Skill tells the Orchestrator how to use that Tool; its code implements it.
2. **Keep data separate from code.** Declare recursive input/result schemas,
   including list items, object fields, allowed extra values, nullability,
   defaults and numeric bounds. Missing and null are different. Defaults apply
   only to missing consumer inputs, never to invalid/null values or bad outputs.
   Recipe references bind typed data to `inputs["local_name"]`; they never paste
   runtime values into Python source. This interface still needs runner support.
3. **Review meaning before activation.** For authored versions, author review,
   supported Q1 audits, behavioral validation and human Q2 establish agreement
   between prose, binding metadata and code. Trusted bootstrap uses its distinct
   evidence contract in check 4.
   Parsing and matching schemas alone do not prove behavior. IBS checks approved
   structured records; it does not interpret prose or call an LLM to approve a
   Tier-0 task at startup.
4. **Record approval separately from selection.** Follow skills.md's exact
   `skill-association/1` and `skill-association-approval/1` target contracts.
   The association references stable UUIDs. The trusted approval record identifies
   the exact reviewed revisions/checksums and dependency combination. Individual
   component approvals or a task manifest do not establish combination approval.
   New combinations need their required evidence before coherent activation;
   unchanged dependencies and old running combinations do not need reapproval
   merely because a replacement exists. Authored combinations require trusted
   Q1, human Q2 and behavioral evidence. Only verified system_seed bootstrap
   provenance permits null q2_ref with required Q1/integrity and behavioral
   evidence; source/status labels alone never qualify. These are not grants.
5. **Pin the complete workflow.** IBS selects one consistent approved catalogue
   snapshot. Retain the Recipe revision, variant, exact `step_link`, selected
   steps/order, input layout, all component/dependency revisions/checksums and
   association approval identifiers with the task before execution. Matching and
   assembly must use the same generation. Resume/child execution/retry retains
   that selection; do not match again or read latest midway. Keep old artifacts
   while tasks/checkpoints need them. BuildInstruction stays ephemeral; retain
   task snapshot references through the continuation contract. Retain actual
   immutable Tool implementation artifacts, not just metadata checksums.
   Incompatible or unapproved newest active combinations fail before effects;
   never silently select older versions or replay the failure as Tier 2.
6. **Make failure and retry exact.** Use skills.md's failure contract. Attempt
   counts are positive integers including the initial dispatch and survive
   waits/reclaims. Retries require explicit eligible outcomes and verified
   read-only or durable deduplication evidence. A timeout does not prove no
   effect occurred. Never replay a completed effect because output validation,
   another step or reply posting failed. Cancellation, stale attempts, live Tool
   policy and resource limits remain effective before every retry. Persist
   dispatch intent/count and confirmed/unresolved effect state through the
   supported durable recovery contract. VM memory is not a checkpoint;
   crashes never automatically replay the whole Recipe or reset counts.
   Fence the old generation; only supervised, reconciled recovery may
   replace the global VM. No parallel VM or silent per-chat/Tier-2 fallback.
7. **Prepare bindings and check live policy.** Follow toolskills.md for one
   Tool binding and tools.md for actual registration/loading and identity.
   Metadata never executes code or grants permission. Verify parameter,
   adapter and recursive usage contracts agree. Every retained version/alias
   of one Tool receives its current global policy; invalid identity mappings
   fail closed. Recheck technical constraints and freshness before dispatch.
8. **Report support honestly.** A Markdown design is not an activated component.
   Inspect actual stores, validators, host adapters and the selected Monty path.
   Missing schema, binding, approval or runtime support is implementation work,
   not permission to invent fields/APIs or claim completed enforcement.

## Binding Skill definition (v3)

A **Skill** is one reusable tool-usage pattern for the Orchestrator. It comprises
**both prose instructions and explicitly associated executable PythonCode**:
the prose explains purpose, parameters, prerequisites and result/error handling;
the PythonCode implements that usage. “Leaf Skill” means this same unit, not a
different kind of Skill. A broader domain or multi-tool overview belongs to an
**Extension**, documented by its ExtensionCatalogue; a **Recipe** defines the
ordered workflow and references reusable components by UUID.

**Tool + ToolSkill belong to the Rust side.** The Tool provides the primitive;
the ToolSkill describes its IBS binding. Binding executes nothing and grants no
permission. The Orchestrator executes the associated PythonCode, which calls
`host.<tool>(...)`; the kernel checks the current global tool policy.

**Storage is not the definition:** today Skill prose is stored in `reborn_skills`
(classes 1–3), while executable code is stored separately in `reborn_python_code`
(class 22). The current composer emits `SkillRef.body` and
`ComposedStep.executable_code` separately. The target requires an explicit,
validated UUID/revision association between the two parts; separate rows are
permitted and do not make the Skill prose-only. A code example inside prose is
documentation, not an implicit executable entry point. The class labels
`skill_rusty`, `skill_monty`, `skill_llm` are existing consumer classifications,
not leaf/domain hierarchy levels. Classes 10 and 50 are Orchestrator/Scaffold
records sharing the table, not additional tool-usage Skill types.

**Execution and validation:** Tier 0 uses the associated PythonCode without an
LLM interpreting prose. Tier 1 can use prose in its explicit LLM steps. Never
execute prose as Python. ToolSkill references belong to Rust binding steps;
class-22 entry points belong to Orchestrator execution steps. Keep unrelated
tasks/attempts isolated and preserve needed typed state within each Recipe,
including child execution and waits. Q1/Q2 and the documented dependent-chain
exception remain applicable. See the actual `knowledge`/`stepnumber` Recipe
schema; explanatory channel names are not insert fields. These definitions do
not implement an association schema, binding engine or new runtime path.

## Binding Tool and ToolSkill definitions (v3)

A **Tool** (class 0) is a registered Rust-side primitive. An Orchestrator call
through the supported `host.<tool>(...)` boundary performs one declared operation
and returns a result, classified failure or explicit wait/handle. A Tool does
not choose the task workflow. An operation selector can expose several primitive
operations; this does not justify hiding a multi-Tool task in new Rust code.

A **ToolSkill** (class 13) is reusable metadata describing one Tool usage's
IBS binding: the Tool identity, supported callable/adapter, parameters,
prerequisites and result/error expectations. It executes nothing, grants no
permission and normally requires no compilation. Several ToolSkills can describe
different bindings of one Tool; several Skills can reuse a compatible binding.
Its stored text or quoted call signature is not executable PythonCode or Skill
instructions for the Orchestrator.

**The Orchestrator needs the Skill to know how to use the Tool.** The Skill's
prose explains that usage and its associated PythonCode implements it. IBS uses
the ToolSkill to prepare the compatible binding. Neither a ToolSkill alone nor
an unassociated code example completes a Skill. Tier 0 executes the approved
associated code without an LLM interpreting prose; explicit Tier-1 LLM work can
use the prose as context.

Resolve the stable Tool UUID, dispatch capability ID, host callable, adapter and
exact retained implementation explicitly. A name, metadata row, successful build
or nonempty artifact path does not prove registration/loading. Pin actual
implementation artifacts as well as definitions; never substitute a mutable
file or latest handler under the same name. Apply the same Tool's current global
policy across every retained version and dispatch alias; missing/conflicting
identity mappings fail closed. Approval/version selection never grants permission.

Use one Rust-channel ToolSkill component step immediately followed by its
matching Orchestrator-channel PythonCode step. Each component step includes one
UUID. Pure logic needs no artificial binding. Internal PythonCode composition
and the documented direct dependent-chain exception remain allowed, with every
actual Tool binding covered; independent Tool calls require separate steps.
In persisted Recipe JSON use `knowledge` and `stepnumber`, not explanatory
`channel`/`step_id` fields. Read recipe.md's actual schema before authoring JSON.

Inspect current constructors and consumers. ToolSkill `tool_name`, `param_schema`,
`param_template`, `content` or `includes` do not themselves establish a full
typed binding, executable association, immutable revision or loaded callable.
Do not invent API fields or execute metadata to conceal missing runtime support.
ToolSkill creation needs validation/approval; a genuinely new Tool implementation
needs its separate build, verification and supported registration/loading path.

> **Local test/model connections:** Before remote tests or provider setup, read
> [LOCAL_TEST_ENV.md](LOCAL_TEST_ENV.md) if present. It contains the operator's
> SSH aliases, host roles, inference endpoint and verified prerequisites. The
> file is Git-ignored; its absence on another checkout is not an empty config.

> **Primary Rule — read this first:**
> When a user asks you to add, change, or fix a capability in BrassClaw, your first answer
> is a **Recipe** — not Rust code. The component library (Recipes + PythonCode + Skills +
> ToolSkills) is where almost all behaviour lives. New Rust code is only warranted when a
> genuinely new system-level Tool is needed that no existing Tool can provide. When in doubt,
> ask: "Can this be done by wiring existing tools in a Recipe?" If yes, write the Recipe.

## Purpose and Precedence

Read [Development reasoning and validation policy](docs/development-policy.md)
before implementation. It is authoritative for development check selection,
evidence reuse and stopping, superseding older blanket test/lint instructions.
Use source inspection and LLM reasoning to diagnose and complete a coherent
change before compiling. Product Tier-0 rules do not limit development reasoning.
The disk-space and cleanup rules below remain mandatory and unchanged.

`AGENTS.md` is the quick-start routing map for AI coding agents entering the codebase. It is not the full architecture spec. Read the relevant subsystem spec before changing a complex area. When a crate spec exists, treat it as authoritative.

Start with these deeper docs as needed:

- `CLAUDE.md`
- `crates/brassclaw_reborn_cli/AGENTS.md`
- `crates/brassclaw_reborn/CLAUDE.md`
- `crates/brassclaw_reborn_composition/CLAUDE.md`
- `crates/brassclaw_agent_loop/CLAUDE.md`
- `crates/brassclaw_llm/CLAUDE.md`
- `crates/brassclaw_reborn_webui_ingress/CLAUDE.md`
- `tests/e2e/CLAUDE.md`

## Architecture Mental Model

**Component authoring ground truth:** [recipe.md](recipe.md) defines ordered
workflows, typed inputs and the actual persisted IBS schema; [skills.md](skills.md)
defines one Tool usage with prose plus associated PythonCode, recursive contracts,
exact-combination approval and retries; [tools.md](tools.md) defines Rust primitives,
retained implementations, live policy and recovery; [toolskills.md](toolskills.md)
defines IBS binding metadata and its authoring/storage constraints.

The Skill tells the Orchestrator how to use the Tool; its associated PythonCode
implements that usage. The ToolSkill tells IBS how to prepare the binding and
executes nothing and grants no permission. Recipes sequence reusable usages and
pure logic, bind typed inputs/results and define completion. Use one UUID per
component step; internal code composition is allowed. Normal Tool use pairs a
Rust binding step with its immediately following executable step. Independent
Tool calls require separate steps; only the documented dependent-chain exception
permits multiple calls in one body, with all bindings covered.

Semantic agreement is reviewed before activation, not inferred by IBS from prose.
IBS checks trusted exact-combination approval and pins one coherent workflow and
complete dependency graph, including actual Tool artifacts. Missing/incompatible
newest active contracts or approval fail assembly; never silently downgrade or
enter Tier 2. Running/resumed tasks keep their original selection. Authored
versions require Q1/human Q2 and behavioral evidence; only verified system_seed
bootstrap provenance permits null q2_ref with required integrity/Q1 and behavioral
evidence. Source/status labels are not approval evidence.

Current global Tool policy covers all retained versions/aliases before every
dispatch, independently of component approval. Preserve technical constraints,
freshness, durable attempt counts and effect reconciliation across recovery.
Never replay completed effects or entire Recipes after a crash/unknown outcome.
These are target requirements, not runtime acceptance claims; historical examples
do not override the four guides.

BrassClaw Reborn is organized in five conceptual layers:

- **Products** own UX and surface-level composition. They wire together loops, capabilities, and host access for a specific deployment shape (CLI, web, daemon). Products do not implement agent logic directly.
- **Loops** own agent behavior. They manage planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. A loop is the unit of agentic execution. Product code must not implement a second loop or bypass the loop runner.
- **Kernel** owns authority. It controls trust decisions, secret resolution, safety policy enforcement, sandboxing, capability grants, and session identity. Kernel boundaries are not negotiable from product or loop code.
- **Infrastructure** — Shared services: LLM providers, Postgres persistence, embeddings, extensions, and observability. Lives in `crates/`.
- **Component Library** — Recipes, Skills, ToolSkills, PythonCode snippets, and ExtensionCatalogues stored in Postgres. **This is where most new capabilities are added.** Existing supported contracts can add component rows without a new primitive; missing schema/runner support still requires implementation. See `builtin_bootstrap.rs` for how first-party components are seeded.

New Reborn work that genuinely requires new infrastructure belongs in `crates/`. New *capabilities* belong in the Component Library first.

### Simplified v3 authorization target (binding)

`simplified_v3.md` sections 1.1 and 9 supersede older operation-approval and
scoped operator-access requirements. The instance-token operator administers
all supported functions without user, tenant, project or feature-role checks.
Tools use current instance-wide allow/block settings and technical parameters,
checked by the kernel before every dispatch, including an already running
recipe. ToolSkill binding grants no permission. There is no additional
invocation/run/attempt tool approval or fingerprinted approval lease.

Run claims and attempt identifiers still fence cancellation, stale execution,
replies and idempotency; they are not tool grants. External-service authentication,
authored Q1/human Q2 and trusted bootstrap integrity requirements remain,
alongside sandboxing, network/secret enforcement and resource limits.
Existing scoped stores and operation-approval code are legacy implementation
until the coordinated dispatch/data cutover. Do not extend those paths as v3
requirements or disable technical enforcement to bypass them.

Only an actual No-Match enters Tier 2. Matching/DB errors, ambiguity and begun
recipe failures must remain distinct; never replay a failed recipe as Tier 2.
Running tasks retain their selected component revisions. Live tool policy is
checked independently of those fixed component revisions. Monty task time and
allocation budgets are separate from the shared live-heap limit. The target
`max_duration_secs` default is 600 seconds of executing VM time per task,
excluding idle/queue/external waits; it never limits global Monty lifetime.
Shared memory defaults to an adaptive budget based on available RAM, memory
pressure and reserve, with an optional operator cap. Unsafe manual reductions
are rejected; automatic reductions below the live heap remain pending while
safe reclamation and admission backpressure apply. All valid settings changes
are live, with desired/effective state visible.

Token budgets default to disabled (`token_budgets_enabled = false`). When
disabled, retrieval, prior knowledge, history and task consumption have no
artificial token caps, including hardcoded retrieval/assembly constants.
Token accounting remains observability; model context/output limits remain
technical constraints. Time, allocation and memory limits are independent.

**Implementation status (2026-10-06):** this section specifies the binding target,
not completed functionality. Shared verified component boot, exact accepted-input
lookup, admission-pending recovery, attempt-addressed cancellation, parent/child
snapshot links and native PostgreSQL fixtures provide prerequisites. The instance
policy authorizer and prepared-dispatch recheck are initial infrastructure; they
do not establish a complete production/global-settings cutover. Global Monty,
live task/adaptive memory budgets, intent CRUD/preview and removal of legacy
operator scopes/operation approvals still require implementation and acceptance.
See `docs/plans/simplified-v3-implementation.md`; never mark the full plan complete
or claim improved speed without the production-path tests and measurements.

## Orchestrator-First, LLM-Minimal Design (Mandatory)

**The orchestrator IS the execution engine. Rust makes tools available. The LLM
is consulted ONLY when a task requires creative reasoning, composition, or
irreversible decisions the Recipe must confirm. Deterministic usages are Tier 0
when eligible; shell and spawn_subagent Recipes are always Tier 1.**

This principle governs all Recipe, Skill, PythonCode, and ToolSkill authoring.

### Global Monty lifecycle (binding target architecture)

**Upgrade prerequisite:** Follow `simplified_v3.md` Phase 3a’s Monty 1.0 gate
before global production wiring. The v0.0.16 custom-tracker proof is test-only
and cannot implement the new API. Monty 1.0 removes the allocation-count limit;
preserve existing settings until their explicit migration rather than silently
ignoring them. Rust and Monty must share the effective duration revision and
one task compute account; a persisted WebUI edit alone is not runtime uptake.


**Exactly one global Monty orchestrator starts during system startup and stays
alive in the background for the lifetime of the BrassClaw instance.** It starts
after migrations, component seeding and integrity verification, before turn
workers, trigger producers and ingress are enabled. Readiness requires a live
orchestrator waiting for work; merely constructing a driver or seeding Python
code does not satisfy startup.

Chat messages and other admitted inputs are work items delivered to this
already-running orchestrator. A turn is a bounded task, not a new global VM or
OS process. Completing or cancelling a turn must not terminate Monty. While
idle it awaits work without polling or consuming LLM tokens. Only instance
shutdown or a supervised fatal-runtime recovery replaces the global VM.

Conversation history, task state, replies, signals, tool bindings and execution
authority remain associated with explicit conversation/run/message IDs. Global
orchestration never means one shared chat history or a shared approval grant.
Waiting for approval, auth or a child run must leave the orchestrator able to
process the events needed to resume that task. Rust owns transport, VM hosting,
durable admission and kernel enforcement; Python/Recipes own task sequencing.

**Implementation gap:** current `PersistentMontyDriver` creates a VM lazily per
`TurnScope` in `MontySessionRegistry`. This is existing code, not the target
architecture. Follow `simplified_v3.md` Phase 3a for the cutover. This lifecycle
contract supersedes older per-input/per-conversation lifecycle descriptions in
crate docs and plans; it does not override kernel or Recipe authoring rules.

### Turn Execution Flow — binding target

Every user input travels one of two paths within the already-running global
orchestrator. **Understand this before authoring anything.** The lifecycle
cutover is specified in `simplified_v3.md` Phase 3a.

```
User Input
    │
    ▼
Global Orchestrator (Monty) — already running since system startup
    │
    ▼
Intent-Matching System  (resolve_intent / fetch_for_turn)
    │
    ├─── MATCH ──────────────────────────────────────────────────────────────────┐
    │                                                                            │
    │  Composition system fetches the matched Recipe                            │
    │      │                                                                     │
    │      ▼                                                                     │
    │  IBS (Instruction-Building-System)                                         │
    │  step_link + StepDescriptions JSONB  →  build_instruction()               │
    │  →  BuildInstruction { rust_steps, orchestrator_steps }                   │
    │      │                                                                     │
    │      ▼                                                                     │
    │  Orchestrator executes steps in order:                                     │
    │    channel:"rust"         → bind ToolSkill into Monty namespace            │
    │    channel:"orchestrator" → PythonCode: result = host.<tool>(...)          │
    │                              ↳ Rust Tool executes, returns result          │
    │    channel:"orchestrator" → (Tier 1 only) LLM step with recipe context     │
    │    channel:"orchestrator" → host.post_reply(...) → user                    │
    │                                                                            │
    │  History saved → task completes; global Monty awaits further work.         │
    │                                                                            │
    └─── NO MATCH ───────────────────────────────────────────────────────────────┘
         │
         ▼
     Orchestrator assembles LLM prompt (Tier 2 / Non-Matching-Mode):
         1. User's question
         2. Conversation history for this turn
         3. base-prompt prefix  (added by Kohai — precompiled from the entire
            component library: tools, recipes, skills, descriptions; ~250k–1M tokens)
         │
         ▼
     LLM answers over the assembled prompt → posted to user
         │
         ▼
     Sempai intercepts the completed turn →
     proposes new Recipe + intent examples →
     Q1 (auto audit) → Q2 (human approval) →
     next match uses approved Recipe (Tier 0 if deterministic; else Tier 1)
```

**What this means for authoring:**
- **Match path = Recipe.** Every capability on the Match path lives in a Recipe. To add behaviour, add a Recipe — not Rust.
- **IBS is the compiler.** It reads the Recipe's `step_descriptions` JSONB at match time and produces the `BuildInstruction`. Ephemeral — never stored. You never call IBS directly; you author correct `step_descriptions`.
- **Two-step tool invocation — always in this order:** `channel:"rust"` binds the ToolSkill (makes `host.<tool>` available), then `channel:"orchestrator"` PythonCode calls it (`result = host.<tool>(...)`). One does nothing without the other.
- **No-Match path = base-prompt.** The base-prompt is compiled from the component library by Kohai. It is not hardcoded. Adding components grows what the LLM knows in Non-Matching-Mode.
- **Sempai closes the loop.** Successful Tier-2 patterns may become approved Recipes after Q1+Q2. Only deterministic Recipes qualify for Tier 0; explicit LLM work remains Tier 1. The library grows with use.

### Component Roles — What Each Type IS

| Component | What it is | What it is NOT |
|-----------|-----------|----------------|
| **Tool** (class 0) | Registered Rust primitive; one declared operation returns a result, classified failure or explicit wait/handle through the host boundary. | A Recipe workflow planner or autonomous agent loop |
| **ToolSkill** (class 13) | Rust-side binding descriptor for IBS: which Tool to bind, param schema, preconditions, error policy. Lives in `reborn_tool_skills`. | Orchestrator instructions; it carries no prose for the Orchestrator, only metadata for IBS |
| **PythonCode** (class 22) | A Python code snippet stored in the component library. The Orchestrator runs it as a step. A tool-calling snippet normally calls `host.<tool>(...)` once; pure logic makes zero calls. Only the documented dependent-chain exception permits additional calls. | An executor — the Orchestrator is the executor; the PythonCode snippet is what gets run |
| **Skill** (class 1–3) | One Orchestrator tool-usage pattern: prose instructions **plus associated executable PythonCode** (class 22). | A multi-tool domain overview (belongs to an Extension), or a Rust binding descriptor |
| **Recipe** (class 21) | A turn template — like RNA. Carries `step_descriptions` JSONB listing component UUIDs in sequence. IBS reads it to assemble the `BuildInstruction` the Orchestrator executes. | A program; the Recipe is a template, IBS assembles the runnable structure from it |
| **ExtensionCatalogue** (class 23) | Domain overview: `task_groups[]` pointing to recipe names | A re-documentation of individual components it owns |

**ToolSkill vs Skill — do not conflate:**
- **ToolSkill** (class 13): consumed by **IBS** to prepare a Rust Tool binding. Referenced in Rust binding steps. Stored descriptive metadata is not Orchestrator Skill prose or executable code.
- **Skill** (class 1–3): Orchestrator instructions plus associated PythonCode. Prose supplies explicit Tier-1 LLM context; the PythonCode supplies deterministic execution. No Rust binding metadata.

**Recipe Tool operations are invoked by the Orchestrator.** A Rust-channel
binding prepares availability; the matching PythonCode calls `host.<tool>(...)`.
Rust infrastructure still hosts the VM, transports work and enforces the kernel;
it must not introduce a second workflow executor. An unpaired binding is a
required authoring/Q1 rejection; verify actual enforcement rather than assuming
a current array schema proves it.

### The Two-Channel Execution Model

```
channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute — availability only)
channel: "orchestrator"   → PythonCode calls host.<tool>(...) to ACTUALLY run the tool
```

Every `rust` step MUST be immediately followed by a matching `channel: "orchestrator"`
PythonCode step. Pure-logic PythonCode makes zero Tool calls; Tool-calling
PythonCode normally makes one. Only the direct dependent-chain exception below
permits multiple calls in one body; independent calls require separate steps.
The orchestrator **never** calls Rust directly — it always goes through `host.<tool>(...)`.

### Tier Decision Hierarchy

0. **Rust gate (ask this before anything else):** Does this task require a new system-level capability not provided by any existing Tool? If **no** → author a Recipe that calls existing Tools. Do not write Rust. Only if a genuinely new primitive is needed should you proceed to write a new Rust Tool, and even then it must be accompanied by a full set of components (Recipe + PythonCode snippet + ToolSkill + Leaf Skill) created or reused through supported stores/first-party or extension seeders.
1. **Tier 0 first**: Can the task be done deterministically with known inputs? → Author a Tier-0 Recipe with a PythonCode snippet the Orchestrator will run. This is the default target.
2. **Split by variant**: Each variant has one predictable input layout and workflow, with verified intent examples. Compatible variants may share a Recipe. Split incompatible layouts/operations; do not require an LLM merely to select a known deterministic variant.
3. **Tier 1 only when necessary**: Use an LLM for creative composition, input/choice evaluation requiring LLM judgment, or confirmation required by the Recipe. Deterministic input validation, branching and result handoff remain Tier 0. Shell and spawn_subagent Recipes are always Tier 1.
4. **One leaf skill per approach**: A leaf skill describes exactly one approach to one tool. If a tool has 3 common usage patterns, author 3 leaf skills — not one monolithic skill that bundles them. A skill should never describe multiple tool calls.
5. **10+ intent examples per recipe**: More examples = better routing precision. Cover both command-style inputs and natural language.

### PythonCode input pattern (v3 target; runtime support required)

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports except stdlib, no network.
# Target: IBS binds typed inputs separately from Python source.
# inputs is the required target mapping, not a currently guaranteed VM symbol.
result = host.tool_name(param=inputs["param"])
```

**Current VM symbol (target typed `inputs` support is separate):**

| Symbol | Purpose |
|--------|---------|
| `host.<tool>(...)` | Call a registered host tool/capability as a first-class callable (the rust-channel step binds it into the Monty namespace) |

> **Retired intrinsics (never use):** `__execute_action__`, `__execute_code_step__`,
> `__execute_actions_parallel__`, `__check_budget__`, `__emit_event__` — all retired in v3.
> Any PythonCode body that uses these will fail Q1. Call `host.<name>(...)` directly.
> See `builtin_stuff_v3.md` Step 27 for the full migration record.

**Required:** the body must assign `result = <value>` before returning.
**Forbidden:** `import os`, `import subprocess`, `exec(`, `eval(`, `open(` — scanned at Q1.

**Monty-owned Recipe execution context (binding target architecture):** the
matched Recipe goes into IBS, which produces the `BuildInstruction`; the global
Monty Orchestrator executes its steps and owns their execution context. Values
and intermediate results needed by later steps remain available within that
Recipe execution. PythonCode steps do **not** have to start with a fresh empty
state dict `{}`. Do not discard required state between steps or merge otherwise
separate steps merely to work around the current executor's isolation.

When Monty delegates a step to a child VM or process, Monty manages the handoff
of the required inputs and returned results into the same Recipe execution
context. A separate execution boundary must not break the Recipe's data flow.
IBS compiles the instructions; Rust provides transport, hosting, tools and kernel
enforcement; Monty owns sequencing and intermediate state. Runtime results are
data, not Python source to interpolate into the next step.

**Isolation applies between unrelated task executions, not automatically between
steps of one Recipe.** Keep each execution's context associated with its exact
conversation, run and attempt. A retry or replacement attempt must not inherit
another attempt's state implicitly; any checkpoint restoration is explicit and
validated. Preserve needed state across waits/resumption and release transient
state when the task completes or is cancelled. Child execution does not grant
additional authority, and claim tokens or secrets must not enter model-visible
state. Existing fresh-step execution is an implementation limitation to repair,
not an authoring requirement.

One PythonCode step makes zero Tool calls for pure logic or normally one for a
Tool usage. The direct dependent-chain exception below remains valid. Never
combine independent Tool dispatches into one PythonCode block, including through
internal code includes. A leaf Skill still describes exactly one Tool usage.

### What Forces Tier 1

- Content composition (write_file, apply_patch, user-composed shell commands)
- Ambiguous intent where the LLM must choose between distinct alternatives
- Irreversible operations benefiting from LLM confirmation
- User-supplied strings whose validation requires LLM judgment. Deterministic
  type, format, range, path and recursive-schema checks do not force Tier 1.
- Conditional decisions that require LLM reasoning. Deterministic branching or
  passing step A's runtime result to step B is handled by Monty within the Recipe
  execution context and does not, by itself, force Tier 1.

### Q1 required authoring gates (verify runtime enforcement)

- **One-component rule:** every `type:"component"` Recipe step has exactly one
  `include` UUID. Internal PythonCode includes are allowed separately; resolve
  and pin all nested versions. Reject empty/multi-component step references.
  Current array schemas do not themselves enforce this rule.

- **Rule 1**: Tier-0 `orchestrator_steps` may ONLY contain PythonCode (class 22). The prose part of a Skill is not a Python entry point. Reference its associated class-22 PythonCode for deterministic execution; do not insert the prose row as an executable Tier-0 step.
- **Rule 2**: If `llm_call_required == false` AND `rust_steps` has tool bindings, then `orchestrator_steps` MUST contain ≥1 PythonCode UUID. A rust-only Tier-0 recipe is rejected.
- **Rule 3**: A Tool-calling PythonCode snippet normally makes one `host.<tool>(...)` call; pure logic makes zero. Multiple **independent** dispatches (tool A and tool B are separately useful, their outputs do not flow directly into each other) require separate PythonCode snippets — one per tool call. Only the direct dependent-chain exception below permits additional calls in one body.

  **Exception — dependent sequential chain:** If tool B's input is the direct runtime output of tool A (B literally cannot run without A's result), both calls may share one PythonCode body. This is valid because they execute in a single `host.run_program` context and share local scope. The pair must form a single logical unit (e.g. sweep → store, read → transform). Example:
  ```python
  bundle_parts = host.sweep_validated_components(user_id=inputs["user_id"], project_id=inputs["project_id"])
  result = host.store_prefix_bundle(
      user_id=inputs["user_id"], project_id=inputs["project_id"],
      bundle=bundle_parts["bundle"], generation_ms=bundle_parts["generation_ms"]
  )
  ```
  The `inputs` mapping in this target example requires runtime implementation.
  Two `host.*` calls, one body — valid because `bundle_parts["bundle"]` is the direct input to the store call. Do **not** use this exception to bundle unrelated tool calls for convenience.
- **Rule 4**: A leaf skill should describe exactly one tool usage pattern. Avoid bundling multiple tool calls or approaches into one skill body.
- **§shell-guard**: Any Recipe using `builtin.shell` is `llm_call_required: true`. **Always. No shell command is ever Tier 0**, regardless of whether the command string is fixed or user-supplied. Known-safe commands (e.g. `cargo build`) may be Tier 1 at high confidence, never Tier 0.
- **§spawn_subagent-guard**: Any Recipe referencing `builtin.spawn_subagent` is `llm_call_required: true`. Always.
- **§no-snippet**: Step type `snippet` in `step_descriptions` is rejected. Use `text` (WebUI annotation, no runtime emission) or `component` (loads a component body).
- **§body-scan**: PythonCode bodies are scanned at Q1 for `import os`, `import subprocess`, `exec(`, `eval(`, `open(`, and similar patterns — hard rejection on any match.
- **§channel-isolation**: A ToolSkill UUID must never appear in `orchestrator_steps`. A Skill UUID must never appear in `rust_steps`. Channels must not overlap.

### Recipe Tier Lifecycle — LLM as One-Time Cost

**The LLM is a one-time cost. Recipes are the permanent return.**

Each user-facing operation goes through exactly one of three tiers per turn:

| Tier | Trigger | LLM call | Token cost |
|------|---------|----------|------------|
| **0** | Recipe matched, `llm_call_required: false` | ❌ Never | Zero |
| **1** | Recipe matched, `llm_call_required: true` | ✅ Guided by recipe context | Low |
| **2** | No recipe match (Non-Matching-Mode) | ✅ Full reasoning | Full |

**Tier 2 is the seed.** The first time a user asks something new, no recipe matches. The LLM reasons through it (Tier 2). The **Sempai interceptor** watches every Tier-2 turn, evaluates the outcome, and proposes new Recipes + intent examples. Those proposals enter the **validation queue** (Q1 automated → Q2 human review). Once approved and activated, future matches use the Recipe at its declared, verified tier. A deterministic Tier-0 Recipe makes zero LLM calls; a Tier-1 Recipe retains its explicit LLM steps.

**Pre-seeded extensions skip discovery, not tier requirements.** Trusted
first-party bootstrap may seed validated components through its distinct
integrity-checked system path. Merely setting `source: "system"` is not an
approval bypass. Deterministic seeded Recipes may be Tier 0 from first use;
seeded Recipes requiring LLM steps, shell or spawn_subagent remain Tier 1.
Authored proposals and operator changes retain their required Q1/human-Q2 path.

**The Sempai continues growing the library at runtime.** Novel combinations the author didn't anticipate — e.g. "list tasks filtered by assignee" for a task-management extension — emerge from Tier-2 turns, get proposed by the Sempai, and become active after the required Q1/Q2 and acceptance evidence. Their behavior determines whether they are Tier 0 or Tier 1. The library compounds with use.

**Conceptual Tier-1 workflow — not persisted Recipe JSON:**

```text
Optional explicit reasoning context: one Skill prose UUID
Explicit reasoning/composition: a supported Tier-1 LLM step
Rust binding: one ToolSkill UUID
Immediately following execution: one associated PythonCode UUID
Later usages and final reply: separate steps
```

Do not insert that diagram into `step_descriptions`. Author the actual
`StepDescriptionEntry` structure from [recipe.md, section 7](recipe.md#7-emit-the-actual-persisted-ibs-schema):
entries contain `desc_idx`, `label`, `yaml_source` and `steps`; steps use
`stepnumber`, `knowledge` (`rust` or `orchestrator`), `type` and `include`.
Each component step has exactly one resolved UUID. Variants use a nonempty
`step_link` to select the ordered steps. Human prose/labels do not define
executable logic or transport input values.

The inspected persisted enum accepts `component`, `text` and `snippet`.
Author `component` for references or `text` for annotations; `snippet` is parsed
but rejected by Q1/IBS and must become an approved referenced PythonCode
component. **`llm` is not a supported value of this persisted enum.** A Tier-1
Recipe must use the actual supported reasoning/runner mechanism; neither
`llm_call_required: true` nor a label invents a runnable LLM step. Verify that
path before claiming the Recipe works. Tier 0 contains no prose-driven or LLM
execution; it references the Skill's class-22 code, not its prose row.

**Posting deterministic output without an LLM:** use a separate approved
reply Skill/PythonCode usage and compatible `ts-host-post-reply` binding.
Verify the actual adapter keyword: seed examples use `answer`, while the inspected
Rust handler reads `text`. Do not guess an alias or claim a generic seed body
posts fixed text correctly without validation. Inputs remain typed data;
`builtin.echo` is diagnostic-only, not a user-facing reply operation.

### Extension Authoring Reference

**Read the four ground-truth guides first.** These extension stacks are supplementary worked/historical examples whose schemas, adapters and approval assumptions must be reconciled before reuse:
- `docs/archive/builtin_stuff_v3.md` — historical list with corrected Skill terminology of all built-in v3 capabilities, tool signatures, and ToolSkill/PythonCode templates
- `tomedo_v3.md` — tomedo EMR integration: full reference implementation of an extension component stack
- `docs/plans/zencoder-extension-plan.md` — **best starting point**: Zencoder REST API extension with complete annotated `step_descriptions` JSONB, intent examples, and bootstrap seeding pattern

After the four guides, consult `docs/plans/zencoder-extension-plan.md` when its extension pattern is relevant. It does not override their binding contracts.

## Adding a New Capability (Start Here)

**Work through this hierarchy top-down. Stop at the first level that solves the problem — do not skip ahead.**

| Level | What to do | What gets created |
|-------|-----------|-------------------|
| **1** | Add intent examples to an existing `RecipeVariant` | Rows in `reborn_intent_inputs` only — zero new components |
| **2** | Add a new `RecipeVariant` to an existing `Recipe` | One variant + intent examples — reuses existing PythonCode snippets and ToolSkills by UUID |
| **3** | Reference an existing PythonCode snippet in the new variant's `step_descriptions` | No new PythonCode row — slot the existing UUID into `channel:"orchestrator"` step |
| **4** | Reference an existing ToolSkill in the new variant's `channel:"rust"` step | No new ToolSkill row — slot the existing UUID |
| **5** | Author new component rows in `builtin_bootstrap.rs` or extension seeder | Only missing Postgres components; reuse compatible identities. No new primitive, subject to supported store/runner contracts |
| **6** | Write a new Rust Tool, then do level 5 | Only when no existing Tool provides the primitive needed |

**Before doing anything:** search `docs/archive/builtin_stuff_v3.md` and the existing component library for existing PythonCode snippets, ToolSkills, and Leaf Skills that already cover what you need. Reuse by UUID reference first.

**If you reach level 5, author components in this order** (Recipe-first — define what you want, then fill in what it needs):

1. **Recipe** (class 21) — define the `RecipeVariant`: intent examples, `step_descriptions` JSONB with placeholders for component UUIDs, `variable_patterns` if needed
2. **PythonCode snippet** (class 22) — the code snippet the Orchestrator will run at the relevant step; normally calls `host.<tool>(param=value)` once per Tool-calling snippet, with the documented dependent-chain exception; pure logic makes zero calls; assign `result = ...`
3. **ToolSkill** (class 13) — binding descriptor for IBS: which Tool to bind, param schema, preconditions, error policy. Referenced by the `channel:"rust"` step in the Recipe's `step_descriptions`
4. **Skill** (classes 1–3; also called Leaf Skill) — complete the tool-usage unit with prose and an explicit UUID/revision association to the PythonCode from step 2. The prose guides Tier-1 reasoning; the associated PythonCode executes the usage, including at Tier 0
5. **ExtensionCatalogue** (class 23) — update `task_groups[]` if this belongs to an existing domain

Fill in the UUID references in the Recipe's `step_descriptions` as you create each component.

**Verify:** intent resolves to the intended variant; required Q1/human Q2 and
exact-combination approval evidence exist; the actual runner passes relevant
acceptance cases at the declared tier. Verify zero LLM calls for Tier 0; verify
the explicitly required reasoning steps for Tier 1. A confidence result or Q1
pass alone does not establish activation or behavior.

## Where to Work

| Area | Location |
|------|----------|
| **New user-facing capability (first stop)** | Design the Recipe and reuse existing Skill/PythonCode/ToolSkill UUIDs; create only missing components through supported stores/seeders. New Rust primitives require a genuinely missing system operation; schema/runner gaps require their own implementation. |
| brassclaw CLI binary | `crates/brassclaw_reborn_cli/` |
| Reborn runtime and driver registry | `crates/brassclaw_reborn/` |
| Composition and wiring | `crates/brassclaw_reborn_composition/` |
| Config resolution and profiles | `crates/brassclaw_reborn_config/` |
| Agent loop driver | `crates/brassclaw_agent_loop/` |
| LLM providers and routing | `crates/brassclaw_llm/` |
| Skills system | `crates/brassclaw_skills/` — v3 DB-backed store (`db-store` feature), validation helpers (always compiled). **Do not** consume the `v1-types` or `v2-compat` feature gates from new code — they are migration-importer and legacy bridge paths only. New first-party skills: `PgSkillStore::insert(NewPgSkill { ... })` in `builtin_bootstrap.rs`. |
| Security, safety, prompt injection | `crates/brassclaw_safety/` |
| WebUI v2 server (React SPA) | `crates/brassclaw_webui_v2/`, `crates/brassclaw_webui_v2_static/` |
| WebUI ingress / gateway adapter | `crates/brassclaw_reborn_webui_ingress/` |
| Extensions lifecycle | `crates/brassclaw_extensions/` |
| Host runtime shell access | `crates/brassclaw_host_runtime/` (in-kernel capability host + runtime dispatcher; sandboxed subprocess execution via `services/process_executor` and `sandbox_process/`; first-party tools under `first_party_tools/`) |
| Embeddings | `crates/brassclaw_embeddings/` |
| Recipe-Skill-Tool library | `crates/brassclaw_engine/src/memory/` (types, matcher, validator, similarity), `crates/brassclaw_reborn_composition/src/recipe_store.rs` + `recipe_library.rs` (REST store + loop adapter), `crates/brassclaw_turns/src/run_profile/recipe_lookup.rs` (trait). Recipes use `RecipeVariant` + `step_link` + `StepDescriptions` JSONB + optional `variable_patterns` — read recipe.md/skills.md/tools.md/toolskills.md first; saved_plan_to_v3.md is historical context, not the component authoring ground truth. |
| IBS (Instruction-Building-System) | `crates/brassclaw_engine/src/memory/ibs.rs` + `crates/brassclaw_engine/src/types/ibs.rs` (`build_instruction`, `BuildInstruction`, `IbsRecipeStep`, `ToolBinding`, `ErrorPolicy`). Compiles `step_link` + `StepDescriptions` → `BuildInstruction` at intent-match time. **Never stored** — ephemeral per call, memoised in-process. |
| Component catalog (class codes 4–23) | `crates/brassclaw_engine/src/memory/retrieval_source.rs` (`PostgresSource`, `fetch_for_turn`, `FetchForTurnResult::SplitResult`/`ActionShortCircuit`, `class_code_to_table` — the single source of truth for class→table dispatch; the full class→table table is in `CLAUDE.md` §Component Catalog and is regression-tested against the code). Tables: `reborn_extensions_unified` (4–9, extension packages) + `reborn_specs/tool_skills/plans/summaries` (12–15) + `reborn_actions` (**16**, not 11 — class 11 is unallocated) + `reborn_docus` (17) + `reborn_lessons/issues/notes` (18–20) + `reborn_recipes` (21) + `reborn_python_code` (22, Phase B) + `reborn_extension_catalogues` (23, Phase C). Classes 10 (Orchestrator) and 50 (Scaffold) are **not** separate tables — they live in `reborn_skills`, filtered by `class_code`, alongside classes 1–3. All components carry `dependency_registry JSONB` (Phase J). `reborn_component_catalog` (`crates/brassclaw_pg/migrations/V084__reborn_component_catalog_view.sql`) is a read-only Postgres VIEW — not a table — that `UNION ALL`s all 14 prompt-bearing class tables (excluding `reborn_tools`, class 0) for ad hoc/Settings-API querying; it applies no per-request scope filtering, callers add their own `WHERE`. |
| Settings API / WebUI catalog tabs | `crates/brassclaw_reborn_composition/src/pg_settings_listing.rs` (`PgSettingsListingService::list`, single parameterised query backing every `GET /api/settings/{type}` tab — Skills, Tool Permissions, Actions, Extensions, Orchestrators, Scaffolds, Recipes, ToolSkills, PythonCode, ExtensionCatalogues; a genuinely missing table fails loud with `SettingsListingError::MissingTable`, never silently empty), `crates/brassclaw_product_workflow/src/settings.rs` + `reborn_services.rs` (`RebornServicesApi` trait methods), `crates/brassclaw_webui_v2/src/{descriptors,handlers,router}.rs` (routes). Frontend: `crates/brassclaw_webui_v2_static/.../settings-schema.js` + `settings-tabs.js` (sidebar sections: Runtime Config / Component Catalog / Security & Governance / Access & Ops) + per-tab `*-tab.js` files. The runtime tool-permission list ("Tool Permissions") is UI-distinct from the class-code Skill/ToolSkill catalog tabs — do not conflate them. Note: the old v1 SKILL.md plugin installer ("Skill Packages") UI was removed; the Skills tab now shows `reborn_skills` DB rows only. |
| Validation queue | `reborn_validation_queue` table (V051, Phase A.5). Four-state pipeline: **Q1** `auto` (orchestrated, sandboxed LLM audit) → **Q2** `manual` (operator review — human-only, never automated) → **Q3** `revision` (automated revision by class-09 extension, if flagged) → **Q4** `rejection` (rejected; retained for `q4_retention_days` then wiped). Authored versions require Q1+human Q2. Current system-source inserts set validated directly, but only trusted bootstrap/integrity provenance qualifies for the distinct system_seed approval contract; labels are not evidence or a bypass. **Recovery from Q4 rejection:** read the Q1 audit output, fix the component (check for forbidden symbols, wrong class codes, missing `result =`, channel isolation violations), and re-submit. Do **not** rewrite the capability as Rust because a recipe was rejected — fix the recipe. |
| Builtin bootstrap seeder | `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` (Phase L). Seeds full v3 component stack (Tools + ToolSkills + Skills + PythonCode + Recipes + ExtensionCatalogues) for all 23 first-party tools at boot, if not already present. Idempotent. Orchestrator and system prompt components (`orchestrator:main`, `codeact_preamble`, `codeact_postamble`, `failure_explanation`, `compaction_summarizer_fresh`, `sempai_audit`, `subagent:direction:*`) are seeded here and carry `content_checksum` (SHA-256 hex) for boot-time integrity verification. |
| Seeding boot chain | `crates/brassclaw_reborn_composition/src/component_boot.rs` — shared `seed_*`, queue recovery, integrity checks and prompt initialization, called by `runtime.rs` before workers/trigger producers. Required seeding failures abort boot. WebUI construction no longer supplies runtime prerequisites. Global VM startup is still an implementation gap. |
| Global Orchestrator lifecycle (target) | `simplified_v3.md` Phase 3a: move the shared boot prerequisites out of WebUI-only construction, start one global Monty before workers/ingress, route work through its bounded inbox and retain task-local context. Current transition sites: composition `runtime.rs`, `persistent_monty_driver.rs`, `session_registry.rs`; engine `executor/orchestrator.rs`, `orchestrator/basic_mode.py`; Reborn `turn_runner.rs`. |
| Content integrity check | `crates/brassclaw_reborn_composition/src/content_integrity.rs` (`run_content_integrity_check`) — SHA-256 prose checksum verification for `source='system'` rows. Called at boot after seeding. Hard error on mismatch. Distinct from `boot_integrity.rs` (Phase N queue-consistency check). |
| `BootedDb` newtype | `crates/brassclaw_reborn_composition/src/booted_db.rs` — type-level proof that migrations completed. All seeding and integrity functions accept `&BootedDb`, not raw `Arc<PgPool>`. |
| `brassclaw repair` command | `crates/brassclaw_reborn_cli/src/commands/repair.rs` — force-reseeds all `source='system'` rows using `ON CONFLICT DO UPDATE`. Current mutable-row repair, not v3 revision activation: reconcile affected tasks before mutation; retain artifacts required by continuations. |
| `OrchestratorCodePort` | `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs` — engine-side port for loading the class-10 orchestrator body. Impl: `PgOrchestratorCodePort` in `crates/brassclaw_reborn_composition/src/pg_orchestrator_code_port.rs` (gated `postgres+skills-db`). Driver construction wiring in `runtime.rs`. |
| BasicPromptStore / prefix | `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs` (Phase K.1). Stores the operator-editable base-prompt prefix; regenerated via `regenerate_prefix`. |
| Intent system | `crates/brassclaw_engine/src/memory/intent_system.rs` (`resolve_intent`, 4-class classifier, `record_disambiguation_choice`), `reborn_intent_inputs` table (V028 + V058 variable-template columns). Intent expressions support `%` slot markers for variable capture (Phase M). |
| Monty VM settings | `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs` (`PgMontyVmSettingsStore`, reads/writes `reborn_monty_vm_settings` V034 migration) |
| User chat preferences | `crates/brassclaw_reborn_composition/src/pg_user_preference_store.rs` (`PgUserPreferenceStore`, `reborn_user_preferences` V035 migration) |
| Legacy MemoryDoc migration (v1→v3, read-only concern) | `crates/brassclaw_reborn_composition/src/component_import.rs` (`run_component_import` — migrates old v1 `brassclaw_memory_docs` rows into class-specific tables at first boot on old databases). **Do not add new components via `brassclaw_memory_docs`** — author them directly in `builtin_bootstrap.rs` or the extension seeder. |
| Interceptor / Sempai-Kohai | `crates/brassclaw_interceptor/` (Sempai/Kohai review loop, persona, base-prompt assembly, `SempaiProposalSink`, `SempaiReviewOutcome`). Sempai auto-creates **all** component types (not just recipes) — proposals enter the validation queue at `'pending'`. Wired in composition via `InterceptorConfigService`. |

When a task touches only `crates/` there is no longer a v1 `src/` tree — all v1 code was removed in Phase 6.

## Subagent and Loop Rules

In v3, a subagent's behaviour is defined by the **Recipe it receives** — not by configuring loop driver code. The Orchestrator running the subagent executes the Recipe's steps. To change what a subagent does, change its Recipe; do not modify loop driver or executor Rust code.

- Subagent spawn creates and wires child runs only. It must not implement a second agent loop.
- Child planning, execution, capability calls, checkpointing, gates, retries, and completion must go through the existing loop runner/driver/executor path.
- Host-trusted trigger ingress is sealed by trigger-worker-owned request minting plus private conversation-owned trusted inbound construction.
- Product adapters, product workflow, first-party capabilities, and host-runtime handlers must use untrusted inbound requests and must not mint `TrustedInboundTurnRequest` or call trusted trigger submitter factories.

## Repo-Wide Coding Rules

- No `.unwrap()` or `.expect()` in production code. They are acceptable in tests and for truly infallible invariants (e.g., compiled-in literals, regexes) with a safety comment.
- Keep clippy clean with zero warnings: `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.
- Prefer `crate::` for cross-module imports. `super::` is fine in tests and intra-module refs.
- Use strong types and enums over stringly-typed control flow when the shape is known.
- Use `thiserror` for error types in `error.rs`. Map errors with context: `.map_err(|e| SomeError::Variant { reason: e.to_string() })?`.
- No `pub use` re-exports unless exposing to downstream consumers.
- Comments for non-obvious logic only.
- **Do not introduce new `include_str!()` constants for behavioural prompts or scripts in production code paths.** All prompt bodies are seeded as `source='system'` DB rows via `builtin_bootstrap.rs` and loaded at boot via `OnceLock` accessors (see seeding boot chain in `component_boot.rs`). `include_str!()` is only permitted in `builtin_bootstrap.rs` seed constants and in `#[cfg(test)]` modules. The `.md` and `.py` source files in `crates/brassclaw_engine/prompts/`, `crates/brassclaw_loop_support/prompts/`, and `crates/brassclaw_reborn/src/subagent/directions/` remain on disk as seed sources and test references — never as compiled-in runtime fallbacks.
- `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Use `debug!` for internal diagnostics. Background tasks must never use `info!`.

## Database Rules

- All persistence uses Postgres. In-memory backends are acceptable for unit tests only.
- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers; do not collapse them.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.

## Security Invariants

- Review any change touching listeners, routes, auth, secrets, sandboxing, approvals, or outbound HTTP with a security mindset.
- Do not weaken bearer-token auth, webhook auth, CORS/origin checks, body limits, rate limits, allowlists, or secret-handling guarantees.
- Treat Docker containers and external services as untrusted.
- Session, thread, and turn state matters. Submission parsing happens before normal chat handling.
- Skills are selected deterministically. Component Q2 and external authentication
  have explicit paths; keep secrets and claim tokens out of ordinary chat state.
  Tool dispatch uses current global policy, not per-operation approval grants.
- Persistent memory is the workspace system, not just transcript storage.

### Capability Lease Authority Invariants (`brassclaw_authorization`)

The lease implementation below is legacy until the coordinated simplified-v3
cutover. Preserve its existing technical integrity while it remains in use;
do not add these invocation grants as v3 requirements. In final v3, claims and
attempt identifiers fence execution, while current global policy governs Tools.

Capability leases are authority-bearing records — the rules below are not style preferences:

1. **`PgCapabilityLeaseStore` mutations must be atomic.** Every `revoke`, `claim`, and `consume` must run inside a single Postgres transaction with `SELECT … FOR UPDATE`. Never split the read and write across separate connections or pool checkouts. A TOCTOU gap here is a double-authority bug.
2. **`UPDATE` predicates must include `user_id`.** Reading is scoped to `(id, tenant_id, user_id)`; writing must use the same triple. Omitting `user_id` from the `WHERE` clause allows mutations to cross user boundaries.
3. **`consume` must call `ensure_consumable` and decrement `max_invocations`.** Directly setting `Consumed` without these steps grants additional invocations on multi-use leases and bypasses the unclaimed-fingerprint guard.
4. **`FilesystemCapabilityLeaseStore` must not silently downgrade `CasExpectation::Version` to `Any`.** The indexed-projection fallback (stripping `entry.indexed` for byte-only backends) is acceptable. Downgrading the CAS version expectation is not — it removes the cross-process ordering guarantee. If the backend cannot provide versioned CAS, fail closed.
5. **`LocalFilesystem` is not an accepted backend for authority-bearing stores.** Production wires `InMemoryBackend` (local-dev, under `/tenants`) and `PostgresRootFilesystem` (hosted). Tests that exercise mutation paths must use `InMemoryBackend`, not `LocalFilesystem`.
6. **`issue` must not return success when zero rows were inserted.** `ON CONFLICT DO NOTHING` silently absorbs duplicate-key conflicts; check `rows_affected == 1` before returning the lease to the caller.

## Testing Rules

- Add the narrowest tests that validate the change: unit tests for local logic, integration tests for runtime/DB/routing behavior, E2E or trace coverage for gateway, approvals, extensions, or other user-visible flows.
- Test through the caller, not just the helper. When a predicate/classifier/transform helper gates a side effect (HTTP, DB write, OAuth flow, UI mutation, tool execution) and has any wrapper or computed input between it and that side effect, a unit test on the helper alone is not sufficient regression coverage. Add a test that drives the actual call site at the integration tier or higher.
- Mocks of multi-arg runtime APIs must capture every argument the production caller passes.

## Key Environment Variables

**Bootstrap tier** (fixed set, read before the DB starts — set in the systemd unit's `Environment=` block):

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | Reborn state root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | Per-invocation capability policy: `local_dev` (default), `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`. Controls the security resolver only; does **not** affect which storage backend is used (Postgres is always used). Setting `BRASSCLAW_REBORN_PROFILE` (old composition-profile name) is a hard startup error. |
| `BRASSCLAW_REBORN_LOG` | Log filter for Reborn runtime (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | External Postgres URL. Optional for single-host local deployments (embedded Postgres is used when absent). Required for all non-local `BRASSCLAW_RUNTIME_PROFILE` values. |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded Postgres port (default: 5434) |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | Override embedded Postgres listen addresses (default: `127.0.0.1`). Set to `0.0.0.0` for LAN access. First-boot only (written to `postgresql.conf` by `initdb`). |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Path to master-key file; set only when using passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after the DB is up — set in `secrets.env` via `EnvironmentFile=`):

The *names* of these env vars are stored in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted. Includes: `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, trigger auth tokens.

## Build and Test

> **Mandatory:** Every `cargo build`/`test`/`clippy`/`check` **must** set
> `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` (NVMe) — never build in-place on the
> slow external repo drive. **Before** compiling, check free space on that volume and clean
> it if it is too full:
>
> ```bash
> df -h /Users/ollama/brassclaw-target          # check before every compile
> # If Avail < 15 GB or Capacity > 90%, clean first:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clean
> # Then run the actual command with the target dir set:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo <build|test|clippy|check> ...
> ```
>
> The NVMe target dir accumulates multi-GB artifacts and can fill the 228 GB volume
> mid-build, starving/corrupting the run — the space check + clean is mandatory, not optional.

```bash
# Build the Reborn binary for release/performance work only
cargo build --release --bin brassclaw 

# Format
cargo fmt

# Lint affected packages with relevant features; fix newly introduced warnings
cargo clippy -p <crate_name> --all-targets

# Full lint for CI/release or changes requiring broad acceptance
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# Unit tests for a specific crate
cargo test -p <crate_name>

# Root-package tests when root behavior is affected
cargo test

# Database semantics checks (requires PostgreSQL; narrow by package/test)
cargo test --features integration
```

## Before Finishing

- Confirm whether behavior changes require updates to specs, API docs, or `CHANGELOG.md`.
- Reuse passing relevant evidence; run targeted checks only for new changes or
  unresolved questions. Stop once the reviewed diff and required checks pass.
- Fix introduced warnings and report unrelated baseline warnings separately.
- Re-check security-sensitive paths when touching auth, secrets, network listeners, sandboxing, or approvals.
- Keep the final diff scoped to the task. Avoid unrelated file churn.
- **Capability check:** If behaviour was added or changed — is it expressed as a Recipe + PythonCode, or did it end up as Rust logic that belongs in a Recipe? Rust-only behaviour changes are incomplete unless a genuinely new system primitive was required.
- **Component set check:** If a new Rust Tool was written — do its Recipe + PythonCode snippet + ToolSkill + Leaf Skill + ≥10 intent examples all exist through supported stores/first-party or extension seeders? Are compatible registration, exact-combination approval and behavioral evidence present? A descriptor or seed row alone does not establish executable availability.
