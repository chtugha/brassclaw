# Sempai reference capacity and quality audit

Audited 2026-10-06. **A role-specific, evidence-preserving reference can fit the
current token target while retaining the complete component-authoring contracts.**
This is a measured audit candidate, not a changed compiler policy, published
generation, activated component or deployed server template. Model quality
equivalence has not been measured.

Subsequent operator-authorized implementation/deployment added explicit
`SEMPAI_REFERENCE_POLICY` support. This audit's measurements describe the earlier
audit renderer/compiler identity; the deployable renderer adds navigation and
source metadata and has its own generation/measurements. See the
[deployment and model evaluation](sempai-evaluation-20261006/README.md).

At the audit revision the compiler deliberately required all 247 original source
units and reported Capacity blocked. The all-originals model-visible mode still
does so; the reviewed-policy mode was implemented subsequently. Do not describe
this audit preview as the subsequent published generation.

## Measured alternatives

Measurements use the pinned local Ornith tokenizer and its actual chat template,
without fetching model files or contacting an inference server. The source hashes,
compiler/tokenizer hashes, selected unit inventory and verification results are in
[the receipt](sempai-reference-reduction-verification.json) and
[the selection manifest](sempai-reference-selection.audit.json).

| Reference | Tokens | Interpretation |
| --- | ---: | --- |
| Current mandatory cards | 184,990 | Reproduced estimate including the current header, before final index/envelope; already cannot fit. |
| All original sources, compact metadata | 134,430 | Exact reference text; still exceeds 131,072 before request/output room. Formatting alone is insufficient. |
| Selected reference | 63,157 | Exact reference text, including source locators and safe literal quotations. |
| Selected reference, largest rendered probe | 63,425 | Actual complete tokenization through the compiler's existing injection wrapper. |

The configured preview prefix target is 100,000; the model context is 131,072;
the compiler reserves 31,072 for task/history/evidence/output. These are technical
capacity checks for this deployment, not new global BrassClaw token budgets.
The largest probe leaves **67,647 context tokens**, with **36,575 tokens below
the 100,000 prefix target**. Its shared leading prefix is **63,163 tokens**,
covering **59 complete 1,056-token blocks** (62,304 block-aligned tokens).
Unused room is available for useful approved catalogue context and the conversation;
padding or filling the budget with duplicate instructions supplies no additional
knowledge. The base-plus-Sempai composite needs its own exact capacity measurement.
A fitting standalone reviewer reference does not prove that the complete base
component library also fits.

## What must remain complete

Keep these documents verbatim, including negative requirements, current limitations,
schemas and worked examples. All six reconstruct byte-for-byte from the candidate
evidence excerpts; no LLM summarization replaces them.

| Document | Source tokens | Why retain it |
| --- | ---: | --- |
| `recipe.md` | 8,383 | Actual persisted IBS schema, workflow/variant/input conventions, task pinning and reuse. |
| `skills.md` | 14,409 | Prose plus code, recursive contracts, exact association approval, effect/retry contracts and authoring procedure. |
| `tools.md` | 8,246 | Primitive/implementation boundaries, ABI/artifact and live policy requirements. |
| `toolskills.md` | 8,741 | Binding descriptors, supported authoring contracts and runtime gaps. |
| `sempai-authoring-reference.md` | 2,609 | Provider-specific prompt diagnosis, evidence boundaries, conversation-derived proposals and validation/activation procedure. |
| `packet.rs` | 4,083 | Complete observed reviewer/proposal transport; not an insertion API or Tool catalogue. |

The four binding guides total **39,779 source tokens**. Reducing their examples,
recursive contracts, exception clauses or failure rules is unnecessary to solve
this capacity problem and risks teaching invalid components. Keep their internal
source order and explicit target-versus-implemented distinctions.

## What can leave the resident reference

Preserve all original documents in the immutable source package. Exclusion means
not resident in this reviewer reference, not deletion or replacement of evidence.
Do not assume an omitted source is retrievable at runtime until a real retrieval
adapter exists. Newly relevant work requires an explicit new selection/generation.

| Source | Retained complete source units | Excluded material and reason |
| --- | --- | --- |
| `AGENTS.md` | Global authorization/status, orchestrator-first rule, global Monty lifecycle/upgrade gap, tier hierarchy, Tier-1 triggers and Q1 authoring gates. | Duplicate binding definitions already preserved in the four guides; developer build/test/disk/environment instructions; older capability-lease guidance. |
| `CLAUDE.md` | Component catalogue and class-code section. | Broad subsystem/build/release tours, duplicate contracts and unrelated implementation limitations. Retained scope/filter descriptions are observed storage, not v3 authorization requirements. |
| `simplified_v3.md` | Target image/Recipe contract, complete Phase 0a prerequisites, sections 9 and 10 including DB-only provider implementation/acceptance. | Other migration/UI/performance/cutover inventories and repeated contracts. Global Monty requirements and upgrade gaps remain in the retained `AGENTS.md` unit. |
| Validation queue guide | Queue schema and state unit, including Gate-1-only state 2 and rejection counter. | Superseded validator descriptions, erroneous class/Skill advice, old placeholder/token-budget/bypass instructions and unrelated implementation inventories. |
| Legacy Sempai persona | None. | Role/output format belongs to the separately supplied current host persona/schema; the legacy example omits the preferred multi-class proposal field. |

The selection is **119 complete source units**, not token-driven snippets. The
manifest accounts for all 247 units and fingerprints each document and excerpt.
Production selection must bind the approved document hashes and unit identities;
line numbers alone are navigation, not a safe selector across changed sources.

## Knowledge-quality findings

1. **Older validation instructions conflict with binding Skill semantics.**
   [The per-class table](../../docs/agents-v3/14-validation-queue.md) calls Skills
   “pure narrative” (line 352). `skills.md` requires prose plus explicitly associated
   executable PythonCode. Root precedence reduces ambiguity but leaves contradictory
   advice in the full reference. Exclude that older table from resident content.
2. **The same table misidentifies Notes and carries legacy input syntax.**
   Lines 360 and 366 onward call class 15 Notes and teach `{{vars.NAME}}`/source
   baking. The current catalogue maps 15 to Summary and 20 to Note; the binding
   target passes typed data separately from Python source. Do not compress this
   table into a shorter authoritative summary.
3. **Soft token limits and a blanket seed bypass are unsuitable target advice.**
   Lines 354–363 retain class budgets; line 373 claims `source='system'` skips Q2.
   Disabled global token budgets do not authorize hidden class caps. The binding
   trusted-system-seed evidence contract is distinct from authored Q1/human Q2;
   assigning a source label cannot grant an authored draft approval.
4. **Observed Q1 is Recipe-driven, not the old standalone-validator description.**
   [`q1_orchestrator.rs`](../../crates/brassclaw_reborn_composition/src/q1_orchestrator.rs)
   looks up a class-specific validator Recipe and its executable PythonCode.
   `run_q1_validation` (line 409 onward) defers if either is unavailable. Passing
   citation/schema checks cannot establish actual Q1 completion or semantic
   correctness. Runtime readiness must supply installed validator/evidence status.
5. **Cache-invalidation statements need cache-specific interpretation.**
   The queue guide's planned retrieval memo-cache invalidation is not proof that
   all prefixes remain fresh or that no prefix invalidation exists.
   [`ValidationQueueStore::approve`](../../crates/brassclaw_reborn_composition/src/validation_queue.rs)
   marks the basic prompt store stale after commit when that store is attached
   (line 557 onward), best effort. This does not prove the target immutable
   generation/task-continuation contract or cache refresh succeeded.
6. **The legacy persona must not override the current host response schema.**
   [`sempai_audit.md`](../../crates/brassclaw_engine/prompts/sempai_audit.md) mandates
   an older JSON shape. [`SempaiReviewOutcome`](../../crates/brassclaw_interceptor/src/packet.rs)
   exposes `proposed_components` and labels `proposed_recipe_updates` compatibility
   data. Keep the current persona and response contract outside the factual corpus;
   retaining Rust transport types does not invent supported constructor payloads.
7. **The main size problem is broader scope plus repeated metadata.**
   Full repository administration/development and migration instructions are
   useful elsewhere but are not essential to prompt review and component authoring.
   Repeating document path, platform, version and long revision strings per unit
   adds substantial overhead. Store complete provenance in evidence JSON; render
   document hash/applicability once, with each card's stable ID and exact line range.
   Preserve citation identity and escaped chat delimiters. This is presentation
   compression, not a license to merge contradictory source instructions.

## Static coverage and remaining acceptance

The candidate passes the real compiler's exact-source card validator and all 13
existing topic gates. Compact citation IDs are unique. The actual compiler wrapper
was reused for chat rendering; 18 distinct tokenized request shapes cover thinking
modes, changing tools, missing/scalar/text-part system content and conversation
history. The complete reference is inside their common leading token sequence.
The receipt reports shared tokens and complete 1,056-token blocks. These are local
token/template results, not GPU KV/recurrent-state residency or LMCache reuse proof.

Coverage retains: prompt intent/role/effect preservation; provider authority and
capabilities; current constructor/UUID unknowns; reuse/variants/intent examples;
one component per step and binding-before-dispatch; exact associations/approval;
recursive schemas and missing/null/default semantics; immutable coherent task
selection; child/wait/retry state and live Tool policy; durable effect/no-replay
rules; Q1, behavioral validation, human Q2 and no self-activation; privacy and
untrusted source boundaries; distinct target gaps and real context limits.

Before describing quality as unchanged, evaluate the candidate on the actual
reviewer model with source-backed cases. Compare against full references on a
compatible larger-context deployment or targeted source-complete controls; the
oversized current prefix cannot be a valid 131,072-context A/B baseline.

Required cases include an already-correct prompt; role/tool-call corruption;
missing provider capability metadata; prompt injection/private data; bad Recipe
variants and multi-UUID steps; prose-only Skills; missing combination approval;
invalid recursive inputs/defaults; timeout after an effect; output failure after
completed dispatch; unavailable validator Recipes; human-Q2 routing; current
provider/policy changes; and unsupported constructor fields. Score intended outcome,
contract correctness, citation support and runnable proposals with actual validators
and behavioral evidence. A topic keyword match, fewer tokens or more proposals is
not a quality score.

## Implementation recommendation

Adopt an explicit, reviewed Sempai source-selection policy and compact renderer,
rather than truncating whatever exceeds the token target. Retain complete raw
sources, document/excerpt hashes, selected/omitted unit inventory and parent context
in the evidence sidecar. Bind the selection policy and renderer identity into the
generation fingerprint. Fail closed on source drift, absent mandatory contracts,
broken citations or capacity; preserve originals on every failure. Do not use a
model to summarize away binding rules.

At the audit revision `required_reference_cards`, `PREFIX_HEADER`, full-source usage prompt
and plan describe the original all-documents policy. They must be changed together
when implementing the reviewed policy, with real source-preservation, selection,
missing-contract/drift, citation and template-prefix checks. The current full-source
guard is correct for its declared policy; bypassing it is not this audit's solution.
Keep raw-package completeness separate from model-visible selected coverage.

Audit preview files are under `/private/tmp/sempai-reference-reduction-audit/`:
`candidate.md`, `candidate-cards.json`, `candidate-chat-template.jinja` and
`verification.json`. They are review artifacts and were not published through the
compiler or installed in vLLM. Durable hashes/dispositions are saved alongside
this report; temporary preview files are not installation assets.
