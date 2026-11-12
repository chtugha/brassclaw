# Native cancellation and strict-Clippy repair — 2026-11-12

Status: **acceptance in progress; not released**. The user commissions the two
failed Run-52 native cases and all 20 Run-51 strict-Clippy findings, followed by
changelog/version update and Main/tag publication. This does not close the full
Bob plans or integrate the private N/R/Q/T stack.

## Reviewed source

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
