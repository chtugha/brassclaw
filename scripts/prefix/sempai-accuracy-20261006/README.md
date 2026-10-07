# Sempai accuracy implementation and matched evidence — 2026-10-06/07

The new source contains 24 complete examples: eleven pure-Python programs, four
offline Recipe constructors (same words, different effects), and nine complete
review/blocked-output envelopes. Actual Linux checks pass 91 program probes and
reject six deliberate defect mutations. Recipe checks are offline structure/intent
checks; Tool execution, production Monty, Q1 and human Q2 are not established.

The compiler requires matching source/manifest/checker/validator verification
fingerprints before collecting. Raw tutorial sidecars are retained by content hash.
Complete required guides remain intact. Source-preservation and failed-collection
checks pass. The v9 candidate has 96,921 text tokens, 96,927 shared leading tokens,
and 151/279 selected complete units, under the existing explicit 100,000 rendered
target and 31,072 workspace reserve. No alignment padding or synthesis calls.

Tested v9 generation: `182a5c5fc03ae3385d3d5923174d77cb517ce833990c863bb3d3d8b550a5dff8`.
The earlier `5e343049…` compilation has identical rendered text/template/token IDs
but predates the portable evidence hardening; it was not used for the 60-case run.
Baseline generation: `1b7c3c117e57e5c0efb3355afcc7b40ff73a07251677af8235494908771881b3`.

`frozen-cases.json` was created before tutorial authoring. Its checksum is pinned.
It includes twelve cases per family: Python contracts, recursive/list contrasts,
Recipe data/reply effects, authorized/unauthorized prompt edits, and unknown or
completed effect evidence. Oracles are not compiler sources and are never included
in requests. This is a controlled transfer comparison, not a blind external audit:
the development agent authored both contracts and tutorial; names, bounds, fixture
identities and desired complete solutions are not copied into the tutorial.

All model requests use the same persona, output schema, message-count constraint,
temperature zero, disabled thinking, output allowance and batch setting 8192.
First-pass runs supply no failed response or repair feedback. Raw requests, responses
and independent checks will be retained. Final comparisons require complete matching
case manifests; repair runs cannot count as first-pass results. Metrics cover specified
behavior/structure and exact messages, not full free-text summary correctness or
universal reliability. Neither source review nor a passing candidate activates a
component. The current live proposal sink still loses required full-v3 Recipe fields.

## Baseline result

The complete matched v8a baseline accepts 33/60 cases under these independent
checks: Python 5/12, list/recursive contrasts 7/12, Recipe 0/12, prompt review
10/12, evidence preservation 11/12. All failures are retained. Some Recipe
responses omit the required prior_knowledge_content facts (null); others refuse
a supported export or return class22 instead. The longest repetitive-history
case exhausts 3500 output tokens and returns truncated JSON. It is a failure,
not an interrupted test or a passing conversation-preservation check.

## First expansion (v9)

`v9-comparison.json` records 49/60 accepted cases versus 33/60 baseline, with
sixteen improvements and no paired regressions. Python contracts pass 8/12,
recursive/list contrasts 12/12, Recipe exports 6/12, prompt review 12/12 and
evidence preservation 11/12. Python bodies pass 312/318 individual probes;
whole-artifact acceptance still rejects any failing body. The 49/60 result does
not meet the proposed 90% promotion target.

Recipe failures place new designs in proposed_recipe_updates or duplicate them
there, despite sometimes claiming that array is empty. The actual design/intent
data is otherwise promising; manual inspection of all twelve intent sets confirms
data/reply vocabulary and requested state values. Four Python drafts conflate
missing/null; one also copies a sample range endpoint. Long repetitive history
again exhausts the unchanged 3500-token output allowance and yields truncated
JSON. All raw failures remain in `sempai-accuracy-v9-candidate-20261006/`.

`long-history-output-capacity.json` distinguishes the repetition failure from
required output size. The exact compact envelope with an empty summary is 1539
tokens. The input contains 350 copies of its archived phrase; v9 emits 831 before
truncation. The required output can fit 3500 tokens. Raising that allowance alone
does not correct excess repetition or establish conversation preservation.

Server verification: exact injected token IDs across three request shapes; cold
48.482 seconds, warm 1.195 seconds, native cache-hit delta 96,096 tokens and
external hits zero. The base service SHA is unchanged. `compiled-v9/` contains
the complete portable bundle without secret-bearing host unit backups.

## Focused revision (v10)

`revision-v10/` preserves the added current-host proposal-destination and
presence/range discriminators, full source snapshots and compiled generation
`fd5a917fcb74564acd86a9ae2e29fcaa082107f7c0e0e1df99e98143d1c591b8`.
The reference has 97,779 tokens and retains all mandatory evidence. Only the
complete authoring-procedure source and its reviewed selection fingerprints
change; tutorial programs and their verified receipt remain unchanged.

This revision responds to observed v9 failures. Its same-case run is a matched
development regression comparison, not an unseen holdout. Requests still supply
no repair feedback, failed artifact, oracle or test solution. Repeated-trial,
independent holdout and isolated content/placement ablations remain unperformed.

The complete v10 checks accept 53/60: Python 9/12, recursive/list 11/12,
Recipe 11/12, prompt review 11/12 and evidence 11/12. It introduces three paired
regressions against v8a: python-07, contrast-09 and prompt-11. The last produces
an unfinished object followed by a whitespace run. Python-09 also exhausts
output. Recipe-02 copies Healthy reply intents into the data-only workflow;
the effect check rejects it despite correct container routing. Manual inspection
confirms the other eleven intent sets match their requested words and effects.
All raw responses and real Linux checks are preserved under
`sempai-accuracy-v10-candidate-20261007/`. Both expanded-prefix runs have exactly
the same sixty client request payloads as baseline; control receipts are retained.

Exact v10 server injection and native reuse pass: cold 49.123 seconds, warm
1.095 seconds, 97,152 native hit tokens and zero external hits. No service
environment, weights, Compose directory, batch setting or context setting changes.

## Selection and remaining work

Neither expansion reaches the 54/60 promotion threshold. V10 also regresses
previously passing cases. `promotion-decision.json` records rejection of both
as accepted upgrades and restoration of the previously active v8a generation
`1b7c3c117e57e5c0efb3355afcc7b40ff73a07251677af8235494908771881b3`.
The compiler keeps the verified v9 corpus/layout and evidence guards. The v10
checkpoint is removed from the active authoring source and retained in its full
archived source snapshot; no rejected instruction or raw response is discarded.
Compilation publishes a candidate bundle, not an accepted server deployment.

Restoration is verified in `restoration-receipt.json` and
`restored-prefix-verification.json`: healthy vLLM/LMCache, exact token IDs across
three request shapes, 76,032 native hit tokens, cold 35.213 seconds and warm
0.752 seconds. The base unit SHA remains
`0a61271c8e7901796101d2a2ca8abcb97bb06e8ffb18168bbbb89da826101192`.
The pinned tokenizer emits the same existing qwen3_5 model-type warning in these
runs; exact local/server token identity nevertheless passes. No Cargo checks were
needed for the Python/prompt changes.

The gain from 33 to 49/53 cases is real in this controlled suite, but it does not
establish flawless Sempai behavior or warrant overriding the promotion gate.
Future work needs targeted content/placement ablations, independent unseen trials
and validated bounded repair/decoding controls. Exact message preservation and
whole-artifact behavior must remain gates. Production lossless proposal transport,
Monty execution, association evidence and Q1/human Q2 remain separate prerequisites.
