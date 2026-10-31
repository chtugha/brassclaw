# Final request transport prerequisite — 28 October 2026

**Scoped transport acceptance passed; the ordinary complete-request guard remains
unfinished.** This successor qualifies immutable wire custody, mandatory live
policy comparisons, the final OpenAI-compatible SDK hook and native effect
classification. Its measurement fixtures supply declared counts. They are not a
real tokenizer, accepted pair/profile design or semantic prefix qualification.

## Connected implementation

`brassclaw_llm::final_request_guard` accepts one already selected target and its
trusted native measurement owner. Preparation consumes the actual immutable
`reqwest::Request`, validates method/endpoint and binds the measurement to the
complete original body hash and provider/model identity. The checked object is
single-use and cannot yield an editable request. It reuses the existing
`ProviderDispatchGuard` and configured-count arithmetic after measurement and
again before handing the same request to HTTP. No optional token-budget setting
participates; no copied limit, settings endpoint, database table or provider pool
is introduced.

The existing registry HTTP builder uses the same transport policy implementation:
no automatic redirects or hidden HTTP retries. The SDK hook reuses that selected
client; a reqwest clone shares its existing client rather than constructing a
second provider client. Request bytes are not reconstructed after validation.

`FinalRequestGuard::complete` / `complete_with_tools` install invocation-local
custody through a scoped Tokio context. The supported immutable OpenAI Chat /
ChatMaxTokens adapter families reach `OptionalAuthHttp` after actual message and
Tool-schema conversion. Existing immutable retry/cache/circuit/recording wrappers
forward that support. Unknown, mutable/routed and otherwise unqualified adapters
reject before invocation. Guarded streaming/multipart paths also reject rather
than bypassing the hook. This is supported-transport gating, not a user setting
that disables model limits.

A scoped request bypasses the existing full-response cache so a cached DTO cannot
stand in for an exact prepared-body check. This does not disable model KV caching
or change vLLM/LMCache. A future response-cache optimization must validate the
same exact prepared request before reuse, not restore the old bypass.

The required native `ProviderWireObserver::submission_rejected` callback records
an actual pre-submission barrier. Both existing implementations are updated.
`FinalRequestRejected` remains a sanitized, nonretryable error and does not open
the circuit breaker. The gateway never infers absence of an effect from that
error category: only native rejection evidence with no prior submission-start
marker permits confirmed-unsubmitted classification. A previous started attempt
remains unknown/sent even if a later attempt is rejected.

## Observed checks

Sixteen selected Rust tests, zero ignored, run on actual current test images:

- Eight final-guard tests: four real HTTP/ownership cases and four actual SDK
  cases. They cover body byte equality, live tightening after preparation,
  invalid measurement custody, wrong methods, redirects, cancellation, actual
  strict-schema rewriting, cache bypass, nonretryable rejection, circuit state,
  unsupported-adapter entry refusal and native no-send markers.
- Three existing SDK transport/custody cases remain green.
- One existing configured-token policy case remains green.
- One gateway case distinguishes the same fabricated error category from the
  actual native rejection barrier; only the latter proves pre-entry rejection.
- Two native PostgreSQL canonical-limit cases remain green. The existing case
  now also prepares a final guarded request, removes its canonical model caps
  through the actual revisioned owner and proves local rejection before HTTP.
- One actual global Monty Match/No-Match regression remains green, retaining the
  ordinary single-service lifecycle.

The declared measurement fixtures establish transport and policy behavior, not
numerical accuracy on Ornith or another model. The loopback cache-entry response
is a protocol fixture, not model inference or answer-quality evidence.

Strict connected LLM/Reborn/Composition lib/tests Clippy passes with warnings
denied, as does the standard-feature lib/tests check. Format and whitespace pass.
Actual connected test images are built in 247.44 seconds; strict connected checks
pass in 104.33 seconds. The ordinary global case executes in 87.04 seconds.
Relevant source inventory, all executable hashes and exact positive test counts
are independently checked after completion. No deployment or server changes occur.

## Exact evidence

Receipt: `target/accepted-evidence/final-request-sdk-20261028/run-1/accepted.json`

- Receipt SHA-256: `defb45f43427fa578d1815d633ef17c2bff54d8942a59b54d53b2bb3aaefe016`.
- Seventeen-path successor manifest: `6b2c4087f975da354c7667fcacac5ede7e732abe9994f87fa619c77663d5e1d3`.
- Complete source inventory: `28ec6b982d5950bd6dae4c007408e0fe606d9b738f3cb55236a0f79ef45733f7`.
- LLM test image: `b9d9c96af225821d31db2a01245b58a870db6626a7a4f8030fe75db249483095`.
- Reborn test image: `6805f77cc3c6398f9d6432bc8272f16c826e9ac673496486493a77a81c184453`.
- Composition test image: `5ed3f1db9aaf605d75b3ccd293617a334b9e1f4cc1bc939c0a42636edeb8556f`.
- Actual worker: `a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.

The earlier isolated transport prerequisite ran eight actual tests in
`target/accepted-evidence/final-request-transport-20261028/run-2/`; its first failed
run (a missing Debug bound on the test policy fixture) is preserved. Its original
three-path manifest and images remain historical. The SDK successor does not
rewrite that receipt or the earlier twelve-path budget-selection evidence.
The other owner's current 26-path Auth package is a verified build input here;
this does not broaden this Prefix receipt to that workflow.

## Remaining implementation before ordinary attachment

1. Resolve the accepted pair design and profile/generation through their existing
   owners and task snapshot; retain exact tokenizer/template/artifact/adapter and
   prefix identity. Do not pass unaccepted prose/proposals to a renderer as code.
2. Implement the concrete contained measurement adapter over the existing
   `ModelEncodingBackend`: map the actual final SDK body through the qualified
   provider/default contract, encode the full input once, verify unchanged leading
   prefix bytes and semantic partition, and derive both counts from that encoding.
   Effective output comes from the actual SDK payload, not an earlier DTO.
3. Authenticate the observation's target, original artifacts/template, worker,
   render context, full-body identity and generation before producing
   `FinalRequestMeasurement`. Unsupported multimodal expansion, missing output,
   unknown provider preprocessing and stale evidence block; never estimate.
4. Attach the scoped guard at both ordinary Kohai and Sempai calls and every Tool
   continuation, after the prescribed formatting/review/prefix join and actual
   MCP schema preparation. Preserve the same pair design for a shared pair and
   role-specific generated content. Keep the old tightening fence until its
   replacement is qualified. Missing compatible context must not silently use
   an unguarded provider path.
5. Qualify real tokenizer/provider parity, both targets, overflow with optional
   budgets disabled, stale design/artifact/namespace, concurrent isolated calls,
   lost/cancelled preparation and preservation of the original effect account.
   This report does not close those remaining acceptance gates.

An API called `/tokenize` is not automatically a full-request oracle. Current
upstream [vLLM tokenization protocol](https://github.com/vllm-project/vllm/blob/main/vllm/entrypoints/serve/tokenize/protocol.py)
and [chat-completion protocol](https://github.com/vllm-project/vllm/blob/main/vllm/entrypoints/openai/chat_completion/protocol.py)
have different rendering-input surfaces, including Tool-choice handling. These
are research references, not evidence of the installed server version. Pin the
actual provider implementation and qualify the relevant branches before relying
on such an endpoint; no provider configuration change is authorized by this work.
