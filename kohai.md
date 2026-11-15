# Kohai — design and authoring instructions

> **Current status — 2026-11-14:** use [the combined implementation and ownership
> index](docs/implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


This guide defines what Kohai is, its role in the BrassClaw turn execution model,
and the contracts that govern its behaviour. MUST means required. Read
[recipe.md](recipe.md), [skills.md](skills.md), [tools.md](tools.md),
[toolskills.md](toolskills.md), [simplified_v3.md](simplified_v3.md),
[sempai.md](sempai.md) and [the development policy](docs/development-policy.md).
Kohai's behaviour in Tier-1 and No-Match turns intersects directly with
[docs/reborn/model-token-limits.md](docs/reborn/model-token-limits.md); that
document owns the binding prefix/pair-design/handoff contracts. The concrete
Recipe-migration work for Kohai's Sempai integration is owned by
[docs/plans/sempai_v3.5.md](docs/plans/sempai_v3.5.md) and
[path_to_v3.5.md](path_to_v3.5.md). This guide specifies what Kohai is; it does
not claim completed runtime enforcement or migration.

## 1. What Kohai is

**Kohai is the BrassClaw LLM consumer**: the component that formats the final
outgoing model request, optionally routes it through Sempai for pre-call
correction, inserts the finished prefix, and dispatches the complete request to
the target model. It is not a workflow engine or scheduler.

Kohai is a role, not a component class in the catalogue. It describes the
responsibility boundary of the `host.kohai_complete` Tool call — the moment a
Recipe step (or the No-Match Recipe) hands an assembled prompt to Kohai. Its
behaviour is specified here so agents and authors can reason about what Kohai does
versus what Recipes, the Orchestrator, Sempai and Rust primitives do.

**Kohai is only invoked when a Recipe step (or the No-Match path) explicitly
calls `host.kohai_complete`.** Tier-0 Recipes are deterministic (no LLM step)
and never call `host.kohai_complete`. Tier-1 Recipes include one or more steps
that call `host.kohai_complete`; those steps can appear anywhere in the Recipe
sequence, not necessarily at the end — later steps in the same Recipe can use
the returned answer.

When `host.kohai_complete` is called, Kohai's work consists of exactly three
stages in order:

1. **Format** — produce the dynamic prompt for the final target provider/model pair
   from the assembled prompt dict (`chat_history`, `user_query`, `prefix_placeholder`).
2. **Correct (optional)** — if Sempai is connected and enabled, send the formatted
   dynamic prompt to Sempai and receive the final corrected dynamic prompt back.
3. **Assemble and dispatch** — insert the intended finished prefix unchanged exactly
   once before the (corrected) dynamic prompt, establish the MCP connection, check
   the complete request against the target model's hard input limits, dispatch to
   the model, and return the answer to the calling Recipe step.

Nothing outside these three stages belongs to Kohai. Workflow sequencing, task
intent matching, evidence composition and proposal processing all belong elsewhere.

## 2. Place in the turn execution model

```
Admitted input → Global Orchestrator (Monty)
    │
    ├─── MATCH → Recipe execution (Monty runs PythonCode steps in order)
    │       │
    │       ├─── Tier-0 Recipe (llm_call_required=false)
    │       │         no host.kohai_complete call; Kohai not involved
    │       │         PythonCode steps call other host.* tools only
    │       │         → host.post_reply(...) → done
    │       │
    │       └─── Tier-1 Recipe (llm_call_required=true)
    │                 … PythonCode step(s) … →
    │                 PythonCode calls host.kohai_complete(prompt={...})
    │                 ┌────────────────────────────────────────────────────┐
    │                 │  Inside host.kohai_complete (= Kohai's three stages)│
    │                 │  1. Format dynamic prompt for target pair            │
    │                 │  2. Optional Sempai correction                       │
    │                 │  3. Insert prefix + MCP connect + check + dispatch   │
    │                 │  returns LLM answer to Recipe step                   │
    │                 └────────────────────────────────────────────────────┘
    │                 … further Recipe steps may use the answer …
    │                 → host.post_reply(...) → done
    │
    └─── NO-MATCH → builtin Recipe host-non-match-llm-answer
            PythonCode assembles prompt →
            host.kohai_complete(prompt={...})
            ┌────────────────────────────────────────────────────┐
            │  Inside host.kohai_complete (= Kohai's three stages)│
            │  1. Format dynamic prompt for target pair            │
            │  2. Optional Sempai correction                       │
            │  3. Insert prefix + MCP connect + check + dispatch   │
            │  returns LLM answer                                  │
            └────────────────────────────────────────────────────┘
            → host.post_reply(...) → done
```

The turn execution diagram in [AGENTS.md](AGENTS.md) and
[simplified_v3.md](simplified_v3.md) governs overall routing. Kohai occupies only
the final dispatch segment. Kohai does not choose the prefix, the target model, or
the workflow tier. Those are determined by the retained Recipe/Orchestrator/role
contract before Kohai is involved.

## 3. Prefix selection and pair design

Three contracts must be kept distinct; see
[model-token-limits.md](docs/reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
for the binding specification:

| Contract | Owner and meaning |
| --- | --- |
| Provider/model pair design | One accepted revision per pair: format/standard, framing/join rules, tokenizer/cache constraints, canonical limit references. Generator, Kohai and Sempai all use the same stored accepted revision when targeting the same pair. |
| Prefix profile / content rules | Role/domain knowledge, source/coverage, teaching examples, content ordering. Sharing a pair design does not require identical role content. |
| Prefix generation | Finished immutable text, already formatted for one profile and pair, with pinned design/content/source revisions. Kohai inserts this artifact unchanged. |

**Selection rule:** the retained Recipe, Orchestrator instruction, or role
contract specifies the prefix. When none specifies one, the compatible
`base-prompt` applies — including ordinarily in No-Match. A conflicting explicit
selection must be reported, never resolved silently. An explicitly selected prefix
that is missing, stale or incompatible blocks the call; it must not fall back to
base. Missing base readiness likewise blocks the dependent call.

**Insertion rule (binding target):**

1. Format only the dynamic Orchestrator prompt for the final target pair, using the
   stored accepted pair design. This is the only dynamic-prompt formatting pass.
2. If Sempai is connected and enabled, send this already-formatted dynamic prompt
   to Sempai. Sempai returns the final dynamic prompt. Kohai does not format
   Sempai's return again.
3. Insert the intended finished prefix — the immutable generated artifact —
   unchanged and exactly once before the (corrected) dynamic prompt.
4. After prefix insertion, establish the provider MCP connection and advertise
   Tools (if applicable; see §5), check the complete assembled request against the
   target model's hard input limits, then dispatch.

These four rules supersede every earlier "review before format" or "runtime prefix
render" interpretation. They define the target architecture, not completed serving
migration.

## 4. No-Match turn assembly

In a No-Match turn the Orchestrator (Monty) assembles the Tier-2 prompt context
— user question, available conversation history and applicable dynamic context —
and passes it to Kohai. Kohai then formats and dispatches using the same four-rule
sequence described in section 3. No-Match input uses the base-prompt prefix by
default.

After the assembled prompt is dispatched and an answer is received, eligible
completed-turn review may propose Recipe/intent changes through the Sempai
completed-turn path. That proposal path is separate from the pre-call correction
performed in this turn. No-Match does **not** mean the turn can be replayed
through Tier-2 logic after a Recipe failure. Only an actual No-Match at routing
time enters this path.

## 5. MCP advertisement lifecycle

Kohai connects to the MCP server and advertises available Tools to the target
provider after final prefix insertion — immediately before sending the prompt to
the model. It disconnects only that request on the complete answer, or on a
reconciled failure or cancellation. It does not disconnect during streaming
progress or on intermediate tool exchanges. Discovery refreshes at startup/restart
and at qualified Skill/Recipe catalogue changes. Existing in-flight calls retain
their advertised contract and normal chat task snapshot.

## 6. Hard input limits

Two per-model input ceilings are always-on hard limits, independent of
`token_budgets_enabled`. Kohai must resolve both ceilings from the provider's
canonical `model_token_limits[model]` entry before dispatch:

- `max_prefix_tokens` — prefix capacity for the target model.
- `max_prompt_tokens` — dynamic-prompt capacity (history, tools, framing).

Kohai also needs the equivalent limits for the Sempai target when Sempai is
enabled, to check the Sempai request size before sending. The two requests —
Sempai request and final target request — have separate model limits and are
checked independently. Short placeholder references in the Sempai structured
handoff must not be used as a size proxy for the final assembled target request.

The complete assembled request (prefix + corrected dynamic prompt + history +
tools + framing) is validated against the target model's hard input ceiling before
dispatch. This check is performed at the shared pre-dispatch boundary,
independently of budgeting mode, review mode, retries and tool continuations.
An oversized request is rejected whole; never truncated silently.

## 7. What Kohai does NOT do

| Responsibility | Actual owner |
| --- | --- |
| Choose the target model, provider, or prefix | Retained Recipe, Orchestrator, role contract |
| Select the workflow tier (0, 1, 2) | Routing and Recipe assembly |
| Compose evidence, transcript boundaries or correlated records | Sempai completed-turn analysis Recipe, PythonCode components |
| Decide whether review is needed | Recipe/Orchestrator contract, operator configuration |
| Reformat Sempai's returned dynamic prompt | Never — Sempai's return is inserted without further formatting |
| Issue a second model call after Sempai correction | Never — only one final target call per turn dispatch |
| Choose a fallback prefix when the selected one is missing | Never — block the call; report the missing prefix |
| Replay the turn after a routing/Recipe error | Never — only actual No-Match enters No-Match handling |
| Run as a second agent loop | Never — the global Monty service owns the loop |

## 8. Sempai pre-call correction interface (binding target, v3.5)

The current implementation already passes the volatile message tail — which
includes the existing conversation history — to Sempai as a JSON-encoded list
of `(role, text)` pairs. Sempai returns `SempaiReviewOutcome`, which Kohai
applies through `recompose()` to reconstruct the corrected message sequence.

What is new in the v3.5 target (compared with the current implementation):
unique protected references, explicitly named protected parts, and a structured
return format that preserves Tool call IDs and result envelopes.

See [sempai.md](sempai.md) for the full Sempai design including the current struct
definition and the five governing rules for the v3.5 successor.

The v3.5 target evolves this interface toward a **structured prompt representation
with protected references**, defined by the shared Kohai/Sempai handoff contract.
The target contract is owned by [model-token-limits.md](docs/reborn/model-token-limits.md)
and must be defined once and consumed by both Kohai and Sempai. The key rules are:

1. **Exactly bound references.** Each protected reference (e.g. `prefix_ref`)
   names a specific already-selected, immutable artifact. Kohai must use the
   originally bound artifact when assembling the final request; it must never
   re-fetch "latest" or silently substitute another generation under the same name.

2. **Sempai knows the referenced content.** The reference is only valid if Sempai
   actually has sufficient knowledge of that artifact's content — either directly
   in its qualified Sempai prefix, or via an explicitly transmitted summary with
   the same contract. A name alone is not sufficient. For different target prefixes
   the reference must match the knowledge Sempai actually receives.

3. **History and tool context are transmitted explicitly.** Kohai sends the
   original conversation history relevant to this turn, including tool call results,
   as explicit content. This context remains task-scoped; it must not be absorbed
   into a standing Sempai prefix. History must be recognisable as context so that
   historical statements are not misinterpreted as new instructions.

4. **Editable parts are explicitly named.** Sempai may improve the formulation of
   the current task text. Protected sources, original messages, the selected prefix,
   and technical parameters remain unchanged. Kohai's recompose step must reject any
   outcome that modifies protected references or removes required content.

5. **Kohai assembles the final request deterministically.** After receiving Sempai's
   structured outcome, Kohai: verifies that all required references are present and
   no unknown references were added; substitutes the originally bound content for
   each reference; assembles the complete request within the target model's limits.
   This assembly step requires no further LLM call.

**Update to the "use Sempai's return unchanged" rule:** The former rule that
Kohai must use Sempai's return without modification is superseded in the v3.5
structured handoff. The updated rule is: Kohai resolves protected references
deterministically and assembles the final request; it must not apply any further
content reformulation of its own during this assembly. The reference resolution
itself is not a content reformulation — it substitutes the originally bound
immutable artifact. Kohai's editorial involvement ends when it sends the
structured handoff to Sempai; what follows is mechanical reference resolution.

The current `SempaiReviewOutcome` `(role, text)` pairs are insufficient to
represent Tool call IDs and result envelopes faithfully. The successor contract
must preserve those structures. Until the successor contract is fully specified and
qualified, existing tasks retain the current handoff contract without modification.

The concrete handoff schema (illustrative only — not yet a supported BrassClaw
schema; see [model-token-limits.md](docs/reborn/model-token-limits.md) for the
binding definition when published):

```json
{
  "target_context": {
    "prefix_ref": "<exactly bound prefix generation identity>",
    "history_ref": "<original conversation state for this turn>"
  },
  "history": ["… original messages …"],
  "editable_task": "… task text open for optimization …"
}
```

Kohai's final assembly resolves `prefix_ref` → the pinned immutable prefix
artifact, and reconstructs the complete request. No further reformatting is
applied after Sempai's return.

## 9. Custody and evidence

The model-answer custody owner is
[`pg_monty_model_response.rs`](crates/brassclaw_reborn_composition/src/pg_monty_model_response.rs).
The related-model response owner stores Sempai's separate review call record.
Both must be extended if the v3.5 structured handoff requires additional fields
(original context, reference bindings, Sempai request, Sempai response, assembled
target request). Database migrations are required when stored formats or their
strict readers change. Old stored records must not be reinterpreted as the new
format.

## 10. Implementation status and remaining work

| Area | Status |
| --- | --- |
| Basic dynamic prompt formatting and dispatch | Source-supported |
| Prefix insertion (four-rule contract) | Binding target; serving migration open |
| Sempai pre-call correction (current `(role, text)` contract) | Source-supported |
| Structured reference-based Sempai handoff (v3.5) | Binding target; contract definition open (see [model-token-limits.md](docs/reborn/model-token-limits.md)) |
| Hard input limit enforcement at pre-dispatch boundary | Binding target; complete wiring open |
| MCP connect/advertise/disconnect lifecycle | Source-supported |
| Custody of Sempai call and final assembled request | Partial; v3.5 extensions open |

The [sempai_v3.5.md](docs/plans/sempai_v3.5.md) and
[path_to_v3.5.md](path_to_v3.5.md) plans own the concrete implementation steps.
This guide describes the target contracts; it does not certify completion.
