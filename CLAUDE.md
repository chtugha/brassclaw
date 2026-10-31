# BrassClaw Development Guide

> **Current status — 2026-10-23:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.

## Development workflow and precedence

Read [the development policy](docs/development-policy.md) before implementation.
It governs source inspection, bounded work packages, self-review, check selection,
evidence reuse and when validation is sufficient. Its rules supersede older blanket
test/lint instructions here. Required subsystem acceptance, component approval and
the disk-space rules remain binding. Reuse this session's current source knowledge
and valid evidence instead of repeating discovery or checks after every edit.
Runtime Tier-0 rules do not limit development reasoning.
The binding sections below describe product contracts. Source-specific and legacy
notes describe implementation boundaries, not alternative contracts or proof of
acceptance. Consult the status index before repeating already completed work.


## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
A Skill needs no dedicated Recipe or matching command to exist or be complete.
Many Recipes may reuse the same approved usage. MCP exposure requires a separately
qualified MCP-call Recipe/variant; reference/test workflows do not imply ownership.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no executable Python payload; declared source-text strings remain data
under the exact command grammar. It has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](docs/implementation-status.md).
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Binding Recipe architecture (v3)

Read all four ground-truth component guides before creating or changing components:

- [recipe.md](recipe.md): ordered workflows, variants, actual persisted IBS
  schema, typed input bindings, component assembly and task version selection.
- [skills.md](skills.md): one Tool usage with prose plus a preloadable function interface,
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
  many Skills expose one Tool usage through preloaded, explicitly associated
  Python functions with declared exports and dependencies;
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
  Orchestrator-channel PythonCode invokes a pinned Skill export whose function
  calls host.<tool>(...). Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

The legacy composer still contains source-substitution and historical step-body
paths. Ordinary PostgreSQL/skills-db execution uses immutable retained selections,
typed inputs, checked single-component steps and preloaded exports on the global
Monty service. Full-library migration, general pure approval, dynamic occurrences
and durable recovery remain open; see the current implementation status index.

## Validation scope successor — binding target, implementation pending

Installed-instance authored changes (**A**) receive their own Q1/behavior/human-Q2
approval. A Skill and its implementing PythonCode form one functional review
subject despite separate immutable component identities; unchanged approved
Tools, ToolSkills and helpers are prerequisites, not new review candidates.
Controlled shipped **system_seed** qualification (**S**) retains the exact
Skill-usage combination record. It qualifies the Tool, then its ToolSkill, then
Skill/code together, reusing independently qualified prerequisites. It neither
approves every dependency through a closure list nor adds installed human Q2.
Provenance belongs to the actual revision/usage, not a catalogue-wide flag; mixed
graphs are allowed. Labels cannot select S or overwrite authored overrides.

Compatible successors preserve contracts needed by still-valid consumers for new
use. Unchanged consumers keep their approvals. One existing catalogue owner
publishes coherent selections: new tasks use active approved successors; running,
waiting and resumed tasks retain exact original exports, dependencies and artifacts.
New/changed Recipes validate their supported workflows and variants, without
reapproving constituent components. Shared compatible ToolSkills require unique
technical usage selection, not duplicated primitives or a new approval service.

This target does not weaken existing readers before the connected cutover.
Today's Single-root submissions, exact association approvals and complete-Recipe
deployment remain their actual supported scopes. Missing successor subjects,
approval writers, compatibility readers and migration are open work in the
existing Validator/Composition/Simplified owners. Historical records and formats
are not reinterpreted or rewritten. The detailed contract is in
[skills.md](skills.md#own-functional-approval-and-successor-compatibility).

### Installation seeds and post-installation changes

Code and first-party components developed and shipped with BrassClaw, including
installation seeds, use the controlled `system_seed` bootstrap path. They need
development review, automated structural/behavioral checks and exact package/
artifact integrity, but **no additional external maintainer approval or human
Q2 action on the installed instance**. This applies to bundled Tools, Recipes,
Skills, PythonCode, validators and the global orchestrator root. Bootstrap
qualification must be noncircular and implemented by the installation owner.

Components added or edited on an installed instance use the authored Q1/behavior/
human-Q2 path. Shipping a reviewed successor with an installation upgrade is
distinct from an operator or Sempai edit; it must preserve overrides and retained
old task selections. A `source: system` label cannot turn a post-installation edit
into a bundled seed. Missing bootstrap machinery is implementation work, not a
request for external approval.

### Required authoring and startup checks

1. **Define the usage and workflow separately.** A Skill is one Tool usage:
   prose plus explicitly associated preloaded Python functions. A Recipe orders usages and
   pure-logic components, maps inputs/results and defines completion. A Skill
   never hides a multi-Tool task. Creating a reusable Skill independently is
   allowed; verify its function together with its implementing code through a supported contained review
   or consuming/test workflow. No dedicated per-Skill Recipe is required; qualify
   a separate MCP-call Recipe/variant only for intended MCP exposure.
   A Tool supplies the primitive; a ToolSkill supplies IBS binding metadata.
   The Skill tells the Orchestrator how to use that Tool; its code implements it.
2. **Keep data separate from code.** Declare recursive input/result schemas,
   including list items, object fields, allowed extra values, nullability,
   defaults and numeric bounds. Missing and null are different. Defaults apply
   only to missing consumer inputs, never to invalid/null values or bad outputs.
   Recipe references bind typed data to `inputs["local_name"]`; they never paste
   runtime values into Python source. The retained ordinary path supports typed
   inputs; remaining consumer migration is tracked in the implementation index.
3. **Review meaning before activation.** For authored versions, author review,
   supported Q1 audits, behavioral validation and human Q2 establish agreement
   between prose, binding metadata and code. Trusted bootstrap uses its distinct
   evidence contract in check 4.
   Parsing and matching schemas alone do not prove behavior. IBS checks approved
   structured records; it does not interpret prose or call an LLM to approve a
   Tier-0 task at startup.
4. **Record approval separately from selection.** `skill-association/1` remains
   the technical stable-UUID relationship. A validates the changed component's own
   functions; Skill/code share one review with the unchanged counterpart as context.
   Changed roots keep separate IDs and receive coordinated own approvals; no extra
   authored combination approval or unchanged-consumer Q1/Q2. S retains exact
   `skill-association-approval/1` usage evidence after independently qualified Tool
   and ToolSkill prerequisites. Its closure identifies tested bytes, not a grant
   of all individual approvals. Genuine S uses integrity/Q1/behavior with null
   Q2, without an additional installed human action. Existing authored /1 records
   retain their original exact meaning until the connected successor cutover.
   Neither a task manifest nor caller labels create approval or Tool permission.
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

A **Skill** is one reusable Tool-usage pattern exposed as a **preloadable
function interface**: prose plus explicitly associated PythonCode, exported
names/signatures, helper/constant dependencies and typed invocation/result
contracts. Loading defines functions without executing the usage. The prose
explains the exported commands; associated PythonCode implements them. “Leaf Skill” means this same unit, not a
different kind of Skill. A broader domain or multi-tool overview belongs to an
**Extension**, documented by its ExtensionCatalogue; a **Recipe** defines the
ordered workflow and references reusable components by UUID.

**Tool + ToolSkill belong to the Rust side.** The Tool provides the primitive;
the ToolSkill describes its IBS binding. Binding executes nothing and grants no
permission. The Orchestrator invokes a pinned export from preloaded associated PythonCode,
which calls
`host.<tool>(...)`; the kernel checks the current global tool policy.

**Storage is not the definition:** today Skill prose is stored in `reborn_skills`
(classes 1–3), while executable code is stored separately in `reborn_python_code`
(class 22). The current composer emits `SkillRef.body` and
`ComposedStep.executable_code` separately. The target requires an explicit,
validated UUID/revision association between the two parts; separate rows are
permitted and do not make the Skill prose-only. A code example inside prose is
documentation, not an implicit executable entry point. The class labels
`skill_rusty`, `skill_monty`, `skill_llm` are existing consumer classifications,
not leaf/domain hierarchy levels. Class 10 remains the protected Orchestrator.
Class 50 is a historical Scaffold storage classification, not an active target
component type or protected root. Inventory and migrate actual future-use flows
to normal class-21 Recipes through the existing Composition owner; create a new
Recipe only for a distinct required workflow. Preserve historical bytes and valid
old pins; do not reinterpret a class-50 UUID as class 21 or add a Scaffold validator.

**Execution and validation:** Tier 0 calls the preloaded associated export without an
LLM interpreting prose. Tier 1 can use prose in its explicit LLM steps. Never
execute prose as Python. ToolSkill references belong to Rust binding steps;
class-22 entry points belong to Orchestrator execution steps. Keep unrelated
tasks/attempts isolated and preserve needed typed state within each Recipe,
including child execution and waits. Q1/Q2 and the documented dependent-chain
exception remain applicable. See the actual `knowledge`/`stepnumber` Recipe
schema; explanatory channel names are not insert fields. These definitions are not execution evidence. The retained association/binding
APIs and qualified global path now exist; legacy-row coverage and remaining
qualification are recorded in the implementation status index.

## Binding Tool and ToolSkill definitions (v3)

A **Tool** (class 0) is a registered Rust-side primitive. An Orchestrator call
through the supported `host.<tool>(...)` boundary performs one declared operation
and returns a result, classified failure or explicit wait/handle. A Tool does
not choose the task workflow. An operation selector can expose several primitive
operations; this does not justify hiding a multi-Tool task in new Rust code.

A **ToolSkill** (class 13) is reusable metadata describing a compatible Tool
binding area, potentially shared by several Skills: the Tool identity, supported callable/adapter, parameters,
prerequisites and result/error expectations. It executes nothing, grants no
permission and normally requires no compilation. Several ToolSkills can describe
different bindings of one Tool; several Skills can reuse a compatible binding.
Its stored text or quoted call signature is not executable PythonCode or Skill
instructions for the Orchestrator.

**The Orchestrator needs the Skill to know how to use the Tool.** The Skill's
prose explains that usage and its associated PythonCode defines the preloaded function that implements it. IBS uses
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
> Start a new user-facing capability with a **Recipe design** and reuse existing
> PythonCode, ToolSkills and Skills by UUID. Create only the missing components
> through supported stores/seeders. A reusable Skill may also be created independently
> to grow the library, following skills.md; a reusable ToolSkill can also be
> authored independently and verified with its compatible usage. Adding behavior usually requires no new Rust.
> New Rust is warranted for a missing system primitive or required runtime,
> persistence, binding, lifecycle or kernel support in its owning layer.
> Do not hide a task workflow in a new Rust Tool. The component library is where
> almost all behaviour lives — more Recipes, fewer Rust branches.

### Think before coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

Read actual definitions, selected implementations and affected callers before
depending on their contracts. Resolve technical uncertainty from source first;
use a targeted experiment when necessary. State material assumptions. Ask the user
when a missing decision changes the intended behavior or authorized scope and
cannot be resolved from available context. Continue independent authorized work
while waiting. Choose routine implementation details using the existing contracts.
Explain meaningful tradeoffs and a simpler compatible approach when relevant.

### Simplicity first

Simplicity does not mean reducing Recipe steps. Prefer many purposeful reusable
PythonCode components and explicit result handoffs over a specialized Rust Tool
or a monolithic task-specific Python body. One referenced component per step
permits internal composition under the binding Recipe contract.


Minimum code that solves the problem. Nothing speculative.

No features beyond what was asked.
Avoid speculative abstractions and configurability. Preserve required typed
boundaries, operator settings and failure handling even when they add code.
Simplify unnecessary branching and duplication when doing so helps this change;
line count alone is not a reason to rewrite working code.
For a new capability, look for a Recipe using existing primitives first. Inspect
Rust adapters and stores as needed to verify actual support; reading `crates/`
does not commit the implementation to new Rust.

### Surgical changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

Avoid unrelated cleanup. Update comments and documentation whose meaning changes
with the behavior, including references to moved or renamed code.
Don't refactor things that aren't broken.
Match existing style, even if you'd do it differently.
If you notice unrelated dead code, mention it - don't delete it.
When your changes create orphans:

Remove imports/variables/functions that YOUR changes made unused.
Don't remove pre-existing dead code unless asked.
The test: Every changed line should trace directly to the user's request.

### Goal-driven execution

Define success criteria and the smallest checks that establish them. Complete
connected implementation and self-review before the targeted validation batch,
with earlier checks when a specific uncertainty or shared boundary requires them.
Stop redundant validation once a package's required checks pass; continue the
remaining authorized work until the requested implementation and acceptance are
complete. Follow the development policy for repeat checks and evidence reuse.

Transform tasks into verifiable goals:

"Add validation" → "Trace the caller and invalid-input contract; add focused coverage"
"Fix the bug" → "Diagnose the cause from source and logs; verify the regression at its caller"
"Refactor X" → "Review affected contracts and reuse valid baseline evidence; verify the final change"
"Add capability X" → "Design the Recipe; reuse or create the required Skill/code/bindings and intent examples required by recipe.md; verify the intended variant/tier through the actual runner; qualify changed own A functions/workflows or exact S usage through its trusted bootstrap before activation"
For substantial multi-step tasks, use a brief plan with verifiable outcomes.
Do not create a separate planning document or seek new approval merely to follow
this workflow. A plan can group multiple edits into one connected work package:

1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
Strong success criteria give a clear stopping point. Weak criteria ("make it work")
encourage repeated checks without establishing additional evidence.


**BrassClaw** is a secure, local-first AI assistant built on the IronClaw Reborn
architecture and implemented as a Rust workspace. Model context and output limits
depend on the selected provider/model; they are not one fixed application budget.

## Code Style

> These rules apply to Rust code in `crates/`. Most new capabilities do not require new Rust — see §Orchestrator-First (below) and the Primary Rule at the top of this document.

- Prefer `crate::` for cross-module imports; `super::` is fine in tests and intra-module refs
- No `pub use` re-exports unless exposing to downstream consumers
- No `.unwrap()` or `.expect()` in production code (tests are fine)
- Use `thiserror` for error types in `error.rs`
- Add useful context when mapping errors, preserving the source and classified
  outcome where the error contract requires them. Do not turn cancellation,
  uncertain effects or storage failures into generic success or No-Match.
- Prefer strong types over strings (enums, newtypes)
- Keep functions focused, extract helpers when logic is reused
- Comments for non-obvious logic only
- **Keep new prompt/script embeds in the bootstrap owner.** New `include_str!()`
  constants for behavioral prompts, scripts or runtime reference documents belong
  in `builtin_bootstrap.rs` seed constants or `#[cfg(test)]` modules, as required
  by AGENTS.md. Runtime handlers use the supported stores and retained selections;
  never add an embedded executable fallback. Existing installation-owned guide
  embeds in `prefix_reference_seeds.rs` are an established packaging path, not
  permission to add new embeds outside the allowed locations. Preserve those
  existing embeds and their integrity contract; do not relocate them merely to
  centralize `include_str!()` or add a competing loader. New packaging must satisfy
  both the location restriction and its integrity/upgrade requirements. Compatibility
  prompt rows are seeded through `builtin_bootstrap.rs`; some use boot-time
  `OnceLock` accessors, while preamble/postamble use the Kohai bundle assembler
  described below. Ordinary inline format strings are not executable prompt embeds.
- Keep routine internal/background diagnostics at `debug!`; unsolicited `info!`
  output interferes with the REPL. Use the supported user-status and diagnostic
  paths for information the operator must see. Do not suppress or downgrade a
  real failure merely to keep the terminal quiet; preserve its returned error,
  classified outcome and required durable evidence.
- Test through the caller when a helper gates a side effect through wrappers or computed inputs. Use the smallest real caller-level test that covers the affected inputs and effect; a module test driving the actual caller can suffice. Reuse existing coverage when applicable. Pure helper tests alone do not establish the caller's behavior. See `.claude/rules/testing.md` and the development policy; use real PostgreSQL for PostgreSQL semantics and full E2E only when the claim or acceptance contract requires it.

## Tool Usage Guidelines

### Searching the codebase

Use `rg` for content and `rg --files` for file discovery, or equivalent available
search tools. Read relevant ranges when full output would be truncated. Batch
independent reads/searches; follow dependencies sequentially. Reuse results while
their sources remain current, and expand a search when a shared defect or contract
affects other consumers.


## Architecture

**Component authoring ground truth:** [recipe.md](recipe.md) defines ordered
workflows, typed inputs and the actual persisted IBS schema; [skills.md](skills.md)
defines one Tool usage with prose plus preloaded function exports, recursive contracts,
own functional approval, S exact-usage evidence and retries; [tools.md](tools.md) defines Rust primitives,
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
IBS checks own A approvals or exact S usage evidence and pins one coherent workflow and
complete dependency graph, including actual Tool artifacts. Missing/incompatible
newest active contracts or applicable evidence fail assembly; never silently downgrade or
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

BrassClaw Reborn uses a five-layer model:

1. **Products** — UX surfaces and deployment shapes (CLI, web server, daemon). Products wire together loops, capabilities, and host access. They do not implement agent logic.
2. **Loops and run interfaces** — The execution/lifecycle boundary. Monty/Recipes/PythonCode own task sequencing; Rust supplies admission, transport, authority, accounting and effect custody. Legacy Rust loops are migration inputs, never a fallback for retained execution. Products must not create another workflow loop.
3. **Kernel** — Authority and policy enforcement. Trust decisions, secret resolution, safety policy, sandboxing, capability grants, and session identity live here. Kernel boundaries are enforced; product and loop code cannot override them.
4. **Infrastructure** — Shared services: LLM providers, Postgres persistence, embeddings, skills, extensions, and observability. Lives in `crates/`.
5. **Component Library** — Recipes, Skills, ToolSkills, PythonCode snippets, and ExtensionCatalogues stored in Postgres. **This is where most new capabilities are added.** Existing supported contracts can add component rows without a new primitive; missing schema/runner support still requires implementation. See `builtin_bootstrap.rs` for first-party seeding.

New infrastructure work belongs in `crates/`. Design new *capabilities* in the
Component Library (layer 5) first. Implement demonstrated primitive or
infrastructure gaps in their owning crates, then reuse that support from the
components. The v1 `src/` tree was removed in Phase 6.

### Component Catalog and Class Codes

BrassClaw Reborn stores reusable knowledge artifacts in Postgres tables indexed
by integer **class codes**. Several classes share a table; the mapping is defined
by `class_code_to_table` in `crates/brassclaw_engine/src/memory/retrieval_source.rs`
and mirrored by `PgSettingsListingService` in
`crates/brassclaw_reborn_composition/src/pg_settings_listing.rs`:

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

**V085 migration** adds a nullable `content_checksum TEXT` column to `reborn_skills`, `reborn_tool_skills`, and `reborn_python_code`. For `source='system'` rows seeded by `builtin_bootstrap.rs`, this column holds the SHA-256 hex of the prose field (`body` or `content`). `run_content_integrity_check` (called during shared runtime boot in `component_boot.rs`) verifies these checksums and halts the process on mismatch. Distinct from `content_hash` on `reborn_python_code` (similarity deduplication). Use the supported repair path to restore corrupted system rows after reconciling affected tasks; this content digest is not a complete immutable implementation/association manifest.

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
authored Q1/human Q2 and trusted bootstrap integrity requirements remain,
alongside sandboxing, network/secret enforcement and resource limits.
Existing scoped stores and operation-approval code are legacy implementation
until the coordinated dispatch/data cutover. Do not extend those paths as v3
requirements or disable technical enforcement to bypass them.

Routing uses only the coherent active, approved catalogue. Drafts and
unapproved revisions are excluded before matching. Match and No-Match describe
routing; "unapproved match" and "unsupported match" are not additional routing
outcomes. Complete runner support is required before a workflow is activated.
Missing support is implementation work, and corruption or technical failure in
an active catalogue is an error to repair, never permission to remove that
workflow silently or route it through Tier 2.

Only an actual No-Match enters Tier 2. Matching/DB errors, ambiguity and begun
recipe failures must remain distinct; never replay a failed recipe as Tier 2.
Running tasks retain their selected component revisions. Live tool policy is
checked independently of those fixed component revisions. Monty task time and
allocation budgets are separate from the shared live-heap limit. The target
`max_duration_secs` default is 600 seconds of executing VM time per task,
excluding idle/queue/external waits; it never limits global Monty lifetime.
Shared memory defaults to one startup sizing, then stays constant without
periodic pressure monitoring. Startup/manual budgets have the existing 512 MiB
default as their minimum; low startup capacity readings retain that floor with
visible status. The budget is a ceiling, not reserved memory. WebUI can adjust
larger finite values live. Upgrade raises smaller configured values to this floor
with a new revision and preserves larger values and modes. Automatic adjustment
is optional and slow; mild pressure alone never pauses all admission. Unsafe
manual reductions are rejected; automatic reductions below the live heap remain pending while
safe reclamation and admission backpressure apply. All valid settings changes
are live, with desired/effective state visible.

Token budgets default to disabled (`token_budgets_enabled = false`). When
disabled, retrieval, prior knowledge, history and task consumption have no
artificial token caps, including hardcoded retrieval/assembly constants.
Token accounting remains observability; model context/output limits remain
technical constraints. Time, allocation and memory limits are independent.

**Implementation status (2026-10-23):** ordinary PostgreSQL/skills-db startup
owns one supervised global Monty 1.0 service. Retained typed inputs/preload,
immutable revisions, exact approvals, authored deployment, live Rust/Monty
settings and scoped ordinary/MCP/prefix/native-child paths are implemented.
V176 makes scoped inbound receipt/message acceptance atomic; admission binds its
input in the same PostgreSQL transaction. Explicit children retain parent claims,
original catalogue/IDs and full outcomes without rematching or replay. Complete
raw UTF-8 file capture, independent pure-step coverage rejection and captured
per-step validator observation limits have scoped acceptance. Pure source views
and declarations are prerequisites, not approval; use
[the exact remaining pure-review contract](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract).
General
pure approval, eight validator workflows, five subagent seed migrations, final
prefix/model guards, remaining policy/cap/compatibility migration and durable
recovery remain open. See [the current support and ownership index](docs/implementation-status.md)
and its combined-tree validation record; do not recreate completed infrastructure
or claim full-plan/platform/deployment acceptance.

### Orchestrator-First, LLM-Minimal (Core Design Principle)

**Monty is the task execution engine; the kernel owns authority.** The
Orchestrator sequences Recipe work and runs associated PythonCode; Rust supplies
primitives, VM hosting, transport and kernel enforcement. An LLM supplies
reasoning/content in explicit steps, not execution permission. Deterministic
eligible usages are Tier 0; explicit reasoning/composition and all shell or
spawn_subagent Recipes are Tier 1.

**The Orchestrator and Rust Tools:**
Monty owns task sequencing; Rust Tools perform the operations it calls through
the host boundary, subject to the kernel's current authority checks.

- **Orchestrator (Monty, Python)** owns Recipe sequencing and intermediate state. It runs
  **as one global orchestrator started at system startup and kept alive in the
  background for the instance lifetime**. Each input is a task delivered to
  that existing orchestrator. It reads the
  workflow selected by IBS in a `BuildInstruction`. Composition resolves its
  component references into executable Python steps and prepares Tool bindings
  and typed inputs separately from source. Monty executes those steps in
  sequence: running PythonCode snippets, assembling LLM prompts,
  and posting replies. It never executes Rust directly — it calls registered
  Tools by name via `host.<tool>(...)`.
- **Rust Tools** are registered primitives with selected retained implementations.
  They may use technical service state, operation handles and deduplication;
  they do not own Recipe workflow sequencing or add an agent loop. One declared
  operation returns a result, classified failure or explicit wait/handle.
  **Before writing a new Rust Tool:**
  verify no existing Tool covers the primitive needed. A new Rust Tool is
  incomplete without a ToolSkill + PythonCode snippet + Leaf Skill + Recipe
  created or reused through supported stores/seeders, with compatible binding,
  loading and own A approval or exact S usage evidence. Metadata alone does not establish
  Recipe reachability or Tool permission.

**Tool invocation — first-class callables:** Tools are first-class callables in
the Monty namespace. A PythonCode snippet calls a tool as
`result = host.tool_name(param=value)`. Invoking the binding crosses into Rust,
which checks the kernel boundary, runs the selected Tool and returns its actual
outcome. Verify the actual host adapter and registration path; do not assume
uniform signatures or loading mechanisms. Retired intrinsics remain forbidden.

#### Turn Execution Flow (binding target — read this first)

Successfully routed input takes Match or No-Match within the already-running
global orchestrator. Routing can also fail or require disambiguation; neither
outcome is No-Match. The diagram shows the two successful routing paths, not an
exhaustive error state machine or persisted Recipe JSON. Ordinary startup owns
the global lifecycle; remaining acceptance is listed in the status index.

```
User Input
    │
    ▼
Global Orchestrator (Monty) — already running since system startup
    │
    ▼
Retained intent matching — one coherent catalogue selection
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
    │    channel:"orchestrator"→ host.post_reply(...) → user sees reply          │
    │                                                                            │
    │  History saved. Task completes; global Monty awaits further work.           │
    │                                                                            │
    └─── NO MATCH ───────────────────────────────────────────────────────────────┘
         │
         ▼
     Orchestrator assembles an LLM prompt (Tier 2):
         1. User's question
         2. Conversation history for this input
         3. Kohai formats only the dynamic prompt for its target pair
         4. optional connected/enabled Sempai corrects that formatted prompt
            using its own generated prefix/design; returns final dynamic prompt
         5. Kohai inserts the intended finished prefix unchanged exactly once
            BEFORE that prompt (base if none specified), then final request guard
            provider/model serving still needs qualification
         │
         ▼
     LLM reasons over the full prompt → answer posted to user
         │
         ▼
     Eligible completed-turn review may propose Recipe/intent changes →
     required review/behavior/human Q2 and coherent activation →
     later matches may select the approved Recipe at its verified tier
```

**Key points for authoring:**

- Matched workflow sequencing belongs to Recipes/PythonCode. Reuse existing
  primitives; implement demonstrated infrastructure gaps in their owning layer.
- IBS is the compiler: it reads the Recipe's `step_descriptions` JSONB once at match time and produces the `BuildInstruction` that drives execution. It is ephemeral — never stored.
- The diagram's channel labels are explanatory. Persist `knowledge:"rust"`
  for a ToolSkill binding and `knowledge:"orchestrator"` for its matching
  executable component, following recipe.md. Pure steps need no artificial
  binding; the documented direct dependent-chain exception remains applicable.
- Kohai assembles the selected prefix/context through the supported component
  contracts. Catalogue growth does not imply unlimited active prompt capacity.
- Tier 0 has no LLM step in its execution; Tier 1 follows its explicit reviewed
  workflow; Tier 2 is reserved for actual No-Match. Post-turn review is separate.

> **Monty owns Recipe data flow:** IBS compiles the Recipe into a BuildInstruction;
> Monty executes its steps and retains the intermediate values needed by later steps.
> Fresh empty state per PythonCode step is not mandatory. Child tasks run through
> supported admission on the same global service with explicit typed inputs/results
> and retained selections; do not create a child VM to pass intermediate data.
> Unrelated tasks and attempts remain isolated. Do not merge separate
> Recipe steps or independent Rust primitives to work around missing state handoff.
> Only combine into one Rust tool if
> the operations are genuinely inseparable at the system level (e.g. an atomic DB transaction
> that cannot be split). Convenience and data flow alone do not justify a new monolithic tool.

#### One global orchestrator, bounded tasks (binding target architecture)

**Exactly one global Monty orchestrator starts during system startup and runs
in the background until the BrassClaw instance shuts down.** It is the Monty
VM executing the Python orchestrator body, hosted by Rust in the supervised
worker process. The process boundary contains fatal native runtime failures. Startup completes migrations, component seeding and
integrity verification, then loads and starts Monty before enabling turn
workers, trigger producers or ingress. Readiness means that the global VM is
alive and waiting for work, not merely that its driver was constructed.

Every admitted input becomes a task of that existing orchestrator. Finishing a
reply stores the task history and ends that task; **it does not end or recreate
the global VM**. When idle, Monty awaits work without busy polling or LLM calls.
Only instance shutdown or supervised fatal-runtime recovery replaces the VM.

Global lifetime does not imply shared task context: each work item retains its
conversation, run and exact input-message identity, history, continuations,
reply target and signals. The kernel checks current global Tool policy. Tool bindings and task-local
state are released at task completion. Approval/auth/child-run waits must not
block the events needed to resume them or unrelated admitted work. Rust hosts
the VM, transports work and enforces kernel boundaries; Python/Recipes sequence
the work. A Rust queue consumer must not become a second recipe/agent loop.

**Current wiring:** the ordinary `skills-db` runtime starts and retains
`GlobalMontyOwner` before turn workers and trigger producers. `GlobalMontyDriver`
delivers admitted tasks to that service; it does not create a per-chat VM or run a
Rust-loop fallback. Installation-owned reply/history workflows are qualified from
exact packaged documents and retained native implementations. Authored activation and acknowledged live resource/settings publication exist;
remaining full-library, authority, platform and recovery cutover is governed by `simplified_v3.md`
Phase 3a and its acceptance gates. Older per-chat lifecycle descriptions are
historical, not an alternative execution architecture.

The target separates Recipe/PythonCode task sequencing from Rust infrastructure.
Rust owns startup, admission, transport, authority, accounting and effect custody;
native matching and remaining compatibility consumers also exist. Do not describe
all post-admission behavior as already migrated to Recipes. Change supported
workflow behavior through components first, and repair missing store, binding,
host or continuation support in its owning layer. The status index records which
paths have actually been implemented and qualified.

#### Phase 1 — boot once, receive work (runtime infrastructure)

Monty starts once at system startup after the verified component library is
available. On each admitted input the already-running orchestrator receives a
work item, establishes its explicit task context and **starts the
intent-matching-system**. Starting a task never starts another global
orchestrator. The boot/receive mechanism is the built-in exception, not a Recipe.

#### Phase 2 — intent match (recipe-driven in principle; Rust today)

Reuse the ordinary retained matcher and coherent catalogue selection in the
existing global path. `resolve_intent` / `fetch_for_turn` also appear in legacy
compatibility consumers; inspect the actual caller before changing routing.
Any future Recipe-owned matching sequence must use supported primitives on the
same global service, not a second VM. Matching must retain the exact Recipe,
variant and catalogue selection. Only actual No-Match enters Tier 2; ambiguity,
database errors and unsupported or corrupt active selections remain distinct
errors and must not be converted into No-Match.

#### Phase 3 — dispatch

**Case 1 — Match → Matching-Mode.** The active task receives a component-id
and switches into Matching-Mode:

1. The id is sent to the **IBS** via the composition system.
2. IBS reads the Recipe's `step_link` + `StepDescriptions` JSONB and assembles
   a `BuildInstruction { rust_steps, orchestrator_steps }` by collecting the
   referenced ToolSkill and PythonCode snippet UUIDs from the component library.
   This is an intermediate structure. Composition resolves the exact selected
   component bodies into concrete Python execution steps; runtime values remain
   separate typed input data, never substituted Python source.
3. The composition system uses `rust_steps` to **bind** the declared ToolSkills
   into the Monty namespace (making `host.<tool>` callable for this turn).
4. The Orchestrator runs `orchestrator_steps` in sequence: each step is either
   a PythonCode snippet (which calls `host.<tool>(...)`), a Skill prose supplied to explicit Tier1 LLM
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

1. Eligible history for this exact conversation/input.
2. The user's question.
3. The specific prefix selected by Recipe, Orchestrator or role, or compatible
   base-prompt when no specific prefix is selected. This default is not Tier-2-only.
   Missing/incompatible explicit selection blocks the call instead of substituting
   base. Shared source knowledge is not compulsory whole-base inheritance.

Do not assume a fixed prefix size or claim low latency from precompilation alone.
Large-prefix generation, serving and cache/quality acceptance remain owned by the
prefix plan. Disabled discretionary token budgets do not disable technical limits
or authorize silently truncating required content.

The global orchestrator posts the task's LLM answer into its originating chat, then saves a
**thorough history** so the **kohai/sempai system can build new intents,
skills, recipes, tools and other components**, so future matches can use an
approved Recipe. Deterministic work can become Tier 0; reasoning/composition
that still needs an LLM remains Tier 1. Qualified MCP discovery/commands already enter ordinary chats and use the
same matching/Recipe path; broader library qualification remains open. MCP
does not expose Python, raw Tools or a direct Monty/IBS execution port.

This is **Tier 2**: the supported non-match workflow uses retained instructions
and host boundaries, not an unrestricted alternate agent loop. Reuse supported
component contracts for prompt additions and prefixes. Missing transport, binding
or runtime support remains implementation work; changing a Recipe alone does
not implement those contracts.

#### Prompt assembly ownership (binding target)

Recipes/PythonCode own the sequence of context acquisition and prompt composition
for matched, non-match, validator and Kohai/Sempai work. Rust supplies the supported
model transport, native accounting and effect custody. This ownership target does
not establish that all compatibility consumers have already migrated.

Use the [shared pair-design/selection contract](docs/reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).
The generator finishes and immutably stores each profile's prefix already formatted
for its concrete provider/model pair. Generator and Kohai use the same stored,
accepted design revision for that pair; Sempai also uses that revision when the
same pair is its provider/model. Role content rules and generated prefixes remain
separate.
Orchestrator prompt → Kohai formats only the dynamic prompt for the target pair
→ optional connected Sempai optimizes/corrects that already formatted prompt and
returns the final dynamic prompt → Kohai inserts the selected finished prefix
unchanged exactly once before it → full actual-request guard → model.
There is no second Kohai formatting stage after Sempai's return.
The final join uses Sempai's returned dynamic prompt, not the original or the
pre-correction version. Without Sempai it uses Kohai's initially formatted prompt.
Sempai's own generated prefix remains part of its separate review request.
The [explicit two-request handoff](docs/reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
distinguishes the formatted final-target prompt from Sempai's own model request.
The fixed final prefix/design/limits are not editable review output. Sempai's own
call uses its pair-specific reviewer generation. No whole-prefix runtime rendering,
independent mutable limits or provider host/template/service changes. All views
edit the same canonical provider policy through its existing endpoint/query;
historical immutable pins remain labelled evidence. Use typed inputs, never source
substitution. Final technical limits apply even with discretionary budgeting off.
This is a binding target, not proof that existing compatibility callers implement it.

The formatting part of a model design must contain the actual
[construction instructions](docs/reborn/model-token-limits.md#formatting-design-must-contain-construction-instructions):
provider-exposed defaults plus researched exact model/template/backend evidence,
pinned sources and concrete prefix/prompt/join rules validated through the selected
adapters. Cache documentation guides only evidenced supported construction choices;
never guess deployment precision/layout or copy irrelevant cache manuals into each
role prefix. Store and consume the resulting pair design, not just notes or labels.

#### Tool Binding — how Tools become callable

A Recipe references one ToolSkill UUID in each Rust binding component step.
IBS/composition must resolve the approved exact binding and Tool implementation,
verify the associated Skill/PythonCode contracts, and prepare the supported
callable before the matching executable step. It executes nothing at binding.

Built-in and extension registration sources can differ, but both require the
actual compatible host adapter and selected implementation to be available.
A seeded descriptor, returned rust_directive or tool name proves neither dynamic
loading nor successful binding. Verify the production caller; current wiring
does not establish all final-v3 binding requirements.

The Skill tells the Orchestrator how to use the Tool: prose explains the usage,
associated PythonCode implements it. The ToolSkill is Rust-side metadata telling
IBS how to prepare that binding; it is neither Skill prose nor executable code.
Several usages may share a compatible descriptor. At completion/cancellation,
release task-local binding references and transient state through supported
cleanup without removing resources needed by other tasks/checkpoints or ending
the global VM. Do not require globally registered Tools to be unloaded per task.

#### Runtime authority — enforced before every Tool dispatch

The same kernel boundary applies in Tier 0, Tier 1 and Tier 2, including
validated Recipes, component creation, validation and Kohai/Sempai workflows.

- ToolSkill binding makes a callable available; it executes nothing and grants
  no permission. Q1/Q2 approve components, not Tool invocations.
- Before **every actual Tool dispatch**, the kernel checks the current global
  instance-wide allow/block policy and technical parameters. A task's pinned
  component revision does not freeze permission. Blocking a Tool also blocks
  its next call in an already running or resumed Recipe, including retries.
  Apply that decision to every retained version/capability ID/callable alias
  mapped to the same Tool UUID; missing/conflicting mappings fail closed.
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

Recipes have human-readable explanations and machine-readable ordered component
references, variant selection and typed input/result bindings. The selected
immutable workflow defines execution; prose labels are not executable logic.
Repeated tasks may return different data because inputs or external state change.
Reuse and edit Recipes/components to change supported behavior; add Rust only
for missing primitives or required infrastructure support.

**Component reuse hierarchy — always work from the top down, stop at the first level that solves the problem:**

| Level | Action | Cost |
|-------|--------|------|
| 1 | Add intent examples to an existing `RecipeVariant` | Zero new components — just new rows in `reborn_intent_inputs` |
| 2 | Add a new `RecipeVariant` to an existing `Recipe` | One new variant + intent examples; reuses existing PythonCode snippets and ToolSkills |
| 3 | Reference existing PythonCode in Recipe `step_descriptions`, selected by the variant's `step_link` | No new PythonCode row needed |
| 4 | Reference an existing ToolSkill in a step with persisted `knowledge:"rust"` | No new ToolSkill row needed |
| 5 | Create only missing PythonCode/ToolSkill/Skill/Recipe rows through supported stores/seeders | Reuse compatible components; no new primitive unless needed; store/runner gaps remain implementation work |
| 6 | Write a new Rust Tool + full set of components | Only when no existing Tool provides the primitive |

**Check the component library before authoring a new component.** Reuse supported
usages and workflows; ordinary documentation or infrastructure edits do not require
a fresh inventory of the entire library.

This is a reuse order, not permission to mutate approved versions. Changes use
reviewed immutable successors and coherent activation; running tasks retain their
original selections. Infrastructure gaps are repaired in their owning layer.

**Required authoring rules** (verify actual Q1/runtime enforcement):

- **One Skill per required Tool-usage pattern:** reuse a compatible existing
  Skill first; create a distinct usage when its contract or behavior requires it.
  Do not hide unrelated usages in one monolithic Skill or author every imaginable
  Tool pattern merely because it might be useful later.
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

Authoring ground truth: [recipe.md](recipe.md), [skills.md](skills.md),
[tools.md](tools.md) and [toolskills.md](toolskills.md).
The built-in archive, tomedo reference and Zencoder plan are historical/worked
examples; verify their schema, bindings and signatures before reuse. They do not
supersede the ground-truth input, approval, version or Tool-policy contracts.

### Recipe tiers and approval lifecycle

Reusable Recipes can avoid repeated LLM reasoning for eligible deterministic
work. Tier-1 execution and optional post-turn review can still consume model
tokens. Do not infer total task cost or latency from a tier label alone.

The three tiers map directly to what happens on a given turn:

| Tier | Required execution behavior | LLM use during that execution |
|------|-----------------------------|-------------------------------|
| **0** | Matched, qualified deterministic Recipe with no LLM step; shell and spawn_subagent are excluded | None |
| **1** | Matched Recipe requiring explicit reasoning/composition, shell or spawn_subagent | As required by its supported, reviewed workflow |
| **2** | Actual No-Match enters the supported non-match workflow | Model reasoning with the assembled context |

`llm_call_required` is a declaration to validate against the actual workflow, not
proof of the tier or permission to omit a required LLM/confirmation step.

**Why Tier 2 matters:** When no approved Recipe matches, the model reasons through
the admitted input. Completed-turn review may propose reusable components or
intent examples; existing proposal infrastructure includes `PgSempaiProposalSink`.
Recipe-owned Sempai semantics and complete validator routes remain work tracked
by the status index and `path_to_v3.5.md`. A proposal or `validated` label alone
does not activate a workflow. Changed authored components need own trusted Q1,
behavioral evidence, human Q2 and coherent activation before later tasks select
them. First-party bootstrap follows its separate contract below.

**Why pre-seeded extensions exist:** Trusted first-party bootstrap supplies
validated components through a distinct integrity-checked system path, without
waiting for Tier-2 discovery. Setting `source: "system"` on an authored proposal
cannot bypass Q2. Seeded deterministic Recipes may be Tier 0; seeded Recipes
requiring LLM work, shell or spawn_subagent remain Tier 1. Authored updates and
operator overrides follow their required Q1/human-Q2 approval path. Pre-seeding
encodes reusable usage and workflow knowledge; it does not grant Tool permission.

**Sempai proposals can grow the library.** Novel combinations and reusable
patterns must still pass the applicable review and activation path. Missing
primitives or runtime contracts require implementation, not a new approval label.

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

### Consumer-Tag Gating (§3.9)

This describes compatibility consumers, not the v3 instance-token authority model.
Remaining migration is owned by simplified v3; never extend these as new grants.

Components carry `consumer_tags[]` that control which agent roles may access them. The `sender_class_code` on a turn maps to a consumer tag; `PostgresSource` enforces a `SEC-01` validation gate — only `Validated` components are returned. Actions (class **16**) are exempt from the prior-knowledge token budget. (Class 11 is unallocated — see the class code table above.)

### Legacy validation queue labels (§3.5.1)

These stores remain in use. The complete class-specific review workflows and
standalone pure approval are still owned by validator_v3.md. Historical body
checks below do not replace retained preload/export validation.
The labels do not define the v3 check set: Q1 is the required automated review
workflow, with LLM work only where its supported checks require it. A queue
transition or clean LLM response alone does not establish the required own functional Q1.

| Queue | Code | Meaning |
|-------|------|---------|
| Q1 | `auto` | Auto-extracted; awaiting LLM audit |
| Q2 | `manual` | Operator review required (`05:validator` tag present) |
| Q3 | `revision` | Automated revision by class-09 extension |
| Q4 | `rejection` | Rejected; retained for `q4_retention_days` then wiped |

The Q4 retention description is legacy queue behavior, not authorization to delete
LLM history, immutable reviews, selected revisions or artifacts needed by tasks.
Preserve that evidence under the binding retention/recovery contracts when migrating
or repairing queue consumers.

Current legacy state transitions are enforced by `is_valid_transition` in
`brassclaw_product_workflow::recipes`; its class-10/50 clean-audit rule is an
implementation observation, not a future Scaffold validator. The open class-50
retirement closes new authoring/import/approval/publication paths while preserving
historical decoding and valid old task selections. Class-10 protection remains.

**Recovery from Q4 rejection:** Read the actual audit output and identify the
failed contract. Historical checks for top-level `result =` assignments do not
define the retained preload interface: use declared exports and typed returns.
Create a corrected successor and resubmit through the supported path; do not
mutate an approved original. Missing audit evidence is incomplete, not proof of
a specific candidate defect. **Do not rewrite a capability as Rust merely because
a Recipe was rejected.** Common failures include:

- Tool usage missing its required `host.<tool>(...)` call; pure-logic
  PythonCode may make zero calls; retained preloads return through a declared export
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

This compatibility retrieval helper is not the retained ordinary matcher.
Only actual No-Match may enter Tier 2; errors remain errors on the global path.

`PostgresSource::fetch_for_turn` in `retrieval_source.rs` replaces the old "load all docs" path:
1. Calls `resolve_intent` with the user query
2. On `Match`: fetches the exact component by ID from the appropriate class table
3. On `Disambiguation`: returns `FetchForTurnResult::Disambiguation(candidates)` to the orchestrator
4. On `NoMatch` / error: falls back to UNION ALL keyword retrieval (DB-less helpers in `retrieval_dbless.rs`)

### Monty VM Settings (§3.10)

**Runtime baseline:** ordinary startup uses the supervised Monty 1.0 worker.
Phase 3a governs remaining lifecycle/resource/recovery acceptance; do not repeat
the completed version migration. The v0.0.16 custom-tracker proof is historical
and cannot implement the current API. Monty 1.0 removes the allocation-count limit;
preserve existing settings until their explicit migration rather than silently
ignoring them. Rust and Monty must share the effective duration revision and
one task compute account; a persisted WebUI edit alone is not runtime uptake.


`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (originally V034). `max_duration_secs` bounds one task's executing VM time, never global orchestrator uptime or idle wait. The supervised Monty 1.0 worker and Rust share the effective task-settings revision and retained compute account. V100 preserves removed allocation-count settings as retired evidence; it does not substitute an ineffective allocation limit. Shared heap, task compute, stdout/value limits and token accounting have distinct meanings. Production startup and live publication use the persisted instance settings, not a DB-less duration fallback. Remaining control limits and acceptance are recorded in `docs/plans/simplified-v3-implementation.md`.

### Orchestrator Code Load Path

Ordinary `postgres+skills-db` startup enters `global_monty_startup::start` after component boot. It retains the exact packaged `global_mode.py` revision through `global_root_seed::retain_packaged_global_root`, then verifies the installation catalogue, exact component selections and actual native artifacts before starting `GlobalMontyOwner` and its single supervised worker. Retaining a packaged draft alone is not approval or readiness. The legacy `basic_mode.py` / class-10 `orchestrator:main` / `PgOrchestratorCodePort` path is not the production per-chat execution path. `GlobalMontyDriver` delivers admitted work to the already-running service without creating a VM or falling back to a Rust loop. A live service pins its verified root revision; replacement requires supervised reconciliation. Fatal failure is a service failure, never permission to replay external effects or enter Tier 2. Authored-catalogue activation exists; full-library qualification and durable fatal recovery remain Phase 3a work.

The preamble/postamble (`codeact_preamble`, `codeact_postamble`) are also class-10 rows seeded by `seed_orchestrator()`. They reach the LLM via the Kohai prefix bundle assembled by `do_assemble_bundle` in `interceptor_config_service.rs` — not per-turn by the executor. No `OnceLock` or `init_*` call is needed for them; the bundle assembler queries `reborn_skills` directly.

### Boot Sequence (seeding + integrity)

The seeding boot chain lives in `crates/brassclaw_reborn_composition/src/component_boot.rs`, called by `runtime.rs` after migrations and before turn workers and trigger producers. Required seed/recovery failures abort runtime construction. Every required prompt is loaded before initializing process-wide prompt stores. WebUI construction only attaches its facade and UI services. The ordinary `skills-db` path now starts the singleton owner, completes the actual work-wait handshake and attaches the live settings owner before work producers. Durable fatal continuation/replacement and full catalogue acceptance remain incomplete. Required startup order (see `simplified_v3.md` Phase 3a):

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
  → retain exact packaged global root and verify installation catalogue/artifacts
  → start exactly one global Monty; await its initial work-wait handshake
  → enable turn workers and configured trigger/channel producers
  → expose ready ingress / attach RebornWebuiBundle
```

`BootedDb` (`crates/brassclaw_reborn_composition/src/booted_db.rs`) is a newtype that enforces migration-before-seeding at the type level. `run_content_integrity_check` (`content_integrity.rs`) is **distinct** from `run_boot_integrity_check` (`boot_integrity.rs`) — the former checks SHA-256 prose checksums, the latter re-queues non-validated components.

### `brassclaw repair` Command

`brassclaw repair [--dry-run]` force-reseeds all `source='system'` component rows from compiled-in seed constants using `ON CONFLICT DO UPDATE` (unlike the normal seeder which uses `ON CONFLICT DO NOTHING`). This is the existing mutable-row repair path, not immutable v3 activation. Reconcile affected tasks before mutation and retain artifacts needed by continuations; follow the supported operator recovery path after updates. The `repair_builtin_components()` function lives in `crates/brassclaw_reborn_composition/src/repair.rs`; the CLI entry point in `crates/brassclaw_reborn_cli/src/commands/repair.rs`.

### PKC Formatting Split (§3.13/§3.14)

`format_prior_knowledge_for_llm()` in `orchestrator.rs` produces deterministic JSON from `PriorKnowledgeResult` items: ordered by `(class_code asc, prompt_uid asc)`, `class_code_label()` for string names, NULL fields omitted. The `formatted_content` surface is the only surface sent to the LLM; raw `content` is never sent.

### Interceptor Architecture (§3.15)

The ordinary global path now retains completed-turn review/effect/accounting
custody. Recipe-owned semantic conversion is scheduled in path_to_v3.5.md;
this older loop-host description applies to remaining compatibility consumers.

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

Use the existing async I/O interfaces and runtime adapters. Use `Arc<T>` for
shared ownership when needed; choose synchronization for the actual access pattern.
Do not add an `RwLock` by default or hold a blocking lock across `.await`.

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

Build only from this common repository root and Git index. Use one serial Cargo
execution and the canonical NVMe target below; `brassclaw-target2` and mutable
validation build mirrors are retired. Immutable validation archives preserve
historical evidence and must not become alternative development/build trees.
Before publishing, stage the reviewed common tree, verify it, and use an ordinary
fast-forward push; never copy an old validation snapshot over current source.

> **Mandatory on this host:** use the configured
> `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` (NVMe), supplied by Codex.
> Invoke Cargo directly without an inline target override or shell wrapper.
> Before each compilation, check free space on that volume:
>
> ```bash
> df -h /Users/ollama/brassclaw-target          # check before every compile
> ```
>
> If available space is below 15 GB or usage exceeds 90%, preserve required
> executable evidence and clean disposable build artifacts before compiling.
> Follow AGENTS.md and the development policy; never clean the repository's whole
> `target` directory, which contains retained evidence. Do not clean routinely
> when capacity is sufficient. Other hosts use their configured canonical target.

The commands below are examples to select for the affected claim, not a checklist.
Keep Cargo executions serial under `/private/tmp/brassclaw-target-cargo.lock`.
Use relevant features and targets; passing tests can also establish compilation
for the targets/features they actually built. Required lint and acceptance remain.

```bash
cargo fmt --all -- --check                              # submission formatting
cargo test -p <crate_name> <relevant_test>               # focused behavior
cargo clippy -p <crate_name> --all-targets -- -D warnings # affected-package lint
# Add relevant feature flags; PostgreSQL checks cover database semantics.

# Authorization crate — capability lease contract tests (no DB required)
cargo test -p brassclaw_authorization

# Authorization crate — PostgreSQL integration tests (requires BRASSCLAW_PG_URL)
BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5434/brassclaw \
  cargo test -p brassclaw_authorization --features integration

# Optimized local binary for required release/performance checks
cargo build --release --bin brassclaw

# Run with logging
BRASSCLAW_REBORN_LOG=brassclaw=debug cargo run
```

### Avoid redundant rebuilds

`cargo build` on this workspace is slow. Capture output once and inspect it multiple times — do **not** rerun the build just to see different output:

Use a fresh log path for a new validation batch; `build.log` below is illustrative.
Keep earlier logs needed for evidence or diagnosis instead of overwriting them.

```bash
# Run this as a separate shell invocation; leave the pipeline as its final command.
# pipefail makes a Cargo or log-write failure fail this invocation.
set -o pipefail
cargo build -p brassclaw --bin brassclaw 2>&1 | tee build.log
```

Record that invocation's exit status before inspecting logs. Run log inspection
in a separate invocation so its status cannot replace the build result:

```bash
rg -n 'warning|error' build.log
```

`rg` exits with status 1 when it finds no matching lines; that is not the build
status. Read complete relevant diagnostics from the saved log. Neither finding
error text nor finding none substitutes for the recorded build result.

Before repeating any build, test, check or lint, inspect the captured output and
state what relevant edit or unresolved question requires another run. Reuse
passing evidence for unchanged paths; avoid redundant check/build/test sequences.
See [the development policy](docs/development-policy.md) for the stopping rule,
stable build configuration and caller-level validation requirements.

E2E tests: see `tests/e2e/CLAUDE.md`.

## Creating Releases

### Automated Release Process

Release publication uses GitHub Actions. Do not manually build/upload replacement
release assets. Local builds for development, acceptance or performance are allowed
when required by the development policy.

### How to Create a Release

After required release acceptance and explicit release authorization, push the
approved version tag. These commands illustrate the syntax; use the actual
approved version, not the example version:

```bash
git tag v1.3.0
git push origin v1.3.0
```

GitHub Actions will automatically:

1. Build the application and Monty worker for the configured release targets
   (currently Linux x86_64, macOS ARM64 and macOS x86_64)
2. Generate SHA256 checksums for each binary
3. Create a GitHub release with auto-generated release notes
4. Upload all artifacts (binaries + checksums) to the release

### Monitoring Builds

- **Workflow runs**: https://github.com/chtugha/brassclaw/actions/workflows/release.yml
- **All actions**: https://github.com/chtugha/brassclaw/actions

### Current release targets

- **Linux x86_64**: `x86_64-unknown-linux-gnu` (current release workflow)
- **macOS ARM64**: `aarch64-apple-darwin` (Apple Silicon)
- **macOS x86_64**: `x86_64-apple-darwin` (Intel)

### Release Workflow Details

Use [.github/workflows/release.yml](.github/workflows/release.yml) for current
targets, build commands and artifact publication. The
[archived CI/CD guide](docs/archive/CICD_SETUP_DOCUMENTATION.md) provides historical
background only; verify its commands against the current workflow before reuse.

### Important Notes

- **Never manually build and upload binaries** - always use the automated workflow
- **Tag format**: Use semantic versioning with `v` prefix (e.g., `v1.2.9`, `v1.3.0`)
- **Build time**: Depends on runner capacity and caches; inspect the actual run.
- **Artifacts**: The current matrix publishes the application, Monty worker and
  both checksums for each of three targets (12 files). The workflow is authoritative.

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
│   ├── brassclaw_pg/               # Postgres pool, migration runner, versioned SQL migrations
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

When modifying a module with a spec, read it first and reuse that knowledge while
it remains current. Binding v3 guides and the current implementation index take
precedence over historical subsystem descriptions. The development policy governs
iteration and check selection; required subsystem acceptance remains binding.

| Module | Spec |
|--------|------|
| `crates/brassclaw_reborn_cli/` | `crates/brassclaw_reborn_cli/AGENTS.md` |
| `crates/brassclaw_reborn/` | `crates/brassclaw_reborn/CLAUDE.md` |
| `crates/brassclaw_reborn_composition/` | `crates/brassclaw_reborn_composition/CLAUDE.md` |
| `crates/brassclaw_reborn_config/` | `crates/brassclaw_reborn_config/AGENTS.md` |
| `crates/brassclaw_agent_loop/` | `crates/brassclaw_agent_loop/CLAUDE.md` |
| `crates/brassclaw_llm/` | `crates/brassclaw_llm/CLAUDE.md` |
| `crates/brassclaw_safety/` | `crates/brassclaw_safety/AGENTS.md` |
| `crates/brassclaw_embeddings/` | `crates/brassclaw_embeddings/AGENTS.md` |
| `crates/brassclaw_reborn_webui_ingress/` | `crates/brassclaw_reborn_webui_ingress/CLAUDE.md` |
| `crates/brassclaw_engine/` | `crates/brassclaw_engine/CLAUDE.md` |
| `tests/e2e/` | `tests/e2e/CLAUDE.md` |

## Token Budget

Discretionary token budgets default off. When disabled, do not impose fixed
retrieval, history, assembly or task quotas. Accounting remains enabled; actual
provider/model context and output limits remain technical constraints.
Use the canonical provider/model policy and prefix owner for request sizing;
do not introduce an independent 8,192-token application budget or promise that
historical large-prefix experiments fit the current model. Required final request
guards and remaining consumer migration are tracked in the implementation index.

## Skills

A Skill is one Tool-usage unit: **prose plus an explicitly associated
preloadable function interface**, with declared exports/dependencies and pinned
invocation. [skills.md](skills.md) governs its contracts, association approval
and validation; [recipe.md](recipe.md) governs workflow assembly and execution.
[tools.md](tools.md) defines the primitive and live policy;
[toolskills.md](toolskills.md) defines its IBS binding descriptor.
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
| 50 | Historical Scaffold | Legacy rows in `reborn_skills`; no new active target class or separate validator. |

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

See `.env.example` for documented environment examples. Check the actual config
loader and relevant subsystem guide for supported variables, defaults and precedence.

### Key Reborn Variables

**Process/bootstrap inputs** (read by the relevant startup/configuration code;
these are not all necessarily consumed before the DB starts). Non-secret values
may be set in the service environment; keep secrets in the supported secret path.

| Variable | Default | Purpose |
|----------|---------|---------|
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | Reborn state root |
| `BRASSCLAW_RUNTIME_PROFILE` | Entry-point dependent | Compatibility runtime-policy selector. The ordinary local CLI builder defaults to `full`; serve preserves its supplied policy and uses `local_dev` only when its policy helper has none. Inspect the actual caller before changing defaults. Explicit values include `local_safe`, `local_dev`, `local_yolo` and `full`. It does not select storage. The old `BRASSCLAW_REBORN_PROFILE` is rejected at startup. Do not extend legacy profiles as v3 authorization requirements. |
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

The `brassclaw_reborn_composition` default features are `postgres` and
`root-llm-provider`; `skills-db` additionally selects the ordinary retained
catalogue/global execution path. Inspect the actual product manifest and feature
propagation before choosing validation flags; a package-default build alone does
not prove the product's `skills-db` path was compiled. Do not add
`#[cfg(not(feature = "postgres"))]` production composition/factory fallbacks.

The `RebornCompositionProfile` enum and all composition-level profile selection have been removed. There is no `local_dev` vs `hosted` composition split — Postgres is always the backend. `BRASSCLAW_RUNTIME_PROFILE` controls only the per-invocation capability policy (security resolver), never the storage backend. `BRASSCLAW_REBORN_PROFILE` is a hard startup error.

- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.
- General DB-backed settings use `brassclaw_config`; provider definitions use
  `brassclaw_llm_providers`. Bootstrap environment and secret material remain
  distinct, and specialized revisioned settings have their own existing owners.
  Do not copy them into a competing universal configuration store.

## WebUI v2

WebUI v2 is a React SPA served at `/v2` from the `brassclaw` binary.

- Built with the `webui-v2-beta` cargo feature flag
- Static assets embedded via `crates/brassclaw_webui_v2_static/`
- Server routes and bearer-token auth live in `crates/brassclaw_webui_v2/`
- Gateway adapter in `crates/brassclaw_reborn_webui_ingress/`
- Start with `brassclaw serve` (default: `127.0.0.1:3000`)
- For non-loopback listeners, use `serve --host 0.0.0.0` only with a non-yolo profile; `local-dev-yolo` with `--confirm-host-access` refuses non-loopback binds

## Job State Machine

This is a legacy job-status overview, not the global Monty run/attempt recovery
contract. Moving a job back to `InProgress` does not authorize replaying a Recipe
or completed effect. Use retained claims, effect reconciliation and the supported
continuation path; generic durable recovery remains open in the status index.

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

- Full v3 operator/legacy-authority, product-surface and data cutover is incomplete.
- Immutable component selection and scoped native retention exist; general old
  native-artifact loading across incompatible upgrades and durable recovery remain
  qualification/migration work.
- Native child execution is implemented, but R09 parent bootstrap/activation and
  all five historical subagent Recipe migrations remain open.
- Eight class-specific validator workflows, general standalone pure approval and
  qualified dynamic branch/item occurrences are not complete.
- Prefix source-only compilation/measurement and per-model policy storage/UI exist;
  target-aware serving, final request guards, designs, Sources and quality/cache
  acceptance remain open. Sempai semantic conversion follows `path_to_v3.5.md`.
- Transport/parser and optimistic-conflict policies still include fixed values;
  see `docs/runtime-limits-audit.md`. Do not claim all limits are adjustable.
- Combined local checks are scoped evidence. Full platform/release and fresh
  installed-instance acceptance must be completed before making those claims.
