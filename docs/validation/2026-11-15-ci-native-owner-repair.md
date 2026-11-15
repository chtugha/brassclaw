# Main CI native-owner repair — scoped acceptance

Source: Main `90e679de264c88b109f861aadecf02741c837e75` plus captured Root
changes. This repairs actual failing Main CI callers; it does not complete B1,
activate components, release a version or establish full cross-platform CI.

## Actual defects and repair

The standalone Monty control harness compiled Composition's private admission,
completion and review modules into a different crate. Its obsolete crate-root
dependencies caused compiler errors. All five retained-execution cases now live
under Composition's actual test module and use its real private admission and
shared kernel/evidence fixtures. Their original test bodies and assertions are
preserved. The Q1 fixture uses the actual production modules rather than duplicate
source inclusion with lint exemptions. Reborn CI explicitly runs both owners;
normal interpreter, worker and standalone control gates remain.

OAuth test helpers have crate-local visibility. The invalid Screen-frame fixtures
first observe busy state, preventing a final notification before the intended
invalid update. Production IPC behavior is unchanged. The existing root Tool
coverage inventory now names the actual private Pure Case tests and a new real
file-presence test, which checks both present and absent scoped files without
changing either. The root trace fixture remains explicitly legacy test support;
this does not add a production Rust-loop fallback or qualify an ordinary Recipe.

Replay and CLI smoke jobs use Ubuntu 24.04 and explicitly install the bundled
PostgreSQL runtime's `libxml2.so.2` ABI. Their actual failing Ubuntu-latest logs
showed this missing dependency. Local Apple results do not certify these Linux
jobs; their post-push GitHub results must be evaluated separately.

## Evidence and preserved failure

Evidence lives under `target/validation/bob-native-repair-20261105/` and
`target/validation/ci-owner-repair-20261115/`.

- Run122 failed on a shared test-helper visibility and an unused moved import.
  Both were corrected without weakening assertions or suppressing diagnostics.
- Run124 strict all-targets lint passes in 75.411 s with zero diagnostics/warnings.
  The actual ten-package graph includes PostgreSQL/skills-db, migration,
  test-support and both ingress features, including dev-in-memory-session.
- Run125 native Composition/ingress/OAuth programs and normal worker pass in
  824.083 s with zero diagnostics/warnings. Actual Cargo paths, profiles,
  features and SHA-256 are retained; source is unchanged.
- Run126 native root trace/coverage integration programs and Cargo-required normal
  root binary pass in 413.093 s with zero diagnostics/warnings and unchanged source.
- All 55 Screen tests pass inside the sandbox in 20.072 s.
- Run127 standalone Rust 1.96 all-targets Clippy passes in 114.935 s with zero
  diagnostics/warnings and unchanged source. Its final IPC was accepted and
  actually received; the recorded Screen socket directory is empty.

All **28 selected source/native cases pass**, zero ignored, with exact nonempty
test selection, unchanged source and executable hashes:

| Actual program and boundary | Cases | Batch duration |
| --- | ---: | ---: |
| Composition observed-value fingerprint | 1 | 3.177 s |
| OAuth HTTP/provider/session integration using own localhost server | 11 | 2.927 s |
| Composition retained/Q1/workflow/behavior/persistence/authenticated HTTP | 13 | 87.183 s |
| Root built-in coverage inventory | 1 | 2.014 s |
| Root actual file read/write and present/missing traces | 2 | 7.537 s |

The thirteen Composition PG cases include all five moved retained-execution
cases: blocked-intent cancellation/rollback; actual live kernel policy and
preserved successful effects; unbound-source rejection before execution; explicit
child result handoff; and unsafe preload rejection. The seven existing native Q1
cases and actual authenticated review route also pass. Their evidence remains
scoped to the existing review contracts, not general authored approval.

Independent unit/OAuth cases run inside the sandbox. PG cases use only their own
dedicated temporary clusters, through the existing native fixture owner outside
the sandbox for the previously established shared-memory restriction. No existing
database or provider is used. Completed test/PG/worker processes and Screen socket
termination are checked separately from successful tests and accepted final IPC.
Final chat notifications for these completed native batches were actually received.

## Build resources and remaining boundaries

After native execution, the system disk exceeded the existing 90% gate. Two exact
closed, unused Composition incremental sessions from the old graph were audited
and deleted under the native lock, with no active compiler/test/PG/worker or open
selected files. Shared storage meant the first deletion freed only 1.3 GiB; the
second restored about 27.5 GiB free and 88% usage. The current source and all
Run125/126 executable hashes remain identical. Current builds disable incremental
compilation; current programs, libraries, other graphs and evidence are retained.
No extra target, second native build or cloud compilation is introduced.

The combined-build plan now records the actual outstanding PG test cleanup
boundary: the Modal native-test collector's TERM/KILL timeout is not orderly
cluster shutdown. Its separate test-mode qualification remains open. These direct
native cases do not activate that mode or certify its cancellation path.

After accepted CI repair publication, continue the remaining B1 trust/behavior/Q2
work before B2 and B3. Neither this test relocation nor a coverage list creates
approval or permission. Keep the original Main-CI failures, post-push results and
remaining product implementation distinct.

The subsequent `90e679de` GitHub run separately exposes a global-runtime restart
assertion (Tool journal count 194 -> 200), CLI bootstrap observation/utility
deadlines, and one embedded PostgreSQL start failure. Those complete original
logs are retained as jobs `132800524439` and `132800680485`. They are not repaired
or accepted by this package. Root continues their diagnosis after publishing this
independently accepted checkpoint; neither required CI rollup is claimed green.
