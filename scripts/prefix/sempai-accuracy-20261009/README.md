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
model context. Automatic injection and native reuse passed in [the v22 priming receipt](v22-prime.json). The plain [external restore check](v22-external-cache.json) passed with zero native hits and 112,992 extra LMCache hits after displacement, followed by the correct READY answer. This qualifies the inspected in-memory MP path, not disk/restart persistence. The [structured external restore check](v22-external-cache-structured.json) also passed with zero native hits, 112,992 extra LMCache hits and correct READY JSON. Current-source knowledge probes remain separate.
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
