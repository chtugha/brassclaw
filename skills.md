# Skill definition and authoring instructions

> **Current status — 2026-10-23:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


## Review concerns — resolved in the documentation

The 2026-10-06 review identified three specification issues, now resolved in
this guide. These corrections define the target contract; they do not establish
implemented runtime/store enforcement or completed behavioral acceptance.

The subsequent audit's approval-provenance, Recipe-snapshot and retry-contract
gaps are also resolved below. Exact-version approval evidence is distinct from
task selection; the workflow itself is pinned; retries have explicit counts,
evidence and outcome classifications. These remain target specifications.

1. **Resolved in this guide: separate semantic approval from IBS checks.**
   Section 10 assigns prose/code consistency to author review, Q1/Q2 and
   behavioral validation before activation. Section 11 limits IBS to structured
   contracts, approved associations and selected revisions; it does not interpret
   prose or introduce an LLM call into Tier 0.
2. **Resolved in this guide: recursive schemas and null/default rules.**
   Section 7 now specifies list elements, nested object fields, typed extra
   fields, missing versus null values, defaults and producer/consumer
   compatibility. Recursive validation is implemented in the retained value/input contracts;
   acceptance of an individual usage still requires its actual selected path.
3. **Resolved in this guide: numeric bounds and computed Tool arguments.**
   Section 9 now defines a portable bounded-integer transport profile, checks
   inputs and computed `limit` before dispatch, and specifies oversized-number
   rejection cases. This resolves the documentation issue; actual binding,
   transport and Monty execution support still require implementation evidence.

This guide defines a BrassClaw Reborn v3 **Skill** and the exact procedure for
creating one. MUST means required. A Skill is incomplete if its prose, function exports/signatures, dependency
load order, pinned invocation, executable association, inputs, Tool binding,
or result contract is unresolved. A dedicated Recipe, matching command or MCP
publication is not part of Skill completeness. Behavioral qualification of the
own function with associated code is required before productive use; S additionally retains its exact usage-combination qualification.

Read [AGENTS.md](AGENTS.md), [recipe.md](recipe.md), [tools.md](tools.md),
[toolskills.md](toolskills.md) and [the development policy](docs/development-policy.md).
The binding architecture and operator decisions take precedence over historical examples. This guide
specifies authoring requirements; it does not implement new store fields,
versioning, typed-input transport or validation APIs.

For development, complete one bounded usage change and its affected consumers,
review the connected diff, then run the smallest sufficient validation batch.
Reuse current source knowledge and applicable evidence; do not rebuild or repeat
the full acceptance matrix after each edit. An early targeted experiment is
appropriate when source inspection cannot resolve a blocking uncertainty. Draft
development and local verification are distinct from approval and activation.

## 1. What a Skill is

**A Skill is one reusable Tool-usage pattern exposed as a preloadable Python
function interface: prose plus explicitly associated executable PythonCode,
declared exported names/signatures, typed contracts and dependencies. Loading
its definitions makes the selected functions available for later invocation;
it does not execute the usage or enable a Tool.**

Its prose explains when the usage fits, the exact input and output contracts,
prerequisites, how the Tool is used, and how results and failures are handled.
Its associated PythonCode defines the functions implementing the usage. The
interface declares which names callers may invoke and which helpers/constants
remain internal.
Both parts belong to the same usage unit, even when stored in separate rows.

For example, one file-reading Tool can support distinct Skills for reading a
whole file and reading a specified line range. These usages can reuse the same
primitive, ToolSkill or parameterized PythonCode when their contracts fit.
Changing the purpose does not automatically require a new Rust Tool.

“Leaf Skill” means this same one-usage unit. It does not introduce another
component kind or hierarchy level. Classes 1–3 are existing consumer
classifications, not leaf/domain/scaffold layers. A domain overview belongs in
an Extension, documented by an ExtensionCatalogue. Class 10 is the protected
Orchestrator. Class 50 is historical storage only: future-use workflows belong
to normal Recipes, not a Scaffold validator, root privilege or executable alias.
Preserve old immutable rows and valid task pins through the existing migration owner.

## 2. Place the Skill in the complete architecture

The model is **Rust Tools, many ToolSkills, many Skills, many small PythonCode
components, and Recipes that instruct the orchestrator how to use these
components to fulfill task goals**.

| Component | Responsibility | Does it execute a Tool? |
| --- | --- | --- |
| Tool, class 0 | Rust primitive providing an operation | Only when invoked by Python through the host boundary |
| ToolSkill, class 13 | Rust-side IBS descriptor identifying a Tool usage binding, parameters and technical requirements | No; binding executes nothing and grants no permission |
| Skill, classes 1–3 | One usage unit: prose plus an explicitly associated preloadable function interface | Its pinned exported function implements the usage; prose is never executed |
| PythonCode, class 22 | Small reusable executable building block, Tool-calling or pure logic | A Tool-calling body invokes `host.<tool>(...)` |
| Recipe, class 21 | Task instructions: ordered component references, bindings and result flow | The orchestrator executes its assembled Python steps |
| ExtensionCatalogue, class 23 | Domain overview and Recipe inventory | No |

A ToolSkill describes a binding for IBS. A Skill describes a purpose and usage
for the orchestrator. They are distinct, even if they refer to the same Tool.
ToolSkill metadata is not a substitute for Skill prose. Skill prose does not
install a callable or grant permission.

Pure-logic PythonCode components are useful independent building blocks. They
do not each need to be called a Skill: this Skill definition concerns one Tool
usage. Recipes can reference pure-logic components alongside Skill executables.

### What the Skill must leave to a Recipe

A Skill MUST NOT own a whole task's multi-Tool sequence. Discovery, read,
transform, write, final reply and completion are distinct operations to connect
in a Recipe. Do not put “first use Tool A, then use Tool B” into one leaf Skill.

A Skill may state a prerequisite such as “a known, validated path is required.”
It may identify related usages for authors. It must not implicitly dispatch
another Tool or rely on prose causing the orchestrator to discover missing
inputs. The Recipe explicitly supplies prerequisites and later actions.

## 3. How IBS and the orchestrator use it

1. Intent matching selects a Recipe variant and its input layout.
2. IBS/composition selects the Recipe steps and reads their referenced
   components and internal dependencies.
3. At task start IBS resolves the newest activated, approved versions from a
   consistent catalogue snapshot and pins UUID/version/checksum references in
   BuildInstruction. This includes the matched Recipe revision, selected variant,
   exact `step_link` and input layout, Skill prose, its associated PythonCode,
   ToolSkills, Tool implementations and nested PythonCode components.
4. The composition system prepares executable Python, Tool bindings and any
   explicit reasoning context needed by the Recipe.
5. Composition preloads the selected function definitions and pure dependencies;
   the orchestrator invokes the selected export with typed step-local inputs.
   The kernel checks current
   global Tool policy and technical rules before every actual dispatch.
6. Monty preserves typed inputs/results in the exact task execution context,
   including child execution and waits. Other tasks and attempts stay isolated.

**Tier 0:** reference the associated class-22 PythonCode for execution. An LLM
does not read Skill prose to invent a call. The orchestrator invokes a preloaded,
pinned export. A Python example inside the prose does not become an executable
entry point.

**Tier 1:** the Recipe may explicitly provide Skill prose to its reasoning or
composition step. Subsequent Tool calls still use validated bindings and the
supported execution path. Do not infer an LLM call merely from a prose step or
an annotation. The actual runner must implement it. Recipes using `builtin.shell`
or `builtin.spawn_subagent` are always Tier 1, including fixed commands and
approved seeds. Deterministic validation or passing typed data alone does not
require an LLM.

Every Recipe component step references **exactly one component UUID**. A
ToolSkill binding step is followed by its matching PythonCode invocation step.
Preloading occurs in assembly preparation and adds no effectful Recipe step.
Skill prose used as explicit Tier-1 context has its own component step. It
must not be combined with PythonCode in a multi-component `include` list.

PythonCode may internally compose smaller PythonCode components. That internal
graph is separate from the Recipe step's one reference. IBS validates and pins
the entire graph, including its order, input contracts and symbol dependencies.
Internal composition must not hide independent Tool dispatches.

## Preloadable function interface (binding v3 target)

This is the binding Skill interface. Selected retained storage, typed-input,
preload and invocation support is implemented; this does not certify every
usage, legacy row or recovery path. Use the current implementation status index
for remaining work and qualify the selected caller. Legacy step-body executors
remain migration inputs, not an alternative v3 interface. An include here means
qualified component assembly, not Python filesystem imports or arbitrary `exec`.

### Exports and invocation contract

Every Skill MUST declare its public function names, exact signatures, parameter
and return contracts, associated PythonCode UUIDs and the one Tool usage each
entry implements. Names are valid nonreserved Python identifiers, not identity.
Use explicit keyword parameters or a single declared `inputs` mapping; do not
accept arbitrary `*args`/`**kwargs` as an untyped dispatch interface. Multiple
exports are permitted only for compatible entry forms of the same one-Tool
usage; independent Tool usages belong to separate Skills and Recipe steps.
Helpers and constants are declared separately and are not public MCP commands.
Pure PythonCode can export reusable helpers without becoming a zero-Tool Skill.

`skill-association/1` identifies one entry-point code UUID and one input/result/
failure contract, not a map of independent overloads. Multiple export forms of
that association must obey that same usage contract. Different contracts require
separately represented/reviewed usages or reusable wrappers; do not add invented
per-export fields to the association. The selected code's `input_contract` and
`result_contract` must match its association's `inputs` and `result` records.
The Skill interface, code's preload declaration and Recipe invocation selection
must also agree on public name, implementation symbol and parameter mapping;
reject mismatches rather than reconstructing an interface from prose.

The immutable Skill interface declaration belongs to the Skill revision and
its content checksum. It specifies public name -> associated code UUID and
implementation symbol, signature, input/result/failure contracts, dependencies,
ToolSkill/Tool association and any fixed/computed arguments. These are required
logical records, **not new fields accepted by current INSERT APIs**. Implement
supported retained storage/reader/validator contracts before activation. Keep
`skill-association/1` and `skill-association-approval/1` exact formats unchanged;
include the complete interface declaration in the checksummed Skill content and
review evidence. No unrecognized fields are appended to those JSON formats.

Recipes continue to reference one stable component UUID per component step.
For a Skill usage, the class-22 reference plus the selected Skill interface
resolves the invocation export and typed local input layout. A component with
multiple exports needs an explicit supported retained invocation selection;
never infer an export from prose, array order or a mutable name. If that
selection cannot be represented by the current runner, it is implementation
work: do not add invented export fields to persisted Recipe JSON.

Resolve the usage association and invocation export separately. The current
retained binding resolver requires exactly one selected Skill association for
the PythonCode/ToolSkill/Tool/callable tuple. Multiple matching Skill records in
the selected graph are an assembly ambiguity, even if their names or schemas
agree. Do not choose the first row or invent a tie-breaker; retain the intended
usage closure or implement explicit qualified selection in the owning path.
Reuse elsewhere in the library is allowed and does not require retaining all
unrelated usages in every task. Do not remove a required usage merely to resolve
ambiguity. Reuse one association when the usage contract is identical;
otherwise qualify explicit usage selection in the existing owner when a real
shared-code profile requires it. Do not copy code solely to evade ambiguity.
The present tuple lookup has no export-name key; distinct exports of the same
code UUID alone do not make identical tuples unambiguous. The
[dependency directions and canonical Recipe links](recipe.md#dependency-edges-and-canonical-recipe-links)
keep those relationships distinct from execution dependencies.

### Selection, dependencies and loading order

1. Select one coherent activated, approved catalogue snapshot at task start.
   Pin exact Skill/interface/code/dependency/association revisions/checksums,
   exported-symbol resolution, approved combinations and Tool implementations.
   No version numbers are authored into Recipes.
2. Resolve the complete declared PythonCode dependency graph by stable UUID.
   Validate classes, signatures, symbol references, qualified imports and
   recursive contracts. Reject undeclared dependencies, missing/incompatible
   exports, dependency cycles and name conflicts before any task effect.
3. Load in deterministic dependency-first topological order. Break ties by
   stable UUID within the already selected graph, not mutable display names.
   Define pure helper functions/constants before their dependent entry points.
   Deduplicate only identical retained code artifacts with compatible symbol
   environments; sharing source bytes does not share task-bound globals.
4. Preload only qualified definitions and immutable constants. No Tool calls,
   model calls, I/O, task scheduling or effectful initializers/default expressions
   run during loading. Validate default expressions and allowed module loading;
   do not trust a file merely because its top-level statement is `def`.
5. Install a collision-free task-local export table mapping each public name to
   exact owning Skill, code revision/checksum, implementation symbol and binding
   context. Name resolution is infrastructure, not `getattr`/`eval` generated
   from runtime data. The loader must not overwrite an old task's export when
   another task selects a newer revision.
6. An invocation calls that retained export with validated typed data and captures
   its returned value as the step result. A function uses local variables and
   `return`; the invocation boundary assigns `result = selected_export(...)`.
   Preloading the definition and invoking it are separate operations. Independent
   calls retain their separate Recipe execution occurrences and bindings.

The global service may retain/cache several approved immutable code generations.
Availability of cached library code does not expose every export to every task.
Each task uses its declared pinned closure and isolated symbol/data context.
Loading an activated successor prepares future tasks; running/suspended tasks,
child invocations, retries and resumes keep their original export resolution.
No latest lookup or rebinding through the same function/host name is permitted
mid-task. Retain old artifacts while task/checkpoint references need them.

### Tool availability and authority

**Preloaded code does not automatically enable a Tool.** The matching ToolSkill
must prepare the exact compatible retained host binding for the invocation.
In a Recipe, its Rust binding step immediately precedes the matching class-22
invocation step; loading function definitions is not that binding. Every actual
`host.<tool>(...)` dispatch checks current global Tool policy, cancellation,
attempt freshness and technical auth/sandbox/network/secret/resource limits.
Neither an exported name, loaded function nor approval grants permission.
Host-function aliases/proxies must resolve to the task's retained implementation,
while its live policy remains current; activating a successor must not redirect old
functions to a newer handler. All actual calls inside helpers count toward the
usage's declared dispatch/effect contract. Keep independent usages separate;
only the existing qualified direct dependent-chain exception remains available.

### Shared code, constants and isolated mutable state

Reusable code and genuinely immutable constants may be shared. Scalars and
recursively immutable tuples are suitable constants; a mutable list/dict/set
is not made immutable by an uppercase name. Arrays, buffers and working objects
are allocated inside each invocation/task or supplied as task-owned typed data.
Copy/validate mutable defaults for every invocation. Forbid shared mutable
function defaults, captured task objects, caches, global result arrays and
closures over another task's inputs or host binding. Helpers inherit the owning
task/attempt binding context, never an ambient global authority object.

Within a task, pass results explicitly along its typed Recipe edges. Between
concurrent tasks, child tasks and attempts, preserve the documented isolation
and explicit continuation contract; invocation-local temporaries are fresh.
Durable effects/counts belong to the recovery owner, not a shared Python list.
A same-task reusable buffer is allowed only when its lifetime/ownership is
explicitly retained and cannot cross an attempt or child boundary implicitly.
Sharing compiled code is an optimization; if safe shared execution environments
are unsupported, load isolated function environments in the one global service.
Do not introduce a per-task VM or shared mutable namespace as a shortcut.

### Independent usage, qualification and deployment

A Skill is reusable independently of any particular consuming Recipe. Its local
function contract does not contain user-message captures or a mandatory Recipe
backlink. A consuming Recipe supplies its own typed input sources and sequencing.
A test/reference Recipe may establish behavioral evidence, but is not an owning
component or a required permanent companion. Review can itself run as a validator
Recipe; this does not require the subject Skill to own an invocation Recipe.

Keep own A function evidence, exact S usage-combination evidence, consuming-
workflow evidence and MCP command qualification distinct. A Q1/behavior/human Q2
and verified S integrity/behavior requirements apply to their actual subjects. Reuse unchanged evidence only for its exact
covered revisions and claims; usage approval alone does not approve a new workflow.

Current ordinary deployment publishes complete qualified Recipe graphs. A Skill
can be submitted/retained independently; deploying it through a consuming Recipe
does not require an additional per-Skill or MCP Recipe. Independent publication of
unused library components is not established by this contract and must not be
claimed as implemented or supplied through a second catalogue/activation store.

### MCP and acceptance

Only a usage selected for MCP exposure needs a qualified MCP-call Recipe or
equivalent existing Recipe variant. That publication supplies an unambiguous
matching command and typed input/result contract, then binds the matching
ToolSkill and invokes the preloaded export. No MCP declaration is required for a
complete Skill or its use in another qualified Recipe.
MCP tools/list is derived from available activated, approved mcp-call-skill-recipes,
resolving their exact associated Skill interfaces; raw Skill presence is not a
listing entry. The MCP server is always running while BrassClaw is running.
Kohai establishes/advertises its provider connection only after final prefix
addition and immediately before sending the model request; on receipt of the
complete answer it disconnects that request's connection while the server stays
alive. Keep streaming/tool exchanges and concurrent requests correctly owned.
Rebuild the list at startup/restart and refresh it on qualified Skill/Recipe
activation/catalogue changes. Unapproved additions are not advertised; existing
advertised contracts and already selected chat tasks retain their identities.

The existing optional `mcp_call` declaration belongs to the immutable Recipe
revision (`mcp-call-skill-recipe/1`); it is not a required Skill field. Absence means
no MCP listing, not an invalid Skill or an exclusion from ordinary Recipe reuse.
An explicitly declared invalid contract still fails its owning publication checks.
Changing/removing that declaration uses normal revision/review/catalogue publication;
there is no implied independent mutable exposure switch. Independent consumers
remain valid only while their own exact approvals, selection and live policy do.
Already captured tasks retain their selection; a withdrawn/changed command before
task capture must fail rather than execute another contract or enter Tier 2.

MCP tools/list supplies the exact command sentence, variable positions/types,
formatting rules and examples. The server accepts only the completed listed
command, creates a new normal chat and sends it as one user message. The existing
global orchestrator receives it through ordinary chat ingress, matches the
execution Recipe first, executes it and posts the normal result into that chat.
MCP reads the correlated terminal chat response, forwards it to the client and
closes the chat using the existing supported lifecycle interface. It has no
direct Monty/IBS/Rust Tool execution connection and accepts no executable Python
payload. A declared source-inspection input may contain Python text as string
data, including quotes/newlines/marker text, if its exact listed command grammar
permits it. This does not execute the text or introduce universal escaping.
Reject unsupported raw values before lossy conversion; do not blacklist genuine
strings merely because they resemble an upstream repr/depth-limit placeholder.
Reuse equivalent Recipes; private/protected Skills are not automatically exposed.
An ordinary invocation creates no component and repeats no Q1, behavioral review
or human Q2. Authoring/changing the execution Recipe remains separately reviewed.
Existing input/result/kernel checks and retained workflow/effect state remain
owned by the normal execution path. Chat closure must not delete evidence,
discard pending runs or cause a command to be resent after a lost reply.
No MCP-specific execution architecture or normal-chat routing change is required.

Before activation, demonstrate effect-free preload, deterministic graph loading,
export collisions/cycles/missing dependencies, fixed-argument enforcement, exact
old/new version coexistence, live policy changes after preload, fresh mutable
arguments/defaults across repeated and concurrent calls, child/wait/resume
isolation and durable no-replay recovery. Reuse applicable evidence or establish
missing coverage through the exact selected Monty and qualified retained host path. Shared loader/recovery acceptance
belongs to its owning implementation milestone; each new usage still needs its
exact required evidence, without repeating unrelated infrastructure acceptance.
Upstream supports Python
functions but not arbitrary package imports; it does not supply this BrassClaw
export loader or isolation contract automatically. See [Monty language support](https://pydantic.dev/docs/monty/limitations/language/)
and [host-value/proxy behavior](https://pydantic.dev/docs/monty/limitations/host-values/).

## 4. Decide whether a new Skill is needed

Inspect relevant existing component rows, `builtin_bootstrap.rs` and the affected
extension/host seeders. Use [the historical built-in inventory](docs/archive/builtin_stuff_v3.md)
as a supplementary lead, not current activation evidence. Compare actual contracts,
not only names. Reuse a current inspection; do not inventory the entire library
again for every small edit.

Use this decision order:

1. The same Tool usage already exists: reuse its Skill and associated code.
2. Only the user's wording differs: update the appropriate Recipe's intent
   examples rather than creating a duplicate Skill.
3. The task order differs: compose another Recipe using the existing usages.
4. The same Tool has a distinct purpose or parameter/result approach: author a
   new Skill, reusing compatible PythonCode and ToolSkill references.
5. The usage needs a missing small Python operation: author a reusable
   PythonCode component, not a whole task-specific program.
6. Only if no existing Tool provides the primitive, author a new Rust Tool
   through the explicit build/validation/registration workflow in recipe.md.

Do not create a Rust Tool because a Recipe needs many steps. Do not merge
distinct usages into a large Skill to reduce the number of components. Missing
runner, binding, persistence, lifecycle or kernel support may require Rust changes
in those owning layers; this does not justify a new workflow-specific Tool.

## 5. Define the usage contract before writing the prose

Record the applicable contract below, referencing exact existing contracts rather
than copying them. Use an explicit justified “not applicable” where appropriate
and identify unresolved support. Guessed or unresolved execution requirements
prevent activation; a draft may record them as remaining work.

```text
Stable Skill UUID (existing or allocated through the supported creation path):
Optional consuming/test Recipe references and evidence (not ownership):
Optional MCP publication reference (command/layout belongs to that Recipe):
Name:
Class/consumer classification and reason:
One-sentence purpose:
When this usage applies:
Related cases requiring a different Skill or Recipe:
Existing Tool UUID, registered callable name and implementation contract:
Required ToolSkill UUID and binding contract:
Associated PythonCode UUID and local input/result contract:
Public export names/signatures and implementation symbols:
Declared helper/constant dependencies and deterministic preload order:
Pinned export resolution and mutable-state ownership:
Required inputs, types, validation and source in the consuming Recipe:
Optional inputs, defaults and normalization rules:
Prerequisites, authentication and technical limits:
Exactly one Tool usage and its arguments:
Returned data, error signals and completion meaning:
Effect type, idempotency and bounded retry behavior:
Tier restrictions for consuming Recipes:
Internal PythonCode references, if any:
Association/compatibility evidence:
Acceptance cases and expected Tool arguments/results:
Missing runtime/store support:
```

Names and descriptions MUST match the current store/validator constraints.
Prefer the existing `skill-<tool>-<purpose>` naming style, but do not invent
length limits or assume a name is an identity. Names are labels; UUIDs identify
components. Record selected version numbers in the task manifest, not Recipe
references.

## 6. Write the prose using this exact structure

The following is the authoring template for a Skill's prose body:

```text
Purpose
Describe exactly one Tool usage and its intended result.

Use when
State the conditions that make this usage appropriate.

Required components
Identify the Tool, ToolSkill and associated PythonCode by stable UUID and name.
State required internal code dependencies separately.

Inputs
List each local input name, type, meaning, required/default behavior and validation.
Distinguish a local component input from the user-message capture name.

Prerequisites
List required prepared inputs, external authentication and technical constraints.
The consuming Recipe is responsible for supplying them.

Exports and preload
Declare the public name/signature and associated implementation symbol.
List helper/constant dependencies, dependency-first order and exact pinning.
Loading defines functions only; mutable state is invocation/task-owned.

Execution
Explain this usage's argument mapping and the exported function to invoke.
Its associated implementation calls host.<tool>(...) only during invocation.
The matching ToolSkill binding and live kernel checks remain required.

Result
Describe the actual returned type/fields and what constitutes success.
Explain what a consuming Recipe may use next, without invoking another Tool.

Failures
Describe missing/invalid input, denied policy, auth errors, Tool errors and bad results.
Define stop/retry behavior and idempotency constraints; do not claim success on error.

Limits and tier
Document effect type, supported limits and conditions requiring Tier 1.

Optional consumers and qualification references
Identify existing consuming/test workflows when useful; their absence does not
make this Skill incomplete. Keep command/capture/reply contracts on the consuming
Recipe. Link a separately qualified MCP-call Recipe only when exposure is intended;
MCP receives its completed command, never Python. These are navigation/evidence
links, not reverse execution dependencies.

Examples and acceptance
Give representative input/result examples and failure cases.
Any code printed here is documentation, not an implicit executable association.
```

Write concrete instructions. Do not say “use the appropriate Tool,” “handle
errors normally,” “get the required data,” or “call other Skills as necessary.”
Specify what is required. Keep secrets, claim tokens and real private payloads
out of prose and examples. A usage contract does not waive kernel enforcement.

## 7. Associate the executable PythonCode explicitly

The cross-guide [ownership table](recipe.md#ownership-across-the-four-guides)
identifies which records supply workflow, preload, binding and native authority.
A complete Skill requires a validated relationship to its executable code.
Store that relationship through the supported association mechanism. At minimum
the authoring record identifies the Skill UUID, PythonCode UUID, relevant
ToolSkill/Tool UUIDs and input/result compatibility. Runtime assembly records
their selected immutable versions and verifies the association at those versions.

### Exact association record: target format `skill-association/1`

Use this exact format through the retained association contract and supported
review/selection owners. It is not a field accepted by the legacy `NewPgSkill`
constructor. Verify the selected store/reader/validator path rather than
reinventing implemented association support or claiming all callers enforce it.

| Field | Required value/rule |
| --- | --- |
| `format` | Exactly `skill-association/1` |
| `skill_uuid` | Stable UUID of a class-1/2/3 Skill |
| `python_code_uuid` | Stable UUID of its class-22 entry point |
| `tool_skill_uuid` | Stable UUID of the class-13 binding descriptor |
| `tool_uuid` | Stable UUID of the class-0 primitive |
| `callable` | Exact registered `host.<name>` identifier; no expression |
| `inputs` | Object keyed by unique step-local names matching `[a-z][a-z0-9_]*` |
| `arguments` | Object mapping actual Tool parameter names to declared local input names |
| `code_arguments` | Object declaring remaining Tool arguments computed or fixed by approved code |
| `result` | Explicit successful-result contract |
| `failure` | Explicit stop/retry contract; not a claim that error handling is already wired |

The record has exactly these fields; reject unknown fields or duplicate JSON
keys. The four role UUIDs must be distinct, valid, non-nil and resolve to the
indicated classes.
Identity references omit component versions. The approved Skill version owns
an immutable association record; IBS resolves and records exact selected
versions/checksums in the task manifest, including internal code dependencies.

### Own functional approval and successor compatibility

This is the binding successor target for authored changes (A), not a claim that
current Single-root/association/deployment APIs already implement it.

- Every changed authored revision needs complete own Q1, observed behavior and
  human Q2. A Skill and its associated implementation are one functional subject:
  check prose, actual exports, inputs/results, error/effect handling and the use
  of already approved dependencies together. Retain separate physical component
  IDs. If both change, coordinate their own approvals atomically; if one changes,
  the unchanged approved counterpart is context and is not approved again.
  Standalone pure PythonCode retains its own route. Reuse class-22 subchecks,
  not a second full validation of the same Skill implementation.
- The existing review/submission owner must define a strict successor subject
  with exact changed roots and partner context, per-root base/version CAS,
  complete rule coverage, actual run/attempt/interpreter custody and exact human
  view. One logical review uses existing durable work/outbox, verdict and Q2
  mechanisms. Partial results cannot approve a subset of a coordinated change;
  an uncertain commit reconciles original IDs/bytes. No second queue or API is
  presumed available, and an approved pair is not permanent exact-version coupling.
- A successor preserves every still-needed public contract of valid consumers
  intended for new use, including transitive exports, inputs, strict result
  shapes, semantics, effects and retry guarantees. Inputs may not narrow and
  outputs may not lose guarantees. Missing/null/defaults differ. Adding a required
  input or a field rejected by an old strict result consumer is not automatically
  compatible. Preserve the old interface or use an explicitly distinct interface/
  identity with properly reviewed changed consumers. Drafts do not indefinitely
  block successors; only retained historical tasks keep their original graph.
- Test old consumer fixtures as evidence for the successor, without creating
  new consumer revisions or Q1/Q2. Trace requirements through the existing
  revision/selection stores, not a new consumer registry. Declared contracts and
  meaningful behavior/semantic evidence are required; parsing and finite examples
  cannot prove arbitrary Python equivalence. Reused facts retain their exact
  rule/source scope; behavior also retains input, artifact, subject and run custody.
- The existing catalogue owner checks all changed providers against needed
  contracts and concurrent consumer changes in one coherent generation/CAS.
  Compatible A dependency replacements need no new combination approval and
  unchanged consumers retain their approvals. Technical assembly remains strict;
  absence of a supported reader is implementation work, never approval bypass.
  New tasks resolve active approved successors; old task/child/wait/resume selections
  remain exact, with actual artifacts and live global policy checks.
- New or changed Recipes validate their complete supported variants/workflows,
  typed handoffs, effects, failures and completion. This includes real calls to
  approved exports, not renewed component approval. Cover finite declared variants
  and relevant boundaries/relations; do not enumerate infinite values or every
  historical version product. MCP publication has its separate command contract.

Provenance is determined per actual revision/usage. An authored Skill may use
valid shipped Tool/ToolSkill prerequisites without changing their S provenance.
An S exact record does not transfer to an unreviewed mixed selection; compatible
schemas alone do not transfer S evidence. Shipped upgrades preserve operator
changes. No operator mode flag turns an installed edit into a seed.

For S, the installation owner qualifies prerequisite Tool/native implementation,
then ToolSkill binding, then Skill/implementing code together, followed by
consuming workflows in dependency order. Use trusted native tests/binding adapters,
not the future dependent Skill to approve its own prerequisites. Existing valid
prerequisites are reused. Exact S usage evidence covers the tested Skill usage;
its dependency list is not individual approval of every member. Missing separate
prerequisite evidence is a targeted installation/review-owner successor, not a
new seed database, fake Skill or rewrite of existing evidence. S remains outside
the normal authored Q1 queue, with development review/integrity/behavior and no
additional installed or external-maintainer Q2. `q2_ref=null` is correct only for
verified S provenance.

Existing `skill-association/1` technical records and the exact `/1` approval format
below keep their meanings. Existing authored exact approvals remain usable within
their original profiles. A needs a deliberate strict successor in the current
review owner, with all prepare/activation/task/child/restore/private/prefix/MCP
readers connected before new-profile publication. No blanket reapproval of old
consumers, invented historical individual coverage or shutdown of valid old
profiles. General independent authored publication is still an open owning-plan
cutover, not supplied by this guide change.

### Exact-version association approval evidence

This unchanged format is the current retained exact-review contract, continues
for S, and preserves existing authored exact approvals. Its authored combination
requirement describes the existing profile, not the successor A function contract
above. The stable-UUID association expresses identity and contracts, not approval.
Keep a separate immutable **`skill-association-approval/1` record** for that
combination, committed by the trusted validation/approval mechanism. This is
the format consumed by the retained association review path, not an invocation
approval lease or a field on `NewPgSkill`. Parsing a self-authored record or
setting an `approved` label establishes neither trusted provenance nor human Q2.

The record has exactly these fields; reject duplicate/unknown keys:

| Field | Exact rule |
| --- | --- |
| `format` | Exactly `skill-association-approval/1` |
| `approval_id` | Valid non-nil UUID identifying this immutable approval record |
| `association_checksum` | SHA-256 of the exact immutable stored association bytes reviewed, as 64 lowercase hexadecimal characters |
| `components` | Nonempty list of unique `{uuid, class_code, version, checksum}` references covering Skill, PythonCode, ToolSkill, Tool and the complete transitive dependency graph |
| `validation_mode` | Exactly `authored` or `system_seed` |
| `q1_ref` | Nonempty durable reference to the successful required automated validation/integrity evidence for this combination |
| `q2_ref` | Nonempty durable reference to human Q2 for `authored`; null for the distinct integrity-checked `system_seed` bootstrap path |
| `behavioral_refs` | Nonempty list of durable references to the required observed behavioral acceptance evidence for this combination |

Each component reference has exactly the four indicated fields. `uuid` is valid
and non-nil; `class_code` is its actual integer class; `version` is a positive
integer (never a boolean); `checksum` is 64 lowercase hexadecimal SHA-256
characters identifying the approved immutable content/implementation artifact.
There is exactly one selected revision per UUID in a combination. The checksum
contract must cover execution-relevant metadata and dependencies, not merely
prose while allowing mutable argument schemas or Tool artifacts. Native
artifact/ABI retention and verification follow tools.md; a component
metadata checksum alone does not prove the executable bytes are retained or loaded.

The Skill reference identifies the owner of the association; the remaining
references must match its UUID relationships and the approved internal graph.
Evidence must resolve to trusted successful records for exactly this combination;
an identifier string alone is insufficient. For `authored`, human Q2 reviews
the combination and behavioral evidence. `system_seed` is restricted to the
existing trusted first-party bootstrap/integrity path; authored changes cannot
claim that mode to evade Q2. Inherited dependencies do not need new individual
approval merely because they are reused, but the new combination needs its
required association review and evidence.

At task start, IBS verifies that the selected revisions/checksums exactly match
a committed valid approval record, then pins its `approval_id` with the selected
association and component manifest. A manifest records **what was selected**;
the approval record establishes **what was reviewed**. Neither grants Tool
permission. Missing/mismatched evidence fails assembly before effects; matching
types or similar prose cannot substitute for it.

A changed component, dependency graph or association requires a new approval
record covering the resulting combination before that combination is activated,
even when its declared contracts remain compatible **in S and the existing
legacy authored exact profile**. The new A successor instead uses own functional
approval and proven compatibility as specified above. Unchanged Skill prose does
not need an artificial new version solely to record a new dependency approval;
the separate record identifies the reused Skill revision and new code revision.
Activate related changes coherently. Old records and exact implementation
artifacts remain available for tasks retaining the old combination; replacement
alone does not require their approval to be repeated. Recipes and the reusable
association continue to reference stable UUIDs without version numbers.

### Recursive value schemas

Every input, result, computed argument, object field and list element uses the
same finite recursive value schema. Inline child schemas are required; schema
references, expressions and inferred shapes are unsupported. Reject unknown
schema fields, duplicate keys at any nesting level and cyclic/non-data values.
Configured depth/size limits remain technical constraints, never silent
truncation of schemas or data.

| Schema field | Exact rule |
| --- | --- |
| `type` | Required: `string`, `integer`, `number`, `boolean`, `null`, `list` or `object` |
| `nullable` | Optional boolean, default false; permits null in addition to the declared non-null type |
| `checks` | Ordered declarative constraints, default `[]`; explicitly required for top-level inputs and computed arguments |
| `items` | Required only for `list`: one value schema applied to every element, including elements of an empty list's declared type |
| `fields` | Required only for `object`: object mapping nonempty field names to field schemas; `{}` is permitted |
| `allow_extra_fields` | Required only for `object`: boolean |
| `extra_fields` | Required exactly when `allow_extra_fields: true`: one value schema applied to every undeclared field value; forbidden otherwise |

A list has one homogeneous element contract, not a tuple or an untyped bag.
Nested lists/objects repeat these rules at every level. Object keys are strings.
With `allow_extra_fields: false`, an undeclared key is an error even if its value
is null. With true, extra values must validate against `extra_fields`; allowing
extra keys never grants arbitrary untyped data or permission to access an
undeclared field without a presence check.

An `integer` is a mathematical integer excluding booleans; `number` accepts
finite numeric values, including integers, but never booleans, NaN or infinity.
Validation does not round, parse strings or otherwise coerce types. Transport
and Tool bounds must also be declared and checked, as in section 9.
`type: null` accepts only null and MUST omit `nullable` (a redundant flag is
rejected). Other types accept null only with `nullable: true`. On an allowed
null, skip that node's non-null checks and children; null is not an empty string,
empty list or empty object.

`checks` is metadata, not Python source. Supported constraints are
`{kind: "min_length", value: N}` for strings (Unicode code-point count, N a
nonnegative integer), and `{kind: "min"|"max", value: N}` for numeric types
(N finite and an integer for an integer schema). Reject constraints on the
wrong type, unknown kinds and contradictory bounds. `max_field` has the form
`{kind: "max_field", input: "other_input"}` and is allowed only on a top-level
numeric input. Both that input and its target must be declared numeric top-level
inputs guaranteed present and non-null after binding. Validate all inputs/defaults
first, then perform these cross-input comparisons; schema iteration order must
not affect the result. Nested/relative cross-field expressions are unsupported.

### Presence, null and defaults

Top-level input schemas and object field schemas additionally have a required
boolean `required`. Other nodes (result roots, computed-argument roots, list
items and `extra_fields`) have no `required` or `default`: a concrete value is
already present there. Computed-argument roots additionally carry `depends_on`
and `meaning` as specified below.

- `required: true` demands key presence, and forbids `default`. A present null
  still has to satisfy the node's nullability; required does not mean non-null.
- A top-level input with `required: false` MUST declare `default`. An optional
  nested input field may declare one, or remain absent when omitted. Code may
  not unconditionally access an optional field that can remain absent.
- For input binding only, apply defaults to missing keys, recursively within
  present objects or a default object. Never replace an explicitly supplied
  null or an invalid value with a default. A missing parent is not manufactured
  merely because a child has a default.
- Every default must validate against its entire recursive schema, including
  child requirements and numeric bounds. Null is a valid default only for
  `type: null` or a nullable schema. Reject invalid defaults during authoring;
  they must also participate in cross-input checks at binding time.
- Result and computed-argument schemas forbid defaults at every depth. Missing required producer
  fields are errors; missing optional producer fields stay absent. Output
  validation never invents data. A receiving optional input may subsequently
  use its own default when the binding explicitly treats an absent optional
  producer field as missing. A required consumer needs a guaranteed field or
  an explicit Recipe branch that checks its presence.
- Materialize default lists/objects independently for each task/binding;
  never share mutable default data across executions.

For example, this **input schema fragment** requires a list of objects. Each
object has a required string `name` and an optional nullable string `note`:

```json
{
  "type": "list",
  "required": true,
  "checks": [],
  "items": {
    "type": "object",
    "fields": {
      "name": {"type": "string", "required": true},
      "note": {"type": "string", "required": false, "nullable": true, "default": null}
    },
    "allow_extra_fields": false
  }
}
```

`[{"name":"Ada"}]` binds as `[{"name":"Ada","note":null}]`. An explicit
`note: null` stays null. Missing `name`, numeric `note`, a non-object element or
an undeclared object field fails binding. For a result schema, remove the
root `required` and all defaults (`checks` may remain); an omitted optional
`note` then remains absent rather than being fabricated.

`arguments` maps each parameter to one local input, without embedded templates.
The supported record describes this one usage, not arbitrary expression
execution. `code_arguments` declares computed or fixed arguments as
the recursive value schema plus `depends_on` and `meaning`: `depends_on` is a
list of local input names (empty for a constant), and `meaning` describes the
contract in plain text. Actual computation lives in the associated approved
code, never in metadata evaluated as source. The two argument maps cannot
overlap; together they must account for every argument supplied by the usage.
Reject undeclared dependencies and incomplete required Tool arguments.
`checks` is required (an empty list is permitted); numeric computed arguments
use the `min`/`max` constraints defined above. Validate the actual computed value
and its transport/Tool representation before dispatch, not only the input
dependencies or the explanatory `meaning` text. The current generic computed-
argument validator checks declared schemas, not the truth of `meaning` or the
formula. A string schema with `meaning: "fixed operation"` does not constrain its
value to that operation. Enforce required fixed selectors and permitted operations
through the reviewed code and supported Tool/adapter contract, with behavioral
coverage; if a runtime equality gate is required, implement it in that owning
contract. Do not invent `const`/`enum` fields in this schema or claim such a gate
already follows from a prose description.

The result contract is a recursive value schema without root presence/default
fields. Object results declare `fields` and `allow_extra_fields`; list results
declare `items`; scalar results have neither. Declare all consumed fields and
validate the actual successful export return value recursively before
downstream use.

### Result ownership and validation boundaries

The Tool/adapter owns its primitive payload, error classification and completion
certainty. The ToolSkill describes that boundary; it does not transform data.
Associated PythonCode may deterministically adapt a successful payload within
its declared usage. The association's `result` and code's `result_contract`
describe the export's return value, which the runner validates before publishing
it as the Recipe step result. They need not equal the primitive's broader schema.
For a pass-through export, the primitive payload and usage result must satisfy
the same declared success contract; for a transformation, review and qualify
both the source payload assumptions and the returned shape. Do not turn a Tool
failure into fabricated successful data or silently discard an unresolved effect.

The ordinary retained runner validates the completed export result; that is not
proof of a separate recursive check on every raw native payload before Python
sees it. Required primitive/adapter validation must exist on that boundary.
A binding step produces no user result. Recipe result edges consume validated
execution results and name the producer step and supported field path; optional
fields, list indexing, guards and dynamic occurrences need the selected layout's
actual support, not an expression invented inside a reference string.

### Exact failure and retry contract

`failure` has exactly `{action, max_attempts, idempotency,
idempotency_evidence_ref, retryable_outcomes}`. These declarations require a
supported runner policy; they neither execute recovery nor grant permission.

- `action` is exactly `stop` or `retry`.
- `max_attempts` is a positive integer excluding booleans, including the initial
  Tool dispatch. `stop` requires 1; `retry` requires at least 2. Thus 3 allows
  at most the first dispatch plus two retries, subject to cancellation, current
  policy and resource limits. Persist the count for the logical step invocation
  across waits/resumption; worker reclaim or a replacement attempt cannot reset
  it or authorize replay. Keep invocation identity and execution-attempt fencing
  distinct; these identifiers are not Tool grants.
- `idempotency` is exactly `not_assumed`, `read_only` or `deduplicated`.
  `not_assumed` requires `stop`, a null `idempotency_evidence_ref` and an empty
  `retryable_outcomes` list. `read_only` requires a nonempty durable evidence
  reference verifying that repeating this exact usage cannot repeat a mutation.
  `deduplicated` requires a nonempty durable evidence reference to a tested Tool
  contract that reuses one durable idempotency key and the same arguments,
  prevents duplicate effects, and retains deduplication records for the full
  continuation/retry period. A Tool name or prose claim is not evidence.
- `retryable_outcomes` is a duplicate-free list containing only
  `confirmed_no_effect_transient` and/or `unknown_completion`.
  `stop` requires an empty list; `retry` requires a nonempty list and verified
  `read_only` or `deduplicated` evidence. No generic `error`/`timeout` category
  is accepted.

`confirmed_no_effect_transient` means trusted Tool/adapter evidence establishes
that the failed call caused no effect and the failure is temporary.
`unknown_completion` means dispatch may have completed but its outcome is not
known, for example a lost response or timeout. A timeout alone never proves
that no effect occurred. Unknown completion may be retried only when explicitly
listed and the read-only/deduplication evidence covers that uncertainty; a
deduplicated retry must use the original key and arguments. Otherwise stop or
enter an explicit Recipe reconciliation path, without claiming failure/success
that has not been established. Reconciliation is a separate usage/step, not
an implicit extra Tool call inside this Skill.

Never retry a confirmed completed operation because downstream validation,
reply posting or another step failed. A stored completed result is reused;
deduplication recovery may retrieve its result but cannot repeat its effect.
Policy denial, invalid input, authentication requiring user action, cancellation,
stale attempt, malformed successful output and permanent failure are not
retryable outcomes in this format. Recheck current Tool policy, attempt freshness
and cancellation before every permitted retry. Exhaustion stops with the last
classified outcome; it does not reset the budget, replay earlier steps or fall
through to Tier 2. If the adapter cannot establish an allowed classification,
the default is stop, not inferred retry eligibility.

**Current execution support:** the ordinary retained runner rejects `retry` or
`max_attempts != 1` during preparation. It accepts one bound Tool dispatch per
executable step and rejects additional/unbound calls. Parsing a broader failure
declaration is not executable retry support. The durable invocation/evidence
adapter and recovery qualification must exist before activating retries; until
then use `stop` with `max_attempts: 1` on that path. Do not implement retries by
calling the Tool again inside the export, recursively creating child tasks or
resetting the logical invocation in another Recipe step. See
[recipe.md's conditional dependent-chain rule](recipe.md#6-specify-steps-bindings-and-result-flow).

### Recursive producer/consumer compatibility

IBS checks that every value allowed by a producer's declared contract is
accepted by its consumer, including nested structure, nullability and bounds.
For direct bindings, require matching declared types (no implicit conversion),
producer bounds no wider than consumer bounds, string minimum lengths at least
as strict as the consumer's, and recursively compatible list
items and object fields. A nullable producer cannot feed a non-null consumer
without an explicit guard/adaptation step. Empty lists and objects are valid
only when their declared schemas permit them.

For whole-object bindings, every required consumer field must be guaranteed
present in the producer. Optional producer fields can feed optional consumer
fields with the missing/default rules above. If the consumer rejects extras,
the producer must reject extras too and declare no keys unknown to the consumer.
If the consumer permits extras, any producer field unknown to it must satisfy
its `extra_fields` schema, as must the producer's own extra-value schema when
extras are permitted. Extra keys cannot establish guaranteed field presence.
If a producer's permitted extra key could match a declared consumer field,
its extra-value schema must also satisfy that field's schema. Checking only the
consumer's extra-value schema is insufficient for such a key.

For a selected nested field/list element, validate the declared path and its
type at every level. Optional/null parents and potentially absent list indices
require explicit presence/null/bounds handling before access. Defaults do not
repair an invalid or null producer value. If compatibility cannot be established
structurally, reject that direct edge and use an explicit validated pure-logic
adapter or guarded Recipe branch. Runtime validation remains required even when
declared schemas are compatible; report failures with the exact field/index
path, stop dependent execution and do not replay the producer Tool call.

Recipe capture names and input-reference syntax belong in Recipe bindings,
not this reusable association. Associations describe local inputs independently
of how a user phrases a task.

Do not infer the relationship from equal names, neighboring rows, a prose code
block or a ToolSkill name mentioned in a sentence. A Markdown association record
is useful design evidence but does not replace runtime/store enforcement.

One Tool-usage export normally performs one independent `host.<tool>(...)`
call and returns its result. The invocation boundary captures that return value
as the step result; an export need not assign a global `result`. Pure-logic
components make zero Tool calls. The direct dependent-chain exception in recipe.md remains valid,
but does not justify a multi-Tool leaf Skill. Keep the Skill's purpose one usage;
do not hide a larger workflow inside its entry point.

The retained PythonCode runner reads step-local typed values from
`inputs["local_name"]` through the supported `recipe-input-layout/1` contract.
Legacy source substitution is not this path. A component can reuse internal code helpers,
but their inputs, outputs, assembly order and selected versions must be explicit.
Validate the resulting assembled code as well as individual components.

Never use `import os`, `import subprocess`, `exec(`, `eval(`, `open(` or retired
intrinsics. Use the registered host boundary. Do not assume ordinary CPython
features or imports work in the pinned Monty runtime.

## 8. Define input binding and result flow precisely

The Skill declares **local inputs**. The consuming Recipe determines where
their values come from: captured user input, a typed constant, or an earlier
step's result. This lets the same Skill/code be reused in different Recipes.

Use recipe.md's exact binding convention:

- Names match `[a-z][a-z0-9_]*`; declare types and required/default behavior.
- `%` marks positional slots in intent templates, not in PythonCode.
- `{{vars.name}}` is a whole-value input reference in Recipe parameter
  metadata, not Python source and not a general expression language.
- The runtime supplies validated values through the step-local input mapping.
  Strings with quotes, backslashes, newlines or Python-looking text remain data.
- The step publishes its export's validated return value under its stable step
  identity. Later steps bind that result or declared fields through the task execution context.

No runtime output or user input may be pasted into approved Python source.
Code assembly combines validated PythonCode components; data binding supplies
values. They are different operations.

Do not assume legacy `variable_patterns` rejects invalid values: that capture
helper can retain raw positional values after refinement fails. The retained
runner validates typed bindings and keeps intermediate results in the task's
selected execution context; unrelated tasks and attempts remain isolated.
Child execution uses explicit typed handoff and the captured catalogue on the
global service. It does not inherit arbitrary Python locals. Qualify each
consumer's actual path; general crash recovery and dynamic occurrences remain open.

## 9. Worked authoring design: read a specified line interval

The archive's whole-file and range Skills illustrate useful separate usages of
one primitive. Its `range` argument is historical: the current first-party
[read implementation](crates/brassclaw_first_party_extensions/src/coding/file.rs)
uses `path`, `offset` and `limit`. Never copy an old binding descriptor without
checking it against the actual Tool implementation and host adapter.

The following is a **target authoring design**, not a complete deployment
package or an activated component. Retained typed-input support exists, but
this illustrative interval usage has not thereby been qualified. UUIDs below are illustrative
identities for the component and Recipe roles; replace them with allocated/resolved real UUIDs
before submission. A compatible offset/limit ToolSkill must be reused or created;
an archive descriptor accepting only `range` does not satisfy this example.

The native reader also supports a distinct complete UTF-8 capture profile;
see [the actual raw-file contract](docs/reborn/raw-file-snapshot.md). That profile
has no range arguments and a different result schema. Its primitive acceptance
neither changes this interval usage nor approves a raw Skill or R10 component.

### Numeric transport and Tool bounds for this usage

This example uses a deliberately portable integer profile: line numbers and
the computed `limit` are integers in **1..2147483647** (`2^31 - 1`, inclusive).
This is this Skill's limit, not a claim about Monty's maximum integer or a
global limit on other Skills. It fits both 32-bit and 64-bit Rust `usize`.

The inspected Tool decodes `offset` and `limit` with JSON `as_u64()` followed
by `usize::try_from`. Its representation bound is therefore
`0..min(18446744073709551615, usize::MAX)` on the selected host. This Skill
deliberately uses the smaller positive range above. IBS must verify that the
selected VM, transport and host adapter preserve every integer in that profile
exactly; if unsupported, assembly fails explicitly rather than truncating,
wrapping, rounding, clamping or silently changing the approved profile.

Transport these values as typed integers, encoded as JSON integer numbers at
a JSON boundary, never floating-point numbers or Python source. For captured
line numbers, accept only nonempty ASCII decimal digits (`[0-9]+`), normalize
leading zeros, and compare the normalized decimal value with `2147483647`
before numeric conversion. Zero, oversized values, signs, whitespace, decimal
points and exponent notation fail binding. Direct typed constants/results
must also pass the same range check; booleans are not integers.

After validating `1 <= start_line <= end_line <= 2147483647`, compute
`limit = end_line - start_line + 1`, then validate `1 <= limit <= 2147483647`
before calling the Tool. Subtraction first keeps intermediate arithmetic in
range. The backend's `start_line` index is at most `offset - 1`, so its
`start_line + limit` is at most the requested `end_line`; this bounded profile
also prevents overflow in that inspected backend calculation. Runtime checks
are required even though the mathematical bound follows from valid inputs.

### 9.1 Skill prose

The prose must include the exported signature and the preload/isolation contract
defined above; the illustrative UUIDs remain placeholders, not active records.

```text
Purpose
Read one inclusive line interval from a known permitted text file.

Use when
The caller supplies a path and positive integer start_line and end_line.
No discovery, pagination, editing or final reply is included in this usage.

Required components
Skill: 11111111-1111-4111-8111-111111111111 (skill-read-file-interval)
PythonCode: 22222222-2222-4222-8222-222222222222 (pc-read-file-interval)
ToolSkill: 33333333-3333-4333-8333-333333333333 (ts-read-file-offset-limit)
Tool: 44444444-4444-4444-8444-444444444444 (existing read_file primitive)
Internal code dependencies: none for this example.

Inputs
path: required nonempty string; evaluated under the Tool's mount/path rules.
start_line: required integer in 1..2147483647; inclusive first line, one-based.
end_line: required integer in start_line..2147483647; inclusive last line, one-based.
No defaults. Missing, invalid and wrongly typed inputs stop before dispatch.

Prerequisites
The selected path must support and qualify typed input binding and the
retained offset/limit host adapter.
The kernel must allow the Tool and technical filesystem access.
The target must be a permitted nonsensitive readable text file within Tool limits.

Exports and preload
Export read_file_interval(path, start_line, end_line) from the associated code UUID.
No helpers, dependencies, mutable constants or defaults. Resolve and retain this
export for the task; loading performs no Tool call. Inputs/results stay task-owned.

Execution
Preload the associated read_file_interval definition and dependencies without
calling the Tool. Invoke read_file_interval with typed inputs once; it computes
limit = end_line-start_line+1
and validates limit in 1..2147483647 before calling
host.read_file(path=path, offset=start_line, limit=limit).
No dynamic evaluation and no source substitution are used.

Result
Return the Tool's successful object unchanged:
content: string, with line-number prefixes produced by the Tool.
total_lines: integer, count of all lines in the file.
lines_shown: integer, count of selected lines actually returned.
truncated_by_default: boolean; false when this usage supplies an explicit limit.
path: string, resolved scoped path.
A successful empty selection beyond EOF is not a failed read.

Failures
Stop on binding error, policy denial, authentication/host error or Tool failure.
Do not treat an error payload as success. Do not retry automatically.
Malformed successful results fail the declared output check before downstream use.

Limits and tier
Read-only usage. The implementation rejects files larger than the live
max_coding_read_bytes limit (default 10 MiB) and probes for binary content.
Technical limits remain authoritative.
Deterministic prepared inputs can be used by Tier-0 Recipes; no LLM is needed.

Optional example consuming/MCP-call Recipe and command
Example Recipe: 55555555-5555-4555-8555-555555555555, variant read-file-interval.
This command contract belongs to that example Recipe, not the Skill interface.
Command: read % from line % through line %
Slots: literal path string, decimal start_line, decimal end_line, in that order.
Use the exact separators; no shell quoting, escaping or whitespace trimming.
The command path is nonempty, has no CR/LF or leading/trailing whitespace,
and cannot contain the separator " from line ". Other path strings require
a separately defined compatible command form or typed input route.
Line slots contain only ASCII decimal digits and satisfy the bounds above.
Valid: read /workspace/notes.txt from line 2 through line 3
Invalid: read /workspace/notes.txt from line 0 through line 3
MCP advertises this form only after its Recipe and capture path are qualified.

Examples and acceptance
For alpha/beta/gamma/delta on four lines, interval 2..3 returns numbered beta
and gamma, total_lines=4, lines_shown=2 and truncated_by_default=false.
Zero/reversed/oversized bounds are rejected before dispatch. An end beyond EOF returns
only remaining lines. No reply is posted by this Skill.
```

### 9.2 Associated PythonCode

```python
# Preloaded associated implementation; definition performs no Tool call.
def read_file_interval(path, start_line, end_line):
    # The invocation boundary validates string/integer types, excluding bool.
    if not (1 <= start_line <= end_line <= 2147483647):
        raise ValueError("Invalid or unrepresentable line interval")
    limit = end_line - start_line + 1
    if not (1 <= limit <= 2147483647):
        raise ValueError("Invalid or unrepresentable interval length")
    return host.read_file(path=path, offset=start_line, limit=limit)
```

The interface exports `read_file_interval(path, start_line, end_line)` from the
associated code UUID; dependencies: none; no mutable constants or defaults.
The following illustrates the invocation after the matching ToolSkill binding.
The retained runner resolves the selected export and supplies these typed inputs;
this is not another preload component or a fresh global-name lookup:

```python
result = read_file_interval(
    path=inputs["path"],
    start_line=inputs["start_line"],
    end_line=inputs["end_line"],
)
```

The one Tool call implements the usage; interval arithmetic is deterministic.
The association below maps the direct arguments and declares the computed
argument for that transformation. There is no `limit` input: it is computed in
approved code and must be checked during compatibility review and before
runtime dispatch. These guards supplement typed binding; they do not replace
its rejection of booleans, nonintegers or unsupported transport. Verify guard
and exception support in the selected Monty runtime before activation.

### 9.3 Machine-readable target association

```json
{
  "format": "skill-association/1",
  "skill_uuid": "11111111-1111-4111-8111-111111111111",
  "python_code_uuid": "22222222-2222-4222-8222-222222222222",
  "tool_skill_uuid": "33333333-3333-4333-8333-333333333333",
  "tool_uuid": "44444444-4444-4444-8444-444444444444",
  "callable": "host.read_file",
  "inputs": {
    "path": {"type": "string", "required": true, "checks": [{"kind": "min_length", "value": 1}]},
    "start_line": {"type": "integer", "required": true, "checks": [{"kind": "min", "value": 1}, {"kind": "max", "value": 2147483647}, {"kind": "max_field", "input": "end_line"}]},
    "end_line": {"type": "integer", "required": true, "checks": [{"kind": "min", "value": 1}, {"kind": "max", "value": 2147483647}]}
  },
  "arguments": {"path": "path", "offset": "start_line"},
  "code_arguments": {
    "limit": {
      "type": "integer",
      "depends_on": ["start_line", "end_line"],
      "checks": [{"kind": "min", "value": 1}, {"kind": "max", "value": 2147483647}],
      "meaning": "Inclusive interval length: end_line minus start_line plus one"
    }
  },
  "result": {
    "type": "object",
    "fields": {
      "content": {"type": "string", "required": true},
      "total_lines": {"type": "integer", "required": true},
      "lines_shown": {"type": "integer", "required": true},
      "truncated_by_default": {"type": "boolean", "required": true},
      "path": {"type": "string", "required": true}
    },
    "allow_extra_fields": false
  },
  "failure": {
    "action": "stop",
    "max_attempts": 1,
    "idempotency": "not_assumed",
    "idempotency_evidence_ref": null,
    "retryable_outcomes": []
  }
}
```

### 9.4 Recipe input binding and step references

For a design template `read % from line % through line %`, positional values
map to task inputs `path`, `start_line`, `end_line`. The command form uses the
exact literal path and separator restrictions in section 9.1. Validate decimal captures, then convert them to integers without
`eval` and apply the association's input checks before the read dispatch.
The current retained capture helper returns refined strings and immediately
validates them against task-input contracts; it does not itself convert decimal
captures to integers. This command therefore needs a supported typed refinement
or explicit pure conversion step with string task inputs and integer step outputs.
Do not feed string captures directly to integer schemas or claim the design is
activated until that path is implemented and qualified.

The logical step-local reference mapping is:

```json
{
  "path": "{{vars.path}}",
  "start_line": "{{vars.start_line}}",
  "end_line": "{{vars.end_line}}"
}
```

These whole-value references describe typed binding, never Python substitution.
This fragment is not a complete retained input layout or an extra SQL field.
Persist the actual `recipe-input-layout/1` task-input/constant/result bindings
and retained invocation selection through the supported Recipe document owner.
If conversion is a pure step, bind the converted integers from its declared
result fields instead of the original string captures. Use separate component
steps with one UUID each:

```text
Optional explicit Tier-1 context: Skill 11111111-1111-4111-8111-111111111111.
Rust binding: ToolSkill 33333333-3333-4333-8333-333333333333.
Immediately following Python execution: 22222222-2222-4222-8222-222222222222.
Later operations and final reply are separate usages and steps.
```

A Skill can be complete without a dedicated invocation Recipe or a larger product
workflow. Behavioral evidence remains necessary before productive use and may
come from a supported contained review or an existing consuming/test workflow.
The example Recipe is a separate workflow contract, optionally qualified for MCP.
The illustrative Recipe UUID and steps above do not supply its complete persisted
layout, invocation selection, activation or trusted approval evidence.

### 9.5 Exact expected result

Assume an explicitly provisioned readable nonsensitive test file
`/workspace/notes.txt` with UTF-8 contents `alpha\nbeta\ngamma\ndelta\n`, and
an authorized mount/host adapter exposing the inspected primitive. Inputs
`path=/workspace/notes.txt`, `start_line=2`, `end_line=3` produce Tool arguments
`path=/workspace/notes.txt`, `offset=2`, `limit=2` and this successful payload:

```json
{
  "content": "     2│ beta\n     3│ gamma",
  "total_lines": 4,
  "lines_shown": 2,
  "truncated_by_default": false,
  "path": "/workspace/notes.txt"
}
```

This expected payload follows the inspected source, not a recorded test run.
The current backend's offset uses one-based numbering, clamps to EOF and returns
line-numbered content. The Skill deliberately rejects zero despite the backend
accepting it. Path technical enforcement, backend read limits and the host
adapter's actual parameter contract must still be verified through execution.

## 10. Create, validate and activate

Follow this order for one coherent usage change; it does not require a new
planning document, subagent or a build after every step:

1. Define a concrete reusable Tool usage and establish why an existing Skill
   does not fit. Define it independently of any dedicated Recipe. Establish its
   behavioral cases through the supported contained review/consumer path. Author
   a separate MCP-call Recipe only if MCP exposure is intended.
2. Define the usage contract and inspect the actual Tool signature/results.
3. Reuse compatible PythonCode and ToolSkill UUIDs. Author missing small code
   components/binding descriptors only where needed.
4. Write the Skill prose and explicit executable association. Keep the prose
   and code's purpose, inputs, defaults, errors and outputs consistent.
5. Submit the draft/new versions through supported component stores and the
   validation/proposal path. Do not invent schema fields or an available host API.
6. Q1 checks required structural/safety/reference/compatibility rules and
   available automated semantic/behavioral audits. Establish prose/code
   consistency through the review and behavioral evidence described below;
   syntax or a matching schema alone proves no behavior. Verify which checks
   are actually implemented and record unresolved limitations.
7. For installed-instance authored changes, human Q2 reviews that consistency
   and evidence for the changed own functions, jointly for Skill/implementation,
before activation; unchanged dependency approvals are reused. An agent does not approve
   its own proposal or relabel it `source: system` to bypass Q2.
   Shipped first-party components instead use the controlled `system_seed`
   bootstrap: development review, required Q1/integrity and behavioral evidence,
   with no additional external maintainer or installed-instance human Q2. A
   source/status label does not establish that provenance.
   Use own A function evidence in the supported successor, exact S usage records
   in bootstrap, or the existing exact authored path before that cutover. A task
   manifest or successful schema check cannot replace the applicable evidence.
8. Publish approved selections through the supported coherent deployment path,
   currently complete Recipe graphs, preserving compatible code/Skill/binding
   relationships. Verify actual consumers. Qualify declared command/MCP contracts
   separately; a dedicated per-Skill Recipe is not required.
9. Report selected UUIDs/versions, evidence and limitations. A saved Markdown
   design is not an activated component.

### Semantic consistency is an authoring and approval responsibility

Before activation, the author reviews the Skill prose against its exact
PythonCode, internal code components, ToolSkill and Tool implementation. Check
purpose, argument computations, defaults, prerequisites, effects, success/error
meaning and tier restrictions. Behavioral validation through the supported
execution path must establish representative success, boundary and failure
cases; distinguish observed evidence from expected results inferred from source.

Q1 contributes the automated audits and checks it actually supports. It must
not label arbitrary Python semantics as proven merely because parsing or
contract validation succeeds. For authored changes, human Q2 reviews the semantic
consistency and behavioral evidence for the specific proposed versions and association. Verified
installation-owned seeds use their distinct bootstrap review/integrity contract.
Record their approval/evidence through supported validation mechanisms, not an invented
store field. Incomplete semantic review or missing required acceptance evidence
prevents activation; it is not deferred to a matched Tier-0 task's startup.

IBS consumes those approved structured records. It does not reinterpret the
prose, rerun semantic review, infer a computation from `meaning`, or ask an LLM
to establish correctness at task startup. Tier-1 reasoning explicitly requested
by a Recipe remains separate from component approval and deterministic assembly.

Creating Skill prose, PythonCode or ToolSkill metadata does not normally require
Rust compilation. A genuinely missing Rust Tool has its own Tier-1 author/build/
test/registration workflow. Required infrastructure repairs belong to their
owning layer. Editing Rust seed constants or embedded reference documents can
require a rebuild and seed/integrity acceptance despite adding no primitive.
Compilation alone neither approves nor activates a component.

### Storage today versus the target

Today Skill prose is in `reborn_skills`; executable code is in
`reborn_python_code`; ToolSkills are in `reborn_tool_skills`. The current
`NewPgSkill` constructor carries name, description, body, class, consumer tags,
examples, source, validation status, checksum and existing storage identity
fields. It has no explicit executable-association field. Do not insert invented
fields and claim the store persisted them.

Use supported stores/seeders such as `PgSkillStore::insert(NewPgSkill { ... })`
for their actual contracts. This legacy insert stores the caller's
`validation_status`; `source: system` does not promote pending content. A caller
supplied `validated` string is not exact-combination approval. On a name/scope
conflict `Ok(None)` leaves the existing row unchanged; resolve and verify its
contract before reuse. Supported retained revisions/review/deployment owners
provide the separate v3 path; the legacy insert is not an immutable update API.
New code must not adopt `v1-types` or `v2-compat` as
the v3 authoring mechanism. Runtime component storage is database-backed; a
filesystem `SKILL.md` is not the v3 storage/activation path.

First-party bootstrap requires verified `system_seed` provenance and its exact
package/integrity and behavioral evidence; `source: system` alone is insufficient.
Existing system seeds require their checksum/upgrade workflow. Editing a Rust
seed constant can require rebuilding and seed/integrity acceptance even though
it introduces no new Rust primitive. Do not mutate a live approved system row
or run destructive repair as a substitute for immutable version activation.

## 11. Versioning and updates

A component UUID is its stable identity. An approved version is immutable.
Installed-instance authored updates create new draft versions and pass required
Q1, behavioral validation and human Q2 before activation. Reviewed shipped
successors use the controlled seed upgrade path, preserving operator overrides
and old task selections without additional installed-instance human Q2.
Replacing the current version does not invalidate the previous one.

Recipes reference UUIDs without version numbers. At task start IBS selects and
pins the newest activated approved versions, including the complete
Skill/PythonCode association, applicable own A approvals or exact S/legacy
approval records and internal code graph, in BuildInstruction.
Incompatible active contracts fail composition; do not silently select an older
version to hide the incompatibility.

Running/suspended tasks and child steps keep the original pinned selection.
Retain old bodies, associations and implementation artifacts while those tasks
or resumable checkpoints need them. Resume does not reselect newest. Approval
does not need rechecking merely because a newer version was activated;
integrity, attempt freshness and current global Tool permission still apply.

Immutable revisions, retained selections and exact association enforcement exist
in the retained stores/runner. Legacy mutable rows do not supply those contracts.
General library migration, standalone pure approval and recovery remain open.

### Pin the matched workflow, not only its executable components

The task snapshot MUST include the matched Recipe UUID/revision/checksum,
selected variant identity and immutable revision/checksum, exact `step_link`,
selected step range/order and variable/input layout. If a variant or layout is
embedded in the Recipe rather than independently versioned, identify it within
that pinned Recipe revision; do not invent an independent version API. Pin all
execution/context dependencies and association approval identifiers alongside
this workflow selection.

Matching and IBS assembly must agree on one consistent catalogue generation.
Do not combine a variant or `step_link` selected from an old Recipe with steps
from a newly activated Recipe. If concurrent activation prevents a coherent
selection, restart resolution against one snapshot before effects or fail
composition explicitly. This is not a No-Match or permission to enter Tier 2.

Retain the snapshot with the task before execution. Running/suspended tasks,
child steps, retries and checkpoint restoration keep the original Recipe,
variant, layout, step selection and component revisions. Resume does not rerun
intent matching or select an updated workflow. New tasks select the newest
activated approved workflow coherently. BuildInstruction remains ephemeral;
retain task snapshot/revision references through the existing task continuation
contract, not a new persistent instruction table.

### Exact compatibility checks during IBS assembly

At task start, using one consistent catalogue snapshot, IBS MUST check:

1. Each UUID resolves to the required class and newest activated approved
   immutable version; checksums and the exact Skill-owned association are present.
   Recipe/variant/layout/step_link selection belongs to the same snapshot as
   its execution components and is pinned with them.
2. The selected technical association has required valid evidence: own A function
   approvals/compatibility in its supported successor, or matching exact S/legacy
   `skill-association-approval/1` in that profile. Identity/checksum integrity stays
   exact. In the new A profile directed required/provided contracts replace the
   current exact code/association-schema equality only through qualified owning
   readers; unknown fields or silent coercion are not accepted generally.
   Do not derive compatibility or approval from prose purpose or `meaning` text.
3. The approved code's declared input contract is complete and compatible with
   Recipe binding declarations; supported static checks reject undeclared
   references. Required inputs have declared sources and optional defaults
   validate recursively. Reject duplicate bindings, undeclared inputs and unsafe
   implicit conversions. Validate concrete values when their binding becomes
   available; startup cannot inspect results of steps that have not run yet.
4. The callable exists on the selected host path. Required Tool parameters are
   declared; argument names/types match both ToolSkill metadata and the registered
   Tool contract. Include computed/fixed argument schemas and dependencies;
   validate their actual values before dispatch. Approval and behavioral
   evidence establish that the code implements the declared calculation;
   IBS does not prove it from explanatory text.
5. Internal includes resolve without cycles, symbol clashes or conflicting
   input contracts; assembled code obeys Tool-call grain and parses in Monty.
6. Producer result schemas satisfy consumer bindings recursively, including
   list elements, object fields, presence, nullability, extra fields and numeric
   bounds. Reject unsafe field/index accesses and forward dependencies; validate
   actual results before downstream use. Successful empty data is distinct
   from failure. Defaults apply only to missing consumer inputs, not bad outputs.
7. All selected versions, code includes and the association are pinned in
   BuildInstruction and retained by the task. Failure is explicit: do not
   silently use older versions, redo Q2 for originals or fall through to Tier 2.

These are deterministic structural/contract checks, not semantic interpretation
of prose or proof of arbitrary Python behavior. Section 10's author review,
required Q1/behavioral review and authored Q2 or verified seed qualification
establish semantic consistency before activation;
startup verifies the approved records without repeating that approval process.
A compatible new A PythonCode revision needs its own complete function review
with Skill context and successor compatibility, not a new approval of the
unchanged Skill or exact combination. Changed meaning/contracts cannot be called
compatible: preserve required old behavior or review the explicitly changed
interface/consumers through their own routes. S keeps new exact usage evidence. An already running task's
old approved combination remains valid and does not need reapproval because
another combination becomes active. Current global Tool policy is checked
independently at each dispatch.

## 12. Acceptance matrix and checklist

The matrix states expected behavior, not completed runtime test evidence. Use
the selected supported binding/IBS/runner/Tool path. Apply relevant cases at
the usage, shared-interface or activation milestone that owns the requirement;
reuse applicable evidence rather than rerunning the whole matrix per edit.
Verify a usage through a supported contained review or consuming/test workflow.
No dedicated per-Skill or MCP Recipe is required. The command-capture cases below
apply to the example command Recipe; they are not general Skill-completeness gates.

| Case | Inputs/setup | Expected behavior |
| --- | --- | --- |
| Valid interval | Four-line fixture; start=2, end=3 | One call with offset=2, limit=2; exact payload above; zero Tier-0 LLM calls |
| Missing input | Omit path, start_line or end_line | Binding fails before Tool dispatch; no fabricated/default required value |
| Wrong type | A boolean, decimal or unconverted string as a line number | Binding fails before dispatch; booleans are not integers |
| Invalid interval | start=0 or start=3, end=2 | Binding rejects before dispatch; no negative/zero computed limit |
| Oversized line number | start=1, end=2147483648, supplied as a capture, typed constant or prior result | Binding rejects before dispatch; no rounding, clamping, wrap or Tool call |
| Beyond Tool representation | Captured end=18446744073709551616 (`2^64`) | Decimal bound check rejects before conversion, VM handoff and Tool dispatch |
| Maximum valid interval | start=1, end=2147483647 on four-line fixture | offset=1, limit=2147483647; four lines returned without arithmetic overflow |
| Maximum valid starting line | start=end=2147483647 on four-line fixture | offset=2147483647, limit=1; successful empty selection |
| Invalid computed argument | Computed limit is zero, negative, noninteger or above 2147483647 | Computed-argument validation fails before dispatch; no Tool effect |
| Unsupported integer transport | Selected VM/adapter cannot preserve the declared integer profile | Assembly fails before execution; no silent profile reduction |
| End beyond EOF | start=3, end=9 in four-line fixture | offset=3, limit=7; two lines returned, total_lines=4; no automatic pagination |
| Start beyond EOF | start=8, end=9 | Successful empty content, lines_shown=0, total_lines=4; distinct from failed read |
| Hostile string | Quotes/newlines/Python-looking text in path | Source/checksum unchanged; value remains one string; Tool path rules may reject it |
| Tool blocked | Globally blocked before dispatch, including after a wait | Kernel refuses dispatch; no new Tool effect; pinned versions grant no permission |
| Tool failure | Missing file, denied mount, binary/oversized target or host failure | Stop with classified error; no success result and no implicit retry/reply |
| Malformed result | Missing total_lines or wrong field type | Output validation fails before downstream consumption; no replay of completed call |
| Semantic mismatch before activation | Prose promises an inclusive interval but code omits the last line | Required author/Q1/behavioral review and authored Q2 or seed qualification resolve the mismatch before activation; matching schemas alone are insufficient |
| Approved Tier-0 assembly | Exact approved association and compatible structured contracts exist | IBS verifies records/checksums/contracts with zero LLM calls; it does not reinterpret prose |
| Approval association mismatch | Selected code checksum/version does not match the approved association records | Assembly rejects before effects; IBS does not infer fresh approval from similar prose |
| A successor without new exact-combination record | Own Skill/code function approval and needed compatibility are valid in the qualified successor profile | No new combination review; verify own records and technical bindings |
| S exact usage mismatch | Individual components valid but selected exact usage lacks its required S record | Assembly rejects before effects; individual approvals do not substitute for the S usage evidence |
| Compatible code update | New code passes own A function review with unchanged Skill context and preserves needed contracts | Skill approval stays unchanged; coherent new tasks use successor, old tasks keep exact prior selection; S retains its distinct exact route |
| Retry count (requires a qualified retry adapter) | max_attempts=3; transient failures persist | At most three dispatches including the initial one; waits/reclaims do not reset the count |
| Invalid retry metadata | max_attempts=2.5, action=retry with not_assumed, or unknown outcome label | Authoring rejects the failure contract before activation |
| Unknown completion without evidence | Timeout after dispatch; no verified read-only or durable deduplication evidence covering that uncertainty | Stop/reconcile explicitly; no inferred no-effect failure or automatic redispatch |
| Safe deduplicated recovery (requires a qualified recovery path) | Approved retry covers unknown_completion with tested durable deduplication | Same key/arguments retained; recover outcome without a duplicate effect, subject to live policy and count |
| Completed effect, later failure | Tool completed, but result validation or reply failed | Do not replay the completed Tool call or earlier workflow steps |
| Nested data mismatch | An integer appears where a nested string/list element is declared | Recursive validation fails at the exact field/index path before downstream use |
| Missing versus null | Optional nullable input/default=null; compare absent key, explicit null and wrong-type value | Absent key receives null default; explicit null stays null; wrong type fails, never defaults |
| Required nullable field | required=true, nullable=true; compare absent key and explicit null | Absent key fails; present null succeeds |
| Null-only schema | type=null with omitted nullable; then with an added nullable flag | Null alone succeeds; redundant nullable flag is a schema error |
| Optional nested field | Input field omitted with and without a default | Apply declared default only when present parent exists; otherwise retain absence and require guarded access |
| Unknown nested key | Object forbids extras, or permits extras with a string schema | Forbidden key fails; allowed extra string succeeds; allowed extra integer fails |
| Invalid default | Default violates child requirements or integer bounds | Authoring validation rejects; no activation with an invalid default |
| Optional producer to required consumer | Producer can omit a field needed unconditionally downstream | Assembly rejects direct edge; explicit presence-handling branch is required |
| Nullable producer to non-null consumer | Producer may emit null | Assembly rejects direct edge without explicit guarded adaptation |
| Independent mutable defaults | Two task bindings use one object/list default | Each receives independent data; mutation cannot leak between tasks |
| Internal code | Entry point includes two approved pure-logic helpers | One Recipe component reference; all nested versions pinned and contracts checked |
| Bad internal graph | Cycle, missing helper or conflicting symbol | Assembly fails before effects; no silent skipping or fallback |
| New incompatible code | Newest approved code changes inputs/results without compatible association | New task composition fails explicitly; no silent downgrade; old tasks remain unchanged |
| Update during pause | Task A pinned version 1; qualified version 2 activated through the applicable authored or seed path while A waits | A resumes with retained version 1; task B starts with compatible newest version 2 |
| Activation race | Task start overlaps replacement activation | One consistent old/new catalogue snapshot and manifest; no accidental mixed reads |
| Recipe activation race | Variant/step_link changes between matching and IBS assembly | Coherent workflow snapshot or explicit pre-effect resolution failure; no mixed Recipe/step revisions |
| Recipe update during pause | New Recipe changes order/layout while old task waits | Old task resumes original variant, step_link, layout and steps without matching again |
| New draft/unapproved version | Draft version 3 exists while approved version 2 is active | New task uses version 2; draft neither replaces it nor invalidates older task snapshots |
| Cancellation/stale attempt | Cancellation during wait or old result arrives | No new dispatch/reply from stale attempt; results/state remain isolated |
| Command capture contract | Decimal line slots and paths with separators, CR/LF or leading/trailing whitespace | Valid slots reach explicit typed conversion; prohibited command forms fail without read dispatch; no silent quote removal or trimming |
| Shell/subagent tier | Fixed or parameterized shell/spawn_subagent usage | Consuming Recipe remains Tier 1 regardless of seed provenance or determinism |
| Broader declaration on current runner | retry/max_attempts>1 or a second host call in one execution step | Reject unsupported preparation/call; no inferred retry adapter or hidden dependent chain |
| Multiple matching usages | Selected graph contains two Skills with the same code/binding/Tool/callable tuple | Reject ambiguity; no first-row selection or fresh latest lookup |
| Adapted successful result | Primitive payload is transformed by the export | Review both contracts; validate export return before Recipe consumers; preserve native error/effect state |


- [ ] Exactly one reusable Tool usage is fully defined without a mandatory Recipe backlink.
- [ ] Exact usage evidence is separate from consuming-workflow and optional MCP qualification.
- [ ] Missing MCP publication does not invalidate an otherwise qualified usage.
- [ ] Existing Skills, ToolSkills and PythonCode were searched and reused.
- [ ] A complete skill-association/1 target record links Skill/code/ToolSkill/Tool UUIDs;
  actual store support is verified rather than assumed.
- [ ] Tool/binding/callable names, parameter names and result types match source.
- [ ] Inputs, defaults, prerequisites, effects and errors are concrete.
- [ ] Recursive schemas cover every list element, object field and allowed extra value.
- [ ] Missing/null/default semantics and producer/consumer compatibility are checked at every depth.
- [ ] The preloaded export returns its typed result; the invocation boundary
  captures it, with Tool-call grain and effect-free preload enforced.
- [ ] Internal code dependencies have explicit order/contracts and no cycles.
- [ ] One component per Recipe step; binding and execution are correctly paired.
- [ ] Typed data stays separate from assembled source, including hostile strings.
- [ ] Relevant acceptance-matrix cases produce verified expected arguments/results;
  expected payloads inferred from source are distinguished from executed evidence.
- [ ] Failure/denial/cancellation/retry cases do not report false success or replay effects.
- [ ] Retry count includes the initial dispatch and survives waits/reclaims; eligible outcomes and durable idempotency evidence are explicit.
- [ ] Tier-0 cases make zero LLM calls; Tier-1 restrictions are observed.
- [ ] Authored versions pass Q1 and human Q2; no approval bypass is used.
- [ ] Author review, required Q1 and behavioral evidence establish semantic
  consistency; authored human Q2 or verified seed bootstrap evidence is recorded.
- [ ] IBS checks structured contracts and exact approved associations without interpreting prose or calling an LLM for Tier-0 assembly.
- [ ] IBS pins compatible newest approved versions and all nested dependencies.
- [ ] Own A function/compatibility records or exact S/legacy usage records cover their actual scope; a task manifest creates no approval.
- [ ] Recipe revision, variant, step_link, step selection and input layout share the pinned snapshot and survive resumption.
- [ ] Updating a Skill/code component does not change a running/suspended task.
- [ ] Relevant seed/integrity and actual execution checks pass where applicable.
- [ ] Missing association/version/input/runner support is reported explicitly.

Use focused checks under the development policy. Read `LOCAL_TEST_ENV.md` if
present before remote tests/provider setup. For this guide, check the diff,
relevant links, examples and consistency and run `git diff --check`. This document
is also embedded as a runtime reference: static documentation checks do not
establish a new packaged baseline, seed integrity or runtime qualification.
Apply the development policy's embedded-content checks at that milestone.
Creating or activating actual components needs the applicable schema,
approval, integrity and execution evidence. A skipped check or zero selected
tests is unverified behavior, not a pass.

## References

- [Recipe authoring, input bindings and version contract](recipe.md)
- [Built-in inventory and historical usage examples](docs/archive/builtin_stuff_v3.md)
- [Skills subsystem guide](docs/agents-v3/05-skills-system.md)
- [Skill store and current constructor](crates/brassclaw_reborn_composition/src/pg_skill_store.rs)
- [PythonCode store](crates/brassclaw_reborn_composition/src/pg_python_code_store.rs)
- [ToolSkill store](crates/brassclaw_reborn_composition/src/pg_tool_skill_store.rs)
- [Association formats and validation](crates/brassclaw_skills/src/association_contract.rs)
- [Retained association review records](crates/brassclaw_engine/src/memory/retained_reviews.rs)
- [Retained binding and unambiguous usage selection](crates/brassclaw_engine/src/memory/retained_tools.rs)
- [Retained runner dispatch, result and retry boundaries](crates/brassclaw_engine/src/executor/retained_recipe.rs)
- [Retained typed input layout and capture handling](crates/brassclaw_engine/src/memory/retained_inputs.rs)
- [IBS builder and current instruction type](crates/brassclaw_engine/src/memory/instruction_builder.rs)
- [Composer and current component routing](crates/brassclaw_engine/src/memory/composition.rs)
- [Q1 orchestration](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs)
- [Current file-read arguments and result payload](crates/brassclaw_first_party_extensions/src/coding/file.rs)
- [Current numeric input handling](crates/brassclaw_first_party_extensions/src/coding/inputs.rs)
- [Current file-read technical limits](crates/brassclaw_first_party_extensions/src/coding/config.rs)

### Preloadable-interface authoring completion

- [ ] Public exports/signatures and private helpers/constants are explicit.
- [ ] The selected interface and complete dependency/symbol graph are checksummed,
      approved and pinned; loading is deterministic and effect-free.
- [ ] Invocation resolves the selected export; old tasks do not rebind to latest.
- [ ] ToolSkill binding remains separate; live kernel checks cover helper calls.
- [ ] Mutable arrays/defaults/closures/results are task/attempt/invocation-owned.
- [ ] Recipes and MCP use canonical invocation forms, not body reconstruction.
- [ ] Actual loader/store/inspector support and Monty acceptance are demonstrated.
