# Recipe authoring instructions

> **Current status — 2026-10-23:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


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
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](docs/implementation-status.md).
Legacy step-body implementations must be migrated to this interface before
being accepted as updated v3 implementations.


This is the authoring checklist for BrassClaw Reborn Recipes. Follow it in order.
MUST means required. A Recipe with an unresolved input, component reference,
binding, result handoff or execution dependency is incomplete. Do not label it
working, validated or Tier 0 without the corresponding evidence.

The [development policy](docs/development-policy.md) governs development iteration:
design one bounded workflow change, complete connected edits, review the relevant
diff, then run the smallest sufficient validation batch. Reuse current source
knowledge and applicable passing evidence. Early experiments are appropriate for
concrete unresolved questions. This guide's authoring and activation requirements
do not require repeating all checks after every edit or seeking human Q2 for a
local draft. Required own A function/workflow review or exact S qualification and release acceptance still apply.

Read [AGENTS.md](AGENTS.md), [simplified_v3.md](simplified_v3.md) and the
[development policy](docs/development-policy.md). Their binding contracts take
precedence over historical examples. This file documents authoring; it does not
implement missing runtime features. Legacy observations explicitly describe the 2026-10-06 composer. The retained
ordinary path and remaining gaps are reconciled in the current status index.

## 1. Understand exactly what you are creating

### Architectural goal: more reusable steps, fewer specialized Rust Tools

The central model is: **Rust Tools, many ToolSkills, many Skills, many small
PythonCode components, and Recipes that tell the orchestrator how to use those
components to fulfill a task's goals.** Each has a distinct role. A Recipe is
the task instruction that selects, orders and connects component usages; IBS
assembles what those instructions require, and the orchestrator performs the
task by executing the resulting steps.

V3 does not aim to minimize the number of Recipe steps. It aims to make complex
behavior explicit as a sequence of small, reusable, editable steps. Prefer
more well-defined steps over a specialized Rust Tool that hides the whole
workflow inside one operation. Step count alone is not a quality metric; each
step must have a useful purpose and a clear input/output contract.

Keep reusable Rust Tools as the primitives. Build many ToolSkills describing
their Rust-side IBS bindings, and many Skills describing how the orchestrator
uses a Tool for a particular purpose. Binding descriptors and usage instructions
are separate responsibilities; neither replaces the other. Each
Skill includes an explicit preloadable function interface backed by associated
PythonCode, with exported names/signatures and private dependencies. **Build a large library
of small, reusable PythonCode components as well.** These are executable
building blocks, similar to objects in the sense that they can be referenced
and recombined; this analogy does not require Python classes or object-oriented
inheritance. Components cover individual Tool usages and pure-logic operations.
Give each a clear purpose, input contract and result contract, independent of
the particular Recipe that uses it.

Recipes are the way programs are specified from that library. IBS/composition
assembles the selected small PythonCode components into ordered executable
steps and prepares their supporting components. Many different programs can
reuse the same PythonCode building blocks in different sequences and with
different inputs. Do not write a fresh, task-specific Python program for every
Recipe when its behavior can be assembled from existing components.

Recipes instruct the orchestrator how to utilize those usages in order, connecting their inputs and
results. A complex Recipe can therefore achieve what a dedicated Rust workflow
Tool would have achieved, by executing Python steps over existing primitives.

This keeps behavior in the component library: Recipes can be altered, copied
and recomposed without adding a Rust Tool for every task. Reuse a Skill and its
associated PythonCode by reference; create a new usage when its purpose or
contract differs. Do not duplicate a primitive merely because the new workflow
has a different user-facing name.

The intended library therefore contains Rust Tools, many ToolSkill binding
descriptors, many Skill usages, many small executable PythonCode components,
and Recipes instructing the orchestrator how to use them to achieve task goals.
IBS/composition assembles the executable steps and supporting components from
those Recipe instructions; the orchestrator owns task sequencing and execution.
Adding a reusable PythonCode component expands what future Recipes can express;
burying that behavior inside a large Recipe-specific body reduces reuse.

### Recipes for creating Tools or ToolSkills

A Recipe may instruct the orchestrator to author a genuinely missing Rust
Tool. Source generation/composition is Tier 1. Use an existing approved
build/compiler Tool, or the shell Tool if it provides the required primitive,
to compile and test the generated artifact. Shell Recipes remain Tier 1.
Do not add a new Rust Tool merely to invoke an already available compiler.

A ToolSkill is a binding descriptor, not a Rust implementation; creating one
normally requires schema/reference validation and approval, not compilation.
If its referenced Tool is genuinely new, the Tool implementation is what must
be compiled. Reuse an existing ToolSkill if its binding contract fits.

Authoring, build/test, validation, activation and use are explicit workflow
stages. Compilation alone does not approve, register or grant permission to a
Tool. Authored components pass Q1 and human Q2 before activation through the
supported registration/loading path. A generated artifact is not automatically
available as `host.<tool>(...)`. A running task's pinned component set does not
silently acquire the newly created component; use a subsequent task or an
explicitly specified, validated continuation/version-selection contract.

Generated Rust/Python source is an intentional code artifact to be reviewed
and validated. This is separate from normal parameter binding: input values
passed to its authoring/build Tools still travel as data, not as substitutions
that modify an already approved PythonCode body.

**Authoring rule:** decompose the behavior first. Add a new Rust Tool only for a
genuinely missing primitive, never merely to collapse a Recipe's steps. Required
runtime, persistence, binding, lifecycle or kernel repairs belong in their owning
Rust layers and do not justify embedding task sequencing in a Tool.
Current executor limitations are runtime gaps to repair, not reasons to move
otherwise expressible Recipe behavior into a dedicated Rust workflow Tool.

A Recipe (class 21) is a **step-by-step instruction plus an explicit inventory
of the components needed to assemble and execute those steps**. It declares
variants, typed inputs/results, component references, order and completion/failure
behavior; IBS assembles the selected workflow and Monty executes it.

The Recipe tells IBS **what to assemble, in which order, and what each step
needs**. IBS/composition builds the executable Python chiefly by assembling
existing PythonCode components, supplying captured input values, and preparing
the associated Skills, ToolSkills and existing Rust Tools for the orchestrator.
The orchestrator then invokes each pinned exported function or pure step in
the specified order; preloading definitions is assembly preparation.
It calls a Rust Tool only when that step's Python calls `host.<tool>(...)`.

The component inventory is linked to individual steps by UUID; it is not just
a loose list of names. Each variant specifies an invocation pattern, intent
examples, variable patterns and a `step_link` selecting its steps. A complete
Recipe must say how values extracted from the user's matched message become
step inputs and how results become inputs to later steps.

| Component | Responsibility | Placement |
| --- | --- | --- |
| Tool, class 0 | Existing Rust primitive that performs an operation when called | Registered host capability |
| ToolSkill, class 13 | Descriptor for binding one Tool usage; grants no authority | Rust-channel component reference |
| PythonCode, class 22 | Executable implementation of a usage or pure logic | Orchestrator-channel component reference |
| Skill, classes 1–3 | One usage: prose **and an explicitly associated preloadable function interface** | Prose for explicit Tier-1 context; associated class-22 code for execution |
| Recipe, class 21 | Orders and connects the components | Recipe store |
| ExtensionCatalogue, class 23 | Domain overview and Recipe inventory | Extension catalogue store |

### Ownership across the four guides

The guides define complementary contracts. Use the owning guide below instead
of introducing another schema, approval mechanism or execution owner.

| Boundary | Owning guide and record | Required handoff |
| --- | --- | --- |
| Workflow | recipe.md: selected variant, `step_link`, ordered steps, `input_layouts` and `invocations` | Supplies each execution occurrence's typed inputs and selected export; declares subsequent steps, reply ownership and completion |
| Reusable Tool usage | skills.md: Skill interface and `skill-association/1` | Links one Skill, PythonCode entry point, ToolSkill and Tool; declares usage input/result/failure contracts |
| Executable definitions | skills.md: class-22 code and `python-preload/2` declaration | Defines public/private symbols and pure code dependencies; the runner invokes the pinned export without source substitution |
| Binding descriptor | toolskills.md: class-13 `tool-skill-binding/1` | Resolves the declared Tool/capability/callable and compatible adapter; performs no dispatch |
| Primitive and authority | tools.md: class-0 definition, retained native implementation and kernel boundary | Validates actual dispatch, enforces live policy/technical limits and preserves effect certainty |
| Review and selection | skills.md's own A functional review or exact S usage evidence plus workflow/pure owners | Recipe approval covers its own supported workflow, not approval of every dependency; independent pure functions need their own applicable evidence |

These records are parts of the existing immutable revision/selection path, not
new fields on legacy SQL constructors. Approval records remain separate from the
selected task manifest. Reusing a qualified usage neither reapproves it per call
nor automatically approves the consuming workflow. Rust custody of effects and
recovery supports Monty's sequencing; neither owner substitutes for the other.
Independent pure approval's remaining work is specified in
[the pure-component review contract](docs/reborn/pure-component-review.md#exact-remaining-cutover-contract),
not supplied by a usage approval that merely includes that helper as a dependency.

### Dependency edges and canonical Recipe links

Keep the retained dependency graph acyclic. A Recipe retains its selected usage
Skills, binding descriptors and executable components. A Skill depends on its
associated code, ToolSkill and Tool; the ToolSkill depends on its Tool. Code's
preload dependencies identify code libraries/helpers, not a reverse reference
to the owning Skill or consuming Recipe. Retain the complete reachable closure,
including dependencies not printed as separate Recipe steps.

A Skill has no required owning or canonical invocation Recipe. Many consuming
Recipes can reuse its same approved interface/code/binding combination. Existing
"canonical Recipe" links identify a particular reference, test or command workflow;
they are optional catalogue/authoring navigation, not Skill-completeness gates or
mandatory reverse execution dependencies. Recipe -> Skill -> companion Recipe
must not form an execution cycle. Compatible-usage inventories on Tools/ToolSkills
likewise create no reverse edges. Retain/review each actual workflow separately;
a navigation link is neither executable dependency nor approval evidence.

Only intended MCP publications require a qualified MCP-call Recipe/variant with
an exact command contract. Internal/test/ordinary workflows may exist without MCP
exposure. The existing optional `mcp_call` declaration is owned by the Recipe
revision; it adds no required field to `skill-association/1`. A public function
export or a Recipe backlink alone does not establish MCP exposure. The current
deployment path publishes complete Recipe graphs, not independently activated
unused library entries; using that path needs no additional per-Skill Recipe.

A code example in Skill prose is documentation. It is not an executable entry
point. Tier-0 invocation references the associated PythonCode UUID and resolves its
selected preloaded export through the retained interface.
Record the Skill/PythonCode UUID association explicitly; do not pretend that
matching names establish a validated association. Recipes reference stable
component UUIDs without version numbers. During task-start assembly,
IBS/composition reads and resolves the exact component versions, including the
validated Skill/PythonCode association, and pins them in the BuildInstruction.
The runtime retains that resolved selection for the task. Where the store or
instruction type cannot enforce this yet, document that limitation.

The flow is:

```text
matched intent + Recipe variant + user message
  -> selected step_link and structured step descriptions
  -> IBS BuildInstruction
  -> referenced component bodies + captured input values
  -> composed execution steps
  -> Monty invokes pinned preloaded exports or pure logic
  -> host tool calls, checked by the kernel
  -> reply and task completion
```

In the current code, assembly has two stages: `build_instruction` selects and
structures component references; the surrounding composition pipeline resolves
them and builds the concrete Python steps and supporting component data.

`BuildInstruction` is an intermediate Rust data structure, not generated Rust
code. It contains `rust_steps`, `orchestrator_steps`, `variable_patterns`,
`llm_call_required` and `basic_prompt_section_refs`. IBS selects steps, checks
their structure, parses dependencies and partitions channels. It does not prove
that the workflow achieves the user's intent.

**Target version contract:** IBS/composition must read component versions and
emit their exact UUID/version/checksum references in the BuildInstruction,
including all selected Tool, ToolSkill, Skill, PythonCode and dependency
references. The orchestrator executes those resolved versions, never a fresh
“latest” lookup. The legacy BuildInstruction fields alone do not implement
this complete manifest. The ordinary retained path now uses immutable revision
selections, retained input/tool/review preparation and task catalogue references;
reuse those owners rather than adding a second manifest store. The instruction remains ephemeral; retain its version selection with
the task through the supported task snapshot/continuation mechanism, without
introducing a separate persistent BuildInstruction table.

The legacy `compose_orchestrator` path resolves components into `ComposedProgram`:
ordered `steplist` entries with concrete `executable_code`, `skills`,
`rust_directives`, `variables`, `assembled_program` and a tier hint.
Its `host.run_program` delegation is not the ordinary retained preload/invocation
API. Do not author an unpinned top-level body for that compatibility path and
claim v3 execution. Parsing alone does not prove behavior or activation.

The required assembly outcome for **every executable step** is:

1. Its canonical code/interface/export and dependency graph are resolved and
   preloaded without effects; typed invocation inputs are supplied safely.
2. For a Tool usage, its Skill/PythonCode association is identified; any prose
   needed by an explicit Tier-1 reasoning step is supplied as context.
3. For actual Tool calls, its ToolSkill descriptors identify the Tools to bind;
   the runtime makes those callables available before the step runs.
4. Its result contract and handoff to subsequent steps are explicit.

Pure PythonCode needs its own exact export/contracts and approval coverage, not
an artificial ToolSkill, zero-Tool Skill or borrowed dependency-only approval.

This is the authoring target. Merely returning Skill bodies or Rust directives
does not establish that all preparation occurred; current wiring limits are
listed below and must be verified on the selected runtime path.

## 2. Reuse before creating anything

Inspect the relevant component store, seed owners and actual consumers. Use
[the built-in inventory](docs/archive/builtin_stuff_v3.md) as supplementary
historical context, not proof of current signatures or support. Read the actual
Tool signature and returned data; record the reused identities and exact selected
revisions. Do not repeat a whole-library inventory when current relevant evidence
already answers the reuse question.

Choose the first sufficient option:

1. Add intent examples to an existing variant.
2. Add a variant reusing existing steps.
3. Compose existing PythonCode and ToolSkills into a new Recipe.
4. Author missing component rows using existing Tools.
5. Add a Rust primitive only if no existing Tool provides the necessary operation.

Before choosing option 5, write the candidate sequence of existing Tool usages
and identify the exact primitive that sequence cannot provide. “Too many
steps,” “simpler as one Rust function,” and “this task needs several Tools”
are not sufficient reasons. Optimize for reusable components and understandable
data flow, not for the fewest host calls or the shortest Recipe.

Use [the Zencoder plan](docs/plans/zencoder-extension-plan.md) as a worked
authoring reference, but reconcile it with the binding Skill definition and
current schemas. Historical “domain Skill” descriptions belong in an Extension.
Do not copy historical retired intrinsics or treat an example as proof of wiring.

## 3. Write the behavior contract before writing code

Fill out this contract. No field may contain “whatever is needed” or “the agent
figures it out.”

```text
Recipe name:
User-visible operation and completion condition:
Variant key:
Entry kind (public intent matching or qualified private typed admission):
Accepted public requests (at least 10 examples for a public Recipe; variant coverage):
Private entry provenance, explicit Recipe/variant selection and typed inputs:
Similar requests that must NOT match:
Required inputs, types, source and validation:
Optional inputs and explicit defaults:
Tool prerequisites and external authentication:
Ordered steps and exact component references:
Declared Skill exports/signatures and dependency preload order:
Pinned invocation selection and mutable-state ownership:
Result fields consumed by later steps:
Failure, cancellation and retry behavior:
Completion: public reply owner, or private typed result/settlement owner:
Tier and reason:
Current execution path and unresolved runtime dependencies:
Acceptance cases and expected effects:
```

One variant MUST have one predictable input layout and workflow. Split different
operations or incompatible slot layouts into separate variants. Publicly matched
Recipes require at least 10 intent examples, covering command and natural-language
forms; exercise every public variant with its own positive and negative cases.
Private workflows instead qualify their actual trusted typed admission, allowed
origin, explicit Recipe/variant selection, input layout and terminal result. Do
not fabricate public intents, chat messages or reply effects for private work.
Any shared fields required by the actual parser still apply; prose does not waive
constructor validation. MCP publication is a separate qualified public command
contract using ordinary chat matching, not private admission or a direct Tool API.

Choose Tier 0 for deterministic behavior with known, safely validated inputs.
Tier 1 is required for creative content composition, user-supplied strings requiring LLM
validation, ambiguous choices requiring reasoning and irreversible operations
requiring confirmation. Recipes using `builtin.shell` or
`builtin.spawn_subagent` are always Tier 1. Deterministic branching or passing a
runtime result to another step does not by itself require an LLM. Deterministic
storage of supplied/validated content does not itself force Tier 1. A model
response does not supply a required operator confirmation.

Do not obtain Tier 0 merely by setting a flag. Legacy eligibility fields are not
approval evidence for a retained workflow. Verify the actual selected caller,
qualified component graph and behavior, including the absence of LLM calls.

## 4. Specify every input and its extraction

Create an input table before writing PythonCode:

| Input | Source/template position | Captured name | Type | Required/default | Validation | Consuming step |
| --- | --- | --- | --- | --- | --- | --- |
| Example: item identifier | First `%` in `show item %` | `item_id` | String | Required | Exact allowed identifier format | Lookup step |

The table is a specification, not evidence that capture is wired.

### Legacy capture semantics (not the ordinary retained input contract)

1. The intent system selects a matched template and variant. A match alone does
   not produce typed, trusted Tool arguments.
2. `extract_template_slots(template, user_text)` splits the template on `%`.
   Gaps between literal segments become `slot0`, `slot1`, etc., left to right.
3. `capture_variables` pairs `variable_patterns` with those slots **by array
   position**, not by searching the entire user message.
4. A successful pattern renames the slot to `name`. A named capture group can
   refine its value. `pattern: null` only renames it.
5. A failed or invalid regex retains the raw value and positional name. It does
   **not** reject the request. Missing substitutions remain in the Python body.

For a matched template `show item %` and input `show item ABC-123`, this design
pattern refines the first extracted slot:

```json
{
  "name": "item_id",
  "pattern": "^(?P<item_id>[A-Z]{3}-[0-9]{3})$",
  "description": "Required item identifier, for example ABC-123"
}
```

This regex is refinement metadata, not a complete input rejection mechanism.
The workflow MUST explicitly reject missing or invalid required inputs before
any effect. Do not fall back to the raw positional slot after refinement fails.
Do not assume a regex turns a string into an integer or boolean.

Do not use adjacent `%` slots. Define separators and test separator text inside
values, empty values, leading/trailing whitespace and reordered phrases. Examples
with different argument positions require their own verified layouts.

### Verify which path receives the template

The retrieval path calls `capture_variables(matched_template, query, patterns)`.
The legacy `PgCompositionPort::compose_with_pool` calls
`capture_variables(user_input, user_input, patterns)`; its public compose call
does not carry the matched template. Ordinary concrete messages containing no
`%` therefore yield no positional slots on that path.

An author MUST trace the real caller. Do not claim that a `%` example works
through `compose_orchestrator` until template/captured-value transport is
implemented and demonstrated through that path. Do not put literal `%` markers
into user messages as a workaround.

## 5. Keep captured values as data, never executable source

The legacy composer uses plain string replacement for `{{vars.NAME}}`. It
does not escape Python literals or reparse the substituted program before
returning it. A stored body's validated status does not certify the substituted
body.

This pattern is unsafe for unrestricted text:

```python
# UNSAFE EXAMPLE — do not copy for arbitrary user input.
result = host.some_tool(value="{{vars.text}}")
```

Quotes, backslashes or newlines can change the source before any Python-level
validation executes. Adding `.replace(...)` or an `if` inside that body cannot
protect the earlier substitution.

### Binding convention for the v3 target

The following defines the authoring/IBS contract. The ordinary retained input
path implements typed bindings and recursive validation. Legacy composers and
unmigrated seeds must be converted before they can use that path.

1. Input names MUST match `[a-z][a-z0-9_]*`, be unique within the variant, and
   be declared with type, required/default behavior and validation rules.
   Accepted data types are strings, integers, finite numbers, booleans, null,
   lists and objects, as supported by the Tool and Monty value boundary.
2. `%` belongs only to intent templates. It marks an extraction slot, never
   Python syntax. Positional capture is mapped to declared semantic names.
   Missing required values or failed validation reject binding before effects.
3. `{{vars.NAME}}` in Recipe parameter metadata is a whole-value reference to
   a declared input. Its exact grammar is
   `\{\{vars\.([a-z][a-z0-9_]*)\}\}`. No whitespace, embedded expressions,
   attributes, function calls or nested placeholders are allowed. A metadata
   value containing additional text is not a valid input-reference expression.
   Ordinary user data containing these characters is never parsed as a reference.
4. IBS parses such references into binding metadata. It resolves a reference
   to a typed value, not a fragment of Python source. A number stays a number;
   a string containing quotes, newlines or Python-like text stays one string.
5. Reusable PythonCode declares its own local input names and types. The Recipe
   maps each local input to a captured task input, a typed constant, or a named
   earlier step result/field. This separates reusable component inputs from
   any particular user-message layout. Unmapped inputs, duplicate bindings,
   incompatible types and forward result references are errors.
6. The step-local input convention is `inputs["local_name"]`. A mapping export
   can define `def use_value(inputs):` and return
   `host.some_tool(value=inputs["value"])` from inside that function. This is
   invocation code, not a top-level call executed while loading definitions.
   Declare the actual export signature/mapping; the retained runner supplies
   the selected invocation's typed arguments without modifying source.
   Do not assume the legacy executor provides the same contract.
7. Pure-logic components perform transformations and formatting explicitly.
   To construct a message or URL, use component code over typed inputs; do not
   embed expressions in placeholders or interpolate values into Python source.
8. A successful step publishes its export's validated return value under its
   stable step ID in the task execution context. Later bindings select that result or a
   declared field as data. A failed producer does not supply a success result.
   Repeated executions require explicit occurrence/checkpoint identities.
9. Parse/validate the selected PythonCode and binding declarations before the
   first effect. Child execution receives the same typed inputs and returns
   typed results. Resume restores the same task's values and component snapshot.

The current plain-substitution mechanism is legacy implementation behavior,
not an alternative v3 binding convention. Do not invent an implemented typed
transport API or escaping helper. If this contract is not supported on the
selected path, record the missing runtime/schema work rather than claiming
target-compliant execution. Tier 1 does not make source substitution safe.

Test quotes, backslashes, newlines, Unicode, braces, `%`, source-like text and
oversized values. They MUST remain data or be rejected before effects.

## 6. Specify steps, bindings and result flow

Write a step table with **one row per actual execution step**:

For each Tool-usage execution step, record its Skill association, ToolSkill UUIDs
and Tool names. Pure steps declare their own export/contracts/dependencies and
approval coverage, without artificial usages or bindings. This inventory tells IBS what must accompany the
assembled code; do not leave it to the orchestrator to guess from prose.

| Step | Channel | Component UUID/class | Input origin | Output contract | Failure action |
| --- | --- | --- | --- | --- | --- |
| Bind operation | Rust | ToolSkill / 13 | Binding descriptor | Prepared binding; no operation/result value | Stop if unavailable |
| Invoke preloaded export | Orchestrator | PythonCode / 22 | Validated inputs or prior result | Named result fields and types | Stop or explicit safe retry |

Record exact selected revisions in the task selection/evidence, not authored
Recipe references. Binding preparation does not create a producer result edge.

Rules:

- **One component per component step, without exception.** Every
  `type: component` step MUST have exactly one UUID in `include`. To use several
  components, create several ordered steps, each referencing one component.
  This applies to ToolSkills, PythonCode and Skill context components. Never
  place several PythonCode UUIDs in one step and expect their bodies to be
  combined. The legacy composer can skip additional PythonCode bodies
  after selecting the first nonempty one; such a step is an authoring error.
  Authoring/validation MUST reject empty or multi-component `include` lists
  for component steps. Verify enforcement; this document does not add it.
- **Internal PythonCode composition is allowed.** The single referenced
  PythonCode component may declare smaller PythonCode components as internal
  includes. The Recipe step still has one `include` UUID. Internal component
  includes are distinct from the Recipe step's `include` list and from input
  references such as `{{vars.name}}`.
  IBS resolves and validates the entire internal graph, rejects missing
  references, cycles and symbol/input conflicts, and pins every nested version
  in the same BuildInstruction manifest. Assembly order and local contracts
  must be explicit; internal composition cannot conceal independent Tool
  dispatches or bypass the dependent-chain rule. Source assembly combines
  validated component code; runtime inputs/results remain typed data.
  The legacy text composer does not implement a general recursive include
  expansion. The retained path resolves declared preload dependencies through
  `retained_source`/`retained_preload`, with exact revisions and export selection.
  An `includes` storage column alone does not select that path or qualify a graph.
- Every Rust ToolSkill binding step MUST be immediately followed by matching
  orchestrator PythonCode that calls the Tool. A binding does not execute it.
- ToolSkill UUIDs stay on the Rust channel. Skill prose stays off that channel.
- Tier-0 executable steps MUST reference class 22, never Skill prose.
- One usage contains one independent `host.<tool>(...)` call. Pure
  logic with zero host calls is allowed. Current retained preloads define and
  return from declared exports; `result = <value>` describes historical body
  execution only. Do not mix legacy bodies into a preloaded namespace.
- Author PythonCode at the smallest useful reusable grain. Separate reusable
  pure-logic operations from Tool usages where their contracts allow it. Search
  for existing component UUIDs before writing new bodies. Assemble programs by
  referencing components; do not copy their source into a large custom body.
  Small means a coherent operation with clear inputs and results, not splitting
  every expression into an otherwise meaningless component.
- The existing dependent-chain exception is conditional: a later call must
  consume the earlier call's direct runtime output as one logical unit, and
  the selected runner must support and qualify every binding, call occurrence,
  result and effect/retry boundary. It never permits multi-UUID component steps
  or a multi-Tool leaf Skill. The currently implemented retained runner admits
  one binding and one Tool dispatch per executable step; it rejects an unbound
  or repeated call. Author separate binding/execution pairs on that path. A
  prose exception or dependency edge cannot enable an unsupported call chain.
- The dependent-chain exception is not a step-reduction objective. Prefer
  separately reusable steps with explicit result handoffs when the runtime
  supports them. Do not grow monolithic Python bodies to make a Recipe shorter.
- Never use `__execute_action__`, `__execute_code_step__`,
  `__execute_actions_parallel__`, `__check_budget__` or `__emit_event__`.
- Do not use forbidden body patterns such as `import os`, `import subprocess`,
  `exec(`, `eval(` or `open(`. Use registered host capabilities.
- Read actual signatures. Do not guess keyword names, return fields or whether
  an error is raised versus returned as data.

For every result edge, name the producing step, exact field and type, consuming
step and argument, and transport/checkpoint mechanism. Distinguish the
export return value, the Tool's payload and any legacy `host.run_program` outer
response (`ok`, `return_value`, `stdout`, `error`). They are not interchangeable;
use the selected retained result contract, not a compatibility response envelope.

The retained ordinary runner provides task-owned context and explicit typed
result bindings across selected steps and waits. It preserves task/attempt/input
isolation; a Python local in one invocation is not an implicit variable in the
next. Use the declared result/field edge, never source interpolation or an
undefined ambient `previous_result`. The producer of a data edge is the
executable export, not its non-executing Rust binding step. Follow
[the result validation boundaries](skills.md#result-ownership-and-validation-boundaries)
when an export transforms the primitive payload. Legacy fresh-step execution
is not the v3 handoff contract. Complete crash continuation and dynamic repeated
occurrences remain separate open work; VM memory is not a durable checkpoint.

## 7. Emit the actual persisted IBS schema

Authoring diagrams may use `channel` and `step_id`. The current persisted schema
uses `StepDescriptionEntry` containing `steps` with `knowledge` and
`stepnumber`. IBS reads this structured array, not `yaml_source`, `goal` or
prose instructions.

The following is a **partial immutable Recipe-document design, not an insert
request or complete revision envelope**. Replace symbolic UUID tokens with stable
identities from the selected compatible usage. IBS resolves version selection into
the BuildInstruction during task-start assembly, not into Recipe `include`
entries. Do not invent a revision field in this Recipe JSON.

```json
{
  "step_descriptions": [
    {
      "desc_idx": 0,
      "label": "Post a fixed readiness reply",
      "yaml_source": "Human-readable documentation of the same two steps",
      "steps": [
        {
          "stepnumber": 1,
          "knowledge": "rust",
          "goal": "Bind the reply Tool",
          "content": "Bind host.post_reply",
          "type": "component",
          "include": ["<RESOLVED_TS_HOST_POST_REPLY_UUID>"],
          "tool_bindings": [
            {
              "tool_id": "<RESOLVED_POST_REPLY_TOOL_UUID>",
              "tool_name": "post_reply",
              "params": {},
              "error_policy": {"policy": "fail"}
            }
          ],
          "dependencies": null
        },
        {
          "stepnumber": 2,
          "knowledge": "orchestrator",
          "goal": "Post the fixed reply",
          "content": "Invoke the reusable reply export with the bound constant",
          "type": "component",
          "include": ["<RESOLVED_REPLY_PYTHONCODE_UUID>"],
          "tool_bindings": [],
          "dependencies": null
        }
      ]
    }
  ],
  "variants": [
    {
      "variant_key": "fixed-readiness-reply",
      "description": "Post exactly Ready. without input slots",
      "step_link": "0:1-0:E",
      "intent_examples": [
        "say ready", "reply ready", "post ready", "tell me you are ready",
        "please say ready", "respond with ready", "send a readiness reply",
        "give me a readiness response", "answer with ready",
        "please post the word ready"
      ],
      "variable_patterns": []
    }
  ],
  "input_layouts": {
    "fixed-readiness-reply": {
      "format": "recipe-input-layout/1",
      "task_inputs": {},
      "steps": {
        "0:2": {
          "answer": {"kind": "constant", "value": "Ready."}
        }
      }
    }
  },
  "invocations": {
    "fixed-readiness-reply": {"0:2": "publish_turn_reply"}
  }
}
```

The existing packaged reply usage declares this mapping export in its class-22
source (see `global_bootstrap_components.rs` in the references):

```python
def publish_turn_reply(inputs):
    return host.post_reply(answer=inputs["answer"])
```

The constant belongs to the Recipe's `0:2.answer` binding; no new fixed-reply
PythonCode or source substitution is needed. Reuse the exact compatible retained
reply Skill/code/ToolSkill/Tool selection and its declared
`publish_turn_reply` mapping export. Its local `answer` input is a required string
and its result is the published message reference. The example follows that
adapter's `answer` keyword; legacy `text` signatures are not interchangeable.
Loading the function publishes nothing; invocation performs the usage.

This new Recipe/layout still needs its own complete workflow review and activation evidence. Reusing qualified components does not automatically
approve a new workflow. Supply all required Recipe metadata and the immutable
revision envelope through the supported owner. The `input_layouts` and
`invocations` records belong in the retained Recipe document; the legacy SQL
Recipe struct alone does not persist or enforce this complete contract.

Schema rules:

- `desc_idx` is zero-based; step ordinals are one-based and strictly increasing.
- `0:1-0:E` selects description 0, step 1 through its end. IBS creates IDs such
  as `0:1`. Verify every range and fallback target against actual selected steps.
- `knowledge` supports `rust`, `orchestrator`, `both`. Prefer separate binding
  and execution steps; do not use `both` to mix classes or bypass pairing.
- `type: component` loads references. `type: text` is annotation only.
  `type: snippet` is rejected. The current enum has no `llm` step type.
- Every component step has exactly one `include` UUID. Multiple components
  require multiple steps, never a longer `include` list. Annotation steps
  are not executable component steps.
- A prose “LLM step” mapped to `text` does not execute an LLM call. Tier-1
  reasoning MUST be wired through the supported execution path and verified.
- IBS partitions channels; it does not itself execute one interleaved list.
  Verify how the runtime installs bindings before the Python uses them.
- Some legacy helpers use empty `tool_bindings`; the packaged retained usage
  above uses an explicit compatible binding. Follow the selected owner's actual
  descriptor/native preparation path. Neither a nonempty metadata array nor a
  ToolSkill reference proves the callable is registered, retained and available.
- Dependency expressions specify component retrieval, not runtime result flow.

## 8. Make failures and completion explicit

Specify missing inputs, invalid inputs, unmatched/ambiguous requests, missing or
unvalidated components, blocked Tools, authentication failure, malformed Tool
responses, external failures, timeout, cancellation and reply failure.

Validate supplied task inputs, selected declarations and bindings before effects.
Validate computed arguments and prior-step results when they become available,
before their consuming dispatch. Do not require a future result to exist before
its producer runs. Missing/null/empty values follow the exact recursive contracts;
empty text or lists are valid when those contracts permit them.

Stop on failure unless explicit supported recovery applies. Never treat an error
payload as success. Follow skills.md's exact retry contract: `max_attempts` is a
positive integer including initial dispatch and survives waits/reclaims. Retries
need explicit eligible outcomes and trusted read-only or durable deduplication
evidence for the exact usage. Deduplicated recovery retains the original key and
arguments. A timeout does not prove absence of effects; unknown completion needs
its declared safety evidence or explicit reconciliation. Never replay completed
effects because result validation, a later step or reply posting failed. Only
actual No-Match enters Tier 2; begun Recipe failures remain failures.

The exact failure format can describe more than the current runner supports.
Follow [skills.md's current runner limit](skills.md#exact-failure-and-retry-contract):
ordinary retained execution presently requires `stop` with `max_attempts: 1`.
A Recipe must not wrap the same logical invocation in new steps/children to reset
that limit or replay an unresolved/completed effect. Explicit reconciliation is
a distinct operation with its own supported contract.

Error-policy metadata is not proof that retries or fallback are implemented by
the selected runner. Trace and verify the applicable runner path, reusing current
evidence. Define one reply owner using the qualified reply usage; a missing
acknowledgement does not authorize another publication. Do not promise exactly-once
external execution without the operation's supported evidence. `builtin.echo`
is diagnostic-only.
History and completion must use the supported orchestration path without
duplicating a reply or terminating the global orchestrator.

Binding grants no permission. The kernel checks current instance-wide Tool
policy before dispatch. Keep external authentication, sandboxing, secrets,
network rules and resource limits. Do not add legacy scoped authorization or
operation-approval leases as simplified-v3 requirements. Do not expose secrets
or claim tokens in model-visible or ordinary Recipe state.

## 9. Store and approve through supported paths

Use the supported component stores, RecipeVariant/intent registration and
seed/integrity workflow. First-party component seeders include
`builtin_bootstrap.rs`, `seed_builtin_host.rs` and extension seeders.
Design the Recipe first, then identify/reuse its PythonCode, required bindings
and real Skill usages, followed by any ExtensionCatalogue update. Pure logic
does not need an artificial Skill or ToolSkill. Persist dependencies and resolve
real UUIDs before submitting the completed dependent Recipe for review; design
order is not permission to store incomplete production references.

Authored installed-instance changes require trusted automated Q1, behavioral
evidence and human Q2 for the changed own workflow/functions. Shipped first-party
components use controlled `system_seed` bootstrap with exact integrity/Q1/behavior
evidence and no additional human Q2 action on the installed instance. A
`source: system` label alone proves neither provenance nor approval. Do not
change seeders or labels to bypass the installed-instance authored path.
Register intent examples through the actual store/seeder workflow; a list in
Markdown alone changes no routing.

For every new/changed Recipe, cover all supported declared variants, step links,
input layouts, binding/export/dependency profiles, typed handoffs, actual effects,
failures, waits, retries, child results, reply/history custody and completion.
Use complete rule/path coverage and meaningful positive/negative/boundary cases;
no arbitrary case quota, infinite input enumeration or matrix of all historical
component versions. Unsupported runtime profiles remain with their existing
owners and cannot be removed silently to claim success. A executes its own Q1;
S obtains this workflow evidence through controlled bootstrap, not the authored
queue. Run actual approved exports in workflow tests without restarting their
own Q1/Q2. Missing component approval cannot be supplied by Recipe success.

A Skill/implementation pair is jointly function-reviewed under
[skills.md](skills.md#own-functional-approval-and-successor-compatibility), even
though two IDs persist. Compatible A dependency updates leave this Recipe's own
revision/approval unchanged; automatic technical re-resolution is not review.
The current complete-Recipe publication API remains an implementation boundary
until its existing owner delivers independent compatible component activation.

Class-50 Scaffold is no future active type, validator or protected root. Inventory
actual flows first; reuse a fitting class-21 Recipe, or author a normal Recipe for
an actually distinct workflow. An empty legacy inventory needs no dummy Recipe.
Preserve original rows and valid old pins; never reclassify their UUIDs. New
consumer revisions use explicit Recipe identities through supported invocation.
A Recipe UUID in a PythonCode-only step does not implement nested Recipe calls:
reuse qualified native child admission with original parent claims/selections,
or assign missing support to the existing runtime owner before activation.

Check selected definitions in the supported interpreter before effects; parsing
alone does not prove behavior or meaning. Trusted Q1 includes the required checks
for the exact subject and validator selection, not merely a syntax pass. Do not
infer that every binding rule has an
implemented validator merely because AGENTS.md requires it. Inspect the actual
validator and report any enforcement gaps.

### Immutable versions and task-start selection

This is the required versioning contract; document any missing store/runtime
implementation rather than claiming that mutable current rows satisfy it.

- A component has a stable UUID and monotonically increasing version numbers.
  Its approved versions are immutable. Editing creates a new draft version;
  it never overwrites the body or metadata of an approved version.
- A new authored version passes Q1 and human Q2 before atomic activation.
  Activation changes which approved version is current; it does not invalidate
  or delete the previous approved version. Existing system-seed integrity rules
  remain applicable to the separate bootstrap path.
- At task start, IBS/composition reads and resolves the newest activated,
  approved version of the matched Recipe and every component it needs from
  one consistent catalogue snapshot. Include Tools, ToolSkills, Skills,
  PythonCode and transitive/context dependencies.
  Check their contracts and associations together; incompatible newest versions
  fail composition rather than silently selecting older versions.
- IBS emits the selected UUID/version/checksum manifest in the BuildInstruction.
  The runtime retains that manifest with the task before execution and uses it
  to assemble/execute the exact selected bodies and bindings. The Recipe stores
  stable UUID references, not version numbers. No component may be resolved
  again as “latest” halfway through a task.
- Activation concurrent with task startup gives the task a consistent snapshot
  from before or after activation, never a mixture caused by racing reads.
- Running and suspended tasks, child steps and continuations keep their selected
  versions. A retry/resume of the same task does not upgrade components. A new
  task selects the then-current newest activated versions.
- A selected Rust Tool version must resolve to its retained implementation
  handle/artifact, not a mutable file replaced under the same name. Tool
  settings/policy are independent live state, not pinned implementation versions.
- Retain selected versions and validated associations while tasks or resumable
  checkpoints require them. Replacement alone requires no new Q1/Q2 approval
  check on an already selected original. Integrity and stale-attempt checks still
  apply; explicit revocation is a separate operation, not an effect of replacement.
- Current global Tool policy remains independent of component versions and is
  checked before every actual Tool dispatch. Component version retention grants
  no old permission and does not freeze global settings.

## 10. Acceptance checklist — complete before claiming success

This checklist establishes a completed runnable/activated Recipe, not a command
list for every intermediate edit. Apply the relevant cases through the selected
real path and reuse still-applicable evidence. Draft work can be reviewed and
tested before activation, but must retain its incomplete/unapproved status.
Do not run unrelated suites to tick a box; do not waive a required case because
its environment is unavailable. Zero selected or skipped required tests are
unverified, not passed.

- [ ] Existing components were searched and reused wherever their contracts fit.
- [ ] Complex behavior is decomposed into purposeful reusable steps; any new
  Rust Tool is justified by a missing primitive, not by reducing step count.
- [ ] Python steps are assembled from small reusable PythonCode components;
  new components have contracts suitable for reuse in other Recipes.
- [ ] Every referenced UUID resolves to the intended class and approved revision.
- [ ] Recipes contain stable UUID references without version numbers; task start
  IBS assembly resolves the newest activated approved versions as one consistent
  snapshot and pins the exact versions in the BuildInstruction.
- [ ] Approved versions are immutable and retained for running/suspended tasks;
  activating replacements neither changes those tasks nor invalidates originals.
- [ ] Resume/child execution uses the recorded versions, while each dispatch
  independently checks current global Tool policy.
- [ ] Every real Skill usage has an exact validated prose/code/binding association
  and own A function evidence or exact S/legacy usage evidence; independent pure exports have their own
  required approval coverage without an artificial zero-Tool Skill.
- [ ] Structured JSON deserializes into the actual IBS types; no symbolic UUIDs remain.
- [ ] Every component step references exactly one component UUID; empty and
  multi-component `include` lists are rejected by the applicable validation path.
- [ ] Internal PythonCode includes are validated, assembled in explicit order
  and version-pinned transitively; they preserve the Tool-call grain rules.
- [ ] `step_link` selects exactly the intended steps in the intended order.
- [ ] Every ToolSkill binding is paired with the correct executable PythonCode;
  selected usage/export resolution is unambiguous and supported call cardinality
  is observed. Canonical backlinks do not create dependency cycles.
- [ ] The selected execution path exposes every required host callable.
- [ ] Public Recipes have at least 10 intent examples; variant routing and negative
  cases pass. Private workflows qualify allowed entry provenance, exact selection,
  typed input/result and rejection of unauthorized public access instead.
- [ ] Public captures were checked through real matching/composition; private inputs
  through their actual admitted typed path. Neither path invents the other's effects.
- [ ] Missing required or invalid inputs fail before their consuming effect;
  empty/null values follow their declared contracts. Known task inputs are checked
  before effects; later computed/prior-result arguments are checked when available.
- [ ] Hostile text remains data or is rejected before effects.
- [ ] Selected PythonCode and typed binding declarations validate before effects;
  runtime values do not modify source. Test legacy substitution only when a
  compatibility change requires it; it is not evidence of retained typed binding.
- [ ] Every inter-step result handoff works through the actual runner.
- [ ] Representative inputs produce the expected Tool arguments, effects and reply.
- [ ] Failure, retry, cancellation and blocked-policy cases behave as specified.
- [ ] Tier-0 execution makes zero LLM calls; Tier-1 calls occur only as designed.
- [ ] The own A workflow has complete Q1/behavior/human Q2 or S has controlled
  workflow qualification and exact Skill-usage evidence; dependencies retain own approvals.
- [ ] Compatible A successor updates preserve unchanged consumer approvals and
  old task pins; coherent activation does not reapprove them.
- [ ] Component seed/integrity checks pass where seeder data changed.
- [ ] Evidence identifies the tested revisions/path and distinguishes unverified requirements.

Use focused real execution evidence under the development policy. Read
`LOCAL_TEST_ENV.md` if present before remote tests or provider setup. Do not run
Cargo solely to validate ordinary prose; embedded runtime/seed content follows
the policy's applicable integrity checks. A Recipe blocked by missing transport,
binding or context preservation remains incomplete, even if its JSON and Python parse.

## Source references

Retained production contracts and the reusable example owner:

- [Typed input layouts and result edges](crates/brassclaw_engine/src/memory/retained_inputs.rs)
- [Immutable dependency graph and class checks](crates/brassclaw_skills/src/component_revision.rs)
- [Retained instruction and task-selection references](crates/brassclaw_engine/src/memory/retained_instruction.rs)
- [Packaged reply/history source, contracts and layouts](crates/brassclaw_skills/src/global_bootstrap_components.rs)

Shared schema and compatibility consumers (read the actual caller before reuse):

- [IBS schema, selection and variable capture](crates/brassclaw_engine/src/memory/instruction_builder.rs)
- [VariablePattern and ToolBinding types](crates/brassclaw_engine/src/types/ibs.rs)
- [RecipeVariant type](crates/brassclaw_engine/src/types/recipe.rs)
- [Legacy PostgreSQL compose path](crates/brassclaw_reborn_composition/src/pg_composition_port.rs)
- [Retrieval capture and component fetching](crates/brassclaw_engine/src/memory/retrieval_source.rs)
- [ComposedProgram assembly and text substitution](crates/brassclaw_engine/src/memory/composition.rs)
- [Legacy host dispatch/run_program consumers](crates/brassclaw_engine/src/executor/orchestrator.rs)
- [Monty parsing and execution](crates/brassclaw_engine/src/executor/scripting.rs)
- [Recipe validator](crates/brassclaw_engine/src/memory/recipe_validator.rs)
- [Q1 orchestration](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs)
- [Recipe store metadata](crates/brassclaw_reborn_composition/src/pg_recipe_store.rs)

## Retained model-input selection metadata (Exact K3-2 successor)

The retained class-21 component document may declare one model-input policy per
existing variant. This is document metadata in the exact reviewed revision, not
a new `reborn_recipes` insertion column, a `knowledge`/`stepnumber` key or a model
prompt argument. The legacy Recipe-row APIs do not acquire this field implicitly.
Use the retained component-document path that preserves the complete document.

```json
{
  "model_input_bindings": {
    "format": "recipe-model-input-bindings/1",
    "variants": {
      "selected": {"selection": "selected", "profile": "defensive"}
    }
  }
}
```

Missing metadata or an omitted variant preserves Default. Explicit
`{"selection":"default"}` is also supported. Selected names an existing profile;
the native consumer resolves its exact compatible qualification/generation only
from the original captured provider/model selection. Missing or incompatible
selection is an error, without Base or catalogue fallback. The original Sempai
interceptor review keeps its own role contract; this declaration selects the
Recipe's Kohai usages. Different workflows/variants may select different profiles.
Current version 1 does not define per-step override fields.

`{"selection":"prefix_free_workflow","purpose":"prefix_generation"}` declares
intentional absence for a qualified workflow. The other declared purposes are
`basic_provider_configuration_values` and `model_design`. These are reviewed
intent, not proof of qualification or permission. **The current native runner
still rejects this mode before activation/execution until its complete retained
workflow, technical encoding, child/review and evidence contract is available.**
Parsing or staging the declaration cannot enable it. This preserves the required
prefix-free bootstrap/generation target without silently using a default prefix.

Unknown versions, fields, variant names, selection tags, null declarations or
malformed sibling declarations fail. IBS retains the chosen policy in its
original Recipe/variant selection as `recipe-model-input-selection/1`; old
selections without this metadata keep their original byte representation.
Approval/activation, original provider/Auth custody, live Tool policy and final
complete-request validation remain separate requirements. See the actual
`recipe_model_input` parser and the Exact plan's open I2 integration gates.


## Construction selection successor and ordinary Recipe Prefixless boundary

The [shared construction-selection successor](docs/reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
owns new job context and its visible UI proposal. Its submit-time exact generation
check is not a version field in a reusable Recipe. The existing Recipe editor's
qualified successor exposes profile/explicit Prefixless choices and labelled
Default as supported, with no Recipe Generate button; normal review and task
capture remain. General Recipe Prefixless needs actual supported declaration/
reader/native guard/response handling, not an invented bootstrap purpose, null,
missing metadata or per-step JSON override. The existing strict /1 example above
and three bootstrap purposes retain their actual meanings and support limits.
Required construction children use their own captured route/context; the preceding
ordinary Sempai role sentence is not an implicit override for these future calls.
Historical jobs and previous acceptance stay exact. Composition owns missing
mechanics; the Prefix/UI owner consumes them before exposure. No implementation,
component approval or activation follows from this target clarification.
