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

**Kohai is the BrassClaw LLM consumer**: the role that formats the final
outgoing model request, optionally routes it through Sempai for pre-call
correction, inserts the finished prefix, and dispatches the complete request to
the target model. It is not a workflow engine or scheduler.

Kohai is a logical role, not a component class or a universal callable. The
task-bound `host.kohai_complete` primitive is one model-usage interface. Ordinary
global No-Match work uses the separate `host.stream_model` task boundary below.
Neither interface makes Kohai the workflow engine. Tier-0 execution contains no
model calls; a Tier-1 workflow wires its required model usages explicitly and may
consume their results in later selected steps.

For ordinary calls with a selected generated prefix, the binding logical order is:

1. **Format** only the dynamic Orchestrator prompt for the final target pair using
   its accepted design.
2. **Correct, when enabled:** Sempai reviews that already formatted target prompt
   and returns its final dynamic value under the agreed handoff contract.
3. **Insert, guard and submit:** prepend the intended finished prefix unchanged
   exactly once, attach the qualified MCP advertisement where applicable, check
   the actual complete request and submit through the selected native transport.

This role-level order is a binding target, not a claim that one Tool already
implements the complete serving contract or should hide the migration workflow.
Recipes and pinned PythonCode own request composition, explicit model usages,
response interpretation and business sequencing. Rust retains technical encoding,
authority, submission fencing, accounting and effect/response custody. Existing
interceptor orchestration is compatibility implementation to migrate through the
existing owners, not a second large Rust workflow to create. Follow the
[cross-guide ownership contract](recipe.md#ownership-across-the-four-guides).

## 2. Place in the turn execution model

```text
Admitted input -> existing global Monty task
  Match -> retained Recipe / variant / typed inputs
    Tier 0: pinned deterministic exports, no model calls
    Tier 1: explicit qualified model usage, for example host.kohai_complete
            -> complete model-response JSON -> subsequent Recipe interpretation
    -> qualified workflow completion / reply where required
  Actual No-Match -> global_mode.py::_non_match
    visible_capabilities -> build_prompt_bundle -> stream_model
      assistant reply -> return content for the selected reply workflow
      capability calls -> invoke_capability -> append_capability_result_ref
                       -> next model exchange under Monty's sequencing
  Error / begun Recipe failure -> classified failure, no Tier-2 replay
```

The turn execution diagram in [AGENTS.md](AGENTS.md) and
[simplified_v3.md](simplified_v3.md) governs overall routing. Kohai describes the
model-request preparation/dispatch boundary. Kohai does not choose the prefix,
the target model, or
the workflow tier. Those are determined by the retained Recipe/Orchestrator/role
contract before the affected call. The source paths were checked on 2026-11-15:

| Source | Verified boundary |
| --- | --- |
| [global_mode.py](crates/brassclaw_engine/orchestrator/global_mode.py) | `_non_match` uses `host.build_prompt_bundle` and `host.stream_model`, then sequences actual capability results or returns reply content. It does not invoke the legacy `host-non-match-llm-answer` Recipe. |
| [monty_model_capability.rs](crates/brassclaw_reborn/src/monty_model_capability.rs) | `TaskModelCapability`, `Arguments`, `Prompt` and `Message` define the retained `host.kohai_complete` native adapter. |
| [model_completion_components.rs](crates/brassclaw_skills/src/model_completion_components.rs) | The packaged `complete_model_usage` export and its exact Skill/input/result contracts. Source definitions alone do not approve a new consumer. |

### 2.1 Current retained model-usage interface

The native argument object has exactly `prompt`. Its required `user_query` is a
string and `chat_history` is a list of `{role, content}` text messages, with roles
`system`, `user` or `assistant`. Optional `max_output_tokens` is a positive exact
u32 request setting; explicit null, zero, booleans, fractions and overflow are
invalid. Unknown keys are rejected. For example, this is an argument-shape example,
not a published Recipe or a permission grant:

```json
{
  "prompt": {
    "user_query": "Say hello",
    "chat_history": [],
    "max_output_tokens": 128
  }
}
```

The host supplies the original selected model route, prefix/input binding,
scope and invocation identity. `prefix_placeholder`, `prefix_reference` and
caller-supplied model/provider selectors are not accepted native prompt fields.
The three text roles cannot encode structured provider Tool-result replay;
ordinary No-Match retains that protocol through its own task boundary.

The result is the complete actual `LoopModelResponse` encoded as JSON text,
including output, chunks, safe reasoning deltas, effective model profile and
optional usage. It is not merely answer text or an `ok/answer` envelope. A
consuming Recipe must parse the supported response profile and interpret it in
separate qualified steps. Proposals remain data and do not execute Tools.

`SingleSubmission` protects the original logical model invocation from duplicate
submission; ordinary retained execution is stop-only with `max_attempts: 1`.
It does not forbid a separately declared later model step or an ordinary No-Match
model/Tool continuation with its own supported identity and custody. Neither a
new step nor a new attempt may reset an uncertain original or replay its effects.

## 3. Prefix selection and pair design

Three contracts must be kept distinct; see
[model-token-limits.md](docs/reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
for the binding specification:

| Contract | Owner and meaning |
| --- | --- |
| Provider/model pair design | One accepted revision per pair: format/standard, framing/join rules, tokenizer/cache constraints, canonical limit references. Generator, Kohai and Sempai all use the same stored accepted revision when targeting the same pair. |
| Prefix profile / content rules | Role/domain knowledge, source/coverage, teaching examples, content ordering. Sharing a pair design does not require identical role content. |
| Prefix generation | Finished immutable text, already formatted for one profile and pair, with pinned design/content/source revisions. Kohai inserts this artifact unchanged. |

**Ordinary-call selection rule:** the retained Recipe, Orchestrator instruction, or role
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

### 3.1 Qualified construction-selection successor

The binding [visible Maker/Base/Prefixless construction contract](docs/reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
scopes the ordinary prefix requirements for new qualified construction workflows.
Maker → Base → Prefixless is an initial editable UI suggestion, not a runtime
fallback chain. Each actual executing route captures its submitted eligible
profile or explicit Prefixless choice before effects. Required construction
children/reviews keep their own captured compatible context and parent lineage;
they do not borrow the candidate's prefix or look up latest. Ordinary independent
Sempai review keeps its separate reviewer contract.

An approved selected artifact is inserted exactly once under its qualified
design. Explicit Prefixless inserts no generated prefix but retains fixed
instructions, original route/auth/model, technical encoding/protocol, full request
guard and native response/effect custody. Missing support is readiness work, not
a bypass. General Recipe Prefixless still needs its actual qualified reader/runtime
profile; absence, an empty prefix or a new flag cannot enable it. This binding
selection successor does not adopt the separate reference-handoff proposal in
section 8 or establish completed runtime integration.

## 4. No-Match turn assembly

In an ordinary No-Match turn, `_non_match` sequences capability discovery,
host-owned prompt materialization, model dispatch and actual capability-result
handoffs through the task ports named in section 2. The agreed preparation target
uses the same ordinary-call formatting/review/insertion order as section 3;
No-Match uses base-prompt by default. This is distinct from invoking the legacy
named No-Match Recipe or accepting caller-supplied arbitrary source/prefix data.

After the original turn reaches its actual eligible completed No-Match boundary,
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

The MCP server stays running independently of a request connection. Its listed
tools derive only from available activated, approved **MCP-call Recipe/variant
commands**, not raw Rust Tool or Skill rows. A listed completed command enters
a new ordinary chat through the existing matcher/Recipe runner, returns its
correlated terminal result and closes through the supported chat lifecycle.
MCP exposes no direct Python/native execution path and creates no per-call
components or Q1/Q2. See [skills.md](skills.md#mcp-and-acceptance).
The request-local adapter and ordinary-chat bridge exist in
[mcp_provider_gateway.rs](crates/brassclaw_reborn_composition/src/mcp_provider_gateway.rs)
and [mcp_chat_bridge.rs](crates/brassclaw_reborn_composition/src/mcp_chat_bridge.rs).
Their source existence does not qualify every provider's final-prefix/advertisement
ordering or complete lifecycle acceptance.

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

Measure the **actual encoded provider-bound request**, with each piece counted
once. History/Tool context already contained in the dynamic prompt must not be
added again. Check the prefix and dynamic slices against their respective caps,
the derived total-input ceiling using checked arithmetic, and the physical/
deployed context with the actual effective output allowance and protocol framing.
The total is derived, not another editable settings copy. Preserve an explicit
original `max_output_tokens` setting and the supported fallback when missing;
output reservation is distinct from actual usage and discretionary token budgets.
See [the limit meanings and output contract](docs/reborn/model-token-limits.md).

The shared technical pre-dispatch owner checks the final transformed request,
independently of budgeting mode, review mode and supported continuations. The
pre-call Sempai request and the target request are separate guarded envelopes.
Completed-turn review has its own selected workflow/request and never resumes
the original target send. Oversized data is rejected whole, without silent
truncation, fallback or another provider attempt. Source settings alone do not
prove complete runtime enforcement of this target.

## 7. What Kohai does NOT do

| Responsibility | Actual owner |
| --- | --- |
| Choose the target model, provider, or prefix | Retained Recipe, Orchestrator, role contract |
| Select the workflow tier (0, 1, 2) | Routing and Recipe assembly |
| Compose evidence, transcript boundaries or correlated records | Sempai completed-turn analysis Recipe, PythonCode components |
| Decide whether review is needed | Recipe/Orchestrator contract, operator configuration |
| Reformat Sempai's returned dynamic prompt | Current contract: insert the return without further formatting. V3.5 target design: validate the structured response and deterministically insert only the originally bound content; never editorially rewrite the corrected task text. Requires the qualified successor in section 8. |
| Replay an original logical target invocation | Never — single submission/stop-only rules fence that invocation; separately declared later steps or exchanges retain their own supported identity/custody |
| Choose a fallback prefix when the selected one is missing | Never — block the call; report the missing prefix |
| Replay the turn after a routing/Recipe error | Never — only actual No-Match enters No-Match handling |
| Run as a second agent loop | Never — the global Monty service owns the loop |

## 8. Sempai pre-call correction: current contract and proposed successor

The current binding target is the
[explicit handoff contract](docs/reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests):
Kohai formats the dynamic target prompt once; optional Sempai returns its final
dynamic value; Kohai inserts the finished target prefix unchanged once, checks
the complete request and submits. Kohai may validate the return, but must not
format, normalize or rebuild it. This guide does not supersede that contract.

The current compatibility interceptor in
[model_interceptor.rs](crates/brassclaw_reborn/src/loop_driver_host/model_interceptor.rs)
passes the volatile tail as JSON `(role, text)` pairs and applies
`SempaiReviewOutcome` through `recompose`. Its protocol/system-context guards,
leading-system-message copying and legacy proposal side effects are described
in [sempai.md section 6.1](sempai.md#61-current-implementation). That source
support is not proof of the complete final-dynamic-prompt serving target.

### Proposed structured reference handoff — unqualified

A structured representation with protected references is the **desired v3.5
target design — not yet implemented or qualified**. Central contract alignment,
a strict versioned interface and connected consumer qualification remain open.
The existing Prefix/handoff owner must define the successor to the current
prohibition on rebuilding Sempai's return and align the affected plans.
Until the qualified cutover, the current binding target above remains in force;
these guides alone create no supported schema or runtime API. See
[sempai.md section 6.2](sempai.md#62-proposed-structured-reference-handoff--unqualified).

The v3.5 target design requires these properties:

1. A reference binds an exact already-selected immutable artifact and its trusted
   origin; it never means a latest lookup or a caller-selected foreign object.
2. Sempai receives the matching qualified knowledge or explicit evidence/contract
   needed to reason about each reference. A name alone proves no inspection.
3. Original task-scoped history, relevant Tool calls/results and the current
   original request remain explicit, protected, attributable context, not new
   instructions or standing prefix knowledge. Editable task text is a separately
   identified derivative linked to that original request; it does not alter it.
4. The typed response names editable fields and rejects missing, changed, unknown
   or foreign protected references and loss of required content.
5. Kohai validates the structured response and deterministically inserts only
   the originally bound content under an exact placement and byte/role-preservation
   contract. This mechanical resolution is distinct from editorial reformulation:
   Kohai never additionally rewrites the corrected task text or makes another
   model call or unqualified host read. Central contract alignment and qualified
   cutover are required before resolution replaces today's unchanged-return rule.

An illustrative input shape, **not a supported schema or complete protocol**, is:

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

This sketch supplies neither strict recursive input/result contracts nor a
response schema, safe resolver, invocation binding or activation evidence.
Reference resolution would require the original selected data, supported native
reads where necessary and qualified pure transformations. Old stored formats
and captured task selections keep their original meaning; no per-call Q1/Q2
or direct execution API is introduced by the proposal.

## 9. Custody and evidence

For the retained selected Tool invocation, `PgRecipeModelResponses` in
[`pg_monty_model_response.rs`](crates/brassclaw_reborn_composition/src/pg_monty_model_response.rs)
owns actual model-answer custody. Its
[related-model owner](crates/brassclaw_reborn_composition/src/pg_monty_related_model_response.rs)
retains Sempai's distinct work/request/response identity under the original
invocation. Those native records do not sequence or approve the review.

To implement the v3.5 target handoff, inspect these existing records and actual
callers for coverage of original context, exact reference bindings, reviewer
request/response and the effective final target request. Extend only missing
coverage through existing owners, with versioned formats/readers and migrations
where their actual storage contract requires them. Never reinterpret old bytes.
Retain uncertain responses/commits for original-ID reconciliation; missing
acknowledgement, a disconnected provider or failed output validation permits no
repeat model call, reset invocation or original-turn replay.

## 10. Implementation status and remaining work

| Area | Status |
| --- | --- |
| Retained native model-usage input/result and ordinary No-Match driving | Source-supported through the distinct callers in section 2; not full serving qualification |
| Prefix insertion (four-rule contract) | Binding target; serving migration open |
| Sempai pre-call correction (current `(role, text)` contract) | Compatibility source-supported; exact guards and legacy side effects are described in sempai.md section 6.1 |
| Structured reference-based Sempai handoff | Desired v3.5 target design; not yet implemented or qualified. Central contract alignment, strict interface, connected readers/writers and activation evidence remain open |
| Hard input limit enforcement at pre-dispatch boundary | Binding target; complete wiring open |
| MCP request binding and ordinary-chat bridge | Source-supported in the named adapters; final-prefix ordering and complete caller acceptance remain with their owners |
| Custody of Sempai call and final assembled request | Partial; v3.5 extensions open |

The [sempai_v3.5.md](docs/plans/sempai_v3.5.md) and
[path_to_v3.5.md](path_to_v3.5.md) plans own the concrete implementation steps.
This guide describes the target contracts; it does not certify completion.
