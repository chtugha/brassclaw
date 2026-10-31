# Canonical dispatch policy and exact token arithmetic — 26 October 2026

Status: bounded prerequisite passed; automatic final input-limit enforcement
at provider submission remains open.

`PgProviderDispatchGuard` already reads the live provider definition and revision
through the canonical administration owner. Its existing `ProviderDispatchState`
now includes the exact canonical provider/model, that model's configured
prefix/prompt ceilings, the applicable default-model context and the original
requested output limit. These values come from the same read as cache policy and
revision. No additional database query, table, settings field, endpoint, provider
reader, Tool or workflow has been added. These are transient observations, not a
second mutable settings owner or a grant.

Only the context declared for the exact default model is returned. Another
model's cap entry never inherits the default model's context. Absent model limits,
context or positive output reservation stay explicit errors for the comparison;
there is no guessed default, sum-derived context or model-name heuristic.

`ModelTokenLimits.check_measured_request` checks counters from one complete
encoding: prefix plus prompt must equal the whole input; arithmetic must not
overflow; prefix and prompt each stay below their own ceiling; actual full input
plus explicit output stays below context. Only observed consumption is charged,
not unused portions of the ceilings. A disabled prefix requires zero measured
prefix tokens. Counts are not rounded, independently retokenized, cut or repaired.
The comparison has no discretionary-budget switch.

The captured dispatch-state comparison additionally matches exact provider/model
and uses that state's output facts and declared limits. Original observations
remain immutable after a later revision. Existing rejection of incompatible
transport/client changes and tightened ceilings is retained unchanged.

## Executed checks

Unit tests exercise exact caps/context, isolated one-token-over prefix/prompt/
context/output cases, disabled prefix, unused headroom, inconsistent totals,
overflow, invalid limits and absent context/output. State checks reject another
provider/model, missing limits and invalid revisions.

The actual PostgreSQL test captures Kohai and Sempai using the same model name
under different provider policies, verifies their exact revisions and counts,
then tests another model without borrowing default-model context, missing model
policy, a compatible limit increase and metadata successor, role switching
without retargeting the old client, and a real tightened revision. The old guard
still rejects tightening; a fresh selection exposes the new value and its exact
comparison fails against the old larger count. The existing canonical settings/
CAS/preservation test also passes.

Actual live cache/request-policy consumers and ordinary global Match/No-Match/
in-flight model selection execute on current builds. Those regressions exercise
the existing client/cache/submission paths; they do not show that this new count
comparison is automatically connected to the final request.

R1 remains explicitly failed: the new native test used a master-key fixture
shorter than the existing SecretsCrypto minimum. Its library/unit results and
raw native failure are retained under run-1, without acceptance. Only that test
literal changed; all four production files remained unchanged. R2 rebuilt/reran
the complete scoped suite, including the existing canonical settings case.

## Remaining integration

Success of this arithmetic authenticates no supplied counters, accepted prefix,
render-to-wire mapping or deployment geometry. Configured context and requested
output are declared facts, not verified deployed capacity/effective wire options.
Provider adapters may transform/strip parameters; the final guard must bind the
one actually prepared request, effective output reservation, qualified encoder,
accepted prefix and real deployment evidence, then recheck live policy before
cache return/submission. Missing output after stripping or unknown server defaults
cannot be filled with an invented reserve. No stale observation authorizes send.

Existing `ProviderRequestFacts` still carries no actual input envelope. This
package adds the canonical policy/count foundation; it does not silently turn
legacy serving into guarded serving, change token-budget settings, deploy an
application or modify vLLM/Compose/venv/systemd/cache configuration. No general
pure-approval exception or alternate observer/transport was introduced.

## Exact evidence

Receipt: `target/accepted-evidence/prefix-dispatch-policy-20261026/run-2/accepted.json`;
SHA-256 `aac11bdc8f0b44de5f6a4ab927e81c17f484416a41fdf47b9678c1a1805db0f8`.

All 11 executed Rust cases pass:

- `exact-count-policy-unit`: 4 passed.
- `captured-policy-state-unit`: 1 passed.
- `native-canonical-model-limits`: 2 passed.
- `native-live-cache-consumer`: 1 passed.
- `native-request-policy-consumer`: 1 passed.
- `native-global-match-no-match`: 1 passed.
- `native-inflight-model-retention`: 1 passed.

`strict-llm-composition-tests`: passed in 106.15 seconds.
`strict-default-composition`: passed in 36.04 seconds.
`format`: passed in 4.81 seconds.
Whitespace/protected-source integrity pass. The five changed files, full
source inventory, actual executable artifacts and current-worker Cargo-build
evidence, commands, logs and runner are retained beside the receipt. No incomplete
historical 530-input reuse justification is used. Worker SHA-256 `a617985957ccb8c5f3f0263f3ffe9135d9e352a86b8235d4bf5e39eba7e431a8`.
