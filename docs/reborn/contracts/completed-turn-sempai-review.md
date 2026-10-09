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
provider iterations were captured, preserve the effective adjusted prompt as a
separate complete record, resolve the bounded authoritative transcript, or
include every Tier-2 capability result. The Recipe must qualify these inputs
before presenting a successful task as a reusable pattern. Empty packets or
missing results do not mean zero model work or zero effects. Failed forensic
writes and unreconciled effects must be visible, not silently summarized away.
Old settled admissions are not backfilled with invented settlement-time packet
snapshots. Historical reconciliation is explicit and cannot reopen dispatch.

## Recipe authoring record

This is an authoring record, **not activated Recipe JSON**. Resolve real stable
UUIDs and supported usages before emitting `knowledge`/`stepnumber` JSON. Each
usage has a matching ToolSkill binding step immediately followed by its associated
PythonCode export; pure logic has its own PythonCode step and no artificial
binding. Pin the complete graph from one approved catalogue generation.

Input: a required canonical non-nil `review_run_id` UUID string identifying the
original retained event. Input syntax grants no access or execution authority.
The review's own conversation/run/attempt and pinned selections are distinct from
the original task. Results are either a durable reviewed receipt, a classified
incomplete-evidence receipt, or an explicit failure requiring reconciliation.

| Ordered usage | Typed input and output | Current prerequisite |
| --- | --- | --- |
| Read the immutable review event | `review_run_id` → exact event bytes/checksum | Add a qualified read usage over V122; the existing component-DB primitive only reads documentation rows. |
| Resolve and retain the complete evidence bundle | Event reference → original/effective prompts, bounded conversation history, model responses and usage, capability outcomes, Recipe selections, terminal status and completeness diagnostics | Extend supported evidence adapters; use the accepted message and finalized reply boundaries, excluding future/unrelated messages. Do not reread latest component heads or flatten typed Tool replay into text tuples. |
| Claim a deduplicated review attempt | Exact bundle reference and independently selected Sempai model/prefix/Recipe graph → durable attempt reference | Implement claim/count/unknown-provider-outcome retention. No automatic lease-expiry redispatch of an unknown model call. Model/profile/prefix changes cannot silently change an existing attempt. |
| Build the analysis request | Exact bundle + Sempai task instructions → typed model request | Pure logic; untrusted task text remains data. Preserve complete evidence subject to real model context/transport constraints; reject or explicitly qualify unsupported size rather than truncate silently. |
| Ask Sempai | Pinned request → actual provider response and usage/error receipt | Reuse the model primitive with an explicit supported Sempai route, current policy, isolated accounting and technical limits. The original Kohai route is not a substitute. No task Tools are exposed to this analysis call. |
| Validate proposed components | Original response bytes → typed proposals and diagnostics | Reject duplicate JSON keys before conversion to Value, unknown contract fields, invented host APIs, invalid references and unsupported component contracts. Report unsupported proposals rather than pretend they entered validation. Empty proposals are valid and mean nothing was exported. |
| Retain and submit candidates | Valid proposals + stable submission IDs + exact dependency graph → immutable unreviewed subjects and supported Q1 submission receipts | Reuse immutable review-submission storage and the durable validator owner. The legacy sink only supports classes 21/22 and lacks unknown-commit idempotency and complete association support; it is not a full-v3 acceptance shortcut. |
| Record review completion | Exact provider/proposal receipts → durable review receipt | Acknowledge only the reviewed bundle. Retry an uncertain storage commit with the original IDs/bytes. Retain rejected/incomplete evidence and diagnostics without replaying the original turn. |

The post-turn response contract needs analysis and proposals, not adjusted
volatile messages for a Kohai request that has already finished. Prompt redesign
is a separate proposal. Model-written summaries are observations, not behavioral
evidence, completion receipts, approval or Tool grants. Component authoring uses
Q1, actual behavioral validation and human Q2; reviewed installation seeds use
their distinct bootstrap evidence contract. Neither path activates by assertion.

## Delivery and acceptance

The durable event table is a producer journal. **There is not yet a qualified
consumer, delivery acknowledgement or active post-turn review Recipe.** Add
delivery through the ordinary supported work admission into the already-running
global Monty service. Rust owns storage, wakeup/transport and fencing; it must not
become another workflow loop or call Sempai inline from settlement. Admission of
internal review work must not recursively enqueue itself as a new Tier-2 example.
When Sempai is disconnected, events stay retained; reconnect can deliver pending
qualified work. Delivery failures must remain observable without replaying the
original task or silently dropping review work.

Acceptance must exercise the ordinary runtime: No-Match completion and a failure
after publication, complete/missing forensic evidence, bounded same-conversation
history, original/effective prompt separation, multiple provider/Tool iterations,
current policy denial, disconnected Sempai, unknown model and storage outcomes,
stale claims, duplicate delivery and component/prefix/model replacement during
review. Verify one publication and no repeat task effects, exact proposal receipts
and no activation before required review evidence. SQL producer tests alone do
not certify this consumer or full-v3 cutover.
