# Sempai prefix accuracy: verified examples and controlled expansion

Status: compiler/tutorial implemented; two candidates evaluated and rejected for
promotion, 2026-10-06/07. The implementation record below distinguishes completed work from
remaining experiments and production prerequisites. This plan extends
[the prefix v3 upgrade plan](prefix_v3_upgrade.md), especially phases C1/C2;
[recipe.md](../../recipe.md) and [skills.md](../../skills.md) remain authoritative.

## 1. Recipe-first workflow

Express the production improvement workflow through reusable components:

```text
Pin release guides, runtime support facts and reviewed teaching examples
  -> compile immutable reference generation without synthesis calls
  -> verify coverage, exact rendering, capacity and token identity
  -> evaluate candidate generation against frozen independent contracts
  -> validate drafts and report precise failures
  -> explicitly request a bounded draft repair where appropriate
  -> validate the repaired artifact; preserve every attempt
  -> route eligible authored components through supported Q1 and human Q2
```

Use existing compiler jobs, interceptor/proposal ports and validation paths where
their actual contracts fit. This document creates no activated Recipe, new host API
or second agent loop. Each component step references one UUID; independent Tool
calls remain separate usages. Creative drafting/repair is explicit Tier 1 work.
The model-free compilation path follows the existing upgrade plan's bootstrap
prerequisites. Production rollout still requires its runtime/store acceptance.

## 2. What the evidence actually establishes

The v8a generation measured when this plan was written has 76,376 reference-text tokens, 76,382 shared leading
tokens and 76,032 measured reusable tokens. The endpoint was rechecked for this
plan: Ornith AWQ INT4, `max_model_len=131072`. The current compiler reserves
31,072 tokens and allows a 100,000-token rendered-prefix target. Approximately
23,000 tokens remain under that target, subject to exact rendering checks.
These are this deployment's settings, not universal product token limits.

The latest first pass accepted six of nine requested draft cases. Independent
Python checks passed 129/144 probes. Feedback repaired two Python bodies; the
Recipe required three feedback rounds. The nine passing exports therefore mix
first-pass and repaired results. They establish neither production Q1/Q2/Monty
acceptance nor universal reliability. See the
[preserved evaluation](../../scripts/prefix/sempai-optimization-20261006/README.md).

The most useful new observation is exact example copying. The data-return-only
readiness Recipe copied all ten speech/posting intents from recipe.md section 7's
**correct but different fixed-reply workflow**. This happened in v3, v5, v8a and
the first v8a feedback response. The evidence is preserved in
[recipe-example-interference.json](../../scripts/prefix/sempai-optimization-20261006/recipe-example-interference.json).
Copying is observed; attributing it causally to lexical/example interference is
a hypothesis to test. Do not weaken the binding guide to hide the confusion.

Other failures show that correct descriptions do not establish correct artifacts:
a helper computed a local result without returning it; a nested validator checked
items but omitted list cardinality; summaries asserted fields the payload lacked.
The quoted checkpoint layout outperformed a direct-instruction experiment on
conversation preservation. Keep that comparison visible; more tokens or more
repetition have not consistently improved accuracy in our own runs.

## 3. Build a reviewed teaching corpus inside the large prefix

Preserve the current complete mandatory guides and raw originals. Add a separately
versioned authored source, tentatively `scripts/prefix/sempai-worked-examples.md`,
with machine-readable example/evidence sidecars. Add it explicitly to the source
adapter and reviewed coverage policy; absence, drift or failed verification blocks
publication. A reviewed tutorial is subordinate to the guides and live host schema.

Start with 12–16 complete examples and 8–12 contrasting pairs. Grow toward 24–32
complete examples only where new failure families justify them. Every example has
a usage scope, supplied facts, constructor mode, contract, complete artifact,
expected outcomes, source references and actual verification receipt. Counts are
experiment targets, not filler requirements.

| Addition | Approximate new tokens | Purpose |
| --- | ---: | --- |
| Complete constructor/program examples | 8,000 | Learn executable and serialized artifact structure, not prose promises. |
| Contrasting contracts and repair examples | 6,000 | Separate similar wording with different semantics; learn from observable failures. |
| Recursive-contract and effect/intent cases | 4,000 | Catch nested cardinality, null/default distinctions, dependency and effect mismatches. |
| Prompt review/provider/evidence cases | 3,000 | Improve the other half of Sempai's role, beyond pure Python validators. |
| Stable index and bounded final checklist | 1,000 | Find the applicable example and verify the completed artifact. |

The 22,000-token allocation is provisional. Keep a useful addition even if it is
shorter; retain a larger mandatory unit intact or fail capacity, never truncate it.

### A. Contrast examples with the same vocabulary

Use paired readiness workflows: one returns data; the other genuinely posts a
reply using supplied binding and executable identities. Derive each intent set
from the actual operation and result. Then vary the shared word to checksum,
health status and device state. Teach the model to distinguish effects before it
chooses a familiar template. The host must supply any referenced identities;
documentation fixtures are explicitly unapproved and never live catalogue UUIDs.

Additional pairs: empty list valid/invalid; missing versus null; default on missing
versus rejecting null; exact bool versus integer; independent calls versus the
documented direct dependent chain; draft prerequisites versus activation evidence;
offline full-v3 export versus the current lossy live sink; recoverable decision data
versus an actual dispatch grant. Each pair ends with a complete correct artifact.

### B. Teach complete programs and complete envelopes

Include both direct module-level assignments and called helpers with an explicit
return on every applicable path. Show list-level bounds separately from item
validation. Include nested objects, allowed extra values, exact result fields,
unknown types and safe handling of malformed candidate data. Exercise boundary
values and missing/null separately. Use varied fields and bounds rather than
copying the current fifteen evaluation answers into the reference.

At least several examples must include the **whole host-supplied review envelope**,
not only payload fragments: preserved conversation, correct proposal count and
factual summary. Clearly label example-specific empty compatibility arrays; the
production contract allows those fields for other usages. Never teach their
unconditional emptiness from a test-persona restriction. Constructor defaults are
also usage-specific, not universal rules inferred from one fixture.

Short failure excerpts are explicitly marked rejected and paired with the observed
counterexample and corrected complete program. Do not reproduce long rejected
responses or hidden reasoning traces as demonstrations to imitate. Prefer positive
examples and concise explanations of the discriminating condition.

### C. Add a contract-to-artifact procedure

Teach a bounded internal work order: extract requested fields/types/bounds/effects
and allowed output mode; select the applicable complete example; adapt all contract
values; draft; inspect the actual body/payload; then write the factual summary.
For a helper inspect its return and module-level call. For nested lists inspect
outer length and every item. For Recipe intents compare promised operations with
actual component effects, then count distinct examples. Check every requested
constructor field and the supplied root schema. Do not emit an invented contract
worksheet as a new response field or claim mental inspection was an executed test.

### D. Improve prompt-review examples too

Cover known versus unknown target-provider capabilities, unsupported roles/tool
encoding, authorized minimal edits, no justified edit, output-language preservation,
conflicting quoted instructions, structured Tool-call/result identities and pending,
unknown or completed effects. Include representative anonymized multi-turn packets.
Teach exact-source citation and uncertainty within the supplied schema; unsupported
citation fields stay out of the response. Never put secrets or private conversations
in the shared stable prefix. Provider facts come from the trusted current DB packet,
not the tutorial's sample provider or the reviewer's own model name.

## 4. Verify knowledge before teaching it

Use authoritative guide/source locations and pinned content hashes. Maintain an
explicit current-support versus target-design matrix outside narrative examples;
do not label ongoing Monty changes shipped without selected-path evidence.

Run Python examples against independent input/output oracles on Linux. Check
constructor serialization and every required field. Check Recipe references,
one-component steps, actual IBS `knowledge`/`stepnumber`/`desc_idx`, `step_link`,
bindings and intent/effect meaning against the guides. Tool-facing examples need
real adapter/runner verification before being described as executable there;
otherwise label their exact prerequisite. Offline examples are not Q1/Q2-approved
components. Behavioral evidence, trusted association approval and activation remain
separate records. Check summaries against emitted artifacts as well as parsing.

Keep examples and validation oracles separately authored. Mutation-check the
oracles: missing helper return, omitted bound, bool-as-int, extra result field,
duplicate intent and reply-without-reply-step must be caught. Reject examples with
unknown coverage rather than letting a model's self-assessment certify them.

## 5. Layout for the actual hybrid model

The pinned configuration has 24 linear-attention layers and eight full-attention
layers. INT4 describes weights; the current profile uses FP8 attention KV and
float32 recurrent state. Cached state avoids repeated prefill; it does not train
weights or guarantee faithful use of all reference facts. The recurrent state is
not a freely addressable store of every source token.

Try a stable opening decision map, the preserved authoritative corpus, then
clustered complete examples and a short final audit. Give each example stable
semantic headings and explicit scope so neighboring examples do not imply a
universal nonempty/approval/reply rule. Keep the known-good quoted source boundary;
test tutorial placement separately. Compare begin/end with a same-content grouped
layout and sparse section-local reminders. Repeat only compact discriminators,
not all complete programs at every boundary. Do not equate cache block size with
an optimal pedagogical chunk size or insert alignment padding.

All tutorial/index/checklist content is deterministic, before the volatile request.
Keep timestamps, receipts, run identifiers, diagnostics and dynamic provider facts
in sidecars or the existing volatile host packet. Recompute the rendered template,
shared token IDs and common prefix across tools, thinking and system-message
variants. Changing example order creates a new immutable generation and cache.

Aim initially for approximately 92,000–98,000 rendered prefix tokens. Verify the
actual request plus output capacity, not just reference text. Larger token headroom
does not prove GPU KV residency: measure available blocks, preemption, eviction and
native reuse with realistic tails. Preserve the 8192 batch-token setting during
accuracy comparisons; performance tuning is a separate experiment.

For production **base-plus-Sempai**, compile one source-aware composite and dedupe
only identical evidence identities/payloads under the existing composite contract.
The standalone target is not an allowance to concatenate another 100k base prefix.
Keep mandatory base content and declared coverage intact. If the complete composite
and required tail cannot fit 131072, report Capacity blocked or use an explicitly
reviewed partition/compatible provider through the upgrade plan. Do not install
hidden history/retrieval caps: product token budgets default to disabled, while
actual model input/output capacity remains a technical constraint.

## 6. Establish whether extra content causes better accuracy

Treat all fifteen previously inspected cases as development regressions; they are
no longer an untouched holdout. Freeze new tests before building examples and keep
their complete solutions outside the tutorial. Split by problem family, not only
renamed fields. Include long histories, different domains and conflicting but
correctly scoped example vocabulary. Model calls use the exact same host persona,
schema mode, count constraint, output allowance and serving configuration.

Run staged ablations rather than changing everything together:

1. Reuse v8a evidence and pin a fresh matched baseline for newly frozen cases.
2. Add only the highest-value complete/contrast examples, roughly 4k–8k tokens.
3. Add the remaining contract/prompt-review material, roughly 12k–22k total.
4. Change placement while keeping content constant. Test same-size content choices
   where feasible to distinguish topic quality from length. Never add noise padding.
5. After selecting content/layout on development cases, run the untouched holdout.

Use a small twelve-case pilot to reject regressions cheaply. Final evaluation has
sixty unseen cases: twelve each for Python contracts, Recipe composition, provider/
prompt repair, evidence/effect preservation and cross-example confusion. Include
metamorphic pairs within those families: alter bounds, empty-list rules, effect
capabilities or constructor mode and verify the artifact changes appropriately.
Use repeated generation trials with recorded seeds; record that seeds/temperature
do not guarantee identical GPU outputs. Keep the existing temperature-zero baseline
fixed for prefix ablations. Only afterwards compare supported sampling settings,
including the model card's coding preset, with paired trials. Do not mix a sampling
change into a claimed prefix gain or re-enable unlimited reasoning after the observed
output-budget loops.

Measure full first-pass acceptance, useful-draft rate, false blockers, exact
conversation preservation, semantic intent accuracy, summary/payload consistency,
and repair attempts separately. One valid JSON envelope or many passing branches
cannot outweigh one broken requested component. Report per-family results and
uncertainty, not only an aggregate percentage. Suggested promotion targets are
at least 90% first-pass artifact acceptance and no observed critical evidence/effect
regressions in the frozen suite, with consistent paired improvement over v8a. These
are acceptance targets, not promises or proof of zero failure outside the suite.
Reject additions that do not transfer; keep their source/evidence but exclude them
through an explicit reviewed selection revision. Do not tune on the final holdout
and continue calling it unseen.

## 7. Validation and bounded repair remain essential

The stable prefix teaches interpretation; schema decoding catches structure;
independent validators catch artifact behavior. For production, use the existing
review/interceptor and Q1 paths rather than a hidden self-approval loop. Where a
contract cannot be enforced today, record implementation work under phases C1/C2.
The live sink currently drops full-v3 Recipe fields; prompt improvements cannot
repair that transport limitation. Supported Skills/ToolSkills and association
authoring likewise require actual constructor/approval support.

A repair request receives the retained rejected artifact and precise failed
inputs/expected/actual values. It does not receive a fabricated success claim or
silently rewritten original conversation. Set an explicit finite repair allowance
in the chosen workflow; exhaustion retains a rejected draft and its evidence.
Draft repair performs no Tool effects and is distinct from dispatch retry: never
replay completed operations while fixing outputs. Human Q2 remains required.

## 8. First implementation sequence and deliverables

First, author and independently verify the paired data-return/reply examples,
helper-return and recursive-cardinality examples, plus complete reviewer envelopes.
These directly address observed failures and need only the compiler/source policy
and existing offline evaluator/checker paths. Add a small unseen transfer pilot.
Then expand prompt/provider coverage and compare layout/size. Only publish a new
candidate after deterministic compilation, provenance and capacity checks; activate
through the existing reversible prefix override only after its measured result
beats the baseline. Preserve the current working generation for rollback.

Deliver the reviewed example source and receipts, updated source policy/compiler,
immutable generation manifest, untouched test manifest, raw first-pass and repair
responses, per-family comparison, exact cache verification and remaining runtime
prerequisites. No service environment, Compose directory, model weights or context
setting change is part of this accuracy plan.

Research motivates experiments, not Ornith-specific claims: [Many-Shot In-Context
Learning](https://arxiv.org/abs/2404.11018) reports benefits from additional examples
on other models/tasks; [Lost in the Middle](https://arxiv.org/html/2307.03172v3)
shows position-sensitive use of long contexts. Neither proves this quantized model
will improve with every added example. Architecture is corroborated by the pinned
local config and [Qwen's 9B model card](https://huggingface.co/Qwen/Qwen3.5-9B).
The [Ornith model card](https://huggingface.co/cyankiwi/Ornith-1.5-9B-AWQ-INT4)
provides a coding sampling preset, which remains a separate test variable.

## 9. Implementation record

The compiler now requires the complete verified
[worked-example source](../../scripts/prefix/sempai-worked-examples.md), a matching
artifact/probe manifest and a Linux verification receipt. Twenty-four complete
examples address helper returns, recursive contracts, data/reply effects and
provider/prompt/effect review. The real Linux checks pass 91 program probes and
reject all six deliberate defect mutations. Required authoritative documents
remain complete under the reviewed selection policy; all twelve source originals
and both teaching sidecars are preserved by content hash.

Collection rejects absent, changed or unverified teaching evidence before updating
its source inventory. Compilation binds the source manifest and receipt into the
immutable generation identity and ships their portable evidence. Deployment checks
these fingerprints before changing the prefix override. The source-preservation
regression and real-file deployment tamper check pass.

The final candidate is generation
`182a5c5fc03ae3385d3d5923174d77cb517ce833990c863bb3d3d8b550a5dff8`:
96,921 reference tokens, 96,927 shared leading tokens and 97,189 tokens in the
largest template probe. The existing 31,072-token reserve and 8192 batch setting
remain. Exact server token IDs match three request variants. Cold prefill took
48.482 seconds; the next request took 1.195 seconds with 96,096 native cache-hit
tokens and zero external-cache hits. This measures native reuse, not inspected
recurrent-state contents or LMCache persistence.

The experiment tests the combined corpus/layout change in one temperature-zero
trial per frozen case. It does **not** isolate placement, length or individual
example effects, run repeated trials or establish a blind external holdout. The
same development agent authored the independently stored contracts and tutorial;
the cases were frozen before tutorial creation and behavioral oracles are excluded
from model requests. Separate staged ablations remain available if needed to
explain a result; they have not been completed merely by adding this corpus.

Full requests, responses, source fingerprints, Linux checks, deployment receipts
and the matched first-pass comparison belong in the
[implementation evidence](../../scripts/prefix/sempai-accuracy-20261006/README.md).
The baseline accepts 33/60 cases. V9 accepts 49/60 with no paired regressions;
v10 accepts 53/60 with three paired regressions. V10's narrow checkpoint improves
Recipe routing but does not remove effect/example confusion, scalar errors or
decoder repetition. Neither reaches 54/60, so neither is an accepted upgrade.
The previously active v8a generation is restored; the verified v9 teaching corpus
and compiler guards remain, while the rejected v10 checkpoint is archived outside
the active authoring source. Full control receipts confirm identical client requests
for all three runs. A compact exact long-history response fits 1539 tokens;
observed expansion/whitespace loops are rejected failures, not an unavoidable
context-capacity excuse. See the evidence's promotion decision and restoration
receipts for final state. No database component has been submitted or activated. Production Monty,
Q1/human Q2, exact associations, the lossless proposal sink and base-plus-Sempai
wiring retain the prerequisites in the v3 upgrade plan.

## Continuation: source density and protocol boundaries (2026-10-07)

Keep the large prefix objective. The updated curriculum retains all required
originals and 29 complete examples, checked on Linux with 132 behavioral probes
and six rejected mutations. Whole-mapping and parameterized helpers teach presence
and bounds without copying test fixture values. Order them near the volatile task
and include the whole concise contract checklist once. Avoid duplicate example
programs that consume capacity and compete with the current contract.

Serving acceptance precedes semantic scoring. V12's token injection passes but its
native-cache check fails and subsequent structured requests return server errors.
Preserve that evidence; do not call it a semantic model score or assert a cache
root cause. V13 keeps the same source coverage at 96,499 reference tokens and
passes exact injection/native reuse (48.101 seconds cold, 0.905 warm, 96,096 native
hits). Base unit/environment/weights/context/batching remain unchanged.

Separate source/layout improvement from host protocol improvement. Development
experiments may present the trusted current task as plain host text rather than
burying it in escaped packet JSON. Conversation evidence stays data. A typed host
schema may constrain real constructor fields and explicitly unchanged messages;
compact EBNF may constrain formatting outside strings. These are separate measured
controls, never hidden answer keys. Generated code and effects remain independently
validated. The existing live sink is still lossy for v3 Recipe fields; do not send
the offline constructor through it or claim production support.

For production, retain one reusable Tier-1 review workflow: obtain the typed
review packet and explicit edit permissions, compose with the immutable provider
reference, perform the LLM review, validate the whole candidate/evidence, and
return unapproved proposals to the supported queue. Each independent Tool call
needs its own bound usage/PythonCode step; actual catalogue UUIDs/associations,
typed-input support and lossless transport are prerequisites, not invented here.
Read-only echo can be host-owned deterministic data handling. Editable content
requires explicit allowed regions and byte-level preservation elsewhere, including
all original message metadata; test role/text pairs are not a production codec.

A validation-guided repair is an additional bounded attempt. Preserve the previous
response, failed artifact/checks and concrete witness; never replay an effect or
bypass live policy, Q1 or human Q2. Report first-pass and repaired acceptance
separately. Reaching a finite suite after repair is not flawless autonomous coding.
Fresh transfer cases remain independently stored and excluded from the prefix;
they are development-agent authored, not a blind external benchmark.

### Bounded construction and serving prerequisites

The next offline experiment uses two distinct LLM steps: derive a short unverified
branch/workflow plan from the current contract, then author one complete candidate
against that original contract. Exhaustive missing/null/type/bounds/result paths
belong in the plan; actual code, exact UUID sequence and effects still need their
independent checks. A valid plan shape does not establish semantic correctness.
Do not treat a model plan as dispatch instructions or an approval record.

One-description/one-variant decoding is an explicit offline authoring mode, not
a restriction on general v3 Recipes. Empty compatibility arrays and one proposal
are limited to the current one-draft test contract. Protected message content
comes from the current read-only host authorization, never an expected answer.
Authorized edits must remain editable and independently checked.

For hybrid-cache measurements, prime the exact immutable shared prefix with a
short request ending inside its last static block. Verify injected token hashes
and request-local native cache statistics before scoring. Use one namespace and
serial requests throughout; another namespace may evict native state. Preserve
external-restore errors separately. No global cache, service or batching change
is authorized by this test workflow. This is not external-state persistence
acceptance; that requires its own connector/runtime investigation.

If production adopts this review Recipe, the two independent LLM calls and the
validation usage each need their own supported binding/execution step, pinned
catalogue references and exact association evidence. Counterexamples may inform
a bounded authoring repair; no completed operational effect is replayed. Only a
whole validated candidate advances to the Q1/Q2 queue. The development evaluator
implements the experiment, not this production Recipe or its missing runtime
and lossless transport prerequisites.

## Cache correctness gate added by the operator on 2026-10-07

Pause further teaching-example changes until the serving path is isolated.
`cache_salt` namespaces both native and external entries; it cannot repair bad
page transfers. Compare identical messages and decoding controls across cold,
native reuse and verified external restoration. Do not classify corrupt output
or HTTP serving failures as a semantic model score. A native hit can retain a
previously corrupted external restore, so native-hit counts alone are insufficient.

The controlled short sentinel reproduced corruption with 97,152 extra external
tokens and zero native hits. Adding LMCache's required hybrid object separation
alone did not fix it. Source inspection identified the installed adapter's
rank-five-only attention re-paging gate: vLLM registers rank-four buffers with
32-token physical pages while scheduling 1,056-token logical blocks. Upstream
LMCache PR 4253 corrects that mapping. Its exact class and existing helper import
are backported; all other adapter AST nodes, original units, prefix content and
8,192-token batching remain unchanged. Existing services restart only to register
the new layout and discard old in-memory entries. This is the operator's explicit
cache-repair request, separate from the accuracy workflow's prohibition on hidden
service changes.

Acceptance requires correct plain-text and constrained-JSON sentinels on actual
external restores, supported by request-local hit counts and raw responses.
The byte-layout check uses actual imported adapter classes, real vLLM specs and
Torch CPU tensors; it cannot replace live restore acceptance. Retain source,
backup, configuration fingerprints and failed intermediate experiments under
`scripts/prefix/sempai-accuracy-20261007/`. The local dependency backport must be
checked when upgrading LMCache; a package version string alone cannot identify
this patched runtime. A passing sentinel does not establish full benchmark
accuracy, concurrent-load safety, restart persistence or complete model capability.

The post-backport plain and constrained-JSON sentinels both pass, including three
verified full external restores of 97,152 tokens. The plain returning request is
correct in 1.349 seconds and the JSON returning request in 1.380 seconds, with no
HTTP errors. `cache-fix-verification.json` pins the evidence and confirms identical
plain payloads except for namespace values across the experiments. The priming
helper now requires the expected READY content as well as native hits. Model
accuracy must still be measured separately; no new full-suite score follows from
these sentinel checks.
