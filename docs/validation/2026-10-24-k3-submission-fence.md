# K3 — Original-attempt fence at native model submission, 2026-10-24

The bounded successor passes **55 actually executed Rust cases**, including
three explicitly enabled real Ornith integrations, the ordinary global Monty
model caller and 32 architecture cases executed against the current tree with
their unchanged retained executables. Strict affected/root test lints,
default-composition lint, formatting and source/artifact integrity pass.
No ignored or zero-case run is counted. Evidence is retained in
`target/accepted-evidence/k3-submission-fence-20261024/run-1/`.
K3 and the conformity gate remain open.

## Observed boundary

The actual OpenAI Chat Completions HTTP transport now asks the native response
custodian to recheck its original attempt after provider preparation and directly
before submission. The existing PostgreSQL owner verifies the exact invocation,
admission key, run, original claim and dispatch-intent phase. Journal preparation
precedes the final snapshot lock, allowing cancellation to win while a journal
row is blocked. A task-local reservation is never reset for an automatic retry,
including an uncertain check COMMIT. This adds no Tool grant, provider registry,
workflow loop or durable-recovery claim.

The observer separately marks when transport submission can start. Only an
explicit denied barrier with no earlier start can produce the existing private
Rust confirmed-unsubmitted evidence. A provider error, timeout, missing answer
or later rejection after a start cannot establish that evidence. The read-only
getter exposes it to Rust callers without deserializing upstream claims or
granting retry permission. Responses received before cancellation retain their
existing independent wire/adapter/validated-answer custody and reconciliation.

The actual HTTP regression denies a prepared request without consuming another
server request, while retaining call-local complete non-UTF-8 error bytes and
cancelled persistence. The native PG/kernel test holds the real invocation row,
observes the transport preflight blocked through `pg_blocking_pids`, cancels the
real admitted run, then releases the row. It observes zero additional HTTP calls
or wire/adapter/answer rows and the explicit unsubmitted evidence. Its preceding
seven actual model responses, lost-answer unknown outcome, output-validation
failure and cancellation-after-receive cases remain intact. All three real model
cases pass in 58.47 seconds; the ordinary global caller passes separately in
35.62 seconds, retaining one global service and its controlled HTTP protocol
exchanges. That routing fixture is not Ornith inference evidence; the three
separately enabled native custody cases above use the actual Ornith service.

## Evidence and limits

- accepted.json: `4eb4efcfa2a52d22aaacf195e7529f2c9b795ae65629ad7962068e678f887b8a`.
- source.json: `f8596998fdb8842bf77f7576f7b583b1fcf28dd241ad509e327f7745da2472cc`.
- validated-submission-source.json: `d3f7d5bfb9550a49b8891c8125fd34c59ada87bfa159518cebd12a212a698693`.
- Actual composition executable: `1753267912d6565c8ed2a2074f0bdd3ce3c6ae57797cbde4b0dec0a8998c508e`.
- Actual LLM executable: `e38c22609d1d6e5de3f98395646ca07ce156024db385011e9ad341638becbc22`.

Ten changed compiler inputs are preserved as immutable evidence deltas. The
530 worker inputs remain unchanged and the actual worker is hash checked.
Cargo ran serially in screen with the inherited canonical target. Executables
were preserved before required targeted cleanup. The operator's keyless
`http://192.168.10.171:8000/v1` endpoint and
`cyankiwi/Ornith-1.5-9B-AWQ-INT4` were selected explicitly after checking the
reported 131072-token model limit. Nothing was compiled on the test machine.

This qualifies the existing OpenAI Chat Completions native wire path, not all
dedicated protocols/session authentication. The final check establishes a
submission boundary; cancellation after that boundary cannot prove that the
external service received nothing. Full durable recovery, task-wide model/prefix
selection across later steps/waits, embedding publication, role-selection CAS,
complete desired/effective/pending state and K4–K7 remain open. Existing Prefix
render/fit and serving gates remain with their named owner. No Rust fallback or
failed-Recipe replay is introduced.
