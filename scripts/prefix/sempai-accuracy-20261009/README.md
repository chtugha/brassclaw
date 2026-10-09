# Sempai Monty/reference experiments — 2026-10-09

The compiler preserves complete originals, literal whole-unit evidence, current
binding guides and independently verified teaching artifacts. These experiments
are standalone model tests, not component activation or production Q1/Q2.

## Frozen generation v19

Generation `541e3610dd3885c2ab5e243fe07e2fd4aa9a8c2e3ee3a8ab7cb2fee5e6ae8998`
was fixed throughout the completed 60-case and 12-case runs. The model was Ornith
1.5 9B AWQ INT4, served by the existing vLLM/LMCache stack. Clients shared one cache
namespace; concurrent clients serialized at the server. No clean throughput or
latency comparison is claimed.

| Evidence | Result | Limits |
| --- | --- | --- |
| Frozen first pass | 58/60, previous protocol baseline 55/60 | Five improvements and two regressions; finite development suite |
| Actual upstream Monty draft probes | 24/24 candidates, 318/318 probes | Pure logic only; not custom BrassClaw host/loader acceptance |
| Transfer first pass | 10/12 | Two code failures despite valid JSON |
| Transfer after two bounded repairs | 11/12 | Separate repair evidence; remaining helper misses one required branch |
| Monty compatibility answers | 22/22 booleans correct | Reason/citation support checked separately |
| Citation topic support | 17/22 after manual audit | Original 16/22 includes one evaluator spelling false negative; five wrong passage choices remain |
| Verified teaching corpus | 132/132 probes in CPython and Monty, six deliberate defects rejected | Examples are teaching artifacts, not approved components |

The frozen Recipe failures omitted required review/sink prerequisites from the
artifact itself; a warning in composition_summary did not satisfy that check.
The transfer failures initially concerned a missing-only default and a helper with
no return. Feedback repaired the default. Actual AST diagnosis identified the
missing return for the second repair; it then passed 8/9 probes and still failed
its absent-key category. Keep these failures; no checks were weakened.

All first-pass requests exclude local oracles and failed-draft feedback. They use
an explicit two-call model-derived plan and strict decoding protocol; differences
from an original single-call client cannot be attributed solely to prefix content.

## Source refreshes

Concurrent component-guide edits were preserved. Each affected build used a fresh
isolated coherent source package and reviewed whole-unit policy, with affected
teaching reverified on brassclaw2. No integrity check was bypassed or expected digest
patched inside compiler code. Previous generations and failed attempts remain.

The latest v22 snapshot includes the effect-free preload/pinned invocation contract,
canonical execution Recipes, MCP discovery from approved mcp-call-skill-recipes,
ordinary-chat transport and the updated memory-budget floor. Complete upstream
Monty v1.0.0 documentation originals retain official revision and MIT license.
The model-visible selection retains every unit of the four binding guides,
reviewer procedure, teaching corpus, compatibility overlay and transport source.
One whole secondary architecture diagram was omitted as a duplicate; it remains
in the raw source store. Mandatory documents were not truncated.

Latest candidate generation:
`6bacc3a0259e54f581f547823943abc4a699459cff7ef31dcfba5b8d4d592804`

It has 113,903 text tokens and reserves 16,384 tokens against the real 131,072-token
model context. Automatic injection and native reuse passed in [the v22 priming receipt](v22-prime.json). The plain [external restore check](v22-external-cache.json) passed with zero native hits and 112,992 extra LMCache hits after displacement, followed by the correct READY answer. This qualifies the inspected in-memory MP path, not disk/restart persistence. The [structured external restore check](v22-external-cache-structured.json) also passed with zero native hits, 112,992 extra LMCache hits and correct READY JSON. The refreshed [v22 knowledge trial](v22-monty-current-reference-first-pass/reference-evaluation.json)
completed 27/28 JSON answers, with all 27 booleans correct. One seed-q2 response
truncated to invalid JSON and remains rejected. Original citation-topic support
was 18/28; [manual audit](v22-reference-manual-audit.json) found three equivalent
wording/newline matcher false negatives, giving 21/28. Six wrong passage choices
remain. Correct booleans do not establish supported explanations. A repeated
frozen build reproduced the exact generation and manifest in the
[reproducibility receipt](v22-reproducibility.json).
The v19 accuracy score does not automatically become a v22 accuracy score.

## Retained evidence

- [Frozen paired comparison](v14-v19-frozen-comparison.json)
- [Frozen upstream Monty probes](v19-frozen-monty.json)
- [Transfer bounded repairs](v19-fresh-bounded-acceptance.json)
- [Citation manual audit](v19-reference-manual-audit.json)
- [Monty language/runtime checks](monty-reference-runtime.json)
- [Latest teaching Monty receipt](v22-teaching-monty.json)
- [Local bundle integrity audit](local-bundle-integrity-audit.json)

The local v19 Markdown bundle was changed after publication by concurrent edits
and fails its recorded digest. It is retained as observed; its manifest was not
rewritten. Tests instead use verified originals from the model/test machines and
a separately verified immutable local copy outside the changing checkout.

These results support further supervised development with behavioral/citation
gates. They do not qualify flawless general performance or production promotion
without the missing runtime, complete-composite and no-regression acceptance.

## v23 display/navigation trial and v24 development

The v23 generation `a8f7820e08c2b1f5ab86d4b30bd29baa4a1a96fd3b42962fa5c155cb701cde56`
keeps the same v22 original source bytes, selection and teaching verification.
Only display/navigation/audit design changes. Strict JSON example compaction saves
5,633 tokens across 38 units; decoded values and embedded programs are preserved.
The source index costs 5,901 tokens and the audit 327. The resulting prefix has
114,358 text tokens, 114,626 maximum rendered probe tokens and 114,048 cacheable
tokens. Exact injection/native reuse and reproducibility passed.

First pass: frozen 57/60; transfer 11/12; reference 23/28. All reference outputs are
valid JSON and 27/28 booleans are correct. Both prior Recipe-prerequisite failures
are fixed, while two nested validators omit any success-state assignment and one
Recipe repeats an intent. The transfer helper now passes 9/9 probes; the missing-only
default still fails despite the original trusted contract. Its model-derived plan
incorrectly overrides that contract. Reference explanations and booleans can also
disagree; wrong passage selections remain failures. No repairs count as first pass.
The frozen actual upstream Monty trial passes 311/318 probes in 22/24 candidates;
the retained original input receipt binds its unchanged programs. Transfer Monty
passes 33/34 probes in 3/4 candidates. These are pure logic, not custom host/Q1/Q2.
The v19 comparison includes source refreshes, so differences are not solely design.

The v24 candidate adds general original-contract/plan checks, valid-path loop
tracing, final-value uniqueness and proposition/boolean consistency. Grouped
source navigation removes redundant document labels without removing evidence.
Generation `b78d7e396f137b104e0ef2cba5a18de360e235ca3381c411475e25c1a848aa01`
is in compiled-v24-final; earlier build prototypes remain separately retained.
Completed v24 first pass: frozen 59/60, transfer 10/12, reference 25/28.
Linux candidate receipts and upstream Monty receipts are retained. The frozen
Monty check passes 302/302 probes for 23 admitted programs; one requested
program fails the preceding pure-Python gate and is not counted as executed.
Transfer passes 33/34. Missing-only defaults, an omitted requested transfer Recipe,
and a contradictory MCP lifetime verdict remain failures. No production or
flawless-performance claim follows.

Hybrid-cache comparisons use compare_semantic_cache.py to hold complete requests
and decoder constraints fixed while checking cold, native and external restore
telemetry. They change no services and preserve all raw responses. Behavioral
correctness must be assessed separately; differing text alone is not corruption.

The paired MCP-lifetime probe proves a cold miss, 112,992 native hits and
112,992 externally restored tokens with zero native hits. All four task
responses give the same wrong boolean despite a correct explanation. See
[v24-lifetime-semantic-cache.json](v24-lifetime-semantic-cache.json) and its
[separate semantic audit](v24-lifetime-semantic-cache-audit.json). This
particular failure occurs cold too; it does not establish general cache health.

The missing-default writer also verifies cold/native/external paths (114,048
external tokens, zero native hits). All stages emit the same code SHA as the
retained Linux/Monty candidate: 6/7 probes pass and the absent-input default fails.
See [raw paired results](v24-default-semantic-cache.json) and the
[exact-code evidence reuse audit](v24-default-semantic-cache-audit.json). The
previously generated untrusted plan stays fixed; planner regeneration is outside
this probe. No service configuration was changed.

## v25 contract corrections (no inference trial)

The reviewer decoder now emits proposed_components before composition_summary,
with the summary last. Grammar manifest v2 includes object order in its fingerprint;
old reviewer layouts are rejected rather than accepted under the same sorted hash.
Replacement grammars are in v25-protocol/frozen-grammars, transfer-grammars and
plan-grammars. Archived grammars and results remain unchanged.

The compiler/header and authoring procedure distinguish host embedding reference,
preload definitions, invocation bodies, Recipe constructors and prompt review.
Module-level result requirements apply to invocation bodies. Verified historical
worked examples retain their exact programs and are explicitly scoped as such.
The planner selects exact current-contract quotes with aspect labels, rather than
inventing executable guard/result branches. Quote provenance and supplied UUIDs
are checked; classification/coverage/behavior are not proved by those checks.

Six protocol tests pass in the installed XGrammar 0.2.7 environment, including real
accept/reject matching for decoder order. Two compiler tests verify source/evidence
preservation. All 39 regenerated schema/grammar pairs pass exact-layout and artifact
hash checks. See [verification](v25-protocol/verification.json). No model requests,
service changes or prefix deployment occurred. These source changes require a fresh
coherent source capture and normal policy/integrity qualification before compiling
and deploying a successor; do not rewrite old expected hashes or score receipts.
