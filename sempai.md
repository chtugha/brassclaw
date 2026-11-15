# Sempai — design and authoring instructions

> **Current status — 2026-11-14:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


This guide defines what Sempai is, its two distinct usage contracts (pre-call
correction and completed-turn analysis), and the requirements for authoring and
changing its components. MUST means required. Read [recipe.md](recipe.md),
[skills.md](skills.md), [tools.md](tools.md), [toolskills.md](toolskills.md),
[simplified_v3.md](simplified_v3.md), [kohai.md](kohai.md) and
[the development policy](docs/development-policy.md). The binding prefix/pair-design
contracts are owned by
[docs/reborn/model-token-limits.md](docs/reborn/model-token-limits.md). The
concrete Recipe-migration work is owned by
[docs/plans/sempai_v3.5.md](docs/plans/sempai_v3.5.md) and
[path_to_v3.5.md](path_to_v3.5.md). This guide specifies what Sempai is; it does
not claim completed runtime enforcement or migration.

## 1. What Sempai is

**Sempai is BrassClaw's optional reviewer and learning component**: a second LLM
usage that runs either before the final answer model call (pre-call correction) or
after a completed No-Match turn (completed-turn analysis). It is not a second agent
loop, not a fallback for failed Recipe execution, and not a parallel orchestrator.

Sempai is a role that describes the responsibility boundary of:

- the **interceptor** (pre-call correction path), and
- the **completed-turn analysis Recipe** (post-turn proposal path).

Both paths use their selected Sempai provider/model. Ordinary independent review
uses its own generated reviewer prefix; qualified construction reviews follow
their own captured context choice as described in section 4. The v3.5 target
extracts shared reusable PythonCode components for message construction, model
dispatch and strict response parsing (see section 8 and
[sempai_v3.5.md](docs/plans/sempai_v3.5.md)). The current compatibility interceptor
still constructs/parses review messages in Rust; actual broader migration and
acceptance scope stays with the implementation index and existing owners.
Both paths have distinct entry conditions, inputs, results and completion semantics.
Do not collapse their eligibility, domain result or completion contracts. Shared
transport, message construction, parsing and pure helpers may be reused when their
exact input/result/effect contracts fit; distinct workflows do not require
duplicate primitives or code. Different one-Tool usage contracts still need their
own represented/qualified Skill interfaces, not an untyped dispatch wrapper.

## 2. Two distinct contracts — never merged

### 2.1 Pre-call correction

| Field | Contract |
| --- | --- |
| Input | The already-formatted dynamic Kohai prompt for the final target, with trusted target/pair facts and task-scoped context. Protected reference reconstruction is the unimplemented v3.5 target design in section 6.2. |
| Purpose | Optimize or correct the dynamic prompt before the final model call |
| Result | The final dynamic prompt to be used by Kohai, unchanged by Kohai before insertion |
| Completion | Kohai inserts the intended finished target prefix exactly once before the returned dynamic prompt, checks the complete request, and dispatches. No further dynamic-prompt formatting by Kohai. |
| Eligibility | Existing optional connected/enabled pre-call path |
| Sempai prefix | Used for Sempai's own review call only; not returned as Kohai content and not inserted into the target request |

### 2.2 Completed-turn analysis

| Field | Contract |
| --- | --- |
| Input | Immutable completed-turn evidence, original task/selection references and original No-Match outcome |
| Purpose | Propose new Recipes, intent examples, component updates and other learning candidates |
| Result | Analysis and any unreviewed candidate proposals under the post-turn envelope |
| Completion | Retain original answer/evidence and unreviewed candidate subjects through existing submission owners. Later qualification, human Q2 and activation are separate; never edit or replay the original turn. |
| Eligibility | Eligible original No-Match turns only. Matched turns, failed Recipes and validator/reviewer tasks do not enter this path |
| Original turn | Never replayed, re-dispatched or reconstructed to generate evidence |

Implementors and authors must name which contract a step, component or Recipe
implements. Follow original claims, effective model-policy checks, native
settlement and existing recursion exclusion separately for each path. Do not add
a component-set bypass flag. Missing evidence or an uncertain model result remains
incomplete; provider disconnection neither proves cancellation nor permits replay.

## 3. Place in the turn execution model

For an ordinary prefixed model call, the agreed target is:

```text
Explicit Tier-1 model step or ordinary No-Match exchange (no model work in Tier 0)
  -> Kohai formats only the dynamic target prompt
  -> connected/enabled Sempai: review with its own prefix/design
       -> return the final dynamic target prompt unchanged by Kohai
     otherwise: keep Kohai's initially formatted dynamic prompt
  -> insert the finished target prefix unchanged exactly once
  -> qualified MCP advertisement / full guard / native submission
  -> actual response -> Recipe or No-Match continuation -> eventual completion

Only an eligible original completed No-Match turn
  -> separately admitted completed-turn analysis workflow
  -> retained original evidence -> analysis / unreviewed candidate subjects
```

The two branches do not share eligibility or completion merely because they share
model transport. Existing caller support is described in section 6.1 and
[kohai.md section 2](kohai.md#2-place-in-the-turn-execution-model); this diagram
does not certify the serving migration. Construction-call context is separately
scoped in section 4. Reference-based reconstruction remains an unimplemented v3.5 target design in 6.2.

## 4. Sempai provider, model and prefix

For ordinary independent review, Sempai uses the selected profile (`sempai_model`).
Its reviewer prefix is generated independently for its own target pair's design. The
same accepted pair design is shared by Generator, Kohai and Sempai when all three
target the same provider/model pair. Role-specific prefix content remains separate;
same pair design does not mean the same prefix text.

**Selection rule:** an explicitly selected Sempai reviewer prefix that is missing,
stale or incompatible blocks the review call. It must not silently fall back to the
base prompt. The base prefix generation and the reviewer prefix generation are
distinct operations; a successful Use in the WebUI triggers an optional base-prompt
generation dialog, not an automatic reviewer prefix replacement.

**Sempai's reviewer prefix belongs exclusively to Sempai's own review call.** It
must not appear in the final dynamic prompt returned to Kohai, and must not be
inserted into the target request by Kohai. The generator, Kohai and Sempai share
the accepted pair design when targeting the same pair; they do not share the
finished prefix artifact.

**Missing reviewer prefix:** when the selected Sempai reviewer prefix is
missing/incompatible, present the generation dialog (OK/Cancel) as specified in
[model-token-limits.md](docs/reborn/model-token-limits.md#generation-prompts-missing-selection-and-model-use).
An explicit Sempai Use selection also triggers an OK/Cancel dialog for base-prompt
generation of that pair; cancelling declines generation only and does not undo the
committed provider selection.

**Qualified construction successor:** the
[visible Maker/Base/Prefixless contract](docs/reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
scopes these ordinary reviewer-prefix requirements for new qualified construction
workflows. Each required review route captures its own eligible approved profile
or explicit Prefixless context for its actual executing pair before effects.
Keep fixed instructions, lineage, protocol, full request guard and native custody.
Do not implicitly use the candidate/parent prefix, rerun a suggestion chain or
read latest. Ordinary independent Sempai calls retain their reviewer contract.
The construction successor is a binding target with connected runtime work open;
it neither enables general Recipe Prefixless by omission nor adopts the proposed
reference reconstruction in section 6.2.

## 5. Per-target input limits

Two always-on hard limits apply, independent of `token_budgets_enabled`:

- `max_prefix_tokens` — prefix capacity for Sempai's own target model.
- `max_prompt_tokens` — dynamic-prompt capacity for Sempai's own target model.

Required target, not a claim of complete runtime wiring: each review workflow
must use the shared technical pre-dispatch boundary for its
actual Sempai request. Pre-call review and the final target send have separate
guarded envelopes; completed-turn analysis builds its own request and never
resumes the original target send. This does not assign post-turn sequencing to
Kohai or introduce another reviewer-specific settings/guard owner.

Measure the actual encoded request once: selected prefix if any, fixed/dynamic
instructions, evidence/history, Tool protocol, framing and effective output
setting. Do not count context twice because it also appears in a dynamic-prompt
description. Enforce each input slice, the derived checked total-input ceiling,
deployed model context and effective output allowance. The total is derived,
not another editable settings copy. Budget disabling does not remove these
technical constraints. See [the shared limit contract](docs/reborn/model-token-limits.md).

**Short references are never a size proxy for a final target request.** The
v3.5 target handoff requires measurement/guarding of the full resolved
target request after all permitted assembly. Today's agreed target instead uses Sempai's final dynamic value without
rebuilding it and inserts the selected target prefix unchanged once.

Oversized input is rejected whole; never truncated silently. When full evidence
does not fit in a completed-turn review request, only a reviewed multi-stage
inspection Recipe may partition it, retaining coverage, omitted/pending ranges and
exact references. Otherwise return Capacity blocked; never hide incomplete evidence
behind a summary.

## 6. Current handoff and proposed structured successor

### 6.1 Current implementation

The current interceptor ([`model_interceptor.rs`](crates/brassclaw_reborn/src/loop_driver_host/model_interceptor.rs))
already passes the volatile message tail — which includes the existing conversation
history — to Sempai as a JSON-encoded list of `(role, text)` pairs. The tail is
the portion of `request.messages` that follows the leading system messages; it
covers history messages and the current user query.

The unqualified proposal in section 6.2 would add:

- Unique protected references (e.g. `prefix_ref`) in place of repeated full content.
- Explicitly named protected parts that Kohai verifies after Sempai returns.
- A structured return format that preserves Tool call IDs, result envelopes and
  original references intact.

Sempai returns a `SempaiReviewOutcome`
([`packet.rs`](crates/brassclaw_interceptor/src/packet.rs)):

```rust
pub struct SempaiReviewOutcome {
    pub adjusted_volatile_messages: Vec<(String, String)>,  // (role, text) pairs
    pub bridge_messages: Vec<(String, String)>,
    pub composition_summary: String,
    pub proposed_recipe_updates: Vec<serde_json::Value>,
    pub proposed_intent_examples: Vec<serde_json::Value>,
    pub settings_adjustments: Vec<serde_json::Value>,
    pub proposed_components: Vec<ComponentProposal>,
}
```

`recompose()` applies the outcome to reconstruct the corrected message sequence.
Protected rules already implemented in `recompose()`:

- System messages in the volatile tail cannot be replaced.
- Tool results, provider call IDs and result envelopes cannot be described as
  `(role, text)` pairs; if they are present and Sempai changed the volatile tail,
  the recompose step returns an error.
- Sempai cannot remove the entire volatile context.

The current `(role, text)` tuple contract cannot represent provider Tool call IDs
and result envelopes faithfully. The actual guards preserve the complete original
protocol tail and reject tuple edits when those structures are present. This is
an existing protection, not evidence that a structured successor is implemented
or adopted.

The same `review` method copies the original leading system messages, appends the
review persona and sends the volatile tail as JSON. It also passes legacy proposal
fields to `proposal_sink.submit_proposals` after representability validation.
These are actual compatibility behaviors; they do not establish the intended
reviewer/target prefix separation or post-turn-only proposal workflow. Preserve
native custody and existing valid selections while the owning migration replaces
those responsibilities with qualified reusable components. Source presence and
the status label “source-supported” do not certify final serving conformance.
These declarations and the `review` / `recompose` call sites were source-checked
on 2026-11-15; no runtime or deployed acceptance is claimed by that inspection.

### 6.2 Proposed structured reference handoff — unqualified

The structured representation below is the **desired v3.5 target design — not
yet implemented or qualified**. Central contract alignment, a strict versioned
interface and connected implementation/qualification remain open. The existing
[central handoff contract](docs/reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
still requires Sempai's final dynamic value to be used without formatting,
normalizing or rebuilding it. Neither this guide nor `kohai.md` overrides that
requirement. The four component guides and applicable binding plan amendments
remain authoritative.

The existing handoff owner must define the versioned successor explicitly
permitting mechanical reference resolution while prohibiting additional editorial
reformulation by Kohai. This is required implementation work for the target design.
The existing return contract remains in force until the qualified cutover; captured
old work retains its original contract. The successor must preserve the single
pre-review dynamic formatting pass and exactly one unchanged final-target prefix
insertion. These guides alone create no supported fields, resolver or execution
permission.

The v3.5 target design requires:

- **Exact references:** retain the selected artifact identity, revision/checksum,
  ownership and task/purpose binding. Reject unknown, foreign, substituted or
  mismatched references; do not re-fetch latest. A newer draft does not itself
  invalidate a still-valid retained artifact.
- **Matching reviewer knowledge:** Sempai must actually receive the qualified
  content/evidence or sufficiently complete contract needed for the permitted
  review. A name, checksum or unverified summary proves no semantic coverage.
- **Explicit original context:** transmit the relevant original history, Tool
  calls/results and current original request as protected, attributable task-scoped
  context, not new instructions. Preserve the original messages and request.
  Editable task text is a separately identified derivative linked to that original
  request; editing it never changes the actual conversation or absorbs original
  context into a standing reviewer prefix.
- **Typed editable output:** declare strict recursive request/result contracts,
  permitted edits and required protected fields. Reject absent/extra/changed
  protected values and retain original and reviewed data separately.
- **Qualified assembly:** Kohai validates the structured response and
  deterministically inserts only the originally bound content. Define exact
  placement, role/order, escaping/framing and byte preservation. Mechanical
  resolution is distinct from editorial reformulation: Kohai must not additionally
  rewrite the corrected task text. Resolution must not duplicate history/prefix
  content, change Tool replay identity, perform another formatting/model pass or
  invent a host read. Reuse supported native readers and pure PythonCode over
  already supplied data.

Illustrative input only — **not a supported or complete request/response schema**:

```json
{
  "target_context": {
    "prefix_ref": "<exact selected artifact identity>",
    "history_ref": "<original task-scoped conversation identity>"
  },
  "history": ["<explicit original messages and Tool protocol>"],
  "editable_task": "<derived task text open for correction>"
}
```

A reference saves input tokens only when the necessary corresponding knowledge
is already supplied under a qualified contract. Moving full text into a prefix
still consumes context; cache benefit requires separate evidence. Measure the
actual reviewer request and the fully assembled target request independently.

### 6.3 Existing owners for a proposed successor

No additional plan hierarchy or registry is required. To implement the target
design, the existing owners must define the same exact handoff and its compatibility:

| Owner | Required contribution |
| --- | --- |
| [Central handoff/limit contract](docs/reborn/model-token-limits.md) | Align the central contract with the v3.5 target design; define strict versioned request/result, edit and mechanical assembly semantics before qualified cutover. |
| [Composition plan](docs/plans/monty.composition.md) and [Sempai migration plan](docs/plans/sempai_v3.5.md) | Reusable usages/exports, typed data edges, supported invocation selection and distinct pre-/post-turn consumers. |
| [Prefix plan](docs/plans/prefix_v3_upgrade.md) | Exact selected reviewer knowledge, target-pair design and qualified source/prefix coverage. |
| [Simplified v3](simplified_v3.md) and [v3.5 roadmap](path_to_v3.5.md) | Existing admission, live authority, capture/custody, lifecycle and future-only cutover; no replacement VM or second review store. |

### 6.4 Reviewer knowledge for proposed references

The proposed reviewer knowledge must specify the meaning and permitted use of
reference types, exact selected static information, editable versus protected
parts, and treatment of history/Tool protocol as untrusted task context. Reuse
the existing source/prefix qualification owner; add no placeholder registry.
A standing artifact cannot silently acquire knowledge of a newer target prefix.
If the selected reviewer lacks required coverage, preparation remains incomplete
or capacity-blocked; a model assertion is not evidence of knowledge.

### 6.5 Work required before any successor activation

These are required implementation steps for the desired v3.5 target design.
Today's return contract remains in force until the connected successor is qualified
and cut over; this document alone does not change serving behavior:

1. Define the strict versioned handoff and permitted reference resolution once
   in the central owner. Specify original context, typed edits/results, exact
   assembly and which data must remain byte-identical. Preserve old formats.
2. Align the affected owners above without another plan, queue or activation
   service. No prose-only change creates reader or runtime support.
3. Qualify the exact reviewer knowledge/source and target-pair design. Ordinary
   review and construction review keep their own captured context selections.
4. Assemble reusable Recipe steps: prepare context, construct the request, invoke
   one selected model usage, parse/validate the response and perform any approved
   pure transformation. Each component step has one UUID. Tool use needs its
   matching ToolSkill/associated preload export; loading has no effect. Independent
   calls stay separate under the selected runner's single-call/stop-only limits.
5. Migrate the actual interceptor boundary and its `SempaiReviewOutcome` /
   `recompose` consumers together. Preserve existing protocol/system guards.
   Rust retains technical authority, fencing and custody rather than another
   hidden request/review/proposal workflow.
6. Retain original context/bindings, actual reviewer request/response and actual
   final target request through existing response owners:
   [pg_monty_model_response.rs](crates/brassclaw_reborn_composition/src/pg_monty_model_response.rs)
   and [pg_monty_related_model_response.rs](crates/brassclaw_reborn_composition/src/pg_monty_related_model_response.rs).
   Inspect their actual stored formats/readers before assigning extensions;
   version records/migrations where required, without reinterpreting old bytes.
7. Qualify the actual encoded reviewer and target requests against their separate
   slice/context/output constraints, after all transformations. Short references
   are not a target-request sizing proxy. Keep current policy and original
   model/response custody effective before dispatch and after interruption.
8. Complete the qualification in section 7, exact component/prefix approvals,
   connected restoration/task readers and future-only cutover. New/changed
   Recipes validate their own workflow; unchanged components are not reapproved
   per call. Existing exact authored profiles retain their scope until the
   own-functional A successor is connected; controlled S qualification remains
   distinct. See [skills.md](skills.md#own-functional-approval-and-successor-compatibility).

## 7. Required qualification before any proposed handoff activation

Demonstrate all of the following before publication of the v3.5 target successor.
These are qualification requirements, not evidence of implementation or cutover:

| Required evidence | Description |
| --- | --- |
| Missing/unknown/foreign/mismatched references rejected | Assembly rejects missing required or added unknown references, foreign task/context bindings, substituted revisions and mismatches with the selected reviewer knowledge. Stale means invalid for the captured binding/contract; a newer revision alone does not invalidate a still-valid retained selection. |
| Protected content preserved | Recompose/assembly rejects outcomes that modify protected references, system context, Tool call structures or original messages |
| History and tool protocol correct | Tool call IDs, result envelopes and conversation sequence remain intact; no data lost in simplification |
| Reviewer and target prefix not mixed | Sempai's reviewer prefix does not appear in the returned dynamic prompt or the assembled target request |
| Sempai has matching static knowledge | The reference resolves to something Sempai actually received in its prefix or handoff payload |
| Both model requests within limits | Measure each actual encoded request once; check its prefix/dynamic slices, derived total, deployed context, effective output and protocol constraints after all permitted transformations |
| Lost confirmations do not trigger model calls | Unknown commit uses exact-ID readback, not a new model call or Recipe rerun |
| Captured work retains its original contract | Admitted/selected/reserved originals, including selected queued tasks, keep their exact original graph, handoff and custody; queue presence alone is not selection |

Changed components and prefixes receive their applicable qualification. No new
Q1/Q2 is generated per call. A new settings surface is not automatically required
for the handoff format change alone; it is needed only if operators receive new
configurable choices.

Follow the existing [future-only review cutover](docs/plans/sempai_v3.5.md#step-12--cut-over-new-admissions-then-retire-specialized-code-safely):
only future unreserved source events may select a qualified successor. Original
delivery identity and atomic fencing prevent dual consumption; old reservations,
captured selections and unresolved native owners keep their original contracts.
Rollback changes future selection only and cannot re-admit or replay old work.
Current live Tool policy, cancellation and technical limits still apply to
retained selections; pinning a graph does not freeze dispatch permission.

## 8. Reusable component library (v3.5 targets)

The following logical responsibilities belong in shared reusable components that
multiple Recipes can reference. These are target shapes, not existing callable
names or accepted database fields. Inspect existing components first; create a
primitive only for a demonstrated missing system operation.

| Reusable part | Likely owner | Other consumer |
| --- | --- | --- |
| Read one immutable record | Existing store primitive + Skill/PythonCode | Incident review, component review |
| Read bounded conversation/history range | Rust store query with correlation enforcement | Conversation export, reply/history auditing |
| Read retained model exchanges or effect receipts | Rust evidence store reads | Provider diagnostics, effect reconciliation |
| Select a transcript boundary | Pure PythonCode over typed records | Completed-turn audit, conversation summary |
| Correlate evidence and build diagnostics | Pure PythonCode | Read-only evidence audit Recipe |
| Assemble evidence envelope | Pure PythonCode | Incident or component-validation bundles |
| Construct messages from pinned prefix, instructions and evidence | Pure PythonCode with explicit role/order | Prompt review, domain analysis |
| Call one selected model once | Shared Rust transport + one-usage Skill | Kohai explicit LLM step, another reviewer |
| Parse strict JSON into typed data | Qualified pure code, or an existing/shared strict parser primitive where the selected Monty profile lacks equivalent support | Configuration, protocol parsing |
| Interpret proposal envelope and prepare one candidate | Pure PythonCode | Other authoring Recipes |
| Validate one candidate/base/dependency selection | Shared read-only structural/integrity primitive | Operator authoring, validator Recipes |
| Retain one candidate subject | Rust transaction + Skill | Human/operator authoring |
| Aggregate receipts and build completion result | Pure PythonCode | Other batch authoring/review Recipes |
| Append an immutable decision/result record | Rust primitive, no decision-making | Audit and reconciliation Recipes |

## 9. Nonnegotiable preservation requirements

- Exactly one supervised global Monty service. No second consumer VM, per-chat
  executor, Rust agent loop or Tier-2 fallback on errors.
- Only actual original No-Match turns produce the completed-turn analysis. Preserve
  original outcomes, history/reply evidence and failures after publication.
- Never replay the original model calls, Tools, reply or history to reconstruct
  evidence.
- Pin one coherent approved component graph, Recipe variant, step links, input
  layouts, exports, exact associations and actual Tool artifacts at task start.
  Child execution, waits and retries retain that selection.
- Pin the concrete Sempai provider/model and prefix/design/content identity at
  admission. Route/prefix successors affect future selection, not an existing
  review; live policy, cancellation and technical limits still apply per dispatch.
- Preserve original/effective request bytes, actual response/failure bytes,
  provider replay identities, Tool arguments/results and unresolved effects.
- Runtime inputs remain typed data. No interpolation into Python source, `eval`,
  `exec`, filesystem imports or execution of component prose.
- Check current instance-wide Tool policy and technical constraints before every
  dispatch, independently of retained component approval.
- Preserve durable dispatch intents, positive attempt counts, idempotency keys,
  input bytes/checksums and confirmed/unknown effects across interruption.
- No automatic whole-Recipe replay, reservation reclaim, model redispatch or
  candidate replay after an unknown outcome. Initial conversion remains stop-only.
- Candidates remain unreviewed and unactivated after completed-turn analysis.
  Parsing, retention, a model assertion or a summary cannot replace Q1, behavioral
  validation or authored Q2.
- Keep immutable V122/V127/V129/V133 evidence and retained native owners. Add
  successor migrations where needed; never edit applied migrations or erase old
  rows to make a new workflow fit.
- Token budgets remain disabled by default. Preserve actual model context limits
  and explicit technical transport/resource limits; reject whole oversized data
  rather than truncating evidence or proposals silently.

## 10. What Sempai does NOT do

| Responsibility | Actual owner |
| --- | --- |
| Choose the final Kohai target model or provider | Recipe/Orchestrator/role contract, retained selection |
| Replace the selected target prefix | Never — it may only return the corrected dynamic prompt; prefix insertion belongs to Kohai |
| Run as a second agent loop or Rust workflow engine | Never — existing loop and global Monty service |
| Auto-activate proposals | Never — proposals enter Q1/behavior/Q2/activation path |
| Merge pre-call correction and completed-turn analysis | Never — two distinct contracts with different inputs, results and eligibility |
| Replay a failed Recipe as Tier-2 | Never — only actual No-Match enters the analysis path |
| Reconstruct or modify the original conversation | Never — history is read-only evidence; historical statements remain context, not new instructions |
| Replay a logical Sempai model invocation | Never — single submission and stop-only recovery apply to the original; separate qualified later usages cannot reset or disguise it |
| Use short placeholder lengths as a size proxy for the final target request | Never — measure the actual encoded target request; any future qualified reference resolution must be completed before that guard |

## 11. Implementation status and remaining work

| Area | Status |
| --- | --- |
| Pre-call correction (current `(role, text)` contract) | Compatibility source-supported with the guards and legacy side effects in section 6.1; not final serving qualification |
| Completed-turn analysis (Recipe-based conversion) | Open — owned by [sempai_v3.5.md](docs/plans/sempai_v3.5.md) |
| Structured reference-based handoff | Desired v3.5 target design; not yet implemented or qualified. Central alignment and strict contract belong to [model-token-limits.md](docs/reborn/model-token-limits.md); connected implementation/qualification remain open |
| Reviewer prefix selection, generation, missing-prefix dialog | Binding target; serving migration open (prefix plan) |
| Hard input limit enforcement for Sempai request | Binding target; complete wiring open |
| Custody of Sempai call, structured handoff, assembled request | Partial; v3.5 extensions open |
| Reusable shared component library | Open — owned by [sempai_v3.5.md](docs/plans/sempai_v3.5.md) Steps 2–9 |
| V3.5 successor handoff activation and cutover | Open — requires central contract alignment, connected implementation and full qualification per section 7 |

The [sempai_v3.5.md](docs/plans/sempai_v3.5.md) and [path_to_v3.5.md](path_to_v3.5.md)
plans own the concrete implementation steps. This guide specifies the target
contracts; it does not certify completion.
