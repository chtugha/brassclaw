# BrassClaw Development Guide

## Binding Recipe architecture (v3)

Read both ground-truth authoring guides before creating or changing components:

- [recipe.md](recipe.md): ordered workflows, variants, actual persisted IBS
  schema, input bindings, component assembly and task version selection.
- [skills.md](skills.md): one Tool usage with prose plus executable PythonCode,
  recursive contracts, exact-version association approval and retry rules.

For Skill/Recipe authoring, those two guides take precedence over summaries,
archive examples and older subsystem instructions. The rules below describe
binding v3 targets; they do not establish completed runtime/store enforcement.

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
2. **Keep data separate from code.** Declare recursive input/result schemas,
   including list items, object fields, allowed extra values, nullability,
   defaults and numeric bounds. Missing and null are different. Defaults apply
   only to missing consumer inputs, never to invalid/null values or bad outputs.
   Recipe references bind typed data to `inputs["local_name"]`; they never paste
   runtime values into Python source. This interface still needs runner support.
3. **Review meaning before activation.** The author, supported Q1 audits,
   behavioral validation and human Q2 establish that prose and code agree.
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
   merely because a replacement exists. These are not Tool invocation grants.
5. **Pin the complete workflow.** IBS selects one consistent approved catalogue
   snapshot. Retain the Recipe revision, variant, exact `step_link`, selected
   steps/order, input layout, all component/dependency revisions/checksums and
   association approval identifiers with the task before execution. Matching and
   assembly must use the same generation. Resume/child execution/retry retains
   that selection; do not match again or read latest midway. Keep old artifacts
   while tasks/checkpoints need them. BuildInstruction stays ephemeral; retain
   task snapshot references through the continuation contract.
6. **Make failure and retry exact.** Use skills.md's failure contract. Attempt
   counts are positive integers including the initial dispatch and survive
   waits/reclaims. Retries require explicit eligible outcomes and verified
   read-only or durable deduplication evidence. A timeout does not prove no
   effect occurred. Never replay a completed effect because output validation,
   another step or reply posting failed. Cancellation, stale attempts, live Tool
   policy and resource limits remain effective before every retry.
7. **Report support honestly.** A Markdown design is not an activated component.
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

**Execution and validation:** deterministic Tier-0 execution uses the associated
PythonCode without an LLM interpreting prose. Tier 1 can use the prose in its
explicit LLM steps. Prose must never be executed as Python. ToolSkill UUIDs stay
in `channel:"rust"`; executable PythonCode UUIDs stay in
`channel:"orchestrator"`. Q1/Q2, isolation between unrelated tasks and the existing
dependent-chain exception remain applicable. Monty owns intermediate state within
each Recipe execution, including handoffs to child execution. This documentation change does not implement a
new database schema, association editor or runtime path.


> **Local test/model connections:** Before remote tests or provider setup, read
> [LOCAL_TEST_ENV.md](LOCAL_TEST_ENV.md) if present. It contains the operator's
> SSH aliases, host roles, inference endpoint and verified prerequisites. The
> file is Git-ignored; its absence on another checkout is not an empty config.

> **Primary Rule — read this first:**
> Start a new user-facing capability with a **Recipe design** and reuse existing
> PythonCode, ToolSkills and Skills by UUID. Create only the missing components
> through supported stores/seeders. A reusable Skill may also be created independently
> to grow the library, following skills.md. Adding behavior usually requires no new Rust.
> New Rust is only warranted when a genuinely new
> system-level Tool is needed that no existing Tool provides. The component library is where
> almost all behaviour lives — more Recipes, fewer Rust branches.

Read [Development reasoning and validation policy](docs/development-policy.md)
before implementation. It governs iteration, check selection, evidence reuse
and stopping; older blanket test/lint instructions defer to it. Use LLM reasoning
to diagnose and complete a coherent change before compiling. Runtime Tier-0
rules do not limit development reasoning.

1. Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

State your assumptions explicitly. If uncertain, ask.
If multiple interpretations exist, present them - don't pick silently.
If a simpler approach exists, say so. Push back when warranted.
If something is unclear, stop. Name what's confusing. Ask.
2. Simplicity First

Simplicity does not mean reducing Recipe steps. Prefer many purposeful reusable
PythonCode components and explicit result handoffs over a specialized Rust Tool
or a monolithic task-specific Python body. One referenced component per step
permits internal composition under the binding Recipe contract.


Minimum code that solves the problem. Nothing speculative.

No features beyond what was asked.
No abstractions for single-use code.
No "flexibility" or "configurability" that wasn't requested.
No error handling for impossible scenarios.
If you write 200 lines and it could be 50, rewrite it.
Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.
For adding a user-facing capability: the minimum code is often **zero lines of Rust** — a Recipe + PythonCode achieves the same result without touching any crate. Ask "is new Rust actually required here?" before opening any file in `crates/`.

3. Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

Don't "improve" adjacent code, comments, or formatting.
Don't refactor things that aren't broken.
Match existing style, even if you'd do it differently.
If you notice unrelated dead code, mention it - don't delete it.
When your changes create orphans:

Remove imports/variables/functions that YOUR changes made unused.
Don't remove pre-existing dead code unless asked.
The test: Every changed line should trace directly to the user's request.

4. Goal-Driven Execution

Define success criteria and the smallest checks that establish them. Stop when
the reviewed change and required checks satisfy those criteria. Repeat checks
only when relevant edits, failures or unresolved uncertainty justify them.

Transform tasks into verifiable goals:

"Add validation" → "Trace the caller and invalid-input contract; add focused coverage"
"Fix the bug" → "Diagnose the cause from source and logs; verify the regression at its caller"
"Refactor X" → "Review affected contracts and reuse valid baseline evidence; verify the final change"
"Add capability X" → "Design the Recipe; reuse or create the required Skill/code/bindings and ≥10 intent examples; obtain required Q1/human Q2 and exact-combination approval; verify intended variant and declared tier through the actual runner (zero LLM calls for Tier 0, explicit LLM steps for Tier 1)"
For multi-step tasks, state a brief plan:

1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
Strong success criteria give a clear stopping point. Weak criteria ("make it work")
encourage repeated checks without establishing additional evidence.


**BrassClaw** is a secure, local-first AI assistant built on the IronClaw Reborn architecture. It targets 7B-14B LLMs within 8,192-token context windows and is implemented as a workspace of approximately 70 Rust crates.

## Code Style

> These rules apply to Rust code in `crates/`. Most new capabilities do not require new Rust — see §Orchestrator-First (below) and the Primary Rule at the top of this document.

- Prefer `crate::` for cross-module imports; `super::` is fine in tests and intra-module refs
- No `pub use` re-exports unless exposing to downstream consumers
- No `.unwrap()` or `.expect()` in production code (tests are fine)
- Use `thiserror` for error types in `error.rs`
- Map errors with context: `.map_err(|e| SomeError::Variant { reason: e.to_string() })?`
- Prefer strong types over strings (enums, newtypes)
- Keep functions focused, extract helpers when logic is reused
- Comments for non-obvious logic only
- **Do not introduce new `include_str!()` constants for behavioural prompts or scripts in production code paths.** All prompt bodies (`orchestrator:main`, `codeact_preamble`, `codeact_postamble`, `failure_explanation`, `compaction_summarizer_fresh`, `sempai_audit`, `subagent:direction:*`) are seeded as `source='system'` DB rows via `builtin_bootstrap.rs` and loaded at boot via `OnceLock` accessors. `include_str!()` is only permitted in `builtin_bootstrap.rs` seed constants and in `#[cfg(test)]` modules. Single-line format strings are fine inline. This also applies to reference documentation files embedded for runtime use (e.g. `CLAUDE.md`, `AGENTS.md`): if such a file must be included at compile time, its `include_str!()` constant belongs in `builtin_bootstrap.rs` as a `pub(crate) const`, not in any other production file. The backend or handler that uses the constant imports it as `crate::builtin_bootstrap::SOME_CONST`.
- `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Use `debug!` for internal diagnostics (trace analysis, reflection results, engine internals). Reserve `info!` for user-facing status that the REPL intentionally renders. Background tasks must never use `info!`.
- Test through the caller, not just the helper: when a predicate/classifier/transform helper gates a side effect (HTTP, DB write, OAuth, UI mutation, tool execution) and has any wrapper or computed input between it and that side effect, a unit test on the helper alone is not sufficient regression coverage. Add a test that drives the call site at the integration tier or higher. See `.claude/rules/testing.md` for the full rule.

## Tool Usage Guidelines

### Searching the codebase

Use `grep` (content search) and `glob` (file name pattern) tools for codebase searches.
For large output use range-based `read_file` to inspect specific sections without loading
full files. Avoid running raw shell `rg`/`grep` commands when the built-in search tools
cover the need.


## Architecture

**Component authoring ground truth:** [recipe.md](recipe.md) defines workflows
and the actual persisted IBS schema; [skills.md](skills.md) defines Skills,
recursive contracts, association approval and retries. A Skill is one Tool
usage with prose plus explicitly associated PythonCode. A Recipe orders reusable
usages and pure logic, binds typed inputs/results and defines completion. Use
one component UUID per Recipe component step; internal code composition is
allowed. Semantic consistency is reviewed before activation, not inferred by
IBS from prose. IBS verifies exact-combination approval evidence and pins the
Recipe, variant, step_link, input layout and dependency revisions together.
Running/resumed tasks retain that selection. Retry counts include the initial
dispatch; never replay completed effects. Component approval is not Tool
permission: current global policy and technical constraints apply before every
dispatch in every tier. These are target requirements, not runtime acceptance
claims. Historical examples do not override the two authoring guides.

BrassClaw Reborn uses a five-layer model:

1. **Products** — UX surfaces and deployment shapes (CLI, web server, daemon). Products wire together loops, capabilities, and host access. They do not implement agent logic.
2. **Loops** — Agent behavior drivers. A loop manages planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. All agentic execution passes through the loop runner.
3. **Kernel** — Authority and policy enforcement. Trust decisions, secret resolution, safety policy, sandboxing, capability grants, and session identity live here. Kernel boundaries are enforced; product and loop code cannot override them.
4. **Infrastructure** — Shared services: LLM providers, Postgres persistence, embeddings, skills, extensions, and observability. Lives in `crates/`.
5. **Component Library** — Recipes, Skills, ToolSkills, PythonCode snippets, and ExtensionCatalogues stored in Postgres. **This is where most new capabilities are added.** No crate change is required to add a Recipe, PythonCode snippet, or Skill. See `builtin_bootstrap.rs` for first-party seeding.

New infrastructure work belongs in `crates/`. New *capabilities* belong in the Component Library (layer 5) first — only reach for `crates/` when a genuinely new system primitive is needed. The v1 `src/` tree was removed in Phase 6.

### Component Catalog and Class Codes

BrassClaw Reborn stores all reusable knowledge artifacts (specs, plans, lessons, etc.) in unified Postgres tables indexed by integer **class codes**. Each class has a dedicated table — this mapping is the verified source of truth, defined by `class_code_to_table` in `crates/brassclaw_engine/src/memory/retrieval_source.rs` and mirrored by `PgSettingsListingService` (`crates/brassclaw_reborn_composition/src/pg_settings_listing.rs`):

| Class code | Type | Table |
|------------|------|-------|
| 0 | Tool | `reborn_tools` |
| 1 | Skill (skill_rusty consumer label) | `reborn_skills` |
| 2 | Skill (skill_monty consumer label) | `reborn_skills` |
| 3 | Skill (LLM) | `reborn_skills` |
| 4–9 | Extension package (`rusty` 4, `monty` 5, `mcp_server` 6, `mcp_client` 7, `llm` 8, `misc` 9) | `reborn_extensions_unified` |
| 10 | Orchestrator | `reborn_skills` (filtered by `class_code = 10`) |
| 12 | Spec | `reborn_specs` |
| 13 | ToolSkill | `reborn_tool_skills` |
| 14 | Plan | `reborn_plans` |
| 15 | Summary | `reborn_summaries` |
| 16 | Actions | `reborn_actions` |
| 17 | Docu | `reborn_docus` |
| 18 | Lesson | `reborn_lessons` |
| 19 | Issue | `reborn_issues` |
| 20 | Note | `reborn_notes` |
| 21 | Recipe | `reborn_recipes` |
| 22 | PythonCode | `reborn_python_code` |
| 23 | ExtensionCatalogue | `reborn_extension_catalogues` |
| 50 | Scaffold | `reborn_skills` (filtered by `class_code = 50`) |

Classes 10 and 50 are **not** separate tables — Orchestrator and Scaffold rows live in `reborn_skills` alongside classes 1–3, distinguished only by `class_code`. Any caller that queries a nonexistent `reborn_orchestrators`/`reborn_scaffolds` table is buggy.

Class **11 is unallocated** (`class_code_to_table` returns `None`) — Actions are class **16**. The `class_code_to_table_matches_claude_md_table` test in `retrieval_source.rs` parses the table above and fails if it drifts from the code again.

`reborn_component_catalog` (`crates/brassclaw_pg/migrations/V084__reborn_component_catalog_view.sql`) is a read-only Postgres **VIEW** — not a table — that `UNION ALL`s the 14 prompt-bearing class tables above (excluding `reborn_tools`, class 0, which carries no prompt text) into one relation for ad hoc querying. It intentionally does not bake in per-request scope/validation filtering (tenant/user/agent/project scope, `validation_status = 'validated'`, consumer-tag checks) — callers apply their own `WHERE` clause on top, exactly as `PgSettingsListingService::list()` does per-table.

**V085 migration** adds a nullable `content_checksum TEXT` column to `reborn_skills`, `reborn_tool_skills`, and `reborn_python_code`. For `source='system'` rows seeded by `builtin_bootstrap.rs`, this column holds the SHA-256 hex of the prose field (`body` or `content`). `run_content_integrity_check` (called during shared runtime boot in `component_boot.rs`) verifies these checksums and halts the process on mismatch. Distinct from `content_hash` on `reborn_python_code` (similarity deduplication). Run `brassclaw repair` to restore corrupted system rows.

Legacy `brassclaw_memory_docs` rows are migrated into the appropriate class table at boot by `run_component_import` (`crates/brassclaw_reborn_composition/src/component_import.rs`).

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
Q1 and human Q2, sandboxing, network/secret enforcement and resource limits remain.
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

### Orchestrator-First, LLM-Minimal (Core Design Principle)

**Monty (the Python orchestrator) IS the execution engine and the sole
execution authority.** Rust makes tools *available*; an LLM never executes
anything itself — it only writes Python that Monty runs in the sandbox. The LLM
is consulted only when creative reasoning, content composition, or user
confirmation is genuinely required.

**The Orchestrator and Rust Tools:**
BrassClaw has one execution authority — **Monty** — and a registry of Rust **Tools** it calls.

- **Orchestrator (Monty, Python)** is the sole execution authority. It runs
  **as one global orchestrator started at system startup and kept alive in the
  background for the instance lifetime**. Each input is a task delivered to
  that existing orchestrator. It reads the
  `BuildInstruction` assembled by IBS, and executes steps in sequence: binding
  tools into its namespace, running PythonCode snippets, assembling LLM prompts,
  and posting replies. It never executes Rust directly — it calls registered
  Tools by name via `host.<tool>(...)`.
- **Rust Tools** are precompiled callables registered in the host namespace.
  They hold no sequencing logic, no recipes, no state. They execute one
  operation when called and return a result. **Before writing a new Rust Tool:**
  verify no existing Tool covers the primitive needed. A new Rust Tool is
  incomplete without a ToolSkill + PythonCode snippet + Leaf Skill + Recipe
  seeded in `builtin_bootstrap.rs` — without those, the Tool cannot be reached
  by any Recipe.

**Tool invocation — first-class callables:** Tools are first-class callables in
the Monty namespace. A PythonCode snippet calls a tool as
`result = host.tool_name(param=value)`. Invoking the binding crosses into Rust,
which runs the Tool and returns. All host capabilities register the same way —
there are no hidden intrinsics or special-cased Rust paths.

#### Turn Execution Flow (binding target — read this first)

Every user input travels exactly one of two paths within the already-running
global orchestrator. This is the target flow; the lifecycle implementation gap
is described below.

```
User Input
    │
    ▼
Global Orchestrator (Monty) — already running since system startup
    │
    ▼
Intent-Matching System (resolve_intent / fetch_for_turn, currently Rust)
    │
    ├─── MATCH ──────────────────────────────────────────────────────────────────┐
    │                                                                            │
    │  Composition system fetches the Recipe by component-id                    │
    │      │                                                                     │
    │      ▼                                                                     │
    │  IBS (Instruction-Building-System)                                         │
    │  step_link + StepDescriptions JSONB → build_instruction()                 │
    │  → BuildInstruction { rust_steps, orchestrator_steps }                    │
    │      │                                                                     │
    │      ▼                                                                     │
    │  Orchestrator executes steps in sequence:                                  │
    │                                                                            │
    │    channel:"rust"        → Load ToolSkill binding into Monty namespace     │
    │    channel:"orchestrator"→ PythonCode runs: result = host.<tool>(...)      │
    │                             ↳ crosses into Rust Tool, returns result       │
    │    channel:"orchestrator"→ (optional) LLM step if Tier 1                  │
    │    channel:"orchestrator"→ host.post_reply(answer="...") → user sees reply │
    │                                                                            │
    │  History saved. Task completes; global Monty awaits further work.           │
    │                                                                            │
    └─── NO MATCH ───────────────────────────────────────────────────────────────┘
         │
         ▼
     Orchestrator assembles an LLM prompt (Tier 2):
         1. User's question
         2. Conversation history for this input
         3. base-prompt prefix (precompiled ~250k–1M tokens, added by Kohai —
            contains all tools, recipes, skills, component descriptions)
         │
         ▼
     LLM reasons over the full prompt → answer posted to user
         │
         ▼
     Sempai interceptor reviews the completed turn →
     proposes new Recipe + intent examples →
     validation queue (Q1 auto → Q2 human) →
     next match uses approved Recipe (Tier 0 if deterministic; else Tier 1)
```

**Key points for authoring:**
- The Match path is 100% recipe-driven. To add behaviour, add a Recipe — not Rust.
- IBS is the compiler: it reads the Recipe's `step_descriptions` JSONB once at match time and produces the `BuildInstruction` that drives execution. It is ephemeral — never stored.
- A `channel:"rust"` step only *binds* the ToolSkill (makes `host.<tool>` callable). A `channel:"orchestrator"` PythonCode step *calls* it. Two steps, always in that order.
- The base-prompt is assembled by the Kohai from the component library — it is not hardcoded. It grows as new components are added.
- Tier 0: no LLM involved at all. Tier 1: LLM guided by recipe prior-knowledge. Tier 2: LLM over full base-prompt (no recipe matched).

> **Monty owns Recipe data flow:** IBS compiles the Recipe into a BuildInstruction;
> Monty executes its steps and retains the intermediate values needed by later steps.
> Fresh empty state per PythonCode step is not mandatory. If a step runs in a child VM
> or process, Monty manages its inputs and returned results within the same Recipe
> execution context. Unrelated tasks and attempts remain isolated. Do not merge separate
> Recipe steps or independent Rust primitives to work around missing state handoff.
> Only combine into one Rust tool if
> the operations are genuinely inseparable at the system level (e.g. an atomic DB transaction
> that cannot be split). Convenience and data flow alone do not justify a new monolithic tool.

#### One global orchestrator, bounded tasks (binding target architecture)

**Exactly one global Monty orchestrator starts during system startup and runs
in the background until the BrassClaw instance shuts down.** It is the Monty
VM executing the Python orchestrator body, hosted by Rust; it need not be a
separate OS process. Startup completes migrations, component seeding and
integrity verification, then loads and starts Monty before enabling turn
workers, trigger producers or ingress. Readiness means that the global VM is
alive and waiting for work, not merely that its driver was constructed.

Every admitted input becomes a task of that existing orchestrator. Finishing a
reply stores the task history and ends that task; **it does not end or recreate
the global VM**. When idle, Monty awaits work without busy polling or LLM calls.
Only instance shutdown or supervised fatal-runtime recovery replaces the VM.

Global lifetime does not imply shared task context: each work item retains its
conversation, run and exact input-message identity, history, continuations,
reply target, signals and per-invocation authority. Tool bindings and task-local
state are released at task completion. Approval/auth/child-run waits must not
block the events needed to resume them or unrelated admitted work. Rust hosts
the VM, transports work and enforces kernel boundaries; Python/Recipes sequence
the work. A Rust queue consumer must not become a second recipe/agent loop.

**Implementation gap:** current code creates chat-keyed sessions on first use
(`PersistentMontyDriver` + `MontySessionRegistry<TurnScope, MontySession>`).
That lifecycle must be replaced as specified in `simplified_v3.md` Phase 3a.
This target contract supersedes older lifecycle descriptions in crate docs and
plans; the current implementation is not evidence that per-chat VMs are desired.

Only the **basic mode's beginning** is built-in (boot the global orchestrator,
receive an admitted work item, establish its task context, hand off to Phase 2).
Everything else is
**Instructions** — a component, most often a **Recipe**, but also possibly an
**Action** or other instruction component. From Phase 2 onward (intent
matching, Matching-Mode, Non-Matching-Mode, validation, component-creation,
kohai-sempai) it is all instruction/recipe-driven, so functionality changes
need **no code changes — only the recipe is altered**.

#### Phase 1 — boot once, receive work (built-in, the one exception)

Monty starts once at system startup after the verified component library is
available. On each admitted input the already-running orchestrator receives a
work item, establishes its explicit task context and **starts the
intent-matching-system**. Starting a task never starts another global
orchestrator. The boot/receive mechanism is the built-in exception, not a Recipe.

#### Phase 2 — intent match (recipe-driven in principle; Rust today)

In principle Phase 2 is run by **Recipes/Instructions** (ideally a second
Python VM), so intent-matching logic can evolve without code changes. For now
the intent system uses the **already-existing Rust** implementation
(`resolve_intent` / `fetch_for_turn` in `brassclaw_engine`); the
recipe/instruction-driven second-VM version is **future work** — only do what
is necessary for a working intent system. The intent system tries to find a
match and returns either a **matching id** (or whatever identifies the match
exactly) or a **"no match"** message back to the global orchestrator's active task.

#### Phase 3 — dispatch

**Case 1 — Match → Matching-Mode.** The active task receives a component-id
and switches into Matching-Mode:

1. The id is sent to the **IBS** via the composition system.
2. IBS reads the Recipe's `step_link` + `StepDescriptions` JSONB and assembles
   a `BuildInstruction { rust_steps, orchestrator_steps }` by collecting the
   referenced ToolSkill and PythonCode snippet UUIDs from the component library.
3. The composition system uses `rust_steps` to **bind** the declared ToolSkills
   into the Monty namespace (making `host.<tool>` callable for this turn).
4. The Orchestrator runs `orchestrator_steps` in sequence: each step is either
   a PythonCode snippet (which calls `host.<tool>(...)`), a Skill loaded as LLM
   context, or a Tier-1 LLM step. The final step posts the reply to the user.
5. History is stored, task-local bindings/context are released, and the task
   completes. Global Monty remains alive to receive further work.

Matching-Mode covers both deterministic and LLM-guided recipes — the recipe
itself decides whether the LLM is needed:

- **Tier 0** — deterministic, no LLM. Tool calls are baked into `PythonCode`
  leaves; Monty runs them in the sandbox.
- **Tier 1** — LLM-guided. The recipe hands the LLM prior-knowledge / a plan;
  after the LLM responds, post-LLM tool steps are run by Monty.

**Case 2 — No match → Non-Matching-Mode.** The orchestrator has no direct
instruction, so the user's input is sent to the LLM as a **standard prompt**
assembled by the orchestrator:

1. The **chat history belonging to this exact user-input** (few tokens).
2. The **user's question** (few tokens).
3. A huge prefix called the **base-prompt**, where all the information about
   BrassClaw — about all tools, recipes, skills, etc. — is **precompiled**, so
   the LLM's answer is very fast while having access to information starting at
   roughly **250k tokens** and pushable up to **1 million** prefix tokens.

The global orchestrator posts the task's LLM answer into its originating chat, then saves a
**thorough history** so the **kohai/sempai system can build new intents,
skills, recipes, tools and other components**, so future matches can use an
approved Recipe. Deterministic work can become Tier 0; reasoning/composition
that still needs an LLM remains Tier 1. (Future, planned: the global orchestrator is available for LLM
calls **via MCP** to gather information or do whatever the LLM needs — still
routed through the orchestrator, never a classical direct-MCP execution path.)

This is **Tier 2**. It is **not "raw LLM"** — it is a recipe/instruction-driven
non-match routine (only the basic mode's *beginning* is built-in). Because it
is recipe-driven, it can be enhanced with **no code changes**: different prompt
additions for different query types, different prefixes, etc. — only the
recipe is altered.

#### Every LLM prompt is assembled by the orchestrator (ground truth 2)

**Every** LLM prompt — whether it belongs to a Recipe, the non-match path, the
Validation-System, the Component-Creation-System, or the kohai-sempai-system —
is **assembled by the orchestrator**, which tells each system what to do and
how to do it. Every LLM prompt is orchestrated **step by step**: *fetch this
information, now format it for this LLM's needs, now add these sentences to
it*, etc., until the prompt is finally created.

The **kohai is always the last one** working on an LLM prompt, because it
**exchanges the placeholders with the prefix prompts**.

#### Tool Binding — how Tools become callable

A Recipe's `step_descriptions` declares which ToolSkills it needs. IBS adds
them to `rust_steps`. The composition system then **binds** each into the Monty
namespace for this turn — making `host.<tool>` callable. At the end of the
task those bindings are **unloaded**; the global orchestrator remains alive.

- **Built-in Tools** — precompiled into the Rust binary; always registered.
  Their ToolSkills are seeded in `builtin_bootstrap.rs`.
- **Extension Tools** — registered at extension-activation time. Same binding
  mechanism as built-ins; only the registration source differs.

A ToolSkill is the binding descriptor (params, preconditions, error policy) that
tells IBS how to prepare the binding. It describes the Tool for IBS — it carries
no instructions for the Orchestrator.

A Skill (class 1–3) is an Orchestrator-facing tool-usage unit: prose instructions
plus associated executable PythonCode (class 22). Its prose can guide an explicit
LLM step; its PythonCode performs the usage. ToolSkill is the Rust-side binding
descriptor, not either part of the Skill.

#### Runtime authority — enforced before every Tool dispatch

The same kernel boundary applies in Tier 0, Tier 1 and Tier 2, including
validated Recipes, component creation, validation and Kohai/Sempai workflows.

- ToolSkill binding makes a callable available; it executes nothing and grants
  no permission. Q1/Q2 approve components, not Tool invocations.
- Before **every actual Tool dispatch**, the kernel checks the current global
  instance-wide allow/block policy and technical parameters. A task's pinned
  component revision does not freeze permission. Blocking a Tool also blocks
  its next call in an already running or resumed Recipe, including retries.
- External authentication, sandboxing, network/filesystem/secret enforcement,
  resource limits and run/attempt freshness remain applicable. Validation does
  not switch these off, including for outbound HTTP.
- Simplified v3 adds no user/project-role Tool grants, per-operation approvals
  or fingerprinted approval leases. Claims/attempt identifiers fence stale
  execution and cancellation; they are not grants.
- Operator settings control the supported global policy and technical limits.
  There is no mode-specific switch that exempts validated Recipes from kernel
  enforcement. Bind-time preparation cannot replace dispatch-time checks.

This is the binding target. Existing scoped/operation-approval paths are legacy
until the coordinated cutover; documentation does not establish that the new
production path is complete. Never disable enforcement to implement this model.

#### Components are the crucial thing

With this architecture, most tasks are performed by the orchestrator on its
own. The most crucial thing is the **components**: if they are made well, a lot
of different tasks can be performed by **different recipes calling the same
components**. The long-term lever is a large library of tiny, reusable
components — more modules and recipes, fewer Rust branches.

#### Recipe syntax — human-readable AND machine-readable

Recipes (+ the composition system) need a **clever dual-nature syntax**: a
**human-readable, logically-constructed** recipe on one hand, and a
**machine-readable exact logic** on the other that **always reproduces the same
results** from the orchestrator and the Rust code. The goal: with everything
running as intended, **no code changes are necessary** to change behaviour —
only the recipe is altered.

**Component reuse hierarchy — always work from the top down, stop at the first level that solves the problem:**

| Level | Action | Cost |
|-------|--------|------|
| 1 | Add intent examples to an existing `RecipeVariant` | Zero new components — just new rows in `reborn_intent_inputs` |
| 2 | Add a new `RecipeVariant` to an existing `Recipe` | One new variant + intent examples; reuses existing PythonCode snippets and ToolSkills |
| 3 | Reference an existing PythonCode snippet in the new variant's `step_descriptions` | No new PythonCode row needed |
| 4 | Reference an existing ToolSkill in the new variant's `channel:"rust"` step | No new ToolSkill row needed |
| 5 | Author new PythonCode snippet + ToolSkill + Leaf Skill + Recipe rows in `builtin_bootstrap.rs` | New components, no new Rust |
| 6 | Write a new Rust Tool + full set of components | Only when no existing Tool provides the primitive |

**Check the component library before authoring anything new.** The Sempai grows the library with use; over time more tasks are covered by level 1–2 alone.

**Required authoring rules** (verify actual Q1/runtime enforcement):
- **One leaf skill per approach**: Three skills covering three patterns is better
  than one monolithic skill covering all three. If a tool has N common usage
  patterns, author N leaf skills.
- **One predictable layout/workflow per variant**: Compatible variants may share
  a Recipe. Give every variant verified positive/negative intent examples and a
  nonempty `step_link`. Split incompatible operations/layouts into separate
  variants or Recipes. A variant's behavior determines Tier 0 or Tier 1; neither
  intent matching nor approval automatically eliminates LLM work.
- **PythonCode snippets** (class 22): a PythonCode component is a code snippet
  stored in the component library. It is not an executor — the Orchestrator
  runs it. A Tool-calling snippet normally makes one `host.<tool>(...)` call;
  pure logic makes zero. Only the direct dependent-chain exception in recipe.md
  permits multiple calls as one logical unit. Independent dispatches, including
  calls hidden in internal includes, require separate Recipe execution steps.
  The exception does not turn a multi-Tool workflow into one leaf Skill.
  The retired `__execute_action__()` intrinsic is gone — any snippet using it
  fails Q1.
- **Separate prose from the executable entry point**: a Skill contains prose
  and associated PythonCode. Examples in prose are documentation; actual tool
  execution uses the associated PythonCode step.
- **Dual-nature fields (Step B):** every recipe carries BOTH natures on the
  same struct — no separate rendering or transpilation:
  - **Machine-readable exact logic (untouched):** `RecipeVariant.step_link` +
    `Recipe.step_descriptions` → IBS `build_instruction` → `BuildInstruction`
    (`rust_steps` + `orchestrator_steps`). Deterministic; never changed by Step B.
  - **Human-readable explanation (concise — "what happens"):**
    `Recipe.description` (recipe-level), `RecipeVariant.description`
    (variant-level — added in Step B), `StepDescriptionEntry.label` +
    `StepEntry.goal` (step-level).
  - **Q1 gate:** every variant MUST have a non-empty `step_link` and a
    non-empty `RecipeVariant.description` (≤ 512 chars). Enforced in
    `RecipeValidator::validate_recipe` (`check_variant_descriptions`).
    **Never author a variant without `step_link`** — IBS cannot compile it and
    it will not function. Rows with `step_link == None` are legacy database
    artefacts from before v3 migration; do not create new ones.
  - **Read surface:** `RecipeDetail.recipe` is opaque full-engine JSON, so new
    variant fields ride along to the WebUI with no DTO recompile. There is no
    WebUI recipe-authoring route yet (future work).

Authoring ground truth: [recipe.md](recipe.md) and [skills.md](skills.md).
The built-in archive, tomedo reference and Zencoder plan are historical/worked
examples; verify their schema, bindings and signatures before reuse. They do not
supersede the ground-truth input, approval, version or Tool-policy contracts.

### Recipe Tier Lifecycle — LLM as One-Time Cost

**The LLM is a one-time cost per pattern. Recipes are the permanent return.**

The three tiers map directly to what happens on a given turn:

| Tier | Condition | LLM call | Typical cost |
|------|-----------|----------|--------------|
| **0** | Recipe matched; `llm_call_required: false` | ❌ never | Zero tokens |
| **1** | Recipe matched; `llm_call_required: true` | ✅ guided by recipe prior-knowledge | Low |
| **2** | No match — Non-Matching-Mode | ✅ full reasoning over base-prompt | Full |

**Why Tier 2 matters:** The first time a user asks something new, no recipe matches. The LLM reasons through it. The **Sempai interceptor** reviews the completed turn, evaluates the outcome, and `proposed_recipe_updates` + `proposed_intent_examples` enter the validation queue via `PgSempaiProposalSink`. After **Q1** (automated, sandboxed) and **Q2** (human review — mandatory, never automated), the recipe graduates to `validated`. After coherent activation, subsequent matches use the Recipe at its verified tier. Deterministic Tier-0 Recipes make zero LLM calls; Tier-1 Recipes retain their explicit LLM work. Approval does not guarantee zero execution latency.

**Why pre-seeded extensions exist:** Trusted first-party bootstrap supplies
validated components through a distinct integrity-checked system path, without
waiting for Tier-2 discovery. Setting `source: "system"` on an authored proposal
cannot bypass Q2. Seeded deterministic Recipes may be Tier 0; seeded Recipes
requiring LLM work, shell or spawn_subagent remain Tier 1. Authored updates and
operator overrides follow their required Q1/human-Q2 approval path. Pre-seeding
encodes reusable usage and workflow knowledge; it does not grant Tool permission.

**The Sempai grows the library.** Patterns the extension author didn't anticipate — novel combinations, edge-case filters, multi-step flows — emerge from Tier-2 turns. The Sempai proposes them; Q1+Q2 graduates them. The library compounds with real usage, with zero engineering effort after the initial seed.

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

**Posting deterministic output without an LLM:** call `host.post_reply(answer="<fixed text>")` via the builtin `ts-host-post-reply` ToolSkill + `pc-host-post-reply` PythonCode. This is the correct Tier-0 pattern for fixed-text responses (e.g. auth-setup instructions). `builtin.echo` is diagnostic-only and must not appear in user-facing recipes.

### Consumer-Tag Gating (§3.9)

Components carry `consumer_tags[]` that control which agent roles may access them. The `sender_class_code` on a turn maps to a consumer tag; `PostgresSource` enforces a `SEC-01` validation gate — only `Validated` components are returned. Actions (class **16**) are exempt from the prior-knowledge token budget. (Class 11 is unallocated — see the class code table above.)

### 4-Queue Validation Lifecycle (§3.5.1)

| Queue | Code | Meaning |
|-------|------|---------|
| Q1 | `auto` | Auto-extracted; awaiting LLM audit |
| Q2 | `manual` | Operator review required (`05:validator` tag present) |
| Q3 | `revision` | Automated revision by class-09 extension |
| Q4 | `rejection` | Rejected; retained for `q4_retention_days` then wiped |

State transitions enforced by `is_valid_transition` in `brassclaw_product_workflow::recipes`. For Orchestrator (10) and Scaffold (50) classes, `Q1→Q2` requires a clean LLM audit pass.

**Recovery from Q4 rejection:** Read the Q1 audit output — it will identify the specific violation (forbidden symbol, wrong `class_code`, missing `result =` assignment, channel isolation error, `step_link` without `RecipeVariant.description`, etc.). Fix the component and re-submit. **Do not rewrite the capability as Rust because a recipe was rejected.** Fix the recipe. Common Q1 failure causes:
- `host.<tool>(...)` call absent (empty or no-op executor)
- `import os` / `import subprocess` / `exec(` / `eval(` / `open(` in PythonCode body
- `class_code` set to 11 (unallocated) instead of 16 (Actions) or another correct code
- ToolSkill UUID placed in `orchestrator_steps` (channel isolation violation)
- `step_link` present but `RecipeVariant.description` is empty

### Intent System (§3.12)

`resolve_intent` in `crates/brassclaw_engine/src/memory/intent_system.rs` provides 4-class query classification using a single `CASE WHEN` Postgres query against `reborn_intent_inputs`:

- **Class 1** (exact match): returns the matched component ID directly
- **Class 2** (high-confidence): returns the top match
- **Class 3** (disambiguation): returns `Disambiguation` with up to 5 candidates; the orchestrator sends a `role: "disambiguation"` message to the chat UI; the user's selection sends `{disambiguation_choice: component_id}`; `record_disambiguation_choice` stores the selection and increments the score
- **Class 4** (no match / "try it with AI"): falls back to keyword UNION ALL path

### Intent-Driven Retrieval (`fetch_for_turn`)

`PostgresSource::fetch_for_turn` in `retrieval_source.rs` replaces the old "load all docs" path:
1. Calls `resolve_intent` with the user query
2. On `Match`: fetches the exact component by ID from the appropriate class table
3. On `Disambiguation`: returns `FetchForTurnResult::Disambiguation(candidates)` to the orchestrator
4. On `NoMatch` / error: falls back to UNION ALL keyword retrieval (DB-less helpers in `retrieval_dbless.rs`)

### Monty VM Settings (§3.10)

**Upgrade prerequisite:** Follow `simplified_v3.md` Phase 3a’s Monty 1.0 gate
before global production wiring. The v0.0.16 custom-tracker proof is test-only
and cannot implement the new API. Monty 1.0 removes the allocation-count limit;
preserve existing settings until their explicit migration rather than silently
ignoring them. Rust and Monty must share the effective duration revision and
one task compute account; a persisted WebUI edit alone is not runtime uptake.


`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (V034 migration). `max_duration_secs` bounds one task's execution, never the global orchestrator's uptime or idle wait. Memory, allocations, stdout and token accounting must distinguish task budgets from bounded global-service storage. The current `LimitedTracker` lifetime must be audited before reusing it in a global VM; do not assume its counters reset on resume. The legacy `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is a DB-less fallback only.

### Orchestrator Code Load Path

The orchestrator code body (`basic_mode.py`) must be loaded at global-service startup, after integrity verification, via `OrchestratorCodePort` (engine-side port, `orchestrator_code_port.rs`), implemented by `PgOrchestratorCodePort` (`brassclaw_reborn_composition`, gated `postgres+skills-db`). The body is stored as a class-10 `reborn_skills` row with `name='orchestrator:main'`, `source='system'`, seeded by `seed_orchestrator()` in `builtin_bootstrap.rs`. There is no compiled-in fallback — a missing DB row produces `OrchestratorCodeError::NotFound` (run `brassclaw repair` to restore) and must prevent readiness. A live service pins its verified code version; code replacement requires a controlled restart with task reconciliation. Fatal VM failure is a service failure, not a Tier-2 fallback or permission to replay external effects. Current per-conversation loading is an implementation gap addressed in `simplified_v3.md` Phase 3a.

The preamble/postamble (`codeact_preamble`, `codeact_postamble`) are also class-10 rows seeded by `seed_orchestrator()`. They reach the LLM via the Kohai prefix bundle assembled by `do_assemble_bundle` in `interceptor_config_service.rs` — not per-turn by the executor. No `OnceLock` or `init_*` call is needed for them; the bundle assembler queries `reborn_skills` directly.

### Boot Sequence (seeding + integrity)

The seeding boot chain now lives in `crates/brassclaw_reborn_composition/src/component_boot.rs`, called by `runtime.rs` after migrations and before turn workers and trigger producers. Required seed/recovery failures abort runtime construction. Every required prompt is loaded before initializing process-wide prompt stores. WebUI construction only attaches its facade and UI services. Global Monty ownership, its startup handshake and durable task continuation remain to be implemented. Required target order (see `simplified_v3.md` Phase 3a):

```
BootedDb::from_migrated_pool(pool)   ← type-level proof migrations completed
  → seed_builtin_host_components(&booted_db)  ← host.* Tool stack
  → seed_builtin_components(&booted_db)       ← all first-party capabilities + system prompts
       └─ seed_orchestrator() seeds class-10 rows with content_checksum
  → run_boot_integrity_check(&booted_db)      ← Phase N: re-queue non-validated components
  → run_content_integrity_check(&booted_db)   ← HARD ERROR on SHA-256 mismatch
  → init_failure_explanation_prompt(body)     ← OnceLock: failure_explanation
  → init_compaction_summarizer(body)          ← OnceLock: compaction_summarizer_fresh
  → init_sempai_persona(body)  [root-llm-provider]  ← OnceLock: sempai_audit
  → init_directions(general, researcher, explorer, coder)  ← OnceLock: direction prompts
  → wire host/component/Kohai ports without starting work producers
  → load verified orchestrator:main via OrchestratorCodePort
  → start exactly one global Monty; await its initial work-wait handshake
  → enable turn workers and configured trigger/channel producers
  → expose ready ingress / attach RebornWebuiBundle
```

`BootedDb` (`crates/brassclaw_reborn_composition/src/booted_db.rs`) is a newtype that enforces migration-before-seeding at the type level. `run_content_integrity_check` (`content_integrity.rs`) is **distinct** from `run_boot_integrity_check` (`boot_integrity.rs`) — the former checks SHA-256 prose checksums, the latter re-queues non-validated components.

### `brassclaw repair` Command

`brassclaw repair [--dry-run]` force-reseeds all `source='system'` component rows from compiled-in seed constants using `ON CONFLICT DO UPDATE` (unlike the normal seeder which uses `ON CONFLICT DO NOTHING`). Run this after a binary update that changes system prompt content to fix the content integrity check failure. The `repair_builtin_components()` function lives in `crates/brassclaw_reborn_composition/src/repair.rs`; the CLI entry point in `crates/brassclaw_reborn_cli/src/commands/repair.rs`.

### PKC Formatting Split (§3.13/§3.14)

`format_prior_knowledge_for_llm()` in `orchestrator.rs` produces deterministic JSON from `PriorKnowledgeResult` items: ordered by `(class_code asc, prompt_uid asc)`, `class_code_label()` for string names, NULL fields omitted. The `formatted_content` surface is the only surface sent to the LLM; raw `content` is never sent.

### Interceptor Architecture (§3.15)

The Sempai/Kohai review loop intercepts each agent turn:
- `RebornLoopDriverHost` saves a `ForensicPacket` on `on_prompt_assembled` (status `AwaitingKohai`)
- `on_kohai_response` closes it (status `Complete`)
- The interceptor tab (WebUI v2 Settings) exposes: Sempai status/mode, "Reassemble base prompt" button, "Pre-warm Sempai KV-cache" button, persona editor, `components_since_rebuild` badge
- Hidden in DB-less mode

### AI Before User Preference (§7 Q18)

`PUT /api/chat/preferences/ai_before_user` persists to `reborn_user_preferences` (V035 migration) via `PgUserPreferenceStore`. The WebUI chat input shows a pill-style toggle (hidden when the preference store is unavailable / DB-less). When enabled, the assistant sends a preliminary response before disambiguation or gate prompts.

### Key Traits

| Trait | Location | Purpose |
|-------|----------|---------|
| `LlmProvider` | `crates/brassclaw_llm/` | Multi-provider LLM integration |
| `EmbeddingProvider` | `crates/brassclaw_embeddings/` | Vector embedding interface |
| `Hook` | `crates/brassclaw_hooks/` | Lifecycle hook points |
| `TurnCoordinator` | `crates/brassclaw_turns/` | Turn coordination contract |
| `HostRuntime` | `crates/brassclaw_host_runtime/` | Host service access |
| `ComponentPort` | `crates/brassclaw_engine/src/executor/composition_port.rs` | Engine-side port for component lookup (impl: `PgCompositionPort` in composition) |
| `KohaiPort` | `crates/brassclaw_engine/src/executor/kohai_port.rs` | Engine-side port for Kohai LLM bundle (impl: `PgKohaiPort` in composition) |
| `OrchestratorCodePort` | `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs` | Engine-side port for loading the class-10 Orchestrator body from `reborn_skills` (impl: `PgOrchestratorCodePort` in composition, gated `postgres+skills-db`) |

All I/O is async with tokio. Use `Arc<T>` for shared state, `RwLock` for concurrent access.

**LLM data is never deleted.** All LLM output — context fed to the model, reasoning, tool calls, messages, events, steps — is the most valuable data in the system. Never strip, truncate, or delete it from the database. Mark with timestamps, make filterable, but always retain. In-memory HashMaps are caches; the database (via Workspace) is the source of truth.

### Extension and Auth Invariants

Extension and channel onboarding has two distinct identities that must not be conflated:

- `credential_name`: backend secret identity used for storage, injection, and gate resume
- `extension_name`: user-facing installed extension/channel identity used for setup routing and UI

Rules:

- Never route web setup/configure UI directly from `credential_name`.
- Chat and Settings must use the same setup/configure path for installable extensions/channels.
- Generic auth-card UI is only for non-extension credential prompts or pure OAuth launch prompts.
- If an auth flow is for an installed extension/channel, resolve the `extension_name` once in shared backend logic and carry it through the wire contract.
- New auth/onboarding code must reuse the shared resolver/controller path.

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
cargo fmt --all -- --check                              # submission formatting
cargo test -p <crate_name> <relevant_test>               # focused behavior
cargo clippy -p <crate_name> --all-targets               # review new warnings
# Add relevant feature flags; PostgreSQL checks cover database semantics.

# Authorization crate — capability lease contract tests (no DB required)
cargo test -p brassclaw_authorization

# Authorization crate — PostgreSQL integration tests (requires BRASSCLAW_PG_URL)
BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5434/brassclaw \
  cargo test -p brassclaw_authorization --features integration

# Build the Reborn binary for release/performance work only
cargo build --release --bin brassclaw

# Run with logging
BRASSCLAW_REBORN_LOG=brassclaw=debug cargo run
```

### Avoid redundant rebuilds

`cargo build` on this workspace is slow. Capture output once and inspect it multiple times — do **not** rerun the build just to see different output:

```bash
# Capture and display simultaneously
cargo build --release --bin brassclaw 2>&1 | tee build.log

# Analyse the saved log without rebuilding
grep "^error" build.log
grep -n "warning\|error" build.log | head -40
cat build.log | less
```

Before repeating any build, test, check or lint, inspect the captured output and
state what relevant edit or unresolved question requires another run. Reuse
passing evidence for unchanged paths; avoid redundant check/build/test sequences.
See [the development policy](docs/development-policy.md) for the stopping rule,
stable build configuration and caller-level validation requirements.

E2E tests: see `tests/e2e/CLAUDE.md`.

## Creating Releases

### Automated Release Process

This project uses GitHub Actions for automated releases. **Do not build binaries manually.**

### How to Create a Release

Simply push a version tag:

```bash
git tag v1.3.0
git push origin v1.3.0
```

GitHub Actions will automatically:
1. Build binaries for all platforms (Linux x86_64, macOS ARM64, macOS x86_64)
2. Generate SHA256 checksums for each binary
3. Create a GitHub release with auto-generated release notes
4. Upload all artifacts (binaries + checksums) to the release

### Monitoring Builds

- **Workflow runs**: https://github.com/chtugha/brassclaw/actions/workflows/release.yml
- **All actions**: https://github.com/chtugha/brassclaw/actions

### Supported Platforms

- **Linux x86_64**: `x86_64-unknown-linux-musl` (statically linked)
- **macOS ARM64**: `aarch64-apple-darwin` (Apple Silicon)
- **macOS x86_64**: `x86_64-apple-darwin` (Intel)

### Release Workflow Details

See `CICD_SETUP_DOCUMENTATION.md` for comprehensive documentation on:
- Workflow architecture
- Build process details
- Testing procedures
- Troubleshooting guide
- Maintenance instructions

### Important Notes

- **Never manually build and upload binaries** - always use the automated workflow
- **Tag format**: Use semantic versioning with `v` prefix (e.g., `v1.2.9`, `v1.3.0`)
- **Build time**: Expect 10-15 minutes for all platforms to build
- **Artifacts**: Each release includes 6 files (3 binaries + 3 checksums)

## Project Structure

```
crates/
├── Reborn runtime
│   ├── brassclaw_reborn/           # Runtime, driver registry, boot orchestration
│   ├── brassclaw_reborn_cli/       # brassclaw binary (commands, dispatch)
│   ├── brassclaw_reborn_composition/  # Wiring: capabilities, loops, host access
│   ├── brassclaw_reborn_config/    # Config resolution, profiles, home resolution
│   └── brassclaw_reborn_webui_ingress/  # WebUI v2 gateway adapter and ingress
│
├── Persistence
│   ├── brassclaw_pg/               # Postgres pool, migration runner, SQL migrations V000–V086
│   └── brassclaw_embedded_postgres/ # Self-managed embedded Postgres lifecycle
│
├── Agent loops and engine
│   ├── brassclaw_agent_loop/       # Planned AgentLoop driver
│   ├── brassclaw_engine/           # Execution engine: intent matching, IBS, orchestrator executor, tool dispatch
│   │   └── prompts/                # Prompt .md source files (seed source for builtin_bootstrap.rs; test reference only — never loaded at runtime via include_str!() outside tests)
│   └── brassclaw_engine_types/     # Shared engine types and traits
│
├── LLM and embeddings
│   ├── brassclaw_llm/              # Multi-provider LLM integration
│   │   └── providers/              # openai, anthropic, ollama, nearai, bedrock, tinfoil
│   └── brassclaw_embeddings/       # Embedding providers, hybrid search (FTS + vector + RRF)
│
├── Skills
│   └── brassclaw_skills/           # v3 Skill component types, validation, reborn_skills store
│                                   # Feature gates: db-store (PgSkillStore/DbSkillStore),
│                                   #   v1-types (legacy SKILL.md filesystem types — migration only),
│                                   #   v2-compat (MemoryDoc bridge types — skill_tracker only)
│
├── Safety and security
│   └── brassclaw_safety/           # Prompt injection, validation, leak detection, policy
│
├── WebUI v2
│   ├── brassclaw_webui_v2/         # React SPA server, routes, bearer-token auth
│   └── brassclaw_webui_v2_static/  # Static assets for WebUI v2
│
├── Extensions
│   └── brassclaw_extensions/       # Extension lifecycle: install, configure, activate, remove
│
├── Host runtime
│   └── brassclaw_host_runtime/     # Trusted laptop shell access, mount aliases
│
├── Sandbox
│   └── brassclaw_process_sandbox/  # Process sandbox: docker-image validator, capability-lease subprocess gating, scoped filesystems, endpoint allowlists
│
├── MCP
│   └── brassclaw_mcp/              # Model Context Protocol client and session management
│
├── Architecture tests
│   └── brassclaw_architecture/     # Architectural invariant tests
│
└── (additional shared utility crates)

tests/
├── *.rs                            # Integration tests
├── test-pages/                     # HTML->Markdown conversion fixtures
└── e2e/                            # Python/Playwright E2E scenarios
```

## Module Specs

When modifying a module with a spec, read the spec first. Code follows spec; spec is the tiebreaker.

| Module | Spec |
|--------|------|
| `crates/brassclaw_reborn_cli/` | `crates/brassclaw_reborn_cli/AGENTS.md` |
| `crates/brassclaw_reborn/` | `crates/brassclaw_reborn/CLAUDE.md` |
| `crates/brassclaw_reborn_composition/` | `crates/brassclaw_reborn_composition/CLAUDE.md` |
| `crates/brassclaw_reborn_config/` | `crates/brassclaw_reborn_config/CLAUDE.md` |
| `crates/brassclaw_agent_loop/` | `crates/brassclaw_agent_loop/CLAUDE.md` |
| `crates/brassclaw_llm/` | `crates/brassclaw_llm/CLAUDE.md` |
| `crates/brassclaw_safety/` | `crates/brassclaw_safety/CLAUDE.md` |
| `crates/brassclaw_embeddings/` | `crates/brassclaw_embeddings/AGENTS.md` |
| `crates/brassclaw_reborn_webui_ingress/` | `crates/brassclaw_reborn_webui_ingress/CLAUDE.md` |
| `crates/brassclaw_engine/` | `crates/brassclaw_engine/CLAUDE.md` |
| `tests/e2e/` | `tests/e2e/CLAUDE.md` |

## Token Budget

BrassClaw Reborn targets 7B-14B LLMs within an 8,192-token context window.

| Budget item | Tokens |
|-------------|--------|
| Total context | 8,192 |
| Base prompt (prefix + injected components) | 2,048 |
| Remaining for history, tools, response | ~6,144 |

Compaction is triggered when the in-context history would exceed the budget. Workspace memory (persistent, chunked, searchable) is the mechanism for retaining information across compaction boundaries.

## Skills

A Skill is one tool-usage unit: **prose plus explicitly associated executable
PythonCode**. [skills.md](skills.md) governs its contracts, association approval
and validation; [recipe.md](recipe.md) governs workflow assembly and execution.
`PgBasicPromptStore` assembles stored prose into the base prompt; execution uses
the separately resolved class-22 PythonCode. First-party rows are authored in
`builtin_bootstrap.rs` through the supported stores. A complete Skill needs both
parts and their validated UUID/revision association; the current separate-row
representation must not be described as a prose-only architectural definition.

| Class | Existing consumer label | Storage / meaning |
|-------|-------------------------|-------------------|
| 1 | skill_rusty | Skill prose in `reborn_skills`; associated PythonCode is class 22. |
| 2 | skill_monty | Same Skill definition; not a domain-level Skill. |
| 3 | skill_llm | Same Skill definition; consumer classification, not a broader hierarchy. |
| 10 | Orchestrator | Separate component kind sharing `reborn_skills`. |
| 50 | Scaffold | Separate component kind sharing `reborn_skills`. |

Domain context belongs to an Extension/ExtensionCatalogue. Recipes sequence
the reusable usage units. ToolSkills remain Rust-side IBS binding descriptors.


### brassclaw_skills crate — feature gates

The `brassclaw_skills` crate exposes three Cargo feature gates:

| Feature | What it gates | When to enable |
|---------|---------------|----------------|
| `db-store` | `DbSkillStore` (full CRUD, validation queue write path) and `PgSkillStore` (bootstrap seeder) | Required by `brassclaw_engine/skills-db` |
| `v1-types` | `SkillManifest`, `LoadedSkill`, `SkillSource`, `ActivationCriteria`, `GatingRequirements`, `SkillCredentialSpec`, `SkillOAuthConfig`, `ComponentType`, `ComponentTypeSet`, `MAX_PROMPT_FILE_SIZE` | v1→v3 migration importer only. Do **not** enable for new code. |
| `v2-compat` | `V2SkillMetadata`, `CodeSnippet`, `SkillRevision`, `SkillRepairRecord`, `V2SkillSource`, `SkillMetrics` (deprecated — see below) | `brassclaw_engine` `skill_tracker` only. Do **not** enable for new code. |

`v2-compat` implies `v1-types` (because `V2SkillMetadata` embeds `ActivationCriteria`).

**Write path for new v3 skills:** use `PgSkillStore::insert(NewPgSkill { ... })` in
`builtin_bootstrap.rs`. Do **not** use `DbSkillStore` for new first-party skills — it is
the migration-importer and validation-queue write path and carries legacy fields
(`compatibility`, `license`, `allowed_tools`, `setup_marker`, etc.) that are not part
of the v3 component model.

**Deprecated bridge types:** `V2SkillMetadata` and `CodeSnippet` are `#[deprecated]`
since 0.3.0. They are MemoryDoc-backed v2 bridge types used only by
`brassclaw_engine::memory::skill_tracker`. In v3 skill telemetry lives in the
`reborn_skills` DB columns (`usage_count`, `success_count`, `wilson_lower`, etc.).
Do not add new consumers of these types.

The v1 filesystem skill system (`skill_list`, `skill_search`, `skill_install`,
`skill_remove`, `/skills` package UI, repo-root `skills/` directory) was removed
entirely. Do not reference or recreate any part of it.

## Configuration

See `.env.example` for all environment variables.

### Key Reborn Variables

**Bootstrap tier** (fixed set, read before the DB starts — safe as inline `Environment=` in the systemd unit):

| Variable | Default | Purpose |
|----------|---------|---------|
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | Reborn state root |
| `BRASSCLAW_RUNTIME_PROFILE` | `local_dev` | Per-invocation capability policy: `local_dev` (default), `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`. Controls the security resolver only; **Postgres is always the storage backend**. `BRASSCLAW_REBORN_PROFILE` (old composition-profile name) is a hard startup error — remove it from any systemd units or env files. |
| `BRASSCLAW_REBORN_LOG` | — | Log filter (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | — | External Postgres URL; optional for single-host local deployments (embedded Postgres used when absent), required for hosted/production |
| `BRASSCLAW_EMBEDDED_PG_PORT` | 5434 | Override embedded Postgres port |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | `127.0.0.1` | Override embedded Postgres listen addresses. Set to `0.0.0.0` to allow LAN connections. **First-boot only**: `postgresql.conf` is written once by `initdb`; to change on an existing cluster, edit `$REBORN_HOME/postgres/data/postgresql.conf` and restart the service. |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | — | Path to master-key passphrase file; set only for passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after DB is up — set via `EnvironmentFile=` in the systemd unit):

The *names* of these vars live in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted to the DB. Includes `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, and trigger auth tokens.

LLM provider configuration is managed via `brassclaw config set` or the first-run wizard and stored in the DB. See `crates/brassclaw_llm/CLAUDE.md`.

## Database

**Postgres is mandatory. There is no non-Postgres production build path.**

All persistence uses Postgres (`brassclaw_pg` crate + embedded Postgres via `brassclaw_embedded_postgres`). In-memory backends are acceptable for **unit tests only** — never in production code or integration paths.

The `postgres` cargo feature in `brassclaw_reborn_composition` is set as a **required default** (`default = ["postgres"]`). Do not add `#[cfg(not(feature = "postgres"))]` fallback paths to production composition or factory code. If you need a non-postgres code path, it belongs only in test fixtures.

The `RebornCompositionProfile` enum and all composition-level profile selection have been removed. There is no `local_dev` vs `hosted` composition split — Postgres is always the backend. `BRASSCLAW_RUNTIME_PROFILE` controls only the per-invocation capability policy (security resolver), never the storage backend. `BRASSCLAW_REBORN_PROFILE` is a hard startup error.

- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.
- All config lives in the `brassclaw_config` Postgres table; provider definitions in `brassclaw_llm_providers`.

## WebUI v2

WebUI v2 is a React SPA served at `/v2` from the `brassclaw` binary.

- Built with the `webui-v2-beta` cargo feature flag
- Static assets embedded via `crates/brassclaw_webui_v2_static/`
- Server routes and bearer-token auth live in `crates/brassclaw_webui_v2/`
- Gateway adapter in `crates/brassclaw_reborn_webui_ingress/`
- Start with `brassclaw serve` (default: `127.0.0.1:3000`)
- For non-loopback listeners, use `serve --host 0.0.0.0` only with a non-yolo profile; `local-dev-yolo` with `--confirm-host-access` refuses non-loopback binds

## Job State Machine

```
Pending -> InProgress -> Completed -> Submitted -> Accepted
    \                \-> Failed
     \-> Failed       \-> Stuck -> InProgress (recovery)
                              \-> Failed
```

## Debugging

```bash
BRASSCLAW_REBORN_LOG=brassclaw=trace cargo run
BRASSCLAW_REBORN_LOG=brassclaw::agent=debug cargo run
RUST_LOG=brassclaw=debug,tower_http=debug cargo run   # includes tower_http request logging
```

## Current Limitations

1. Reborn runtime: long-lived daemon/service installation not yet supported
2. Reborn runtime: v1→v3 data migration implemented (`migration.rs` Steps 3–7: config.toml, providers.json, secrets master key, libSQL DB → Postgres). Long-lived daemon/service installation not yet supported (see item 1).
3. MCP: no streaming support; stdio/HTTP/Unix transports all use request-response
4. Built tools get empty capabilities; no UX for granting access
5. No tool versioning or rollback
6. Observability: only `log` and `noop` backends (no OpenTelemetry)
7. `brassclaw` not yet included in cargo-dist release artifacts (see issue #3483)
