# Nonexecuting Python source inspection

`builtin.source_inspection` performs one mechanical operation: inspect complete
Python text and return parser facts. It does not run the candidate, choose class
checks, decide a validation verdict, write evidence or activate components.
The validator Recipe owns those steps.

The single-source profile accepts exactly `{"source": "..."}`. Source is a required non-null
string; quotes, Unicode and template-looking text remain unchanged. Empty Python
text is parseable; a consuming validation rule may separately require content.
Caller-selected workers, limits, host objects and extra fields are rejected.
The same Tool also accepts [one complete named inventory](source-inventory-inspection.md)
as `{"sources":{"ID":"..."}}`, with aggregate limits and mechanical compiler
categories. Exactly one profile is allowed; neither returns a review verdict.
The preloadable `inspect_python_source(inputs)` Skill export makes one
`host.source_inspection(source=inputs["source"])` call. Its separate reference
Recipe prepares the matching ToolSkill before invoking that selected export;
that consumer is not required for the Skill to exist or be reused elsewhere.
These component drafts require qualification and coherent activation separately.
Ordinary startup now retains both usages' immutable drafts with the actual native
Tool identity and one shared ToolSkill. The inventory's canonical command Recipe
decodes a JSON list of ID/source pairs, calls the inventory export and publishes
the exact report through the existing reply usage. The pure decoder rejects
duplicates before making a map. Operator successors, original revisions and live
blocks are preserved; retention does not activate these commands or add evidence.

The result is exact JSON text with `format="python-source-facts/1"`, the
acknowledged `preparation_revision`, retained `worker_checksum` and `facts`.
Facts include the source checksum, observed call sites/imports/references and
available preload/function-scope structure. They are observations, not proof that
all paths satisfy a contract or agree with prose. Parsing success is not Q1.

Ordinary `skills-db` startup registers the native built-in handler. After global
Monty and its settings owner acknowledge readiness, the installation owner binds
a private retained copy of the packaged worker and the effective settings view.
A fixed, nonexecuting parser probe checks that copy's protocol and basic facts.
Ingress starts afterwards. An unavailable/unbound backend fails closed.
Retaining bytes and passing that probe do not establish component approval or a
general artifact loader/recovery contract.

Each invocation captures acknowledged settings once. Settings → Monty VM controls
source/compiled-source size, value depth/nodes/bytes, IPC frame size, disposable
worker memory and host response timeout. The native-artifact limit controls the
startup worker-copy admission. Existing retained artifacts are not truncated by
later edits. There is no additional shared built-in 1 MiB input/output fallback
or fixed wall-time ceiling for this Tool. Oversized data rejects the whole
operation. A later edit governs later inspections and cannot retime an accepted
worker operation.
An unsafe reduction that cannot accommodate the retained global root is rejected
before persistence; it cannot destroy the permanent orchestrator to fit a form
value.

Dispatch uses the existing kernel and current instance-wide Tool policy. The
result accounts for actual worker/process and elapsed-time observations and
serialized output bytes. Parser rejection, unavailable workers, containment
failure and capacity limits stay errors; none becomes No-Match or a successful
review. Errors expose classifications without printing private candidate text.
Inspection workers are disposable host primitives, not additional orchestrators;
the one global Monty service remains alive.

This facility supplies a validator prerequisite. Full eight-class review,
semantic/behavioral coverage, trusted verdict persistence, private approved
validator selection and production Q1/Q2 activation remain separate gates.

## Complete retained draft reference

Reuse [source_inspection_drafts](../../crates/brassclaw_skills/src/validator_components.rs)
and its existing ordinary-runtime
[complete native/draft preparation test](../../crates/brassclaw_reborn_composition/src/runtime/tests/source_inspection_tests.rs).
These are the complete record/constructor sources, not a symbolic JSON skeleton.
The installation [identity/retention owner](../../crates/brassclaw_reborn_composition/src/source_inspection_components.rs)
resolves the registered native Tool UUID and artifact, derives stable package IDs,
and retains immutable drafts without activating them. Tests allocate offline IDs;
never copy them as live identities. No new `host.inspect_source` Tool is needed.

| Record/boundary | Actual factory contract |
| --- | --- |
| Native Tool | Registered `builtin.source_inspection`, callable `host.source_inspection`, exact retained implementation/worker and current policy |
| Class-13 descriptor | `tool-skill-binding/1` with that Tool UUID, callable and capability ID |
| Class-22 definition | `inspect_python_source(inputs)` returns `host.source_inspection(source=inputs['source'])`; no call during load |
| Preload | `python-preload/2`, mapping parameter `inputs`, declared export/symbol/default `inspect_python_source`; no private functions/constants/imports/dependencies |
| Skill/interface/association | Class 1, `skill-interface/1` and `skill-association/1`; required non-null `source:string`, code argument map `source → source`, result string; exact Tool/descriptor/code IDs, no Recipe back-edge |
| Failure | `action:stop`, `max_attempts:1`, `idempotency:not_assumed`, null evidence, no retryable outcomes |
| Recipe variant | `selected`, `step_link:0:1-0:E`; step 1 `knowledge:rust`, descriptor include and ToolBinding `{tool_id: actual Tool UUID, tool_name:source_inspection, params:{}, error_policy:{policy:fail}}`; step 2 `knowledge:orchestrator`, code include |
| Typed layout/invocation | `input_layouts.selected` is `recipe-input-layout/1`; required task input `source`; `steps['0:2'].source` is `{kind:task_input, reference:'{{vars.source}}'}`; `invocations.selected['0:2']='inspect_python_source'` |
| Closure/preparation | Retain native Tool draft plus factory drafts through existing store; `read_exact` → `compile_retained_recipe` → `prepare_retained_tool_program` → `InspectedRetainedProgram::inspect_with_limits` |
| Fixed caller | Existing inspector produces `result = inspect_python_source(inputs=inputs)` separately from definitions |

The native result is **JSON text** (`python-source-facts/1`), not a Dict. Bind the
whole returned string to a separately qualified JSON-parse usage if later checks
need fields. Inspecting source never executes it, writes a verdict or grants Q1.
The factory's reference variant is a constrained draft, not evidence of public
intent/MCP qualification or an ordinary approved validator route. Keep actual
component revision checksums and dependency/association evidence from the store;
do not fabricate approval refs. The referenced runtime test exercises registration,
nonexecution, limits/policy and preparation; its old result is not a new run here.

The [inventory factory](../../crates/brassclaw_skills/src/validator_source_inventory.rs)
reuses the same native parser/descriptor for its separate typed `sources` usage.
Private callers supply the map directly; the command workflow uses its existing
pure ID/source-pair decoder and existing reply usage. Duplicate IDs reject before
map construction. This is real source-level reuse, not newly claimed deployment
or a reason to add a second parser or arbitrary-source execution endpoint.


## Shared review subject reader and Skill interface check

The existing checking components share a private class-22 dependency,
`review_subject_reader_draft`, retained by `retain_validator_check_drafts` under
`validator:review-subject-reader`. It exposes no public command or default export.
Its declared private `_validator_review_subject(inputs)` reads the complete typed
subject string once and returns parsed subject/components within that invocation.
`/1` single-root and `/2` Skill/code function subjects remain distinct. It validates
closed shapes, reference/role/association identities, duplicate selections and
comparison lineage; both bases stay comparison data. A changed or unchanged
partner appears exactly once. It authenticates neither caller JSON/checksums nor
prior approval. Original sealed custody remains the native review owner's job.

Source extraction and contract/definition/graph checks reference this one pinned
library through explicit preload dependencies. Their existing public signatures
stay unchanged; source extraction still returns the exact UUID-to-source map.
They do not repeatedly serialize the complete selected graph into an intermediate
public envelope. The existing pure-call rule retains its pure-only scope: a
Tool-using Skill must use its separate call/binding checks, rather than be rejected
merely for being effectful. Retained old tasks keep their old code/dependencies.

`skill_interface_draft` provides the pure `check_review_skill_interface(inputs)`
export with a required `subject_json` string and the normal `skill_interface`
rule-result contract. It checks complete function subjects, usage prose presence,
closed `skill-interface/1`, implementing UUID, declared export signatures and
private/dependency names. Inputs/result/failure agree with the exact association;
boolean and number values remain distinct. Recursive grammar and typed export
shapes reuse existing checking code. Additional implementing exports or different
compatible implementation schemas are not rejected through blanket equality.

These are declaration/shape checks, not complete joint validation. Native source
facts, reachable helper/call/binding coverage, schema variance and still-needed
consumer contracts, actual isolated behavior/effects/errors/retries, prose meaning,
trusted run/attempt/interpreter/settlement and authenticated Q2 remain separate
requirements. No checker runs candidate source, writes evidence, establishes S
provenance or activates anything. Unchanged context receives no new own approval.
The existing single-class preparation refusal for function subjects remains;
these helpers must not be used to turn a partial result into a whole Q1 pass.
