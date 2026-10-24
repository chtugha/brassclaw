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
`host.source_inspection(source=inputs["source"])` call. Its canonical execution
Recipe prepares the matching ToolSkill before invoking that selected export.
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
