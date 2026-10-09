# Completed-turn Sempai review

The workflow is a Recipe executed by the existing global Monty orchestrator.
Pre-dispatch prompt optimization and post-turn component learning are separate
tasks. A prompt-only review cannot substitute for analysis of the completed turn.

## Implemented producer

V122 retains one immutable `monty-completed-turn-review/1` event when an actual
No-Match admission reaches settlement. The event and admission outcome commit
in the same transaction. A rolled-back settlement creates no event; an exact
acknowledgement retry creates no duplicate. Match, disambiguation, routing error
and tasks that never started No-Match do not create Tier-2 review events.
Trusted internal turns are also excluded using the host-owned marker retained
at admission; model or message text cannot opt in or out of this classification.

The event retains public run/turn/conversation/input/runner identities, original
terminal outcome and execution report, exact Recipe selection bytes, recorded
Tool arguments/results/counts and the available forensic packet bytes. Packet
selection is constrained by the original tenant and run and has deterministic
iteration/ID order. Later packet updates cannot rewrite the retained event.
Admission keys, invocation keys, claim checksums and lease tokens are excluded.
Scope identifiers correlate evidence; they do not add feature-role authorization.

A No-Match turn that published its reply and then failed to save history retains
a failed outcome. Review recovery must not repeat its reply, its Tools or its
history write. Unknown effects remain unknown. Existing settlement and invocation
guards retain their authority; this event is not a checkpoint or replay permit.

`evidence_complete=false` is deliberate. The producer does not prove that all
provider iterations were captured, resolve the bounded authoritative transcript,
or include every Tier-2 capability result. The Recipe must qualify these inputs
before presenting a successful task as a reusable pattern. Empty packets or
missing results do not mean zero model work or zero effects. Failed forensic
writes and unreconciled effects must be visible, not silently summarized away.
Old settled admissions are not backfilled with invented settlement-time packet
snapshots. Historical reconciliation is explicit and cannot reopen dispatch.

### Model exchange capture

V127 adds nullable `model_exchange` evidence to forensic packets. The resolved
host interceptor now retains `host-model-exchange/1`: original request JSON bytes,
effective request JSON bytes after validated Sempai adjustment, the returned host
response JSON bytes, or a classified failure observation. Request snapshots
explicitly retain message references, provider-call IDs/arguments/signatures,
structured Tool-result references or resolved safe summaries, and the selected
route/profile. The ordinary message serializer intentionally omits replay data
and must not be used as this evidence serializer. The response includes structured
output, reported usage and available safe text/reasoning deltas.

These are exact snapshots of the host boundary, not provider wire capture or
proof that a provider used a particular cache/prefix. The effective request means
the intended gateway input; an error or missing response does not prove no work
occurred. Preparation failures retain no effective Kohai request. Failed capture
writes remain observable warnings and cannot establish completeness. Cancelled
futures/crashes can leave an unresolved packet. Historical and other capture
paths retain null evidence rather than synthesizing missing data. V122's immutable
settlement snapshot includes the new column for newly settled events; existing
events remain unchanged. Completed-turn delivery and validation still need their
own durable consumer and complete evidence qualification.

## Executable Recipe package

`brassclaw_skills::completed_turn_review_components::drafts` constructs the
private `host-review-completed-turn` Recipe, seven explicitly associated
one-operation Skills and a pure request formatter. The installation owner
supplies real stable UUIDs and registered callable/capability identities; the
factory does not create fictitious Tools or implicit registrations. It emits
actual `knowledge`/`stepnumber` JSON, typed layouts and `python-preload/2`
exports. The full-consumer variant has no intent examples. Each Skill has a
canonical one-usage variant with an exact `post-turn <operation> %` command and
typed `record_bytes` input. All variants are private and must be excluded from
ordinary matching and MCP discovery by the catalogue owner. `verify_package` compares exact
packaged documents and Tool identities; this does not prove native loading,
behavior, combination approval or activation. Each
usage has a matching ToolSkill binding step immediately followed by its associated
PythonCode export; pure logic has its own PythonCode step and no artificial
binding. Pin the complete graph from one approved catalogue generation.

The supported runner is flat and stop-only. The evidence qualification primitive
must durably retain an incomplete-evidence receipt and stop before claim/model
dispatch if evidence is missing. Unknown downstream effects require reconciliation
of exact retained receipts; never rerun the whole Recipe. No original reply,
Tool or history write belongs to this consumer.

The pure formatter requires a host-verified `completed-turn-review-claim/1`:
`attempt_ref`, `bundle_bytes`, `bundle_checksum`, `evidence_complete`,
`prefix_bytes`, `prefix_checksum`, `selection_bytes`. It rejects extra/missing
fields and incomplete evidence. It preserves the pinned prefix before analysis
instructions, keeps untrusted evidence in a user message and exposes no task
Tools. The model primitive must independently verify the request against the
durable claim and enforce pinned routing, policy, accounting and context limits;
checksum strings alone are not proof.

`brassclaw_skills::completed_turn_analysis::decode` strictly parses original
`completed-turn-sempai-analysis/1` response bytes containing `analysis` and
`proposals`. Each proposal has exact `candidate_bytes`, mandatory `base` (null
or an exact reference) and `dependencies`. References require canonical UUIDs,
positive versions and lowercase SHA-256 checksums. Duplicate/unknown keys,
inconsistent bases, duplicate candidate identities, duplicate/self dependencies
and missing direct references fail the entire response. Candidate classes 1–3,
13 and 21–23 are supported here; native Tools, protected roots and scaffold
records need separate authoring machinery. Empty proposals produce zero
candidates. Host-derived submission IDs bind the attempt, exact response bytes,
index and candidate checksum. The actual store still checks full transitive
closures, exact bases and retained bytes transactionally. Parsing does not prove
host API support, prose/code meaning or behavior; structural/Q1 qualification
must reject unsupported executable contracts before activation.

Input: a required canonical non-nil `review_event_ref` UUID string identifying the
original retained event. Input syntax grants no access or execution authority.
The review's own conversation/run/attempt and pinned selections are distinct from
the original task. Results are either a durable submitted-unreviewed receipt, a classified
incomplete-evidence receipt, or an explicit failure requiring reconciliation.

| Ordered usage | Implementation |
| --- | --- |
| Read the immutable event | Registered task-bound primitive reads V122 bytes and verifies SHA-256. |
| Qualify and retain evidence | Resolves the original accepted-message/finalized-reply boundary, model exchange identities and host dispatch counts, typed capability values and effect receipts. Missing/unresolved evidence records an incomplete receipt and stops before model work. |
| Claim review | V129 pins the original event checksum, exact workflow selection, independently captured Sempai provider/model and prefix, with one durable model-dispatch intent. Reservations are never reclaimed automatically. |
| Prepare request | Pure preloaded PythonCode preserves the pinned prefix and separates instructions from untrusted evidence; it advertises no task Tools. The native primitive independently checks every field against its retained claim. |
| Ask Sempai | Calls the retained provider through the explicit `sempai_model` profile with isolated accounting. Actual response/usage or classified unresolved outcome is retained before decoding. Unknown spend retains its reservation. |
| Decode and submit | Strict parser preserves exact analysis/candidate bytes. The real immutable submission store checks bases and dependencies and returns **unreviewed** subject receipts. This is not a Q1 or activation shortcut. |
| Acknowledge | Writes the durable internal review receipt. The global root returns `internal_completed`; dedicated task ports verify that receipt and all executed steps. Ordinary chat ports reject this outcome. |

The post-turn response contract needs analysis and proposals, not adjusted
volatile messages for a Kohai request that has already finished. Prompt redesign
is a separate proposal. Model-written summaries are observations, not behavioral
evidence, completion receipts, approval or Tool grants. Component authoring uses
Q1, actual behavioral validation and human Q2; reviewed installation seeds use
their distinct bootstrap evidence contract. Neither path activates by assertion.

The host retains three messages (prefix, analysis instructions, evidence). The
existing provider gateway merges adjacent system messages into one system
message. At the provider boundary there are therefore two messages: the system
content begins with the pinned prefix, followed by analysis instructions, and
the user message preserves the evidence bundle. Tests must assert the actual
gateway layout rather than expecting an unmodified host-message count. This
normalization does not permit moving evidence into system instructions.

## Delivery and acceptance

With `postgres`, `skills-db` and `root-llm-provider`, runtime startup now creates
an installation-owned review delivery owner before ingress. It submits the
retained private Recipe to the existing global service. Rust owns journal delivery,
registration, fencing and one-operation primitives; Python still owns workflow
sequencing. The consumer does not publish replies, write the original daily memory,
restore original claims or invoke the legacy proposal sink.

Sempai must be configured and `interceptor.sempai_base_prompt` nonempty before an
unreserved event is delivered. The current provider Arc, concrete model override
and prefix bytes are retained independently for each review. Changes affect later
admissions. Internal reviews use their own opaque conversation/run/turn IDs and
produce no recursive Tier-2 events. Source events, operation inputs/answers and
service settlement remain immutable. Reservations with unknown outcomes stay
reserved/uncertain for supervised reconciliation; restart never redispatches them.

Failed service tasks keep their exact native handles, child execution, observed
model responses, Tool answers and bounded retention credits. Shutdown reports
unresolved reviews and quarantines these owners instead of silently destroying
recovery evidence. The operator inspection UI records quarantine observations
without releasing these owners. Actual effect/accounting reconciliation and
general authored-catalogue activation remain separate implementation work. No receipt here proves proposed component
behavior, Q1, human Q2 or full-v3 acceptance.

Acceptance must exercise the ordinary runtime: No-Match completion and a failure
after publication, complete/missing forensic evidence, bounded same-conversation
history, original/effective prompt separation, multiple provider/Tool iterations,
current policy denial, disconnected Sempai, unknown model and storage outcomes,
stale claims, duplicate delivery and component/prefix/model replacement during
review. Verify one publication and no repeat task effects, exact proposal receipts
and no activation before required review evidence. SQL producer tests alone do
not certify this consumer or full-v3 cutover.

### Operator inspection and quarantine observations

Settings → Validation now contains **Post-turn Sempai reviews**. The operator
loads a cursor-paginated list of reserved attempts (50 per page), opens an attempt,
and inspects its original immutable event, pinned work, operation inputs/answers
and recorded settlement. A PostgreSQL statement supplies one coherent envelope;
its exact UTF-8 bytes and SHA-256 identify the observation. The journal-checksum
indicator checks the event link and operation byte checksums. It does not prove
semantic completeness, provider billing, candidate behavior or root quiescence.
Unreserved events waiting for Sempai configuration are not included in this list.

The three operator-only routes use the existing facade and PostgreSQL RecipeStore
adapter: `GET /api/webchat/v2/post-turn-reviews?after=<attempt-uuid>`,
`GET /api/webchat/v2/post-turn-reviews/<attempt-uuid>`, and
`POST /api/webchat/v2/post-turn-reviews/<attempt-uuid>/dispositions`.
Ordinary chat/MCP callers gain no management or dispatch shortcut. Evidence is
private operator data rendered as text, never executable source or HTML. Database
errors are sanitized. An inspection over 32 MiB fails whole; it is not truncated.

Only stopped `failed`, `uncertain` or `incomplete` attempts accept an observation.
The operator supplies a canonical disposition UUID, the inspected checksum and
a nonempty note of at most 16 KiB; actor identity comes from authenticated ingress.
V133 retains the exact inspected envelope and note separately from original work.
Changed evidence requires another inspection. A repeated ID with identical actor,
attempt, checksum and note returns the original receipt, including after an unknown
commit; changed fields conflict. The latest 50 notes and total count are shown.
The immutable original work phase, operation answers and settlement remain intact.

This is the inspection/decision-record prerequisite for a future supervised
reconciliation Recipe. There is deliberately no resume/retry/release operation:
an observation cannot establish whether a provider call or candidate submission
occurred, reconstruct lost native handles, settle accountant reservations, or
activate components. Receipts explicitly report `retention_released: false` and
`work_replayed: false`. The existing review owner/quarantine continues to hold
unknown effects. A future actual reconciliation path must resolve authoritative
model/storage/effect receipts and retained accounting before supported release.

### Recorded development acceptance — 2026-10-09

`runtime::tests::native_post_turn_review_uses_global_service_and_retains_unreviewed_candidates`
passed with real PostgreSQL, the installed global Monty service, retained native
primitives and an explicitly configured deterministic provider fixture. It
verified bounded complete evidence, the actual two-message provider layout,
one Sempai dispatch, seven answered operations, one original assistant reply,
one original learning event, unchanged root VM identity, an actual immutable
unreviewed submission receipt, no activation and successful shutdown. Replacing
the provider and prefix after reservation did not change that review's pinned
provider, explicit model override or prefix. The candidate declares a
`python-preload/2` export and typed input/result contracts; retaining it does not
qualify or activate its behavior.

The existing `native_global_runtime_retains_one_root_across_match_and_no_match`
regression also passed. A separate isolated PostgreSQL check exercised V129's
reserved → qualified → claimed → model_dispatching → model_returned transitions.
These are wiring and storage evidence using a provider fixture, not an Ornith
quality benchmark, billing acceptance or completion of the full failure matrix
above. Strict composition lint validation was blocked by the concurrently edited
MCP bridge's `verify_cached` argument-count warning; no consumer lint warning was
reported in that run. Actual supervised effect/accounting reconciliation and
general authored-catalogue activation remain separate work.

### Operator inspection development acceptance — 2026-10-09

`pg_review_submission::tests::native_http_review_inspection_preserves_quarantine_and_exact_observations`
passed through the mounted operator HTTP routes with real PostgreSQL and V133.
Its synthetic stopped journal fixture checks authentication, live-work rejection,
late-answer/stale-checksum rejection, exact pinned envelope retention, ingress-owned
actor identity, duplicate-ID readback, conflicting retries, unknown request fields,
failed deferred commit with sanitized errors, immutable update/delete/truncate
protection, cursor behavior, invalid IDs and missing attempts. The original work
phase and dispatch count stay unchanged; no settlement is inserted. This proves
management behavior, not execution of a real provider effect or its reconciliation.

Both complete WebUI descriptor-policy contract tests pass. Four frontend interaction
tests pass for exact-ID retry after an unknown save outcome, rapid double saves,
running-work write exclusion and missing secure randomness. JavaScript syntax and
`git diff --check` also pass. These component interaction tests do not establish
browser rendering or a deployed WebUI; no remote deployment was changed.

Strict affected-package linting was attempted for `brassclaw_product_workflow`,
`brassclaw_webui_v2` and `brassclaw_reborn_composition` with `skills-db` and
`root-llm-provider`. It stopped in the concurrently modified dependency
`brassclaw_monty_host/src/service.rs:178` on `clippy::large_enum_variant` for
`SettingsPublication`, before the affected crates completed. This check is
blocked, not passing; no lint suppression or unrelated host change was made.
