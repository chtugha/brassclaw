# Model input ceilings

> **Current status — 2026-10-22:** use [the combined implementation and ownership
> index](../implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


The writable owner is `ProviderDefinition.model_token_limits[model]` inside the
existing PostgreSQL `brassclaw_llm_providers.definition` record. Inference and
Prefix settings use the same editor/query and GET/POST
`/api/webchat/v2/llm/providers`, with the provider's `expected_revision`.

## Original native output setting — implemented prerequisite, 2026-10-30

The typed native prompt may supply optional positive u32 `max_output_tokens`.
The existing original invocation argument journal is authoritative; current
settings or response content cannot refill that original value. It reaches the
existing SDK output parameter and cost quote through the same response custodian.
The quote is an estimate, not actual token usage. An enabled live output budget
rejects an excessive explicit value without clamping; disabled discretionary
budgets preserve the explicit request setting. Technical context limits still
apply at the actual complete-request guard. Missing preserves a valid original
native fallback; null/zero/invalid representations do not become missing.

The bootstrap prompt candidate requires the typed reservation outside fixed
B1/B2 instruction/packet bytes. [Scoped acceptance](../validation/2026-10-30-original-model-output.md)
covers original PG arguments, actual SDK, native final counter and no-field
compatibility in their recorded fixture scopes. It supplies no Free workflow
qualification, activation or accepted design; their remaining owners are unchanged.

## Prefix selection, pair design and assembly — binding target, 2026-10-26

Keep three contracts distinct:

| Contract | Owner and meaning |
| --- | --- |
| Provider/model design | One accepted revision for the provider/model pair: supported prefix and dynamic-prompt formats/standards, framing/join rules, tokenizer/cache compatibility and references to canonical prefix/prompt limits. Generator, Kohai and Sempai use that same design when targeting the same pair. |
| Prefix profile/content rules | Role/domain knowledge, source/coverage requirements, teaching examples, whole-unit selection and content ordering. Sharing a pair design does not require identical role content. |
| Prefix generation | Finished text already formatted for one profile and provider/model pair, with exact character/UTF-8 identity and pinned design/content/source revisions. Kohai never formats this artifact again. |

**Selection:** use the specific prefix determined by the retained Recipe,
Orchestrator instruction or role contract (for example Sempai). When none specifies
one, select the compatible `base-prompt` for the target pair. No-Match therefore
normally uses the base prompt; the base prompt is not restricted to Tier 2.
Conflicting explicit selections must be resolved by the existing declared routing
contract or reported as a conflict, never by an invented implicit priority.
An explicitly selected prefix that is missing, stale or incompatible blocks that
consumer; it is not absence of a selection and must not silently become base.
Missing base readiness likewise blocks the dependent call, not model-free work.

Relevant source knowledge can be shared and explicitly required composites remain
possible. Do not automatically prepend the whole base prompt to every role/domain
prefix. Generate a complete artifact for the selected consumer; an explicitly
compiled composite is one artifact, not concatenated rendered chat templates.

**Four binding rules (operator clarification, confirmed 28 October 2026):**

1. Generate a finished, provider/model-specific prefix for the concrete target
   pair and store it immutably. Formatting this fixed prefix happens during
   generation.
2. The generator, Kohai and Sempai use the same stored, accepted design revision
   whenever they target the same provider/model pair, including when that pair
   also serves as the Sempai provider. Separate role prefixes do not create
   separate pair designs.
3. Kohai first formats only the dynamic Orchestrator prompt for the final target
   pair. If Sempai is connected/enabled, Kohai sends that already formatted prompt
   to it for optimization/correction. Sempai's own call uses its own prefix,
   generated for its selected target pair's design. Sempai returns the final
   dynamic prompt to Kohai.
4. Kohai then inserts the intended finished prefix unchanged and exactly once
   before the final dynamic prompt. It does not format Sempai's return again.

Check the actual complete request after that join, then send it to the target
model. These rules replace every earlier review-before-format or whole-prefix
runtime-formatting interpretation. They define the target architecture, not a
claim that the serving migration is complete.

Sempai's return is already the final dynamic prompt; Kohai validates it against
the retained target contract but does not format, normalize or rebuild it again.
Without Sempai, Kohai's initially formatted dynamic prompt is the final one.

### Explicit handoff between the two model requests

| Stage | Required handoff |
| --- | --- |
| Generate | Store the finished immutable prefix for each concrete provider/model pair and role, using that pair's accepted design revision. |
| Format | Kohai formats only the Orchestrator's dynamic prompt for the final provider/model pair, using the same accepted design revision as its prefix generator. |
| Optional correction | Connected/enabled Sempai receives that formatted dynamic prompt for correction. Its own model request uses its own generated prefix and its own target pair's accepted design. It returns the final dynamic prompt for the original final target to Kohai. |
| Insert | Kohai prepends the intended finished final-target prefix unchanged, exactly once, to the returned dynamic prompt; without Sempai, it uses the initially formatted dynamic prompt. |

The handoff has three distinct dynamic values: the Orchestrator's original prompt,
Kohai's target-formatted prompt and, when review is enabled, Sempai's returned
final dynamic prompt. Sempai receives the second value and returns the third;
Kohai inserts the prefix before that returned value. It must not insert the
prefix before the original or pre-correction value instead. With review disabled,
Kohai uses its target-formatted value directly. Sempai's own generated reviewer
prefix belongs to its separate model request, not to its returned dynamic prompt
or to the final target's prefix insertion.

The design is shared by provider/model pair, including Sempai when it uses that
pair. Role-specific prefixes remain separate artifacts. Formatting Sempai's own
review request does not reformat the final-target prompt carried inside it as
review data. After Sempai returns, the remaining final-target stages are exact
prefix insertion, complete-request validation and submission, with no additional
prompt-formatting pass.

Acceptance must trace these handoffs through the actual callers: one dynamic
formatting pass before correction, the returned final dynamic value used without
a second formatting pass, and one unchanged final-target prefix insertion. Cover
enabled, disabled and unchanged-result review, plus same-pair and different-pair
review providers. Compare the actual generated and inserted prefix characters and
the same-pair accepted design revisions; separate role prefixes need not be equal.
This is required acceptance, not a claim of completed runtime migration.

Sempai's own request uses its separately generated reviewer prefix, made for its
own target-pair design. The already formatted final-target prompt is review data,
not Sempai's transport framing. Generator, Kohai and Sempai use the same saved
accepted design revision whenever they target the same provider/model pair.
The fixed final prefix, selected design and hard limits are not editable reviewer
output. Sempai's review call uses its selected pair's design and its specifically
generated reviewer prefix; same pair means same design, not the same prefix text.
Include actual history, tools/protocol metadata, framing and effective output in
the final guard, also when discretionary budgeting is off. Lossless transport is
allowed; a later mapping that mutates the prefix or moves dynamic content before
it is not qualified serving. Do not restore a whole-prefix runtime render stage.

**One settings authority:** manually edit both input ceilings per registered pair,
including non-role-selected models, through the existing provider revision/CAS
endpoint and shared Inference/Prefix/model-design query. Every editable view
reads/writes the same `model_token_limits[model]` entry, and successful writes
refresh all views; stale writes conflict. No independent mutable role/design/prefix
limit copy. Immutable generations/designs/measurements may retain historical
policy values/references, visibly distinguished from current settings; edits never
rewrite their bytes. Revalidate compatibility/regenerate as needed while retaining
old task selections subject to live hard constraints. Raising a setting cannot
increase physical context or authorize unsupported formatting.

Populate design facts from supported provider metadata APIs, versioned official
documentation, retained model artifacts or explicit operator input. Record origin,
revision and verification status; unknown stays Unknown. Provider refresh must not
silently overwrite manual settings or activate a new design. Reference a documented
format/standard with its version and qualify the actual adapter; a standard's name
alone proves neither tokenization nor compatibility.

Assembly is request-owned. Provider host configuration access, server template
installation/overrides, service changes and global automatic role-prefix injection
are not product configuration options. Prefix generation and cache prefill are
separate operations; a prewarm receipt proves neither persistent cache residency,
free tokens nor answer quality. Historical placeholder/template/prewarm observations
remain evidence for their recorded configuration, not current architecture.

## Formatting design must contain construction instructions

The main model-design deliverable is a filled, evidence-backed **construction
specification**, not only token limits, a format name or a link to documentation.
It tells both the compiler and Kohai exactly how to construct their respective
parts for this provider/model pair. The following are required design contents,
not claims that today's proposal schema already accepts new fields:

| Part of the pair design | Required concrete instructions/evidence |
| --- | --- |
| Exact target and sources | Registered provider/model identity; immutable model/tokenizer/template revisions and hashes; protocol/backend version and evidence of effective defaults. Distinguish base model, quantized derivative and actual deployment. |
| Prefix construction | Supported registered adapter plus validated declarative options; content placement, section/delimiter conventions, exact leading/trailing literals, newline/escaping rules and which role/protocol framing is already present. No vague "use the model format" instruction. |
| Dynamic prompt construction | Exact role/order rules for history, user input, Tool definitions/calls/results, reasoning and supported multimodal parts; provider-default behavior and generation-start/end handling. Kohai applies these rules before optional Sempai optimization/correction; Sempai returns the final target-compatible dynamic prompt and Kohai does not format it again. Identify missing/empty/null and tool-enabled branches. |
| Join boundary | Precise insertion layer, separator ownership and tokenization/special-token ownership for fixed prefix + already formatted dynamic prompt. Preserve prefix characters once; avoid double BOS/EOS/chat markers, trimming or a second whole-prefix render. |
| Format standard/template | Named standard and version only where applicable, backed by the exact default template or documented provider protocol. Map each rule to actual template branches and a supported client adapter; a family/display name is insufficient. |
| Model memory architecture | Evidenced full/sliding/linear/recurrent layer arrangement and relevant state constraints. Separate architecture declarations from actual serving cache behavior. |
| Effective cache construction constraints | Backend-version-specific attention/recurrent dtypes, compatible groups/checkpoints, page/block units, prefix-match boundaries, namespace and offload/restore restrictions where exposed. Weight INT4 is not evidence of KV/state INT4. Use only verified supported constraints to guide whole-unit packing/layout. |
| Capacity references | Canonical editable prefix/prompt ceilings and current policy revision, model/deployed context and effective output reservations; measured overhead belongs to the actual assembled request. No editable copied limits or hardcoded geometry. |
| Worked construction cases | Fixed sample inputs and independently verified expected formatted bytes/token IDs or a precisely documented provider-supported observation. Include Unicode/newlines, tool changes, reasoning settings and continuation/review cases. Trace Kohai formatting before review, Sempai's returned final dynamic bytes and one exact unchanged prefix insertion; assert no second formatter call after review. |
| Unknowns and invalidation | Per-fact provenance/status and required versus optional capabilities. Template/backend/tokenizer changes trigger compatibility review. Opaque/missing facts cannot become an "exact template/cache match" claim by inference. |

Fill this specification using supported provider metadata/tokenization interfaces
where available, original model tokenizer/config/default-template files and
versioned official model/provider/backend documentation. Research is an explicit
observable preparation stage using existing fetch/research/retention usages, not
live browsing inside deterministic Generate. Record original content, exact URL/
revision/hash, relevant source ranges, observation time and which design rules each
source supports. A public model template is not automatically the effective template
of a quantized derivative or hosted endpoint. Conflicts must be resolved with
appropriate target evidence or retained as Unknown; never silently prefer "latest".
Manual entries remain possible and visibly operator-supplied; discovery does not
silently replace manual limits or accepted formatting rules.

The design-generation Recipe collects these facts, asks the selected qualified
Prefix-Maker/design provider to derive the concrete supported declarative rules,
retains the raw proposal and evidence mapping, then validates the actual construction
cases through the existing formatter/tokenizer/request adapters before acceptance.
A researched rule that needs a new adapter is implementation work, not arbitrary
Jinja/Python/shell generated and executed by a model. Extend the existing versioned
model/design records and compatible readers in their owner; no parallel registry or
new guessed fields inserted into historical strict schemas. Store a filled accepted
pair design only after its required facts and cases pass; show which construction,
serving and cache capabilities are actually qualified.

Generate then reads those saved instructions with the selected profile's separate
content rules and current approved sources to produce the finished prefix. Kohai
reads the same saved pair design to format only the dynamic prompt. Provider/model
change selects the appropriate design and target-specific prefix generation; it
never applies generic format defaults or an Ornith memory layout to another pair.
Template/cache research belongs in the design and Prefix-Maker reference, not
copied as irrelevant runtime knowledge into every role/domain prefix. Memory/layout
optimizations need correctness and held-out quality evidence; no arbitrary padding,
loss of mandatory knowledge or claims that formatting changes the server allocator.
Unknown private cache internals cannot be invented: optional cache optimizations
remain unqualified, and missing facts required for exact serving qualification block
that capability. No server configuration change is part of obtaining compatibility.

### Official research entry points; not deployment qualification

For an Ornith investigation, begin with the original
[model configuration](https://huggingface.co/ornith-ai/Ornith-1.5-9B/blob/main/config.json),
[tokenizer configuration](https://huggingface.co/ornith-ai/Ornith-1.5-9B/blob/main/tokenizer_config.json)
and [default chat template](https://huggingface.co/ornith-ai/Ornith-1.5-9B/blob/main/chat_template.jinja).
Capture immutable revisions/hashes rather than retaining these moving `main` URLs
as sufficient evidence. Verify the selected quantized model and serving target
separately; upstream identity alone does not establish either match.

The inspected upstream template has a tools branch preceding first-system content.
This is a concrete case the insertion adapter must qualify; merely prepending text
to a system message does not prove a fixed leading token prefix. If the effective
mapping cannot preserve the required fixed-prefix-first contract, report the
unsupported path rather than install a server override or accept a non-leading
span as equivalent. The upstream config declares mixed attention/state architecture;
actual deployed cache precision/layout still requires serving evidence.

The [vLLM hybrid-cache design](https://docs.vllm.ai/en/latest/design/hybrid_kv_cache_manager/)
explains group-dependent reuse and state checkpoints. It is a research entry point,
not evidence that this endpoint runs that documentation version or layout. Pin the
actual backend-compatible source/version and verify applicability before deriving
cache construction constraints; do not transpose generic Mamba rules onto another
linear/recurrent implementation without evidence.

## Generation prompts: missing selection and model Use

A missing/incompatible selected prefix remains a visible readiness state, with
an **OK/Cancel generation dialog**, not silent substitution by base. Identify
profile/role and exact provider/model pair; explain whether a fresh target-specific
generation is needed. OK admits the existing model-free Generate/Regenerate
operation using the accepted pair design, canonical current limits and reviewed
content. Cancel starts no generation/provider call and leaves an explicit
not-sent/cancelled preparation result. If design/compiler/coverage prerequisites
are absent, show Setup required / Needs redesign / Capacity blocked and the
supported remedy; OK must not pretend unsupported generation can repair them.
No hidden provider-assisted redesign, content clipping or fallback.

After a successful WebUI **Use** selection, show another **OK/Cancel** dialog:
"Generate the base prompt for <provider / model>?" Bind it to the actual committed
selection receipt and refreshed current pair/revision, not an obsolete response.
Trigger it from the explicit Use action/committed receipt, not ordinary query
refresh, page rendering or a token-limit settings save. Reconcile uncertain Use
through its original mutation identity rather than resubmitting it. This applies
to each successful Use, including a Sempai-role selection. If a
compatible current base already exists, show that state and offer explicit
regeneration. OK requests the base profile, not the role prefix; it must not
replace Sempai's selected reviewer prefix. Cancel declines generation only;
it does not silently undo the already committed provider selection. Show Needs
prefix/other readiness blockers as applicable and retain a later Generate action.

Use existing durable generation jobs, receipt/readback, progress, cancellation and
reconnect handling. Repeated clicks/reconnect must not create duplicate effects;
a stale confirmation rechecks selection/design/policy and reports the change.
Generation, validation, publication and consumer activation remain distinct existing
operations. OK authorizes the advertised generation operation, not unreviewed
activation; Generated alone must not release a model call. Show the existing
qualified activation action/readiness after generation where required. Keep the
original request at the supported pre-dispatch preparation/wait boundary until
its exact compatible prefix is selected and ready; bind the finished artifact
before the first provider attempt through the normal retained selection owner.
Do not rewrite an already pinned artifact, replay an attempted model call or
resubmit the original user message to resume. Missing wait/selection support is
an explicit runtime prerequisite, not a modal-local executor or queue.

For a held task, repair generation must be compatible with its retained catalogue,
profile and pair-design selection; a new global source revision is not permission
to change an existing task. Post-Use base generation is new work and captures its
own coherent current selection. Reject unsafe resumption of an already pinned
incompatible artifact; implement missing preparation/wait support in its owner.
Interactive WebUI presents the modal from the shared readiness result. Headless
CLI/API callers receive the supported structured readiness/remedy instead of an
invisible modal or unrequested generation; they still make no blocked model call.

The [Prefix plan](../plans/prefix_v3_upgrade.md) owns implementation and acceptance.
Settings/source-only/diagnostic support is not final serving integration. Qualify
default and explicit selection, conflicts, same/different pair role prefixes,
shared manual edits, exact prefix bytes and final request bounds through actual
callers before claiming that contract implemented.

For the requested current Ornith configuration:

```json
{
  "model_token_limits": {
    "cyankiwi/Ornith-1.5-9B-AWQ-INT4": {
      "max_prefix_tokens": 100000,
      "max_prompt_tokens": 14688
    }
  }
}
```

This is the policy fragment for an ordinary provider update, not a second
configuration file or complete API request. Preserve other model entries and
include the exact provider identity, adapter and revision. Omission preserves
existing limits; `{}` clears the map; null is rejected. Stale revisions conflict.
No provider activation or credential replacement is implied.

Both values are always-on hard input limits, independent of
`token_budgets_enabled`. Disabling discretionary budgeting must preserve them and
all technical size checks. Kohai needs both the active answer target's and Sempai
target's host-resolved limits. Check the complete Sempai request before review,
then the final edited answer request with history/tools/framing and prefix after
review, and repeat technical enforcement at the shared pre-dispatch boundary.
Settings/planner support alone does not prove this runtime wiring is complete.

| Value | Owner / meaning |
| --- | --- |
| Maximum prefix tokens | Canonical per-model map above. Zero disables prefix capacity. |
| Maximum prompt excluding prefix | Same map; includes remaining rendered history, tools and input framing. Positive integer. |
| Total input ceiling | Derived checked sum, 114688 for this request. Never another stored setting. |
| Physical/deployed context | Server observation and declared provider context are evidence/bounds, not aliases for maximum prefix or prompt. The recorded Ornith endpoint observation was 131072; reverify the selected target. |
| Output allowance | Separate request/model constraint. The difference here is 16384; do not store another editable copy of that arithmetic. |
| Model specification/design/measurement | Immutable historical pins and observations, not settings endpoints. |
| Standalone compiler `--context-tokens` | An explicit offline build target. It does not edit live policy and cannot authorize an oversized production prefix. |
| `ProviderTokenBudget.total_input` and per-slice budgets | Optional accounting/assembly policy, disabled with token budgets. These are not per-model prefix or dynamic-prompt settings. |
| `--max-num-batched-tokens` | vLLM scheduler/prefill batch size; not the complete prompt or prefix limit. |
| Encoding worker token/frame/heap ceilings | Technical containment of one preparation operation; not a second model capacity setting. |

The audit found no `max_prefix_tokens` or `max_prompt_tokens` setting in the
previous production Rust/JavaScript/SQL paths. The 114688 prefix preview existed
in standalone Sempai usage instructions and this plan, not as a deployed per-model
database setting. Those current instructions now request a 100000-token prefix
and preserve archived experiments unchanged. No historical receipt is rewritten
to make its measured token counts agree with a new policy.

New immutable prefix model captures include the model's configured limits and
provider revision. Existing records remain historical evidence. Future compiler
and final-prompt consumers must resolve the canonical policy for new work, pin it
for retained work, and check actual target tokenization without silent truncation.
Persistence/UI support alone does not establish final-prompt enforcement or cache
qualification; the production resolver remains a prefix-plan prerequisite.

`plan-prefix-model-budget` is a reusable pure component draft for the successor
generation/design Recipes. It derives a planning budget from pinned policy,
requested design size, deployed context and explicit reservations. It reports
shortfalls rather than changing settings or silently promising less prompt/output.
It does not choose sources, verify policy identity, measure rendered tokens or
qualify activation. Existing measurement-v1 results remain total-probe diagnostics.
The detailed [design contract](../plans/prefix_v3_upgrade.md#budget-aware-design-and-generation-contract-2026-10-22)
requires accepted budget-dependent whole-unit policies, exact composite/partition
measurement and coverage/quality checks before model-facing use.

On 2026-10-22 the running `brassclaw2:3000` application exposed no provider
revisions/model limits and reported `active: null`. Its separate model endpoint
advertised Ornith with 131072 context. Do not write new definition fields into an
older application behind its API: old strict decoders may reject them. Application
upgrade/support and a successful revisioned API readback are required before
claiming these requested values are set on that instance. No inference service
change is required by this policy.

## Open K3-2 successor — retained prefix selection and prefix-free construction workflows

**Operator-confirmed successor; runtime integration remains open.** This section
implements the document contract from [Exact K3-2](../plans/Exact_V3_definition_update_at_K3-2.md),
especially §§4–5.2 and 9–10. It changes future consumer requirements, not historical
acceptance or the selection of an already admitted task. It succeeds the earlier
unconditional post-Use Base dialog and prepared Prefix-Maker requirement for these
workflows. Ordinary chat still follows the existing pair/design/assembly contract.

The existing retained prefix-binding owner distinguishes three semantic states:

| Selection | Required behavior |
| --- | --- |
| `Default` | Apply the existing declared Recipe/Orchestrator/role selection; use compatible Base only when none specifies another prefix. Capture the resulting exact generation before effects. |
| `Selected` | Resolve the explicit profile/prefix/composite to the exact compatible generation and qualification in the same coherent capture. Missing, stale, incompatible or conflicting selection is an error, never Base or prefix-free fallback. |
| `PrefixFreeWorkflow` | The qualified workflow intentionally uses no generated prefix, including its required recursive child/review work. Retain its exact workflow/instruction/target binding; each dispatch proves this selection and omits generated prefix insertion. |

These are proposed internal typed states, not fields to inject into old Recipe
JSON or user model arguments. Extend `RecipeModelPrefix` and its existing native
resolver/capture/response/final-guard consumers, not a second selection system.
`Inline` and `FinalBoundary` remain insertion mechanics for a real prefix;
FinalBoundary does not mean prefix-free. `None`, empty strings and resolver errors
cannot express deliberate absence. Preserve old record versions and native scope.

Prefix preparation, configuration discovery with optional semantic extraction,
design/Create/Redesign work use `PrefixFreeWorkflow` even when other prefixes exist.
Required child/review edges inherit this qualified workflow standard, preserving
their original model/auth/catalogue bindings. Independent ordinary chats do not.
Explicit candidate behavior tests bind the candidate as test subject without
production activation; they do not require an already active Prefix-Maker prefix.

After successful Use, admit one original-ID-bound configuration workflow. Fetch
supported metadata/artifacts first. Use optional prefix-free B1/B2 calls only for
specific evidenced interpretation needs, with the exact prompts, response forms,
non-LLM schema/evidence/value checks and storage projections in Exact §§4.1–4.8.
Provider facts, unverified model suggestions and editable operator ceilings stay
distinct; no hidden overwrite of manual values or copied model-limit store.
Show model context and editable prefix/prompt ceilings before Generate.

For a compatible accepted pair design, offer Base generation/regeneration. If no
usable accepted design exists, offer “Create and approve model design now?” and
navigate to that exact pair. Dialog Cancel preserves committed Use; the already
admitted configuration job is separately visible/cancellable. Reconnect does not
replay Use, model effects or generation. Source/read errors are not “no design”.

Known protocol/template/profile rules construct a design without LLM calls.
Optional model work remains prefix-free; no source-less guess about hidden server
internals. The target pair is the default design executor, with explicit alternate
selection captured before effects. Design approval is direct after all required
checks; View/Review is readable and optional. Acceptance creates no serving choice.

Every profile's generation Recipe includes all its required checks: those checks
are Prefix-Q1, not a second later validator job. Publish only the exact successful
generation and present it for human Prefix-Q2 activation. No Generate/Regenerate/
Redesign acceptance automatically activates a new prefix. Current compatibility,
live limits and selection CAS remain checked; do not rerun unchanged Q1 effects.

Retain two previous distinct published generations per stable profile and exact
pair as the archive target. The operator has explicitly confirmed the
protected-reference treatment: keep older active, resumable or shared payloads
until their live references are released; then collect them automatically. Do not
block publication or end tasks merely to force the count. Qualify the existing store's GC/FK/reader transition before any
new deletion; do not rewrite immutable evidence or disable protective triggers.

The final guard remains mandatory even when insertion is omitted. Count all fixed
instructions, sources, history, tools and framing as actual prompt input. The
BrassClaw guarantee covers its own immutable prefix and qualified request mapping
up to provider handoff, not monitoring undisclosed provider-internal changes.

A generated per-pair Prefix-Maker prefix is only the v3.5 idea in Exact §5.1.
Document the future optional selection/insertion seam in the existing owner; do
not require a current artefact, resolver, migration or feature flag for it.


## Open Prefixless successor — visible Maker/Base/Prefixless construction selection

**Binding target for new qualified workflow revisions; runtime integration is
open.** This is the single selection contract for the future construction work
in [Prefixless redefine at K3](../../Prefixless_redefine_at_K3.md). It changes the
construction instruction-context requirement, not existing job selections,
accepted packets, protected criteria or general component/Prefix approval rules.
Ordinary independent chats/reviews keep their existing selection contracts.
The earlier sections and their anchors remain exact references to their own
contracts; the supersession map below identifies what changes for this successor.
No typed state, API field, schema or qualified dispatch support is added by prose.

### Visible suggestion and submitted choice

Maker → Base → Prefixless is **only the initial visible, editable UI suggestion**
for a construction form. It is not a dispatch preference or a fallback algorithm.
Deterministically suggest an eligible active approved Maker for the executing
pair and purpose; otherwise an eligible active approved Base; otherwise Prefixless.
Offer other approved compatible profiles too. Prefixless remains explicitly
selectable even when Maker or Base is available. No model call computes this
suggestion, and viewing or refreshing the form causes no Use/Generate/activation.

Eligibility means exact qualification and active publication for the namespace,
executing provider/model route, accepted executor design/adapter and instruction
composition required by this work. A stored profile, generated draft or shipped
documentation-only baseline is insufficient. A newer draft does not invalidate
a still-valid active artifact. Do not search archived generations for a hidden
fallback. Missing/incompatible artifacts differ from database, integrity or
resolver errors. Technical failure is an error, not evidence that a prefix is
absent; an unavailable required reader is an implementation/readiness gap.

The visible field precedes every new model-assisted construction admission,
including optional Use/configuration B1/B2 work, design/Create/Generate/Regenerate/
Redesign and their required reviews. Deterministic preparation stays deterministic.
The operator's manual choice wins over the suggestion and survives refresh,
notifications and reconnect. A pair change revalidates it; an invalid choice or
stale displayed generation/revision is a visible conflict, never an automatic
replacement. No extra confirmation modal is required before the explicit action.
Headless callers provide the corresponding explicit supported choice; they do
not acquire a hidden runtime Maker/Base chain.

At submission, validate the actual visible choice against the coherent qualified
selection and displayed revision, then durably bind its exact generation or
intentional absence, executor route, instructions, workflow and original job
before the first affected model effect. Keep the suggestion/reason distinct from
the operator's submitted choice. Do not substitute latest between display and
admission. After capture, changes require a genuinely new explicit job; duplicate
clicks and uncertain-response retries retain original request identity and bytes.
No later fit, policy, transport or measurement failure reselects a prefix or
replays an effect. Eligible retries retain selection and durable attempt state.

### Three qualified request constructions

The following rows name instruction-context choices, not new runtime enum tags.
All fixed task/response/source rules and current technical authority still apply.

| Submitted context | Required construction and qualification |
| --- | --- |
| Approved Maker | Use the exact compatible retained Maker generation once, under its qualified pair design/adapter. It may replace only instruction sections explicitly covered by a qualified replacement contract; otherwise keep fixed instructions. No self-approval by the successor being generated. |
| Approved Base or another compatible approved prefix | Insert the selected artifact exactly once under its actual qualified design/adapter, alongside complete fixed task instructions. Do not prepend an additional whole Base to a selected Maker/role prefix, perform a second format pass, or treat source facts as prefix knowledge. |
| Explicit Prefixless | Insert no generated prefix and require no manufactured artifact or proof that Maker/Base is absent. Keep fixed instructions, original route/auth/model, technical encoder/protocol, full request guard and correlated native response/effect evidence. A capture declaration or measured string alone cannot qualify this branch. |

First construction must remain possible without an existing generated artifact;
it still requires a genuinely supported technical request boundary. Missing
encoder/protocol/guard/admission/response support is explicit readiness work,
not permission for an unqualified template, alternate SDK path or a bypass flag.
Reusing an eligible older active Maker to construct its successor is allowed;
a candidate behavior test binds that candidate as the test subject and does not
activate it or turn it into its own independent reviewer.

B1/B2 remain restricted to their original supplied source/option packets and
strict answer checks. Selected-prefix knowledge cannot fill unknown provider
facts. Preserve old fixed prompt/packet and payload-hash bytes; any genuinely
needed instruction artifact successor is versioned by its existing owner.
The full request identity additionally binds the chosen generation/instructions.
If a prefix conflicts with those fixed contracts, it is ineligible for that
purpose; appending a generic instruction to ignore the conflict is no proof.

### Capture, children and ordinary Recipe configuration

Selection belongs to the actual executing pair, not merely the design target.
Known required child/review routes receive their own visible or stored qualified
choices before admission. Later supported children bind only original retained
route/catalogue data before their first effect. They do not repeat the suggestion
chain, borrow another pair's prefix, invent an absent role or read global latest.
Keep legitimate parent/family/occurrence lineage and reject foreign children.
Same-pair reuse still requires compatible purpose/instruction context; shared
clients do not imply shared request-dependent guards. Normal independent Sempai
review keeps its separate reviewer contract. Exact final-target handoff and
single insertion rules remain; the construction-review request uses its own
captured context, not the candidate or parent's bytes by implication.

The existing Recipe editor exposes the same explicit Prefixless/approved-profile
choices in its qualified successor, retaining the labelled existing Default where supported.
There is **no Recipe Generate button** and no automatic Maker/Base default for
all Recipes. Save/review the Recipe or variant's profile-level choice normally;
resolve and pin its exact generation at task start. Unlike a construction form's
submit-time revision check, this does not bake a displayed generation/version
into the reusable Recipe. No new per-step JSON overrides are implied. General
Recipe Prefixless requires its real qualified reader/runtime profile; do not
encode it as absence, an invented bootstrap purpose or an unknown old-format tag.
Existing strict Default/Selected/PrefixFreeWorkflow records retain their meanings.

### Guard, approval, old jobs and connected implementation

Measure the actual full provider-bound request: chosen prefix if any, fixed and
dynamic instructions, sources, history, tools, framing and real output setting.
Hard prefix/prompt/context/output/protocol constraints, live policy, cancellation
and limits remain effective in every branch. Disabled discretionary token budgets
create no artificial caps or reserve constants and do not disable technical
limits. An oversized request is rejected without clipping, fallback or another
provider attempt. Retain actual guard/response/wire/effect correlation; no success
claim from a serialized getter, declaration or source-only check.

The changed consumer/workflow needs its own qualification against existing prefix
contracts. Unchanged approved prefixes/components gain no extra per-job Q1/Q2;
component A-own/joint and S-bootstrap scopes remain unchanged. Prefix-Q1 and
human Prefix-Q2 stay with the existing generation/publication owner. Generation,
review, publication and activation remain distinct; this choice activates none.

Old running/waiting/resumed jobs retain their actual original contract and exact
payload/design/adapter/qualification pins, including old prefix-free workflows.
Never infer a new mode from a name or missing field. New construction references
protect artifacts through the existing retention owner, even beyond the ordinary
two-predecessor archive target while needed; no deletion or evidence rewrite
follows from this text. Live technical policy continues independently.

The existing Prefix owner owns options/eligibility and construction qualification;
Composition/Simplified own coherent declaration/admission/capture, real native
request/child/response/guard and continuation support. Existing UI/facade and
Validator owners consume those contracts. Connect and qualify the readers before
new writers/seeds/workflow activation. No parallel resolver, client, VM, queue,
review engine or mutable consumer registry. Current native Free rejection,
profile-only Selected data and missing UI/job/Recipe support are explicit gaps,
not implemented capability. Complete the adopted plan's future native PG/SDK/
HTTP/browser/retention cases through the existing owners before claiming uptake.

### Exact scope of superseded construction requirements

Only the following requirements change for this successor's new qualified
workflows. All accepted original bytes and ordinary contracts remain intact.

| Earlier local section and characteristic text | Successor scope |
| --- | --- |
| “Prefix selection, pair design and assembly” — “When none specifies one, select the compatible base-prompt”; “Sempai's own call uses its own prefix” | Still governs ordinary independent calls. New construction calls use the submitted visible choice and required construction-review calls their own captured compatible choice; pair design/framing and final-target handoff remain. |
| “Generation prompts: missing selection and model Use” — unconditional post-Use “Generate the base prompt” dialog | Already superseded by the preceding K3-2 workflow contract. Do not reinstate it or trigger model work before the new choice is available. Preserve committed Use, original mutation/job identity and explicit remedies. |
| “Open K3-2 successor” — “use PrefixFreeWorkflow even when other prefixes exist”; required child/review edges “inherit this qualified workflow standard” | New construction revision uses the exact submitted choice; each required route binds its own context without re-running a runtime preference. Existing admitted old jobs keep their original prefix-free contract. |
| “Open K3-2 successor” — “Use optional prefix-free B1/B2 calls”; “Optional model work remains prefix-free” | Optional model work stays optional and source-bound; new calls use the captured choice, with no deterministic-to-LLM conversion and no weakening of fixed B1/B2 packets. |
| “Open K3-2 successor” — Maker is “only the v3.5 idea” and requires no current artifact/resolver | Maker integration is now authorized future owner work, not restricted to v3.5. Maker availability is never required for first construction; an optional profile row is not qualification or runtime readiness. |

Keep existing anchors usable. The adopted plan's inventory must route current
summaries and consumers to this section and scope protected earlier passages,
without editing their original acceptance or hiding unresolved active conflicts.
