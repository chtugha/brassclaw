# Retained component input metadata

The append-only `component-revision/1` envelope stores an exact immutable
`document` and stable UUID dependencies. It does not activate a component or
establish Q1, human Q2, combination approval or Tool implementation availability.
The Recipe document retains the actual `variants` and `step_descriptions` schema
from [recipe.md](../../../recipe.md). The following metadata is a new supported
retention-document interface; legacy Recipe/PythonCode INSERT APIs do not accept
it and existing rows are not automatically migrated.

Each retained PythonCode document declares `input_contract` and
`result_contract`, using the recursive contracts from
[skills.md](../../../skills.md). These declarations are checksummed together
with `content` and dependencies. A Recipe document adds `input_layouts`, keyed
by embedded `variant_key`. Its selected value has exactly:

```json
{
  "format": "recipe-input-layout/1",
  "task_inputs": {
    "value": {"type": "string", "required": true, "checks": []}
  },
  "steps": {
    "0:1": {
      "text": {"kind": "task_input", "reference": "{{vars.value}}"}
    },
    "0:2": {
      "previous": {"kind": "result", "step_id": "0:1", "path": ["text"]}
    }
  }
}
```

`steps` covers exactly the selected orchestrator steps and each step's declared
local inputs. Binding records have one of these exact shapes:

- `{"kind":"task_input","reference":"{{vars.name}}"}` uses the whole-value
  grammar from recipe.md. User values are never parsed as references.
- `{"kind":"constant","value":...}` supplies typed constant data.
- `{"kind":"consumer_default"}` requires a valid declared consumer default.
- `{"kind":"result","step_id":"0:1","path":["field"]}` selects an earlier
  result or required object field path. Empty `path` selects the complete result.
  Forward references, optional fields without guards and list indexing are not
  supported by this straight ordered layout.

`prepare_retained_inputs` reads contracts and declarations from the same
`RetainedRecipeInstruction` snapshot and checks complete mappings and compatible
data edges before execution. Concrete task and step input binding applies defaults
only to missing values. Concrete result validation never invents data. Technical
capacity errors reject the whole input; they do not truncate it. Monty owns the
task inputs and result context and resolves the prepared references; host adapters
must validate concrete values against these retained contracts before dispatch.

`bind_variant_example` captures only an example present in the selected immutable
variant, requires complete prefix/suffix and every positional separator, and uses
the authoring guide's first-separator convention from left to right. It accepts
no raw-slot fallback, undeclared/duplicate names or replacement of already
supplied task inputs. Refinement must match the whole slot and use only the
declared named group, if any. Strings remain strings; reviewed logic components
perform explicit numeric/boolean conversion. Capture regexes are compiled once
during preparation under technical limits of 64 captures, 32 KiB compiled regex
and 8 KiB DFA cache per capture. This does not select a mutable intent record or
establish that an example passed behavioral review.

`monty_flow` emits the existing `recipe-flow/1` data interface. It retains ordered
step identities and prepared references and returns the last declared result;
the global Python helper executes that flow using the composition response's
bound `inputs`. Technical node/path capacity is checked before handing it over.
`prepare_retained_unbound_program` uses the existing typed composer over exact
retained PythonCode bytes. Tool workflows and nested code require their exact
artifact/assembly adapters; this constructor explicitly rejects those cases.

`prepare_retained_tool_program` prepares a Tool workflow without loading or
dispatching an implementation. A retained ToolSkill document's `binding` has
exactly these fields:

```json
{
  "format": "tool-skill-binding/1",
  "tool_uuid": "<stable Tool UUID>",
  "callable": "host.post_reply",
  "capability_id": "post_reply"
}
```

The retained Tool document declares the same explicit `callable` and
`capability_id` plus its recursive `input_contract`. The ToolSkill declares the
Tool UUID in its dependency set. The Recipe's explicit IBS Tool binding agrees
with that mapping, uses empty `params` and `error_policy:{"policy":"fail"}`:
binding supplies availability, while executable Python supplies actual arguments.
Legacy substitution, Ignore/Fallback and implicit retries cannot prepare a v3
binding. The durable retry/effect adapter must implement the separate failure
contract before retry policies are admitted.

The same retained graph must contain exactly one matching Skill association for
each ToolSkill/PythonCode pair. Its PythonCode input/result contracts agree with
the association, and its declared arguments fit the primitive's retained input
contract. The prepared binding retains all four exact component references and
the original association bytes, keyed by the executable step. It validates
actual Python arguments against the usage and primitive contracts. Nested code
still needs its separate retained assembly adapter. No mutable path-based load
directive is emitted. Preparation does not establish Q1/Q2, combination approval,
an implementation handle or Tool permission; the global host must resolve those
independently before exposing the callable.

This interface currently supports straight ordered dataflow. Guarded control
trees, active-catalogue matching, full component assembly, trusted catalogue activation,
exact Tool artifacts and the ordinary global production task adapter still need
their separate implementation and acceptance. Passing draft preparation tests is
not production cutover or proof of behavioral approval.

`RetainedToolBinding::combination` identifies the complete selected dependency
closure of its Skill usage, including the Skill owner. It excludes unrelated
Recipe steps. Repeated uses of the same Skill share the closure; a technical
aggregate limit rejects oversized combination metadata without truncation.
An approval declaration must match this exact combination, and its references
must separately resolve to trusted successful evidence before ordinary execution.

`RetainedRecipeExecution` is a step hosting primitive, not a workflow selector
or approval service. The global Monty helper requests a selected step and passes
its prepared concrete `inputs`. The primitive verifies retained code/contracts,
uses one task-owned child context and handles mechanical worker boundaries.
Its Tool port must retain the exact implementation and use the actual kernel,
current policy, admission fencing and durable effect adapter. Unsupported retry
policies are rejected. Actual host answers, worker errors and command-ledger
origins remain private evidence after a later failure; they are not replay grants.
An execution/output failure or a dropped active feed prevents further feeds on
that execution. Instance settlement must release/reconcile actual contexts and
started host operations. This primitive does not certify those durable gates.

`PgComponentRevisionStore::read_exact_in_transaction` lets the catalogue owner
read exact revision bytes in the same repeatable-read/serializable view used for
matching, active selection and approval resolution. It rejects weaker isolation
and leaves commit/rollback with that owner. It supplies no active/latest lookup
or approval conversion for legacy validated rows.

## Immutable authored review record retention

`V102` adds append-only `reborn_skill_association_approvals` and
`reborn_component_review_evidence`. These stores have no public authoring or
approval writer, and do not import legacy `validated`, `source=system` or
`q2_actor` markers as v3 evidence. Required trusted Q1, behavioral-validation,
authenticated human-Q2 and controlled system-bootstrap producers remain work.
A database/storage fixture is not observed behavioral acceptance or human review.

`association_review_store::retain_authored_association_reviews` reads the exact
`skill-association-approval/1` bytes and referenced immutable evidence in the
caller's repeatable-read/serializable transaction. This adapter addresses
references using canonical non-nil UUID strings. It checks the complete selected
usage graph against the actual immutable revision rows in that same view, then
checks Q1, every behavioral record and human Q2 against that exact combination.
Missing/failed evidence, wrong classes/revisions/checksums, overlapping stage
identities, duplicate behavioral references and mismatched Q2 review sets fail.
System-seed records explicitly remain unsupported until their distinct integrity
and observed-behavior producer adapter is implemented. No automatic downgrade to
authored or legacy receipts is allowed.

The infrastructure evidence envelope `component-review-evidence/1` has exactly:
`format`, `evidence_id`, `kind`, `validation_mode`, `association_checksum`,
`components`, `succeeded`, `report`, and `reviewed_evidence`. For this authored
reader, kinds are `q1`, `behavior` and `human_q2`, mode is `authored`, and a
successful record must contain a structured report. Components use the exact
four-field references from `skills.md`. Q2's `reviewed_evidence` must equal the
Q1 plus complete behavioral-reference set of its approval. Other stages carry
an empty set. This envelope stores provenance supplied by the trusted producer;
its shape or a self-authored success report cannot establish that provenance.

Record SHA-256 covers exact bytes. SQL rejects updates, deletes and truncation.
The reader retains actual approval, evidence and selected revision bytes privately;
they are not model-visible state. Technical aggregate capacity rejects the whole
read rather than dropping records. This primitive does not activate a catalogue,
validate Recipe/pure-logic meaning, verify native implementation artifacts or
issue a Tool grant.

`memory::retained_reviews::retain_prepared_tool_reviews` connects IBS's prepared
bindings to this record reader. Its exact approval-ID map must cover precisely
the selected Skill usages, with no missing, unrelated or reused approval IDs.
Repeated uses retain one record set for that Skill's complete selected closure.
Only a consistent selecting transaction is accepted. Ordinary catalogue/admission
wiring must additionally enforce producer provenance, Recipe approval and active
generation; the draft behavioral-validation path does not imply those gates.

## Stable Tool identity and live policy

`LiveStableToolPolicy` supplies the existing kernel authorizer port using rules
keyed by stable Tool UUID. A trusted capability-to-Tool mapping and all technical
rules are published together at one revision. All retained versions and registered
aliases mapped to a Tool read the same live allow/block rule. Missing mappings
fail closed. Binding preparation can compare `tool_identity(capability_id)` with
the retained Tool UUID before supplying an implementation.

Publication cannot remove an existing mapping or reassign it to another Tool.
A running/suspended task may still retain that capability's implementation;
blocking changes the Tool rule rather than detaching its old dispatch identity.
Proven-drained mapping reclamation and durable settings publication remain
separate lifecycle work. The mapping is trusted registration data, not evidence
that two differently named primitives implement the same approved operation.
The ordinary production policy/store cutover and artifact/ABI verification still
need their acceptance; this source does not certify that cutover.


## Matching and IBS in one database view

`resolve_intent_in_transaction` reads intent matching in its caller's
repeatable-read/serializable transaction, without committing or mutating score
telemetry. Ordinary `resolve_intent` still increments the selected row. SQL and
snapshot failures remain errors, never No-Match. This lookup does not establish
an activated v3 catalogue or migrate legacy intent rows to approved generations.

`compile_matched_retained_recipe` links that exact match to one embedded variant
in the retained Recipe revision using its exact intent expression and step link.
Absent/mismatched metadata, non-Recipe results and multiple compatible-looking
variants are explicit errors. A shared link does not identify an input layout;
the compiler must not choose the first variant or reread latest. It retains the
existing variant, link, selected order and revision graph through actual IBS.
Approval, activation and actual implementation checks remain separate gates.

Legacy disambiguation now verifies the stored row UUID, component UUID/class and
scope under a row lock, commits scoring, and returns the actual template/link.
Ranking limits distinct workflows after deduplication/spread filtering, so many
matching templates cannot hide a different workflow. Candidates carry row/link
identity through the host/PKR JSON mapping. None of this authorizes a resume to
rematch an already selected v3 task.


## Durable stop-only selection and invocation evidence

`PgMontyAdmission` retains a Recipe's exact revision, embedded variant, input
layout, selected order, workflow classification and full component references
before an executable feed. V103 stores this selection immutably by run and Recipe
UUID. Exact repetition is idempotent; a changed selection is rejected, including
at a step that has not executed yet. A different Recipe in the same run has its
own selection and local step namespace. This stores selection, not approval.

The retained executor supplies its actual task handle and selected step to the
trusted Tool port at the observed child HostCall. `begin_tool_invocation` locks
and checks the real turn claim and admission, verifies the unchanged selection,
then commits exact binding/association references and arguments before dispatch.
The unique run/Recipe/step key fences a repeated flat invocation, including an
uncertain prior call. Only initial `stop`/`max_attempts=1` is supported. Repeated
workflow occurrences, retries and durable deduplication require their separate
logical invocation/continuation contract; these records do not authorize them.

`record_answer` persists the actual Return/DomainError/TerminalError before child
resume or output validation. Its private original-address handle may record a
late answer after cancellation. Exact answer repetition is idempotent; a changed
answer, reset, deletion or truncation fails. Storage failure retains an uncertain
dispatch intent and never grants replay. The instance owner must keep the actual
host answer until persistence/reconciliation succeeds. SQL and host errors stay
classified; arguments and private keys have no diagnostic/model-visible surface.

The native behavioral caller exercises actual PostgreSQL, IBS, the global Monty
Recipe helper, its child execution and the real JSON kernel handler. It preserves
a successful Tool result through failed output validation, cancellation and a
later live-policy denial. A separate actual-database failure case rejects answer
persistence after the real Tool returned, then stores its late original result.
These are unapproved draft validation paths, not ordinary application acceptance.

Trusted review/activation and implementation provenance remain required. The
normal global task factory must invoke selection retention before the first
feed, integrate the journal with the actual dispatch adapter, and retain task
snapshots/approval IDs through waits and recovery. This prerequisite supplies
neither a production factory nor a claim of completed external effects.


## Contained executable source preflight

`RetainedRecipeExecution::new` requires an `Arc<InspectedRetainedProgram>` made
by actual contained inspection of that exact retained program. Every use is
checked against its prepared binding; repeated uses of one immutable code
revision share one inspection. The currently supported adapter accepts pure
logic with zero host calls and single-Tool steps with one matching direct host
call. Dynamic host access, unsupported imports/intrinsics, missing result stores
and unbound/multiple call sites fail before executable feeds. Unsupported
dependent-chain binding coverage remains an explicit adapter limitation, not
an authoring prohibition. Actual worker errors retain their original source
and receipt privately.

The immutable inspection can be shared across task hosts. Parser observations
do not prove reachability, global result assignment, behavior or prose/code
agreement. Q1 semantic review, real behavior, human Q2, catalogue activation,
actual Tool implementation provenance and live kernel policy remain separate.
