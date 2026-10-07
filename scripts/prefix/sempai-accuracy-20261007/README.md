# Further Sempai accuracy experiments — 2026-10-07

This is a continuation of the frozen 60-case development suite. Validators and
behavioral oracles remain unchanged. First-pass, serving errors, decoding controls
and feedback repairs are distinct results; none is production Q1/Monty/human Q2.

Twelve additional transfer cases in `fresh-cases.json` were frozen before the new
teaching examples were written. They change fields, bounds, effects, language and
history. The development agent authored them, so they are not a blind external test.
Only instructions/messages/modes go to the model; oracles stay local.

The teaching source grows from 24 to 28 whole examples. Linux verifies 121 behavior
probes and rejects the same six deliberate defects. New examples show whole-mapping
helpers, parameterized bounds, recursive exact booleans and duplicate packet views.
Original examples and all required guides remain. Verification receipts and source
fingerprints are generation-pinned; old snapshots and failures remain preserved.

V11 (`783660eccd6b11370c56018c4811965b66696c1eb95ec5dae61fceb8519f238c`)
contains 96,873 reference tokens. Its seven-case pilot accepts three whole outcomes:
python-09, contrast-09 and prompt-11. Missing/null, complete result objects, new
Recipe placement and repetitive-history preservation still fail. Valid JSON alone
is not acceptance. The partial checker receipt is retained separately from the
completed pilot receipt. Exact server injection and native cache reuse pass: cold
48.251 seconds, warm 1.190 seconds and 96,096 native hit tokens; external hits zero.

V12 (`408ebe3f81c5812b4646ce18b5c7b5f291910419743a04268c8e710af6d7c1d7`)
adds a concise source-derived contract checklist outside the quoted corpus. It
contains 97,134 reference tokens. Exact token injection passes, but the warm request
reports external hits (96,096) and zero native hits; native verification fails.
All 60 subsequent requests return HTTP 500 with grammar-token rejection. This is
an unusable serving experiment, not a semantic accuracy score. Cache observations
do not establish causation. `v12-serving-failure.log` and the failed run remain.
The evaluator now retains HTTP error bodies and aborts after three consecutive
server errors, preventing additional futile accuracy requests.

V13 removes the duplicate checklist card while retaining its complete source once
at the terminal boundary and all mandatory evidence. It contains 96,499 reference
tokens. The original host unit, environment, weights, context and batch settings
remain untouched. It is a candidate pending serving and accuracy acceptance.

Optional decoder controls are separate experiments. Compact EBNF comes from the
same trusted JSON schema via the installed XGrammar 0.2.7, with fingerprints. A
stronger offline schema specifies supported constructor field names/types and empty
compatibility arrays. An explicitly read-only review may constrain the original
input message array verbatim. Explicitly authorized edit cases remain unconstrained
by expected text; generated code, intents, UUIDs and behavior are never supplied as
answers by these grammars. These controls have not been wired into production.

## Completed v13 baseline and protocol pilots

V13's unchanged-client complete run accepts **54/60** (Python 8/12, recursive/list
12/12, Recipes 11/12, prompt review 12/12, evidence 11/12). Every one of the sixty
request payloads equals v8a's baseline payload; paired regressions against that
33/60 baseline are zero. `v13-comparison.json` and request-control proof preserve
that distinction. The six rejected cases remain: python-08/09/10/11, recipe-06 and
evidence-11. The Recipe repeats the correct UUID in an extra execution step; the
UUID string and output array are correct. Manual review confirms all twelve intent
sets match the supplied state words and data/reply effects. Summary/prose semantics
are not fully certified by the automated acceptance gate.

The first protected/direct-contract pilot accepts two of those six cases. The
repetitive archive is preserved exactly in 1,608 output tokens; the Recipe sequence
is fixed. Four code requests produce extra drafts, all rejected. Adding an explicit
at-most-one proposal constraint for this one-draft offline contract accepts five
of six without feedback. Python-08 still omits its missing branch. These partial
pilots are **not** 60-case scores and must not be merged into the baseline.

## V14 candidate

The 29th whole example demonstrates presence-first output initialization with
different fields/labels/bounds, retaining all earlier examples. The complete
missing-output object is initialized before a key-presence guard; the guard sets
null/invalid and then exact valid outcomes. This is output construction, not an
input default or coercion. Actual Linux verification passes 132 probes and rejects
six deliberate defects. No frozen fixture solution, name, UUID or range enters the
curriculum. All eleven other source documents match v13 byte-for-byte.

Generation `3c6b8ac31b98e5b0c418befd071efb0fd5ded723613aa6f70aec8cfcae2dea1f`
contains 97,822 reference tokens and 92 complete shared blocks. Source/compiler,
portable bundle and evidence are preserved under `revision-v14/`. Full-suite and
fresh-case measurements are recorded below. Protocol options remain per-request test
controls, not a production cutover or a claim of prefix-only gains.

## Rejected construction-plan and cache experiments

`v14-two-stage-first-pass` completed the frozen suite with **39/60**, including
nine HTTP 500 failures. Remaining failures include duplicate Recipe descriptions.
It is rejected. `v14-typed-salted-pilot` adds structural one-description/one-variant
decoding and a fresh request namespace, but aborts after three server errors;
namespace separation alone does not establish correct hybrid state restoration.

`v14-primed-pilot` first warms a short prompt ending in the shared prefix's last
complete block. It then passes all independent artifact checks for python-08,
python-09 and recipe-06. However, a second namespace was warmed concurrently with
its long-history evidence test; that test returns HTTP 500. The second namespace's
warm request has zero native hits and 97,152 extra LMCache hits and produces an
incorrect one-token response. This experiment is confounded by competing cache
residency and cannot establish either full accuracy or connector causality.

`prime_sempai_prefix.py` verifies actual injected token fingerprints and that each
short warm-up stays within the static checkpoint block. It requires request-local
native hit statistics, not aggregate counters polluted by concurrent clients.
It changes no settings and primes no Tool execution. Subsequent accuracy tests
must run serially in that same namespace. These are cache-residency experiments,
not a claim that external recurrent-state persistence is fixed.

## Completed serial first pass

`v14-serial-first-pass` completes all sixty cases with **55/60**, zero HTTP errors,
and 60/60 conversation checks. Python 11/12, recursive/list 9/12, Recipes 11/12,
prompt review 12/12 and evidence 12/12. Every planner/writer request reports native
reuse and zero extra external tokens. This combined two-call/schema/presentation
experiment is distinct from v13's unchanged-client 54/60; it has four regressions
and five gains against that run. Do not claim prefix-only or monotonic gains.

The rejected cases are python-10 (null merged with missing), contrast-01/02/07
(wrong invalid-container result) and recipe-05 (missing lossless-sink prerequisite).
All original drafts, partial/final checks and bounded feedback remain retained.
The evaluator automatically captures its loaded source and pins repair parent
receipts, failed response bytes and feedback. The reporter verifies that chain
and records first-pass versus bounded-repair acceptance separately.

`v14-fresh-first-pass` is **excluded**: a refactoring bug referenced the moved
schema variable after planning, so no final drafts were requested. Its source was
reconstructed and matched to its recorded fingerprint, and all failed receipts
remain. The corrected actual planning execution path passes using a retained real
provider response. Client/protocol execution failures now abort immediately.
The corrected transfer rerun below supplied no failed plans or test answers.

`v14-repair-round1` repairs only the five rejected cases from actual bounded
feedback. Four pass, yielding **59/60 after one repair round**, still **55/60 first
pass**. recipe-05 repeats its sink warning in composition_summary but omits it from
the portable Recipe's prior_knowledge_content. The validator rejects it again.
Feedback generation now names that artifact field explicitly rather than merely
the failed gate; the second bounded round must modify the actual payload.

The first repair's nested planner provenance label erroneously says “no feedback.”
Its outer repair_from and pinned feedback/parent evidence correctly identify the
run as a repair. The raw receipt remains; this note corrects the label, and future
planner receipts distinguish feedback repairs from first-pass analysis. Neither
repair responses nor failed client runs are merged into first-pass accuracy.

## Completed bounded repairs and transfer checks

`v14-repair-round2` fixes the remaining Recipe payload warning, producing **60/60
accepted after at most two repair rounds**, still **55/60 first pass**. The chained
report `v14-bounded-acceptance-final.json` verifies the original raw drafts,
validator feedback, parent fingerprints and unchanged case contract. This is an
offline authoring workflow; production Q1/Q2, Monty execution and the lossless
transport prerequisite have not thereby been implemented.

`v14-fresh-retest-first-pass` scores **10/12** on the frozen additional cases;
`v14-fresh-repair-round1` reaches **11/12** and round two remains **11/12**.
`v14-fresh-bounded-acceptance-final.json` retains the separate first-pass and repair
counts. These development-agent-authored transfer cases are not a blind external
benchmark. The remaining recursive draft defines its helper without invoking it
or assigning a module-level result; its summary claiming execution is insufficient.
The validator rejects it. A third development repair was prepared but **not run**:
the operator prioritized cache correctness instead. No fixture-specific feedback
was inserted into the immutable reusable prefix.

## Cache investigation: preserve the model/prefix during the comparison

`cache-path-before.json` holds five serial requests with identical messages,
temperature, output limit and prefix generation. Only the displacement request
uses a second cache namespace. Cold and native requests answer `READY.`; after
displacement, the returning request reports **0 native / 97,152 extra external
tokens** and answers `eggerductductductductductductduct`. The following native hit
repeats that corruption. There is no structured-output grammar in this experiment.
This establishes a serving-path error independent of constrained decoding and
does not invalidate the separate semantic rejections measured with healthy native
reuse.

Installed LMCache **0.5.5** and vLLM **0.31.0** support the hybrid path. However,
the running LMCache command omitted `--separate-object-groups`; its installed CLI
defaults that option to false. The installed connector documents the option for
large align-mode prefill steps. Its `all_null_chunk_masks` rejects missing state
only when every layer in an object group has null blocks. Its default grouping
mixes full-attention and recurrent layers, while separation groups recurrent
pages independently. [Official hybrid documentation](https://docs.lmcache.ai/mp/hybrid_models.html)
requires separation for Mamba/GDN hybrids and explains the checkpoint geometry.

`enable_lmcache_hybrid_groups.py` added only the supported LMCache override
`/etc/systemd/system/lmcache.service.d/91-hybrid-object-groups.conf` and restarted
the existing LMCache and vLLM services to register the new layout and discard old
in-memory objects. `cache-hybrid-fix-receipt.json` records hashes: original vLLM
and LMCache units and the existing prefix override are unchanged. The LMCache
base-unit backup is on the server in `/root/lmcache-hybrid-fix-20261007T081452Z/`.
No package, virtual environment, model, Compose directory or prefix content was
replaced. `cache-hybrid-registration.log` records the new engine registration.

Post-fix external-restore checks are recorded separately. A correct native hit
alone cannot certify external restoration. The diagnostic requires a returning
request with zero native hits and at least 97,152 extra external tokens before
its external_restore_check_passed field can be true. The companion structured
test uses a simple fixed JSON grammar to detect serving errors; it is a sentinel,
not an accuracy benchmark. A fresh salt isolates entries but cannot repair a
broken restore path. Preserve one stable salt for a compatible generation rather
than creating one per request.

`cache-path-after.json` records the first attempted fix: object-group separation
alone still restores 97,152 external tokens and answers `agliductduct…`.
external_restore_check_passed is **false**. This rejected step is retained.

The installed adapter has a second, independent bug: `_SubpagedAttentionViewEdit`
matches only rank-five tensors. The observed attention pool is rank four with
32-token physical pages and 1,056-token scheduler blocks (33 physical pages per
logical block). It is incorrectly reported as compressed, and transferred at the
wrong page granularity. [Upstream PR 4253](https://github.com/LMCache/LMCache/pull/4253)
fixes this exact mapping. The backport uses that PR's class from merge commit
`1847435fa401330a5cf7caa6e86aaf873b2a043b` plus its existing contiguity-helper import.
Every other module AST node is unchanged; original, pinned upstream and backported
sources and their diff are preserved in `lmcache-4253/`.

`check_lmcache_4253_layout.py` imports the actual adapter classes and real vLLM
FullAttentionSpec with Torch CPU tensors. The old class rejects both rank-four
layouts; the patched class covers every byte of each 1,056-token page for
contiguous and permuted rank-four buffers and preserves rank-five behavior.
Invalid page counts and non-contiguous page byte layouts are rejected. This
checks mapping, not live transfer. `source-and-layout-review.json` records it.

The guarded `apply_lmcache_4253_backport.py` replaces only that reviewed adapter
file and restarts existing services to discard old in-memory objects. No package
upgrade or vLLM argument change is involved. The exact original adapter is backed
up on brassclaw under `/root/lmcache-4253-backport-20261007T082404Z/`; application
receipt and unit/source fingerprints are retained in `lmcache-4253/`.

### Live post-backport acceptance

`cache-path-backport.json` passes all five plain-text requests. Its returning
request has **0 native / 97,152 extra external tokens**, answers `READY.` and
takes **1.349 seconds**. `cache-path-backport-structured.json` passes all five
constrained-JSON requests with `{"status":"READY"}` and no HTTP errors. Both the
displacement and returning structured requests actually restore 97,152 tokens
externally; the returning request takes **1.380 seconds**. Both receipts report
external_restore_check_passed=true. No prefix/compiler, temperature, output limit
or batching change was used to correct the plain-text failure.

The server now reports all eight attention layers receiving the subpaged edit,
and every attention group has 1,056 physical slots per logical page instead of
32. `lmcache-4253/live-registration.log` retains that runtime evidence. Existing
services are healthy, with original base-unit and prefix-override hashes intact.
The priming helper additionally requires the expected READY answer, preventing
a corrupted native state from being certified solely by its hit counts.

This fixes the reproduced transfer failure, benefiting every compatible request
through this serving path. It is not a new sixty-case semantic benchmark or proof
of arbitrary concurrency, disk/restart persistence or flawless Sempai capability.
Previous native-only semantic scores remain separately identified above. Installed
LMCache still identifies as 0.5.5 **with this local upstream backport**; upgrades
can overwrite it, so verify the source fingerprint and rerun restore checks.

Rollback, if needed, uses the exact adapter backup on brassclaw: stop vLLM then
LMCache; copy `kv_cache_group_edits.original.py` from the recorded backup onto its
recorded target; move only the added `91-hybrid-object-groups.conf` into that
backup directory; reload systemd; start LMCache then vLLM. Do not remove the
existing `90-reference-prefix.conf` or replace the base units. Rolling back also
reintroduces the reproduced corruption; it is recovery guidance, not acceptance.
