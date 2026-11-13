# Native cancellation and strict-Clippy repair — 2026-11-12

Status: **scoped local repair acceptance complete for `1.7.0-rc.5`**.
Validated runtime source is `f8b1f01a7d7dc64008086450da535c5228ba4dc6`.
The following documentation-only checkpoint changes no runtime/embedded input.

- Final full-feature strict Composition Clippy passes on this source, zero
  warnings, Run-56 (80.244 s including guards). Applicable CLI strict lint passes.
  All 20 original diagnostics are closed without suppression.
- Original Run-52 native regressions and additional actual custody cases pass.
  Source1's original latency cause remains unproven; logging is not its repair.
- All 83 changed CLI integration cases pass: complete 70-case smoke, five extension,
  eight PostgreSQL lifecycle. All four replacement provider contracts use the
  actual DB/native auth and retained Monty Recipe path; no runtime fallback.
- Original Pure regression passes in Run-55 with original assertions and terminal
  source/artifact guards. Its earlier 503 remains **unreproduced/unexplained**.
  Measured complete positive Source verification takes most of 875–1312 ms of
  the selected 2000-ms read deadline. No required check or deadline is weakened.

Run-56 reuses exact Run-55 source/artifacts and does not rebuild or repeat native
behavior. Separate actual CLI and Pure accepted receipts state their narrow scope.
No new GitHub/Linux, full-workspace, platform, deployment, general Pure or complete
Bob acceptance is inferred. All failed original receipts remain failed/preserved.
The full private N/R/Q/T stack is not integrated by this checkpoint.

V-01's final read-only audit finds one complete required Source walk, including
structural receipt/eight approvals, twelve checker contexts, workflow fixtures,
parser and selected pins/bytes. No justified small production optimization is
found. Extra private diagnostic bridge `0dd07900d` stays unintegrated. Existing
B2-04/B2-07 own any later shared exact-original revision/reader optimization, with
transaction/owner identity, recursive/schema/closure validation, exact bytes,
independent qualification/native evidence, capacities and historical pins intact.
No global approved cache, latest lookup, new Pure service, blind retry or timeout
increase. All owned validation processes are terminal and the freeze is released.

## Initial reviewed source

Main `2645a88531ee3ea009fc9bf86f7afefeb04465d2` contains:

- `00ab9f8ec`, package V-01 `d47a7e549`: passive original bootstrap-admission
  phases and four lint fixes. The original deadline is unchanged. Diagnostics
  alone do not repair or explain the original Source1 failure.
- `a7f8b1721`, V-02 `568aafe74`: the other 16 lint causes and connected callers.
  Narrow identity/artifact/selection groups replace repeated parameters; boxed
  internal variants preserve serialized fields, null/missing handling and wire
  bytes. HTTP preparation rejection becomes a small internal classification,
  with the same response built at the middleware boundary. Prefix revisions
  remain exact decimal strings.
- `a6649cef5`, V-02 `a8423bd6`: the additional Pure-workflow readback caller
  found during supervisor review. It uses the same original execution artifacts.
- `2645a8853`, V-01 `5940717c2`: original PostgreSQL cancellation/retirement
  custody, actual transport tests and missing-observer-ACK regression. All eight
  imported files are verified byte-for-byte against the package's final commit.

The supervisor reviews, integrates and validates; product changes are package
authored. No suppression, Rust-loop fallback, pool increase, deadline increase,
weaker original assertion or earlier admin-lock release is introduced.

## Actual PostgreSQL defect and repair

Previously `PgClient::take` prevented recycling, but `ClientWrapper::drop`
aborted only the local driver task. PostgreSQL could remain blocked in a row
lock, without processing socket closure. Client scope ending therefore did not
establish server cancellation.

The existing IntentExchange now owns opt-in `TransactionRetirement` before IO.
It captures the original backend PID and full start timestamp before BEGIN,
inside the existing SQL budget. An unconfirmed client is synchronously removed
from the pool before cancellation. One owned cleanup task progresses even when
the Completion observer is parked or the original SQL task aborts/panics.
It sends the original CancelRequest, closes that original connection and then
checks actual absence of the same backend identity through a new pool checkout.
CancelRequest delivery alone is not confirmation. Confirmed COMMIT still
recycles normally and creates no cancellation task.

Cleanup captures the effective cancellation-ACK policy once, with one absolute
deadline covering scheduling, identity fallback, cancellation and observation.
Failed capture, task, transport, pool, deadline or server observation stays
unresolved. Join, settlement, registry release and shutdown consume that actual
result. A late pool release cannot turn a cached failed acknowledgement into
success or replay application work. There is at most one checkout/job per
original, retained within existing admission capacity.

Shared connection policy also preserves the originally selected local plaintext
mode in the CancelToken. TLS preparation is deferred until actually requested;
required TLS still verifies the original hostname and issuer. No plaintext
fallback is added for remote connections.

## Actual completed checks

Local, serial Cargo uses the configured canonical target and native lock, jobs
2, direct commands through the existing launcher. Run-53a source is frozen for
compiler/runtime inputs. No paid cloud build is launched.

| Check | Actual result |
| --- | --- |
| Composition library Clippy, `postgres,skills-db,migrate-from-libsql,brassclaw_pg/test-support`, `--keep-going -- -D warnings` | PASS, 93.023 s, zero warnings; all 20 original diagnostics closed |
| PG all-targets Clippy, `test-support,connection-integration`, `-D warnings` | PASS, 11.418 s, zero warnings |
| Normal `monty_worker` build | PASS, 11.352 s, zero warnings |
| PG library and actual connection test-program compilation | PASS, 49.75 s, zero warnings |
| PG library execution | PASS, 11 actual tests, zero ignored |
| PG connection-policy execution | PASS, 4 actual tests, zero ignored |
| Composition harness compilation | PASS, 1177.663 s, zero warnings; actual unwind sections retained |
| Current-tree architecture checks | PASS, 33 actual cases, 2.129 s |
| Original Source1 native case | PASS, 439.919 s; the earlier startup delay's cause remains unproven |
| Original pool-one/claim-lock native case | PASS, 802.550 s; original pool-one and physical row-lock assertions retained |
| Missing-server-observer-ACK native regression | PASS, 6.915 s; late pool release cannot change the unresolved acknowledgement |
| Actual original IO abort/panic custody | PASS, 763.968 s, both genuine interruption branches |
| Confirmed commit recycling and prestart fences | PASS, 1141.318 s, all three genuine branches |
| Additional applicable retained-consumer regressions | Pending on the final integrated source |

The transport suite uses three real isolated local PostgreSQL 17.11 servers,
one plaintext and two TLS with independent issuers. Only the acceptance CA is
trusted for the test process. It checks real notifications, certificate/hostname
failure, remote-classified TLS refusal without downgrade, and cancellation of
an active query on plaintext, local Prefer on an SSL-capable server, local
Require and remote-classified Prefer. Remote classification uses a loopback
hostaddr; it does not claim external deployment acceptance. All three servers
stop normally. The complete fixture/cleanup duration is 2.239 seconds.

The retained Composition image has SHA-256
`71da88593bc1edd18679d3bfc14fdd0e1310c5839efce341e2d9a3dceb54e63e`;
`__eh_frame` is 13,777,656 bytes, below 16 MiB, and `__unwind_info` is 2,642,544
bytes. Actual test optimization/debug/assertion/overflow settings are retained.

After this completed build, a separately authorized host-Skill update is
published as `064d07a63`, with preserving local merge `bc2bf6089`. Its only tree
changes are the two mirrored host Skill files and a seven-line CHANGELOG entry.
The CHANGELOG modification timestamp is after successful compile completion;
the original successful compile guard is unchanged. The supervisor reads the
complete change, verifies all runtime code/manifests unchanged, checks 11,543
actual compiler dep-info files for new host-doc references and verifies the
original compiled CHANGELOG bytes inside the retained image. The native cases
exercise that exact `2645a8853` image and its original embedded documents. They
do not establish uptake of the later model-request instruction entry; final
release packaging remains a separate gate. Details are in
`native-source-review.json`; the original `source.json` is preserved.

The completed build leaves the system volume at 93.38% used. Under the native
lock, with no compiler/native process active, a manifest-based cleanup removes
126 older complete incremental snapshots while preserving the latest complete
snapshot for every identical crate/profile graph, plus 1083 standalone object
files from two already superseded graphs. It frees 9.185 GiB, restoring 25.311
GiB free and 88.91% used. All 58,956 remaining incremental files retain their
identity/size/timestamp; current and historical retained program hashes verify
before and after. Libraries, fingerprints, source, DB and evidence are untouched.
This is a required capacity repair, not a routine whole-target clean.

## Evidence and publication gate

Local ignored evidence is under
`target/validation/bob-native-repair-20261105/run-53a-native-repair/`, including
source hashes, original runner, Cargo JSON/diagnostics, artifact identities,
`pg-lib-tests.json` and `pg-transport/completed.json`. Original Run-51/52 failed
receipts remain preserved; new evidence does not rewrite them.

The three native cases execute successfully on the exact compiled global-worker
path. Their final full-source guard fails because the externally authored,
untracked `speedup2.md` plan appears during execution. That failed receipt remains
unchanged. A separate `native-first/terminal-source-review.json` proves this is
the only added input, checks all 11,543 actual compiler dep-info files for
references and verifies original/retained executable hashes. Captured runtime
inputs are unchanged; this accepts those three case results, not the new plan or
final release packaging.

Source1's first error was an unresolved `checker_fixture_admission` timeout
during fresh startup. It does not recur in this execution; passive observation
is not established as its repair or a latency explanation. Review additionally
finds interruption custody can be lost when a bounded bootstrap admission is
dropped after its first writes but before returning its binding. V-01 owns the
connected original-only retention/settlement repair and actual negative cases.
No new admission, rematching, replay or increased deadline is permitted.

The user also adds four historical GitHub CLI smoke failures from workflow
`41197330030`, job `124526200902`, source `2522569198d`. V-03 owns the isolated
repair: actual DB-seeded provider catalogue expectations, isolated PostgreSQL
ports, current native authentication, and provider-free Recipe success versus
actual No-Match failure. The exact failed logs are retained locally; existing
warnings are not suppressed. CLI acceptance remains pending.

Changelog/version/tag/push remain pending until required repair acceptance.
No full-platform, full-Pure, full-Bob or deployed-support claim follows from these
scoped checks.

## 2026-11-13 — Reviewed joint candidate

The final connected Run-53a batch passes both native custody cases and its
unchanged-source guard. Root releases that execution freeze, fast-forwards the
three reviewed V-03 commits (`aaa14fe3b`, `d388336b3`, `ea1156e0d`) and integrates
V-01 `78af1ecb7` as `fdfac2491`. The three automatic merges are fully compared:
their only differences from V-01 are the previously reviewed V-02 parameter
groups and borrow correction. No old whole file replaces the integrated code.

V-01 covers all four immediately bounded qualification callers: Checker,
Inventory qualification, Installation observation and Pure-case qualification.
It preselects actual native IDs, requires the pending-reference write and exact
readback before first admission, retains the original native address before its
first write and never restarts admission. Unknown close acknowledgements permit
original-only observation. A domain-separated owner digest covers the moved
admission code and protected original reader without changing historical bytes.
Four new real PostgreSQL cases cover ten admission/pending-commit variants and
an uncertain cancellation-commit readback; their execution remains pending.

V-03 changes four test files only. Its stronger authentication oracle requires
one actual host model attempt, the existing native ConfigurationError rejection,
no model-start/completion/failure event for that original run and zero connections
to the configured local egress trap. A positive finalized-reply event verifies
the event correlation. It distinguishes preparation refusal from a later Prefix
or context failure and keeps genuine No-Match failure. No runtime fallback or
new product diagnostics are added.

Version metadata is prepared as `1.7.0-rc.5`, dated 2026-11-13. Publication and
tagging remain gated by the actual joint compilation and required native/CLI
execution; preparing metadata is not release acceptance.

## Historical Run-54/55 execution notes

Run-54 records warning-free CLI all-targets Clippy (139.981 s), with the actual
full retained Composition dependency features. Its preceding Composition
command incorrectly uses self-qualified feature names: actual Cargo artifacts
show `skills-db` and migration missing. That 62.190-second check is **not**
acceptance of the new custody code. The supervisor catches this from the real
artifact records, corrects the call to explicit package features and adds
required-feature assertions to the remaining runner.

The next worker launcher stops before Cargo because system usage exceeds 90%
(22.087 GiB free); this is a capacity gate, not a worker compiler failure. An
audited cleanup evicts 191 completed metadata-only analysis cache sessions,
749 files, freeing 4.450 GiB. All code-generation snapshots, canonical Cargo
metadata/library hashes, fingerprints, executable images, source, DB and evidence
are preserved. The pending full-feature Composition check cache is also retained.
Capacity returns to 26.658 GiB free, 88.32% used. Original Run-54 records remain
unchanged. Run-54b resumes only required checks on exact `c91042365` inputs and
reuses the genuinely successful CLI check; it does not reuse the wrongly scoped
Composition result.

Run-54b completes the actual joint build warning-free (1325 s reported by
Cargo; 1383.096 s including preceding stages and artifact collection). Required
features and unwind checks pass. Root then integrates the reviewed test-only
temp-directory correction `227179ca0` as `7ce83025c`; only three CLI helper files
change. The successful build's current code-generation caches remain intact.
Actual capacity again requires cleanup: 29 dormant older profile/check sessions,
11,186 files, are removed after manifest, process/lock, hardlink and artifact
checks. This intentionally retires prior profiles, not the successful joint
build's caches. It frees 9.316 GiB; canonical libraries, metadata, fingerprints,
programs, source, DB and evidence remain unchanged.

Run-54c refreshes only the CLI checks/programs in 19.220 s. CLI strict Clippy
passes, and Cargo confirms the application, Composition harness and CLI library
test program are fresh. Their hashes and the ordinary worker hash remain
identical. The successor `source-applicability.json` preserves the earlier
compiler manifest and records exactly the three test-file changes. The existing
architecture programs execute 33 current-tree cases successfully without another
Cargo run.

Actual regression execution starts on this coherent successor image:

| Run-54c case/program | Actual result |
| --- | --- |
| Pending-binding commit loss and caller cancellation | PASS, 7.120 s, both variants |
| Submit/claim/reservation/start unknown-commit deadlines | PASS, 25.257 s, all four variants |
| Submit/claim/reservation/start caller cancellation | PASS, 16.147 s, all four variants |
| Original cancellation unknown-commit readback without reissue | PASS, 6.110 s |
| Function-subject atomic retention and exact unknown-commit retry | PASS, 2.660 s |
| CLI extension program | PASS, 5 cases, 9.420 s |
| CLI PostgreSQL lifecycle program | PASS, 8 cases, 22.728 s |
| Original Source1 positive | PASS, 420.327 s |
| Original Pure return/exception/contract-defect case | FAIL, 435.596 s; post-completion original read returns 503 |
| Complete CLI smoke program, including four provider-contract cases | PASS, 70 cases, 3270.995 s; terminal source/artifact guards pass |

The runtime runner holds the canonical target lock, checks exact nonzero case
counts and ignored cases, and retains terminal failures as failures. Its source
and artifact guards remain active. No final publication acceptance is claimed
while the remaining real cases are running.

The Pure case submits successfully and receives an actual internal completion,
then fails in `get_original_pure_case` at `original_pure_case_tests.rs:210` with
`ServiceUnavailable`. It is not a startup failure or a null test. The failed
batch, complete logs and successful source/artifact guards are preserved.
V-01 owns the connected reader/identity/settlement diagnosis and repair. A
separate `cli-smoke-independent` run uses the same immutable programs to collect
the unrelated smoke results while that repair remains private. No fallback,
weaker assertion, unchanged retry or publication acceptance is introduced.


### Connected historical-read measurement prepared

V-01 supplies private diagnostic commit `9559235b1` after tracing the actual
post-completion reader. Root reviews all six changed paths, test-only task-local
scope, exact error mapping, deadline retention and guarded Boolean relation query.
No causal defect is yet established. The old failed fixture DB has been stopped
and deleted by its real cleanup owner; the next necessary execution therefore
uses the original native case with new observations. It does not retry unchanged
source to seek green. Raw candidate inputs, source, outputs/exceptions, secrets
and SQL messages are excluded. Diagnostic SQL failure cannot establish a cause.

The independent smoke run has now passed the actual DB catalogue, native Codex
binding refusal and DB-selected encrypted-key refusal cases. The Recipe/No-Match
and remaining full-suite results are pending. Source/artifact freeze remains in
force until the terminal guard. No extra Cargo or Modal execution occurs.

All four actual `provider_contract` tests now pass in this run, including the
qualified Reply/History Recipe success and separate genuine No-Match failure.
The rest of the 70-case smoke program remains active; this is local macOS
acceptance of those four cases, not a new GitHub/Linux result.


### CLI acceptance complete; Pure diagnostic compilation active

The full independently executed smoke program completes at `7ce83025c` with
70 passed, zero failed/ignored in 3270.995 seconds (3279.115 seconds including
terminal verification). Its original source and retained/canonical artifact
checks pass. Separate supervisor `cli-smoke-independent/accepted.json` records
only that scope. This closes V-03 local acceptance together with strict CLI lint,
five extension and eight lifecycle cases; no GitHub/Linux result is inferred.

Root integrates V-01's diagnostic `9559235b1` + success-observation follow-up
`226b34526` as `44cce2bf8`/`f8b1f01a7`, with all six result files byte-identical
to the reviewed private result. The old failed Pure receipt remains failed.
Run-55 compiles the diagnostic image under the exact previous joint feature
and target graph in Screen. This temporary measurement has no causal-fix or new
strict-lint claim. Only its one actual original Pure case is scheduled next;
unchanged passing CLI cases are not repeated merely to obtain another receipt.


Run-55 diagnostic compilation passes in 732.427 seconds with zero warnings and
unchanged source, required actual features and retained unwind sections. The
ordinary worker stays byte-identical. This is measurement compilation, not a
causal fix or new strict-lint acceptance.

Post-build capacity is 20.180 GiB free/91.160% used. Nine superseded completed
sessions with newer successful codegen in the same exact graphs would reclaim
only 2.448 GiB and remain above the limit; this first manifest is not executed.
The final manifest adds 50 completed metadata-only sessions containing only the
four approved compiler analysis filenames. Actual approved cleanup removes
59 sessions/2744 files and frees 3.452 GiB, restoring 23.652 GiB/89.639% used.
All nine current same-graph codegen snapshots, 57,995 other incremental files,
canonical/retained executables, libraries, fingerprints, source, DB and evidence
remain intact. Original manifests/receipts are retained separately; no stale
Run-54 cleanup manifest is reused.

The single original Pure case now executes from the Run-55 image in Screen.
It keeps the real worker/backend and original assertions; terminal outcome and
reader observations remain pending. No other passing suite is rerun.


### Run-55 measured original Pure case passes; prior 503 remains unexplained

The original case completes in 390.291 seconds, one pass/zero failed or ignored;
terminal source and all canonical/retained artifact guards pass. The four
historical reads settle successfully (Return, same-original readback, Python
exception and result-contract defect), with no approval created. The separate
measurement acceptance explicitly excludes a causal fix of the prior 503.

| Settled read | Total ms | Final nested H01 result-reference ms | Positive Source acknowledgement ms |
| --- | ---: | ---: | ---: |
| Return | 875 | 13 | 870 |
| Same-original readback | 886 | 11 | 881 |
| Python exception | 1312 | 32 | 1307 |
| Contract defect | 941 | 13 | 936 |

All four capture revision 1/2000 ms; pool size/available is 3/3, waiting zero.
The terminal recheck takes only 1–2 ms. Thus the materially slow measured segment
is the positive Source reader, not pool checkout or that terminal recheck. This
does not prove the earlier failure was deadline-related. V-01 traces the existing
confirmation/qualification path before proposing a connected repair; no budget
increase, generic retry, relaxed validation or untrusted global cache is accepted.
All native/build processes are now terminal and the freeze is released. Main/tag
publication remains pending the source finding and final applicable checks.
