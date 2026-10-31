# Atomic catalogue publication and prefix invalidation — 2026-10-24

**Scoped acceptance passed:** 39 fresh Rust tests, nine exactly qualified reused
Rust results and 21 current UI tests pass. Strict affected/default lints, format,
whitespace and final source/artifact integrity pass. This record covers
the existing publication transition, not generated-prefix serving, general Q1,
arbitrary recovery, a regular application release or installed/platform acceptance.

## Connected implementation

The existing catalogue owner commits its immutable activation, approved catalogue,
active pointer and exact prefix invalidation in one transaction. V177 adds the
invalidation to the original activation record and a composite pointer reference.
It preserves historical records and unknown provenance as NULL. New inserts must
retain the actual root/document references, catalogue/inline checksums and ordered
registered profile revisions. Recording an unsupported profile does not enable its
compiler. Capacity uses the existing adjustable `max_catalogue_bytes`, including
PostgreSQL's actual JSONB text size; there is no new publication quota or queue.

Lost COMMIT acknowledgement is resolved by the original immutable activation.
Reference-only reconciliation cannot author a new request or republish a superseded
generation. Runtime publication is protected by the committed pointer's read lock;
fresh ordinary task capture and source export reconcile the durable target before
using it. Publication/discovery acquire their mutex before a pool lease and release
leases before nested restoration. An observation-only status request does not wait
behind a stalled activation/discovery operation.

The existing status reports desired, runtime and discovery generations separately.
The operator can reconcile `/api/webchat/v2/catalogue/{activation_id}/reconcile`
without resending component content. The route uses the same authenticated operator,
origin and rate policies. Both existing editors retain uncertain exact requests;
inspection/reference reconciliation never rebases them. Failure of a secondary
status read does not turn a confirmed write into an unknown write.

Qualified retained inline context is identified separately from generated-prefix
rebuilding. Failed builds and pending generated serving do not pause independent
deterministic work. Existing tasks keep their original catalogue, exports and
native bindings under live policy; no rematch, effect replay or Tier-2 fallback is
introduced. This does not activate an unqualified generated prefix.

## Actual failed attempts and repairs

- The first disk preflight stopped before compilation because available capacity
  was below the existing floor. Its original output is preserved separately.
- Run-1 compiled without warnings with Rust 1.96.0, then stopped on actual parallel
  Prefix-source drift before tests. The Prefix changes were preserved. The Prefix
  chat subsequently confirmed a stable source window and released its Cargo lock.
- Run-2 passed the build, 32 architecture checks and actual historical-row upgrade.
  Terminating one real PostgreSQL backend then exposed a shared test-relay bug:
  one broken socket terminated the listener and unrelated sessions. Test support
  now contains known connection-level disconnects and records their safe error
  class; malformed protocol and failed relay tasks still fail the transport.
  Application/database results remain actual, never manufactured.
- Run-3 passed the repaired precommit termination, lost COMMIT/readback, source
  capture and failed-build/independent-work assertions. Its actual No-Match request
  contained the new system context. A later new test query incorrectly expected
  Recipe-Tool response custody for this ordinary model port. The successor checks
  its actual settled admission, immutable catalogue/prefix checksums and one
  No-Match exchange. It adds no production record to satisfy the assertion.
- The reported `collapsible_if` warning is fixed with an explicit boolean that
  ends the read guard before short-circuited restoration; no lint is suppressed.

The original failed snapshots, executables, commands and logs remain under
`target/accepted-evidence/k2-catalogue-publication-20261024/`.

## Current verification

Run-4 uses the common root, configured canonical Cargo target, serial lock and
background screen; Rust 1.96.0, incremental/dev/test debug output disabled. Its
composition/PG test build passes with the existing `skills-db`, test-support,
legacy migration, Bedrock and LLM testing feature selection. No second target,
mutable build projection or testmachine compilation is used.

The actual adjacent worker remains SHA-256
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.
Its entry, hosting/API/vendor sources and workspace inputs were compared against
the prior accepted source: 530 unchanged inputs. The exact executable is checked
separately. Unchanged architecture executable inputs/hashes are also verified
before rerunning their boundaries against the current tree.

Completed so far: eleven editor tests and six JavaScript syntax checks; three
backend wire guards; 32 architecture checks; actual V176→V177 record preservation;
the complete authored activation/fault/reconciliation caller; routing/override
restart and historical catalogue restoration; and the existing same-global-VM
child caller with all three acknowledgement cuts. The authored case also covers
postcommit discovery failure with readable pending status, reference-only recovery,
failed prefix compilation with independent provider-free work, a fresh exact inline
model context and concurrent reconciliation with a one-connection pool.

Run-4 also completes the broad deployment/source/model consumer (343.56 s) and both
route contracts. Strict lint then finds one explicit auto-deref in the new test
helper call; it is replaced by the equivalent implicit coercion. The original
actually tested caller source is archived and hash-verified.

Run-5 stops before Cargo when its applicability check detects the newly integrated
Prefix proposal/parser/UI successor. Nine peer baselines exactly match run-4's
source hashes. Run-6 preserves those changes, reruns the affected real deployment/
measurement caller and new rule/proposal/design cases, and retains the unchanged
publisher/upgrade/restart/child/descriptor executions with their actual old artifacts.
Its reviewed diff and reuse rationale remain separate from fresh executions.
All 21 current UI controller cases, seven syntax checks, changed Rust consumer/
declaration/design cases and strict/final checks pass. No new executable is
attributed to an old execution and no failed batch is converted to a success.

Final command results and source/artifact identities belong to the original
run-4 and successor run-6 closure receipts. Generated-prefix serving and K3–K7
remain open; general validator/pure
approval work stays behind the conformity gate.

## Final checks and exact identities

| Execution scope | Actual tests / seconds |
| --- | --- |
| Fresh Rule declarations / proposal readers | 2 + 2 / 0.444 + 1.682 |
| Fresh architecture boundaries | 4 + 28 / 0.246 + 1.509 |
| Fresh full HTTP/PG deployment, original prefix jobs and v1/v2 measurement cases | 1 / 344.648 |
| Fresh retained Monty/design/tokenizer integration | 2 / 1.151 |
| Reused actual PG wire guards | 3 / 0.457 |
| Reused actual old-record V176→V177 upgrade | 1 / 5.171 |
| Reused actual authored publication/fault/reconciliation caller | 1 / 52.164 |
| Reused actual routing/restart/old catalogue reader | 1 / 69.632 |
| Reused actual child, original catalogue and three ACK cuts | 1 / 37.229 |
| Reused actual ingress descriptor contracts | 2 / 0.211 |
| Fresh editor/measurement controller cases | 21 / 0.136 |

The current affected test build passes in 172.885 s; the separate design integration
target build passes in 66.791 s. Strict root/PG/Skills/Composition/Product/WebUI
test lint passes in 74.105 s, PostgreSQL-default Composition lint in 33.973 s,
format in 4.590 s and whitespace in 0.051 s. Commands, package/features, profiles
and original executable paths remain in the receipts. These are check timings,
not a performance benchmark or full platform matrix.

The full current source-only Sempai measurement is 371505 tokens against the
declared 262144-token limit. The real Hugging Face warning remains retained;
capacity is false and no serving activation, truncation or larger model limit is
introduced. All six original model artifact hashes and the real reference Python
environment are recorded. Neither the proposal parser nor these diagnostics
establish design acceptance, general Q1 or model quality.

Archive: `target/accepted-evidence/k2-catalogue-publication-20261024/run-6/`.

| Retained identity | SHA-256 |
| --- | --- |
| Closure receipt `accepted.json` | `45762253e1c57e66adfc66365ced0f25e9b491f1e9427c1640c1052e47525326` |
| Current `source.json` | `1e98e7b0f5a6d7763f73621ae431b297d2c096b79177f48c930c95dc288eeee0` |
| Exact `reused-evidence.json` | `1a8c90cdeafa6bb7b11069635c7da2ddfa8c366e813e960e5bddc25d867c51cd` |
| Current Composition test executable | `6843c6c3e1f640241951d2d3832e2e6b37ad9a751a5d67b97e31fff92874c1d8` |
| Original run-4 Composition executable actually used by reused native cases | `8fb7261fc5e1c6a055ccbd6cb4aecc844ecb332b6bd06048aca31f2a62d89071` |
| Current retained design integration executable | `2f882eebd102bc13161594ebb5fe107ed607bc7507685b57371a8d4d6720f0b7` |
| Final pre-next-package applicability receipt | `0a45c9e7fa32e621dd1afcbc0ab001a433b766a24b117a97a7bb1eb3f5596a6f` |

Every production/source input and all 270 declared documents were verified before
later status edits; both old and new executable hashes were checked. The original
runner, formatter, logs, reference artifacts and failed attempts remain archived.
The build/source freeze is explicitly released to the Prefix chat. No regular
application/release, arbitrary recovery or installed acceptance is inferred.

Subsequent evidence-retention correction: run-6 used the same
`prefix-design-integration` label for its build and test commands. Its runner
opened that log with `w`, so the two actual passing tests replaced the preceding
raw build diagnostics. The original build command, zero return code, duration
and executable identity remain in `accepted.json`; that raw build log cannot be
reconstructed and is not claimed as retained or warning-free evidence. The
function-test output and separately labelled strict lints are retained. Original
receipts/runner/logs stay unchanged. Future batches must use distinct build/test
labels and exclusive log creation; rerunning passing tests would not recover the
lost historical diagnostics.
