# Agent Rules

> **Current status — 2026-10-23:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.

## Development workflow and precedence

Read [the development policy](docs/development-policy.md) before implementation.
It owns iteration, bounded work packages, self-review, check selection, evidence
reuse and stopping validation. Its rules supersede older blanket test/lint
instructions here. Required subsystem acceptance, component approval, security
and disk-space rules remain binding. Product Tier-0 rules do not restrict
development reasoning. Reuse current source knowledge and valid evidence;
repeat inspection or checks only when relevant changes or uncertainties require it.


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
Legacy step-body implementations must be migrated to this interface before
being accepted as updated v3 implementations.


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
   runtime values into Python source. The ordinary retained path supports typed
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
> Start a user-facing capability with a **Recipe design** and reuse existing
> PythonCode, Skills and ToolSkills before creating components. This is an
> implementation decision, not a requirement to invent Recipe JSON in every
> conversational reply. New Rust is justified for a missing primitive or required
> runtime, persistence, binding, lifecycle or kernel support in its owning layer.
> Do not hide a task workflow in a Rust Tool. Inspect actual adapters and stores
> to establish whether the proposed Recipe is supported.

## Purpose and Precedence

`AGENTS.md` routes development work; it is not the complete architecture spec.
Read the affected subsystem guide before changing a complex area. Binding v3
component guides take precedence over historical subsystem descriptions; the
status index establishes implemented boundaries, not approval of missing behavior.
Use source inspection and reasoning before compilation. Follow the development
policy's explicit exceptions for targeted experiments and intermediate checks.

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

BrassClaw Reborn is organized in five conceptual layers:

- **Products** own UX and surface-level composition. They wire together loops, capabilities, and host access for a specific deployment shape (CLI, web, daemon). Products do not implement agent logic directly.
- **Loops and run interfaces** define the execution/lifecycle boundary. In the ordinary v3 path, Monty/Recipes/PythonCode own planning and task sequencing; Rust owns admission, transport, authority, accounting and effect custody. Legacy Rust-loop implementations are migration inputs, never a fallback for failed retained execution. Product code must not implement a second workflow loop.
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

## Orchestrator-First, LLM-Minimal Design (Mandatory)

**The orchestrator IS the execution engine. Rust makes tools available. The LLM
is consulted ONLY when a task requires creative reasoning, composition, or
irreversible decisions the Recipe must confirm. Deterministic usages are Tier 0
when eligible; shell and spawn_subagent Recipes are always Tier 1.**

This principle governs all Recipe, Skill, PythonCode, and ToolSkill authoring.

### Global Monty lifecycle (binding target architecture)

**Runtime baseline:** the ordinary global worker uses Monty 1.0. Phase 3a
retains the remaining lifecycle/resource/recovery gates; do not repeat the
completed version migration. The old v0.0.16 custom-tracker proof is historical
and cannot implement the current API. Monty 1.0 removes the allocation-count limit;
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

**Current wiring:** the ordinary `skills-db` runtime starts and retains
`GlobalMontyOwner` before turn workers and trigger producers. `GlobalMontyDriver`
delivers admitted tasks to that service; it does not create a per-chat VM or run a
Rust-loop fallback. Installation-owned reply/history workflows are qualified from
exact packaged documents and retained native implementations. The full authored
catalogue/resource/settings cutover remains governed by `simplified_v3.md`
Phase 3a and its acceptance gates. Older per-chat lifecycle descriptions are
historical, not an alternative execution architecture.

### Turn Execution Flow — binding target

Successfully routed input takes Match or No-Match in the existing global service.
Errors and disambiguation remain distinct; neither is No-Match. The following
diagram explains the two successful routing paths, not persisted Recipe JSON or
an exhaustive failure state machine. See `simplified_v3.md` Phase 3a.

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
    │    channel:"orchestrator" → PythonCode: result = skill_export(...) -> host.<tool>(...)          │
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
         3. Kohai: dynamic-only target formatting → optional Sempai correction
            → final dynamic prompt returned to Kohai
         4. Kohai inserts finished selected prefix unchanged exactly once BEFORE
            that final prompt (base if none specified) → complete-request guard
            actual provider/model limits; serving still needs qualification
         │
         ▼
     LLM answers over the assembled prompt → posted to user
         │
         ▼
     Eligible completed-turn review may propose Recipe/intent changes →
     required review/behavior/human Q2 and coherent activation →
     later matches may select the approved Recipe at its verified tier
```

**Prefix selection/formatting (binding target):** follow the
[shared pair-design contract](docs/reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).
Recipe/Orchestrator/role selects a specific prefix; without one, use base-prompt,
not only in Tier 2. An unavailable explicit prefix or unresolved conflict blocks
the call instead of silently substituting base. Apply these four rules:

1. Generate the finished prefix for the concrete provider/model pair and store
   it immutably, already formatted for that pair.
2. Generator and Kohai use the same stored, accepted design revision of that
   pair. Sempai uses that revision too when the same pair is its provider/model;
   role-specific prefix content remains separate.
3. Kohai first formats only the dynamic Orchestrator prompt for the final target
   pair. A connected/enabled Sempai then optimizes/corrects that formatted prompt,
   using its own prefix generated for its own target-pair design for the review
   call, and returns the final dynamic prompt to Kohai.
4. Kohai then inserts the intended finished prefix unchanged and exactly once
   before that returned final dynamic prompt (or its initially formatted prompt
   when Sempai is disabled). It does not format the return again. Sempai's own
   reviewer prefix belongs only to its separate review call.

The complete-request guard and send follow this join.
Use the [explicit two-request handoff](docs/reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
when implementing the final-target call and Sempai's separate review call.
All mutable limit views use the same provider owner;
no role settings copy or provider configuration/template changes. Historical
placeholder/prewarm descriptions are not current serving qualification.

Pair-design formatting must be a filled [construction specification](docs/reborn/model-token-limits.md#formatting-design-must-contain-construction-instructions)
derived from exposed provider defaults and researched, pinned model/template/cache
documentation, with exact supported prefix/prompt/join rules and verified cases.
A standards label or research link is not enough; unknown effective facts do not
qualify exact serving. Compiler and Kohai consume that same saved specification.

**What this means for authoring:**

- **Matched workflow sequencing belongs to Recipes/PythonCode.** Reuse existing
  primitives and repair demonstrated infrastructure gaps in their owning layer.
- **IBS assembles execution.** It reads the selected Recipe's `step_descriptions`
  and `step_link` to produce an ephemeral `BuildInstruction`. Authors define the
  persisted workflow; runtime integrations use the supported assembly APIs.
- **Composition prepares execution.** The BuildInstruction is intermediate;
  composition resolves its selected component references into executable Python
  steps and prepares Tool bindings. Typed inputs are supplied separately from
  source. Monty executes those composed steps and owns their result handoff.
- **Tool invocation:** bind the ToolSkill, then invoke its associated export in
  the matching PythonCode step. Persist `knowledge:"rust"` / `knowledge:"orchestrator"`;
  the diagram's `channel` labels are explanatory. Pure logic needs no artificial
  binding; retain the documented dependent-chain exception.
- **No-Match uses the supported non-match workflow.** Kohai adds the selected
  prefix/context. Catalogue growth does not imply unlimited prompt capacity.
- **Sempai may propose reusable patterns.** Proposals need the applicable review,
  behavior and activation evidence. Complete validator routes and Recipe-owned
  Sempai semantics remain tracked work; a label or proposal alone enables nothing.

### Component Roles — What Each Type IS

| Component | What it is | What it is NOT |
|-----------|-----------|----------------|
| **Tool** (class 0) | Registered Rust primitive; one declared operation returns a result, classified failure or explicit wait/handle through the host boundary. | A Recipe workflow planner or autonomous agent loop |
| **ToolSkill** (class 13) | Rust-side binding descriptor for IBS: which Tool to bind, param schema, preconditions, error policy. Lives in `reborn_tool_skills`. | Orchestrator instructions; it carries no prose for the Orchestrator, only metadata for IBS |
| **PythonCode** (class 22) | A Python code snippet stored in the component library. The Orchestrator runs it as a step. A tool-calling snippet normally calls `host.<tool>(...)` once; pure logic makes zero calls. Only the documented dependent-chain exception permits additional calls. | An executor — the Orchestrator is the executor; the PythonCode snippet is what gets run |
| **Skill** (class 1–3) | One Tool usage: prose **plus an associated preloadable function interface**, explicit exports/dependencies and pinned invocation (class 22). | A multi-tool domain overview (belongs to an Extension), or a Rust binding descriptor |
| **Recipe** (class 21) | Ordered workflow with variants, typed input/result bindings, component references and completion/failure behavior. IBS assembles its selected immutable version into an ephemeral `BuildInstruction`. | Executable Python source, a Rust agent loop, or permission to invoke Tools |
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

### The two binding/execution channels

```text
knowledge: "rust"           → prepare ToolSkill binding; no execution or permission
knowledge: "orchestrator"   → invoke pinned export; its actual host call checks live policy
```

Every Rust ToolSkill binding step must be immediately followed by its matching
PythonCode step. Pure-logic PythonCode makes zero Tool calls; Tool-calling
PythonCode normally makes one. Only the direct dependent-chain exception below
permits multiple calls in one body; independent calls require separate steps.
This diagram explains execution roles; author complete persisted steps using
recipe.md. Tool dispatch crosses the supported `host.<tool>(...)` boundary.

### Tier Decision Hierarchy

0. **Reuse and support gate:** For a capability, design a Recipe from existing
   usages first. A new Tool requires a genuinely missing primitive plus its
   compatible ToolSkill, Skill/code and exact usage qualification. A capability
   additionally needs its consuming workflow, and MCP exposure its qualified command. Missing
   runner, binding, persistence, lifecycle or kernel support is infrastructure
   work in its owning layer; it is not a reason to hide the workflow in Rust.
1. **Tier 0 first**: Can the task be done deterministically with known inputs? → Author a Tier-0 Recipe with a PythonCode snippet the Orchestrator will run. This is the default target.
2. **Split by variant**: Each variant has one predictable input layout and workflow, with verified intent examples. Compatible variants may share a Recipe. Split incompatible layouts/operations; do not require an LLM merely to select a known deterministic variant.
3. **Tier 1 only when necessary**: Use an LLM for creative composition, input/choice evaluation requiring LLM judgment, or confirmation required by the Recipe. Deterministic input validation, branching and result handoff remain Tier 0. Shell and spawn_subagent Recipes are always Tier 1.
4. **One Skill per required Tool-usage pattern:** reuse compatible existing
   Skills. Create distinct usages when their contracts/behavior require them;
   do not author speculative patterns or hide a multi-Tool task in one Skill.
5. **Entry coverage:** publicly matched Recipes need at least 10 intent examples
   as required by recipe.md, including command/natural-language and per-variant
   positive/negative cases. Private workflows qualify actual trusted typed entry,
   explicit Recipe/variant selection and completion instead; do not invent public
   intents or replies. MCP publication separately qualifies ordinary chat commands.
   Example count alone does not prove routing precision.

### Preloaded PythonCode and typed invocation

```python
# Illustrative class-22 definition, not a ready-to-install component.
# Declare its export, dependencies and recursive contracts in the supported records.
def use_tool(param):
    return host.tool_name(param=param)
```

IBS binds data separately from source. A Recipe execution step invokes the pinned
export using its typed `inputs["local_name"]` bindings; it does not execute a
top-level Tool call while loading definitions. `host.tool_name` above is a
placeholder: resolve the actual registered callable, adapter and retained artifact
before authoring a component. The ordinary retained path supports typed inputs;
do not describe all legacy consumers as migrated.

Host calls are the supported effect boundary. Imports/helpers/constants must be
declared and supported by the selected Monty profile; do not assume arbitrary
CPython standard-library availability. Direct OS, process, file or network access
does not replace the host boundary.

> **Retired intrinsics (never use):** `__execute_action__`, `__execute_code_step__`,
> `__execute_actions_parallel__`, `__check_budget__`, `__emit_event__` — all retired in v3.
> Reject these during authoring/review; do not infer complete Q1 enforcement from
> this instruction. Use the pinned usage export and its supported host callable.
> See `docs/archive/builtin_stuff_v3.md` Step 27 for historical migration context.

Exports return their declared typed result. A legacy top-level `result = ...`
assignment is not a preload interface. Reject prohibited operations such as
`import os`, `import subprocess`, `exec`, `eval` and direct `open`; parsing or a
substring scan alone does not establish effect-free loading or complete Q1.

**Monty-owned Recipe execution context (binding target architecture):** the
matched Recipe goes into IBS, which produces the `BuildInstruction`; the global
Monty Orchestrator executes its steps and owns their execution context. Values
and intermediate results needed by later steps remain available within that
Recipe execution. PythonCode steps do **not** have to start with a fresh empty
state dict `{}`. Do not discard required state between steps or merge otherwise
separate steps merely to work around the current executor's isolation.

Child tasks use supported admission on the same global Monty service with typed
inputs/results, explicit claims and retained selections. Do not create a child VM
or parallel orchestrator to work around missing state handoff.
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
state. Preserve the supported explicit data edges rather than assuming arbitrary
Python globals or another task's mutable state are available.

One PythonCode step makes zero Tool calls for pure logic or normally one for a
Tool usage. The direct dependent-chain exception below remains valid. Never
combine independent Tool dispatches into one PythonCode block, including through
internal code includes. A leaf Skill still describes exactly one Tool usage.

### What Forces Tier 1

- Creative content composition requiring model reasoning; deterministic storage
  of already supplied/validated content does not itself require an LLM.
- Any Recipe using `builtin.shell` or `builtin.spawn_subagent`, including fixed commands.
- Choices inside a supported Recipe that explicitly require model reasoning.
  Routing disambiguation remains its own outcome; it is not permission to replay
  a routing error or failed Recipe through Tier 2.
- Irreversible operations whose Recipe contract requires confirmation. A model
  response is not operator approval; preserve the actual confirmation boundary.
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
- **Rule 2**: If `llm_call_required == false` AND `rust_steps` has tool bindings,
  `orchestrator_steps` must contain their matching executable PythonCode. Reject
  a rust-only Tier-0 Recipe before activation; one unrelated PythonCode UUID is
  not sufficient coverage for every binding.
- **Rule 3**: A Tool-calling PythonCode snippet normally makes one `host.<tool>(...)` call; pure logic makes zero. Multiple **independent** dispatches (tool A and tool B are separately useful, their outputs do not flow directly into each other) require separate PythonCode snippets — one per tool call. Only the direct dependent-chain exception below permits additional calls in one body.

  **Exception — direct dependent chain:** Apply recipe.md's exact exception only
  when one Tool's runtime output directly supplies a dependent Tool operation
  forming the same logical unit. Cover every actual binding and preserve each
  dispatch's live policy, failure and effect custody. Shared local scope alone
  does not qualify independent calls. This exception does not establish a callable
  named `host.run_program` or turn a multi-Tool workflow into a single Skill.
- **Rule 4**: A leaf skill should describe exactly one tool usage pattern. Avoid bundling multiple tool calls or approaches into one skill body.
- **§shell-guard**: Every Recipe using `builtin.shell` is Tier 1 with
  `llm_call_required: true`, including fixed commands. A tier or confidence score
  does not prove command safety or replace kernel policy and technical checks.
- **§spawn_subagent-guard**: Any Recipe referencing `builtin.spawn_subagent` is `llm_call_required: true`. Always.
- **§no-snippet**: Reject inline `snippet` steps before activation. Use `text`
  for annotations with no runtime emission or `component` for a stable component
  reference. Resolve its retained definition/export through supported assembly;
  loading a preload must not invoke its Tool usage.
- **§source-policy**: Review the selected source/dependency closure against the
  supported interpreter and forbidden-operation rules. Reject prohibited execution;
  a substring occurrence in a comment or string alone is not proof of execution,
  and absence of those strings is not proof of safety or complete Q1.
- **§channel-isolation**: A ToolSkill UUID must never appear in `orchestrator_steps`. A Skill UUID must never appear in `rust_steps`. Channels must not overlap.

### Recipe tiers and approval lifecycle

Reusable Recipes can avoid repeated model reasoning for deterministic work.
Tier-1 execution and optional post-turn review can still consume tokens; a tier
label alone does not establish total cost or latency.

These tiers describe execution after successful routing, not routing errors:

| Tier | Required behavior | Model use during execution |
|------|-------------------|----------------------------|
| **0** | Matched, qualified deterministic workflow; no LLM step, shell or spawn_subagent | None |
| **1** | Matched workflow requiring reasoning/composition, shell or spawn_subagent | As required by the supported reviewed workflow |
| **2** | Actual No-Match enters the supported non-match workflow | Reasoning over assembled context |

Validate `llm_call_required` against actual execution; setting a flag does not
prove a tier. Eligible completed-turn review may propose new components/intents.
Authored changed components need own trusted Q1, behavioral evidence, human Q2 and
coherent activation before later tasks can select them. Existing proposal/custody
infrastructure does not establish complete validator or Sempai migration; consult
the implementation index and `path_to_v3.5.md`.

**Pre-seeded extensions skip discovery, not tier requirements.** Trusted
first-party bootstrap may seed validated components through its distinct
integrity-checked system path. Merely setting `source: "system"` is not an
approval bypass. Deterministic seeded Recipes may be Tier 0 from first use;
seeded Recipes requiring LLM steps, shell or spawn_subagent remain Tier 1.
Authored proposals and operator changes retain their required Q1/human-Q2 path.

Sempai proposals can grow the library only through that supported review and
activation path. A proposal or `validated` label alone is not activation.

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
reply Skill/PythonCode usage and compatible binding from the exact qualified
retained successors. The [reply/history review gate](docs/plans/monty.composition.md#replyhistory-migration-review-gate)
and status index record their scoped acceptance. Reuse applicable evidence;
do not repeat their completed migration before each use. New successors still
need exact interface/code/binding/dependency and behavior review before activation.
Preserve one reply/history owner, completed effects and old task selections.
Resolve parameter names from the selected export and adapter; historical
`answer`/`text` examples are not interchangeable contracts. Internal finalization
and history helpers remain private. `builtin.echo` is diagnostic-only.

### Extension Authoring Reference

**Read the four ground-truth guides first.** These extension stacks are supplementary worked/historical examples whose schemas, adapters and approval assumptions must be reconciled before reuse:

- `docs/archive/builtin_stuff_v3.md` — historical list with corrected Skill terminology of all built-in v3 capabilities, tool signatures, and ToolSkill/PythonCode templates
- `tomedo_v3.md` — tomedo EMR integration: full reference implementation of an extension component stack
- `docs/plans/zencoder-extension-plan.md` — **best starting point**: Zencoder REST API extension with complete annotated `step_descriptions` JSONB, intent examples, and bootstrap seeding pattern

After the four guides, consult `docs/plans/zencoder-extension-plan.md` when its extension pattern is relevant. It does not override their binding contracts.

## Adding a New Capability (Start Here)

Choose the first reuse level that supports the required behavior and contracts.
This ends the reuse search, not the overall implementation or acceptance work.
Use reviewed immutable successors for changes; preserve existing task selections.

| Level | What to do | What gets created |
|-------|-----------|-------------------|
| **1** | Add intent examples to an existing `RecipeVariant` | Rows in `reborn_intent_inputs` only — zero new components |
| **2** | Add a new `RecipeVariant` to an existing `Recipe` | One variant + intent examples — reuses existing PythonCode snippets and ToolSkills by UUID |
| **3** | Reference existing PythonCode in Recipe `step_descriptions`, selected by variant `step_link` | No new PythonCode row; use persisted `knowledge:"orchestrator"` |
| **4** | Reference an existing ToolSkill in a `knowledge:"rust"` binding step | No new ToolSkill row; reference its stable UUID |
| **5** | Create only missing components through the applicable packaged-seed or installed-instance authoring path | Packaged first-party successors use controlled seed/integrity qualification; installed-instance changes use immutable authored submissions and Q1/behavior/human Q2 |
| **6** | Write a new Rust Tool, then do level 5 | Only when no existing Tool provides the primitive needed |

Before authoring a new component, inspect the relevant existing library and its
actual consumers. The built-in archive is historical supplementary context, not
an obligatory full-library inventory for every documentation or infrastructure edit.
Do not edit a seeder or label a row `system` to bypass installed-instance authored
review. A supported low-level insert API alone establishes neither trusted seed
provenance nor coherent activation.

**At level 5, design in this order.** Persist dependencies through the supported
authoring/seeding path before the completed dependent Recipe; do not submit an
incomplete Recipe merely because its design was written first:

1. **Recipe** (class 21) — design the workflow, typed input layout and variants
   with their intent examples, `step_link` and optional `variable_patterns`.
   `step_descriptions` belongs to the Recipe; symbolic UUIDs are design notes,
   not insertable production references.
2. **PythonCode** (class 22) — declare preload exports, signatures, dependencies
   and recursive contracts. Loading defines functions without invoking Tools;
   the pinned export returns the typed result. Pure logic makes zero host calls.
3. **ToolSkill** (class 13) — compatible IBS binding metadata, referenced by a
   `knowledge:"rust"` component step before the matching executable component.
4. **Skill** (classes 1–3; also called Leaf Skill) — for a real Tool usage,
   associate its prose with the exact PythonCode interface and binding. Declare
   exports/dependencies and isolate mutable data. Pure logic remains PythonCode;
   do not invent a zero-Tool Skill or artificial ToolSkill for it.
5. **ExtensionCatalogue** (class 23) — update `task_groups[]` if this belongs to an existing domain

Resolve real stable UUIDs before submitting the completed Recipe for review.

**Verify before activation:** intent resolves to the intended variant; exact
authored Q1/behavior/human-Q2 or trusted system-seed bootstrap evidence exists;
the actual runner passes relevant acceptance cases at the declared tier.
Verify zero LLM calls for Tier 0; verify
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
| Skills system | `crates/brassclaw_skills/` owns component/revision/contracts and validation helpers. `db-store` enables DB stores; `v1-types` and `v2-compat` remain migration/legacy bridges, not new-code dependencies. Composition `pg_skill_store.rs` supplies compatibility inserts used by first-party seeders; use the exact retained seed or authored deployment owner for immutable qualification and activation. |
| Security, safety, prompt injection | `crates/brassclaw_safety/` |
| WebUI v2 server (React SPA) | `crates/brassclaw_webui_v2/`, `crates/brassclaw_webui_v2_static/` |
| WebUI ingress / gateway adapter | `crates/brassclaw_reborn_webui_ingress/` |
| Extensions lifecycle | `crates/brassclaw_extensions/` |
| Host runtime shell access | `crates/brassclaw_host_runtime/` (in-kernel capability host + runtime dispatcher; sandboxed subprocess execution via `services/process_executor` and `sandbox_process/`; first-party tools under `first_party_tools/`) |
| Embeddings | `crates/brassclaw_embeddings/` |
| Recipe-Skill-Tool library | Engine `memory/` owns component types, matching, validation and retrieval. Composition `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` contains `PgRecipeStore`, `PgRecipeLibrary` and `PgRecipeStoreFacade`; `crates/brassclaw_turns/src/run_profile/recipe_lookup.rs` defines the lookup trait. Read the actual constructor/caller to distinguish compatibility lookup from retained ordinary selection. Recipe variants select typed workflows through `step_link`; the four ground-truth guides govern authoring. |
| IBS (Instruction-Building-System) | `crates/brassclaw_engine/src/memory/instruction_builder.rs` implements `build_instruction`; `crates/brassclaw_engine/src/types/ibs.rs` defines the assembled types. Assembly reads selected `step_link` and step descriptions into an ephemeral `BuildInstruction`. Retain the task selection through the supported continuation contract, not a second persistent instruction store. |
| Component catalog (class codes 4–23) | `crates/brassclaw_engine/src/memory/retrieval_source.rs` (`PostgresSource`, `fetch_for_turn`, `FetchForTurnResult::SplitResult`/`ActionShortCircuit`, `class_code_to_table` — the single source of truth for class→table dispatch; the full class→table table is in `CLAUDE.md` §Component Catalog and is regression-tested against the code). Tables: `reborn_extensions_unified` (4–9, extension packages) + `reborn_specs/tool_skills/plans/summaries` (12–15) + `reborn_actions` (**16**, not 11 — class 11 is unallocated) + `reborn_docus` (17) + `reborn_lessons/issues/notes` (18–20) + `reborn_recipes` (21) + `reborn_python_code` (22, Phase B) + `reborn_extension_catalogues` (23, Phase C). Classes 10 (Orchestrator) and 50 (Scaffold) are **not** separate tables — they live in `reborn_skills`, filtered by `class_code`, alongside classes 1–3. All components carry `dependency_registry JSONB` (Phase J). `reborn_component_catalog` (`crates/brassclaw_pg/migrations/V084__reborn_component_catalog_view.sql`) is a read-only Postgres VIEW — not a table — that `UNION ALL`s all 14 prompt-bearing class tables (excluding `reborn_tools`, class 0) for ad hoc/Settings-API querying; it applies no per-request scope filtering, callers add their own `WHERE`. |
| Settings API / WebUI catalog tabs | `crates/brassclaw_reborn_composition/src/pg_settings_listing.rs` (`PgSettingsListingService::list`, single parameterised query backing every `GET /api/settings/{type}` tab — Skills, Tool Permissions, Actions, Extensions, Orchestrators, Scaffolds, Recipes, ToolSkills, PythonCode, ExtensionCatalogues; a genuinely missing table fails loud with `SettingsListingError::MissingTable`, never silently empty), `crates/brassclaw_product_workflow/src/settings.rs` + `reborn_services.rs` (`RebornServicesApi` trait methods), `crates/brassclaw_webui_v2/src/{descriptors,handlers,router}.rs` (routes). Frontend: `crates/brassclaw_webui_v2_static/.../settings-schema.js` + `settings-tabs.js` (sidebar sections: Runtime Config / Component Catalog / Security & Governance / Access & Ops) + per-tab `*-tab.js` files. The runtime tool-permission list ("Tool Permissions") is UI-distinct from the class-code Skill/ToolSkill catalog tabs — do not conflate them. Note: the old v1 SKILL.md plugin installer ("Skill Packages") UI was removed; the Skills tab now shows `reborn_skills` DB rows only. |
| Validation queue | `reborn_validation_queue` stores legacy queue labels; these are not the complete v3 review workflow. Validator Steps 1–12 own exact subjects, eight class routes, durable Q1/behavior evidence, human Q2 and activation readers. Use the actual audit result to prepare a corrected successor; do not require a legacy top-level `result =` assignment or rewrite a rejected Recipe as Rust. Trusted system-seed bootstrap needs exact integrity/Q1/behavior evidence; source/status labels do not qualify. Preserve review/history/artifact evidence needed by retained tasks when migrating rejection retention. |
| Builtin bootstrap seeder | `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` (Phase L). Seeds full v3 component stack (Tools + ToolSkills + Skills + PythonCode + Recipes + ExtensionCatalogues) for all 23 first-party tools at boot, if not already present. Idempotent. Orchestrator and system prompt components (`orchestrator:main`, `codeact_preamble`, `codeact_postamble`, `failure_explanation`, `compaction_summarizer_fresh`, `sempai_audit`, `subagent:direction:*`) are seeded here and carry `content_checksum` (SHA-256 hex) for boot-time integrity verification. |
| Seeding boot chain | `crates/brassclaw_reborn_composition/src/component_boot.rs` — shared `seed_*`, queue recovery, integrity checks and prompt initialization, called by `runtime.rs` before workers/trigger producers. Required seeding failures abort boot. WebUI construction no longer supplies runtime prerequisites. Ordinary `skills-db` startup retains one supervised global Monty service before producers; complete catalogue/settings/recovery acceptance remains governed by Phase 3a. |
| Global Orchestrator lifecycle | Ordinary `skills-db` startup already owns `global_monty_startup.rs`, `GlobalMontyOwner` and `GlobalMontyDriver`, before workers/ingress. Reuse that service; do not repeat the completed startup move or restore per-chat drivers. Simplified Phase 3a owns remaining durable recovery, resource and platform acceptance. |
| Content integrity check | `crates/brassclaw_reborn_composition/src/content_integrity.rs` (`run_content_integrity_check`) — SHA-256 prose checksum verification for `source='system'` rows. Called at boot after seeding. Hard error on mismatch. Distinct from `boot_integrity.rs` (Phase N queue-consistency check). |
| `BootedDb` newtype | `crates/brassclaw_reborn_composition/src/booted_db.rs` proves migration completion for its callers. Read each actual seeder/integrity signature; some retained packaging owners accept the existing pool and explicit scope. A pool parameter alone is not proof of startup ordering. |
| `brassclaw repair` command | `crates/brassclaw_reborn_cli/src/commands/repair.rs` — force-reseeds all `source='system'` rows using `ON CONFLICT DO UPDATE`. Current mutable-row repair, not v3 revision activation: reconcile affected tasks before mutation; retain artifacts required by continuations. |
| `OrchestratorCodePort` | Engine compatibility port in `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs`; implementation in composition `pg_orchestrator_code_port.rs`. This class-10 compatibility loader is not the ordinary global startup path; use the global lifecycle owner above. |
| BasicPromptStore / prefix | `pg_basic_prompt_store.rs` is a compatibility prefix surface. Canonical profile/design/generation/model-policy work belongs to `docs/plans/prefix_v3_upgrade.md`; reuse existing immutable owners rather than introducing another prefix store or compiler. Source-only generation is not accepted serving. |
| Intent system | `crates/brassclaw_engine/src/memory/intent_system.rs` (`resolve_intent`, 4-class classifier, `record_disambiguation_choice`), `reborn_intent_inputs` table (V028 + V058 variable-template columns). Intent expressions support `%` slot markers for variable capture (Phase M). |
| Monty VM settings | `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs` (`PgMontyVmSettingsStore`, reads/writes `reborn_monty_vm_settings` V034 migration) |
| User chat preferences | `crates/brassclaw_reborn_composition/src/pg_user_preference_store.rs` (`PgUserPreferenceStore`, `reborn_user_preferences` V035 migration) |
| Legacy MemoryDoc migration (v1→v3) | `crates/brassclaw_reborn_composition/src/component_import.rs` owns import of old MemoryDoc rows into class-specific tables. Do not add new components via `brassclaw_memory_docs`; use the applicable first-party seed or installed-instance immutable authoring/deployment path. Import is migration work, not a new component authoring route. |
| Interceptor / Sempai-Kohai | `crates/brassclaw_interceptor/` and composition `InterceptorConfigService` supply existing review/proposal and configuration infrastructure. Recipe-owned semantics and full review conversion remain owned by `path_to_v3.5.md`; proposal rows do not establish class Q1, human Q2 or activation. |

When a task touches only `crates/` there is no longer a v1 `src/` tree — all v1 code was removed in Phase 6.

## Subagent and Loop Rules

These rules describe BrassClaw product child tasks, not development-agent
delegation. A child's behavior belongs to its selected Recipe on the same global
service. Change the Recipe for workflow behavior; repair demonstrated admission,
transport or runtime defects in their owning Rust layer without adding a new loop.

- Subagent spawn creates and wires child runs only. It must not implement a second agent loop.
- Child sequencing uses Recipes/PythonCode through supported retained admission
  and execution. Preserve parent claims, selected catalogue, typed data and actual
  terminal outcomes; do not route failed retained execution through a legacy loop.
- Host-trusted trigger ingress is sealed by trigger-worker-owned request minting plus private conversation-owned trusted inbound construction.
- Product adapters, product workflow, first-party capabilities, and host-runtime handlers must use untrusted inbound requests and must not mint `TrustedInboundTurnRequest` or call trusted trigger submitter factories.

## Repo-Wide Coding Rules

- No `.unwrap()` or `.expect()` in production code; tests may use them. Follow
  existing fallible initialization/error contracts for compiled-in invariants.
- Fix warnings introduced by the change. Lint affected packages with relevant
  features; broad all-feature/workspace checks belong to required integration,
  CI or release acceptance, not every edit. Report unrelated baseline warnings
  separately without suppression or unrelated cleanup.
- Prefer `crate::` for cross-module imports. `super::` is fine in tests and intra-module refs.
- Use strong types and enums over stringly-typed control flow when the shape is known.
- Use `thiserror` and existing error types. Add context while preserving sources
  and classified outcomes required by the contract; cancellation, uncertain
  effects and storage failures must not become success or No-Match.
- No `pub use` re-exports unless exposing to downstream consumers.
- Comments for non-obvious logic only.
- **New prompt/script/reference `include_str!()` constants** belong in
  `builtin_bootstrap.rs` seed constants or `#[cfg(test)]` modules. Existing
  installation-owned guide embeds in `prefix_reference_seeds.rs` are an
  established packaging path, not permission for new embeds elsewhere. Preserve
  their integrity/upgrade contract rather than relocating qualified sources.
  Runtime handlers use supported stores and retained selections, never embedded
  executable fallbacks. Some compatibility prompts use boot-time `OnceLock`
  accessors; preamble/postamble use the existing Kohai bundle assembler.
- Keep routine internal/background diagnostics at `debug!` to avoid unsolicited
  REPL output. Use supported operator-status and diagnostic paths for significant
  information. Never suppress or downgrade real failures to keep the UI quiet;
  preserve returned errors, classified outcomes and required durable evidence.

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
5. **Do not substitute `LocalFilesystem` for an authority-bearing backend.**
   Preserve the selected backend's atomic/versioned mutation guarantees. Use real
   PostgreSQL for SQL, locking and persistence claims. In-memory fixtures may
   exercise their own supported test path, but do not establish production
   durability or authorize a production in-memory fallback.
6. **`issue` must not return success when zero rows were inserted.** `ON CONFLICT DO NOTHING` silently absorbs duplicate-key conflicts; check `rows_affected == 1` before returning the lease to the caller.

## Testing Rules

- Select the smallest real check that establishes the affected behavior; reuse
  existing applicable coverage. Ordinary documentation needs diff/link/consistency
  review and `git diff --check`, not Cargo solely for prose. Embedded runtime
  documents follow the policy's seed/integrity requirements.
- When a helper gates an effect through wrappers or computed inputs, test the
  actual caller and relevant inputs/effect. A module test driving that caller can
  suffice; helper-only coverage cannot prove the handoff. Use real PostgreSQL for
  PostgreSQL semantics; use E2E when only the full path or required acceptance
  establishes the claim, not merely because the behavior is user-visible.
- Do not add mocks, fake commands or simulated success to avoid real required
  checks. Confirm that intended tests actually ran; zero selected tests, skipped
  database cases and missing prerequisites are unverified, not passed.

## Key Environment Variables

**Process/bootstrap inputs:** read by the relevant startup/configuration path;
not all are necessarily consumed before the DB starts. Non-secret values may be
set in the service environment; secrets use the supported secret path.

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | Reborn state root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | Compatibility runtime-policy selector; defaults depend on the entry point. The ordinary local CLI builder defaults to `full`; serve preserves supplied policy and its missing-policy helper uses `local_dev`. Inspect the actual caller before changing defaults. It does not select storage. The old `BRASSCLAW_REBORN_PROFILE` is rejected at startup. Do not extend legacy profiles as v3 grants. |
| `BRASSCLAW_REBORN_LOG` | Log filter for Reborn runtime (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | External Postgres URL. Optional for single-host local deployments (embedded Postgres is used when absent). Required for all non-local `BRASSCLAW_RUNTIME_PROFILE` values. |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded Postgres port (default: 5434) |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | Override embedded Postgres listen addresses (default: `127.0.0.1`). Set to `0.0.0.0` for LAN access. First-boot only (written to `postgresql.conf` by `initdb`). |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Path to the master-key passphrase file; set only for passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after the DB is up — set in `secrets.env` via `EnvironmentFile=`):

The *names* of these env vars are stored in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted. Includes: `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, trigger auth tokens.

## Build and Test

Build only from this common repository root and Git index. Use one serial Cargo
execution and the canonical NVMe target below; `brassclaw-target2` and mutable
validation build mirrors are retired. Immutable validation archives preserve
historical evidence and must not become alternative development/build trees.
Before publishing, stage the reviewed common tree, verify it, and use an ordinary
fast-forward push; never copy an old validation snapshot over current source.

> **Mandatory on this host:** use the configured
> `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` (NVMe), supplied by Codex.
> Invoke Cargo directly without an inline target override or explicit shell wrapper.
> Check free space before each compilation:
>
> ```bash
> df -h /Users/ollama/brassclaw-target          # check before every compile
> ```
>
> If available space is below 15 GB or usage exceeds 90%, preserve required
> executable evidence and clean disposable build artifacts before compiling.
> Never clean the repository's entire `target` directory: it holds retained
> evidence. Do not clean routinely when capacity is sufficient. Other hosts use
> their configured canonical target. These disk-space thresholds remain mandatory.

Choose commands for the affected claim; the templates below are not a checklist.
Replace angle-bracket placeholders with actual package/test names before execution.
Keep Cargo executions serial under `/private/tmp/brassclaw-target-cargo.lock` and
coordinate relevant source edits during validation. A build lock does not prevent
another chat from changing compiler inputs. Keep features/profile stable where
the claim permits, and reuse a test build's compilation evidence for its actual
targets. Composition package defaults alone do not prove the product's `skills-db`
path was exercised; inspect actual feature propagation before selecting flags.

```text
# Check formatting before submission when Rust is affected
cargo fmt --all -- --check

# Lint affected packages with relevant features; fix newly introduced warnings
cargo clippy -p <crate_name> --all-targets -- -D warnings

# Full lint for CI/release or changes requiring broad acceptance
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Focused behavior through the relevant caller; add actual required feature flags
cargo test -p <crate_name> <test_filter>

# Root-package tests when root behavior is affected
cargo test

# Real PostgreSQL checks for a package exposing this integration feature
cargo test -p <crate_name> --features integration <test_filter>

# Optimized binary only for required release/performance checks
cargo build --release --bin brassclaw
```

Capture output once and keep the invocation's real exit status; do not rerun to
retrieve diagnostics or replace a build result with a log-search result. See
[CLAUDE.md](CLAUDE.md#avoid-redundant-rebuilds) for the separate build/log example.

## Before Finishing

- Confirm whether behavior changes require updates to specs, API docs, or `CHANGELOG.md`.
- Reuse applicable evidence; rerun only for relevant changes, failed checks,
  remaining uncertainty or required acceptance. Stop validation for a completed
  work package, then continue remaining authorized work. Finish the overall task
  only when the requested implementation and acceptance are complete.
- Fix introduced warnings and report unrelated baseline warnings separately.
- Re-check security-sensitive paths when touching auth, secrets, network listeners, sandboxing, or approvals.
- Keep the final diff scoped to the task. Avoid unrelated file churn.
- **Capability check:** workflow behavior belongs in Recipes/PythonCode. Rust
  changes may supply a missing primitive or required infrastructure in its owning
  layer; they must not hide workflow sequencing or create a fallback loop.
- **Component set check:** If a new Rust Tool was written — do its Recipe + PythonCode snippet + ToolSkill + Leaf Skill + ≥10 intent examples all exist through supported stores/first-party or extension seeders? Are compatible registration, own A approvals or exact S usage evidence, and behavioral evidence present? A descriptor or seed row alone does not establish executable availability.
