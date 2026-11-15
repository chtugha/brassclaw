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

Both paths use a separately selected Sempai provider/model and a separately
generated Sempai reviewer prefix. The v3.5 target extracts shared reusable
PythonCode components for message construction, model dispatch and strict response
parsing (see section 8 and [sempai_v3.5.md](docs/plans/sempai_v3.5.md)); in the
current implementation these responsibilities are handled by specialized Rust code.
Both paths have distinct entry conditions, inputs, results and completion semantics.
They must never be merged into a single component, single response schema or single
eligibility rule.

## 2. Two distinct contracts — never merged

### 2.1 Pre-call correction

| Field | Contract |
| --- | --- |
| Input | The already-formatted dynamic Kohai prompt for the final target, plus trusted protected references to the selected target prefix and task-scoped history/tool context |
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
| Completion | Retain original answer/evidence; candidates enter Q1/Q2/activation without editing or replaying the original turn |
| Eligibility | Eligible original No-Match turns only. Matched turns, failed Recipes and validator/reviewer tasks do not enter this path |
| Original turn | Never replayed, re-dispatched or reconstructed to generate evidence |

Implementors and authors must name which contract a step, component or Recipe
implements. Follow original claims, effective model-policy checks, native
settlement and existing recursion exclusion separately for each path. Do not add
a component-set bypass flag. Missing evidence or an uncertain model result remains
incomplete; provider disconnection neither proves cancellation nor permits replay.

## 3. Place in the turn execution model

```
Sempai involvement (Kohai is not invoked for Tier-0 Recipes; see kohai.md §1):
Input detail for Sempai pre-call correction shows v3.5 target; current sends (role,text) pairs.

              [Tier-1 Recipe LLM step or No-Match turn → Kohai invoked]
                           │
                           ┌─────────────────────────────────┐
                           │  Kohai: format dynamic prompt    │
                           └──────────────┬──────────────────┘
                                          │
                                 Sempai enabled?
                                   yes │       │ no
                                       ▼       │
              ┌─────────────────────────────────────────┐
              │  Sempai pre-call correction              │
              │  Input: formatted Kohai dynamic prompt   │
              │  + protected refs (prefix_ref,           │   ← v3.5 target
              │    history_ref) + history + tool context │
              │  Uses: own reviewer prefix + pair design │
              │  Returns: final corrected dynamic prompt │
              └──────────────────────┬──────────────────┘
                                     │
                           Kohai: insert prefix + MCP connect/advertise
                                   + check + dispatch
                                          │
                           ┌─────────────┴─────────────────┐
                           │   Model response → reply       │
                           └─────────────┬─────────────────┘
                                         │
                       Turn completed as No-Match (eligible)?
                              yes │          │ no
                                  ▼          │ (stop)
              ┌─────────────────────────────────────────┐
              │  Sempai completed-turn analysis Recipe   │
              │  Input: immutable evidence, original     │
              │          No-Match outcome                │
              │  Returns: unreviewed candidate proposals │
              └─────────────────────────────────────────┘
```

## 4. Sempai provider, model and prefix

Sempai uses a separately selected provider/model profile (`sempai_model`). Its
reviewer prefix is generated independently for its own target pair's design. The
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

## 5. Per-target input limits

Two always-on hard limits apply, independent of `token_budgets_enabled`:

- `max_prefix_tokens` — prefix capacity for Sempai's own target model.
- `max_prompt_tokens` — dynamic-prompt capacity for Sempai's own target model.

Kohai must resolve these limits and check the complete Sempai request (including
the structured prompt representation with references, history, and tool context)
before sending it. Separately, Kohai checks the final assembled target request
after prefix insertion.

**Short placeholder references in the structured handoff do not proxy for the
actual size of the final target request.** After Kohai resolves the references and
inserts the full prefix and history, the assembled target request is validated
independently against the target model's limits. Both requests are measured
against their own model's ceilings.

Oversized input is rejected whole; never truncated silently. When full evidence
does not fit in a completed-turn review request, only a reviewed multi-stage
inspection Recipe may partition it, retaining coverage, omitted/pending ranges and
exact references. Otherwise return Capacity blocked; never hide incomplete evidence
behind a summary.

## 6. Structured handoff contract (v3 current → v3.5 target)

### 6.1 Current implementation

The current interceptor ([`model_interceptor.rs`](crates/brassclaw_reborn/src/loop_driver_host/model_interceptor.rs))
already passes the volatile message tail — which includes the existing conversation
history — to Sempai as a JSON-encoded list of `(role, text)` pairs. The tail is
the portion of `request.messages` that follows the leading system messages; it
covers history messages and the current user query.

What is **new** in the v3.5 target (compared with the current implementation) is:
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

The current `(role, text)` tuple contract is insufficient to represent Tool call
IDs and result envelopes faithfully. The v3.5 successor replaces it.

### 6.2 V3.5 target — structured prompt representation with protected references

The v3.5 target introduces a structured prompt representation in which
unchangeable parts are expressed as protected references rather than full text.
This allows Sempai to work with the full semantic context while keeping the
protected parts immutable. The binding contract definition is owned by
[model-token-limits.md](docs/reborn/model-token-limits.md).

**Five governing rules:**

**Rule 1 — Exactly bound references.**
Each placeholder/reference has a unique, immutable meaning. For example,
`prefix_ref` names the specific prefix generation already selected for this task.
When Kohai assembles the final request it uses that originally bound artifact;
it must never re-fetch "latest" or silently substitute another generation.

**Rule 2 — Sempai knows the referenced content.**
A reference is valid only if Sempai actually receives the knowledge necessary to
reason about it — either in its qualified Sempai prefix or via an explicitly
transmitted sufficiently complete contract in the handoff payload. A reference name
alone is not sufficient knowledge. For different target prefixes, the reference
must exactly match the knowledge Sempai receives; otherwise Sempai optimizes based
on wrong assumptions.

**Rule 3 — History and tool context are transmitted explicitly.**
The original conversation history relevant to this turn — including tool call
IDs, results and their structure — is sent as explicit content, not hidden in a
reference. This context is task-scoped; it must not be absorbed into the standing
Sempai prefix. History must remain recognisable as context so that historical
statements are not misinterpreted as new instructions.

**Rule 4 — Editable parts are explicitly named.**
Sempai may improve the formulation of the current task text (`editable_task`).
Protected sources, original messages, the selected prefix, technical parameters
and Tool result structures remain unchanged. Kohai's recompose/assembly step must
reject any outcome that modifies protected references or removes required content.

**Rule 5 — Kohai assembles the final request deterministically.**
After receiving Sempai's structured outcome, Kohai:
- verifies that all required references are present and no unknown references
  were added;
- substitutes the originally bound content for each reference;
- reconstructs the complete message sequence, preserving Tool call IDs and
  result envelopes intact;
- assembles the complete request within the target model's hard limits.

This assembly step requires no further LLM call. The former rule "use Sempai's
return unchanged" is updated: Kohai is explicitly permitted to perform mechanical
reference resolution (substituting the originally bound immutable artifacts) and
must not apply any additional content reformulation of its own during assembly.

**Illustrative schema (not yet a supported BrassClaw schema; see
[model-token-limits.md](docs/reborn/model-token-limits.md) for the binding
definition when published):**

```json
{
  "target_context": {
    "prefix_ref": "<exactly bound prefix generation identity>",
    "history_ref": "<original conversation state identifier for this turn>"
  },
  "history": ["… original messages including tool calls and results …"],
  "editable_task": "… current task text open for optimization …"
}
```

**Token efficiency:** references save tokens only where the referenced content is
already present in Sempai's prefix knowledge. Moving the same full text from the
dynamic prompt into the Sempai prefix does not reduce the total input volume; it
only shifts where it appears. Any cache benefit must be separately demonstrated.
The transmitted history also consumes context capacity.

### 6.3 Adjacent plan alignment requirements

A coherent successor of the Kohai/Sempai handoff requires updates in four existing
plan owners. No additional plan hierarchy is needed; existing affected
Recipe/coverage appendices must describe the same handoff.

| Plan | Required update |
| --- | --- |
| [Composition plan](docs/plans/monty.composition.md) (and affected implementation appendix) | Structured handoff components, data edges and actual consumers |
| [Prefix upgrade plan](docs/plans/prefix_v3_upgrade.md) | What static target information the Sempai prefix must contain and how its exact revision is selected |
| [Simplified v3](simplified_v3.md) | Consume existing request/capture/lifecycle support; add only genuinely missing shared mechanics once |
| [path_to_v3.5.md](path_to_v3.5.md) | Pre-call correction remains separate from post-turn No-Match analysis; keep both contracts distinct |

### 6.4 Sempai prefix — required knowledge for references to work

A placeholder saves tokens only when Sempai already reliably knows the referenced
content. If Sempai only receives a name without understanding the content, it
optimizes based on wrong assumptions. Therefore Sempai's prefix must explicitly
cover:

- The placeholder contract and the meaning of each reference type.
- The statically targeted information (e.g. what `prefix_ref` refers to in this
  deployment context), with its exact revision.
- Permitted changes (e.g. optimising `editable_task`) and protected areas (e.g.
  original messages, Tool call IDs, the selected prefix, technical parameters).
- The rules for history content and tool messages: how to interpret them as
  task-scoped context, not standing instructions.

The existing prefix selection and source management must be extended to cover these
facts. A separate placeholder registry is not needed; include this knowledge in the
Sempai reviewer prefix and qualify it through the prefix upgrade plan. A stale
Sempai prefix must not claim knowledge of a newer target prefix generation.

### 6.5 Successor handoff implementation requirements

A new coherent successor of the Kohai/Sempai handoff must be defined and
implemented before the new behaviour is activated. A Skill or component change
alone does not establish the new behaviour. The following eight areas must be
specified and implemented together (see also [docs/plans/sempai_v3.5.md](docs/plans/sempai_v3.5.md)
Step 6 and [docs/plans/monty.composition.md](docs/plans/monty.composition.md)):

1. **Change the binding handoff contract.** The central binding location is
   [model-token-limits.md](docs/reborn/model-token-limits.md) (§Explicit handoff).
   The successor must explicitly state: which parts Sempai may edit; which parts
   appear only as protected references; how each reference is bound to its original
   content; which history state and tool results are included; what structured
   response Sempai must return; how Kohai assembles the request deterministically;
   and when assembly is complete.
   Protected reference resolution must be distinguished from further content
   reformulation. Old tasks retain the current handoff contract without modification.
   The former rule "use Sempai's return unchanged" is **updated**: Kohai resolves
   references deterministically and assembles the final request; it must not apply
   any further content reformulation of its own during assembly. Reference
   resolution is not reformulation — it substitutes the originally bound immutable
   artifact. This explicitly allows Kohai's mechanical reference resolution while
   excluding additional editorial changes by Kohai.

2. **Align the affected plan instructions.** The concrete implementation belongs in
   the existing plans (see also §6.3). No additional plan hierarchy is needed;
   existing affected Recipe/coverage appendices must describe the same handoff.

3. **Extend Sempai's prefix and its knowledge base.** A placeholder saves tokens
   only when Sempai already reliably knows the referenced content (see §6.4).
   Extend the existing prefix selection and source management to cover the
   placeholder contract, the statically targeted information, the exact selected
   revision, permitted changes, protected areas, and the rules for history/tool
   messages. A separate placeholder registry is not needed.

4. **Adjust the executable component flow as reusable Recipe components:**
   (a) prepare original context and references;
   (b) assemble the Sempai request;
   (c) execute one selected model call;
   (d) parse the structured response;
   (e) validate protected references and editable fields;
   (f) deterministically assemble the target request.
   Steps (d)–(f) are pure data processing; they require no second LLM call and
   no Rust Tool that hides the whole workflow. Concrete component schemas and
   exports must be specified from the actual existing interfaces.

5. **Update the actual interceptor and its data contracts.**
   Directly affected: `model_interceptor.rs` for Sempai request construction;
   `SempaiReviewOutcome` for response representation; `recompose()` for
   assembly — preserving Tool call IDs, result structures, original references and
   provider replay identities. `(role, text)` pairs alone are insufficient for full
   tool message fidelity; the successor must not lose those structures.
   Existing protection rules in `recompose()` must be preserved in the successor.
   The implementation must account for the planned Recipe migration; it must not
   create a second large workflow in the Rust interceptor.

6. **Extend custody and evidence records.** History must have a unique meaning:
   the original conversation state selected for this call. Later messages or
   foreign chats must not be mixed in. The following must be retainable:
   original context, reference bindings used, actual Sempai request sent, actual
   Sempai response received, and assembled target request.
   Use the existing model-answer custody
   ([`pg_monty_model_response.rs`](crates/brassclaw_reborn_composition/src/pg_monty_model_response.rs))
   and related-model response
   ([`pg_monty_related_model_response.rs`](crates/brassclaw_reborn_composition/src/pg_monty_related_model_response.rs));
   extend them where the current contract is insufficient. Database migrations are
   required when stored formats or their strict readers change. Old stored records
   must not be reinterpreted as the new format.

7. **Bind the complete target-request assembly.** After reference resolution, Kohai
   assembles the actual full content within the target model's hard limits using the
   existing encoding/request path. Short placeholder lengths in the Sempai handoff
   must not be used as a size proxy for the assembled target request. The Sempai
   request and the final target request are two distinct requests with their own
   model limits; each is checked independently. No new settings surface is required
   unless the operator receives new configurable choices.

8. **Add targeted qualification, upgrade and evidence.** Before activating the new
   workflow, verify (see §7 for the complete evidence table): missing/unknown/stale
   references are rejected; protected content is preserved; history and tool
   protocol remain correctly attributed; reviewer and target prefix are not mixed;
   Sempai receives the static knowledge matching its references; both complete model
   requests fit within their respective limits; lost confirmations do not cause
   model re-calls; in-flight tasks retain the current contract.
   Changed components and prefixes receive their applicable qualification. No new
   Q1/Q2 is generated per call.

## 7. Required qualification before activation of the v3.5 handoff

Before publishing the new workflow, the following must be demonstrated:

| Required evidence | Description |
| --- | --- |
| Missing/unknown/stale references rejected | Assembly rejects outcomes with missing required references or added unknown references |
| Protected content preserved | Recompose/assembly rejects outcomes that modify protected references, system context, Tool call structures or original messages |
| History and tool protocol correct | Tool call IDs, result envelopes and conversation sequence remain intact; no data lost in simplification |
| Reviewer and target prefix not mixed | Sempai's reviewer prefix does not appear in the returned dynamic prompt or the assembled target request |
| Sempai has matching static knowledge | The reference resolves to something Sempai actually received in its prefix or handoff payload |
| Both model requests within limits | Sempai request checked against Sempai model limits; final target request checked against target model limits after full assembly |
| Lost confirmations do not trigger model calls | Unknown commit uses exact-ID readback, not a new model call or Recipe rerun |
| Old tasks retain current contract | In-flight and queued tasks continue under the previous handoff contract unmodified |

Changed components and prefixes receive their applicable qualification. No new
Q1/Q2 is generated per call. A new settings surface is not automatically required
for the handoff format change alone; it is needed only if operators receive new
configurable choices.

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
| Parse strict JSON into typed data | Existing/shared validator primitive | Configuration, protocol parsing |
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
  admission. Configuration changes affect future work, not an existing review.
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
| Issue a second model call after Sempai's own call | Never — one model call per Sempai invocation |
| Use short placeholder lengths as a size proxy for the final target request | Never — the full assembled target request is measured after reference resolution |

## 11. Implementation status and remaining work

| Area | Status |
| --- | --- |
| Pre-call correction (current `(role, text)` contract) | Source-supported |
| Completed-turn analysis (Recipe-based conversion) | Open — owned by [sempai_v3.5.md](docs/plans/sempai_v3.5.md) |
| Structured reference-based handoff (v3.5) | Binding target; contract definition owned by [model-token-limits.md](docs/reborn/model-token-limits.md); implementation open |
| Reviewer prefix selection, generation, missing-prefix dialog | Binding target; serving migration open (prefix plan) |
| Hard input limit enforcement for Sempai request | Binding target; complete wiring open |
| Custody of Sempai call, structured handoff, assembled request | Partial; v3.5 extensions open |
| Reusable shared component library | Open — owned by [sempai_v3.5.md](docs/plans/sempai_v3.5.md) Steps 2–9 |
| Successor handoff activation and cutover | Open — after full qualification per section 7 |

The [sempai_v3.5.md](docs/plans/sempai_v3.5.md) and [path_to_v3.5.md](path_to_v3.5.md)
plans own the concrete implementation steps. This guide specifies the target
contracts; it does not certify completion.
