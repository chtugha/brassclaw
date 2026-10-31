# Authenticated secret generation primitives — 2026-10-25

**Scoped acceptance:429 passing Rust cases, freshly executed at run2 and exactly
qualified for final run3's removal of three unused test imports. Current strict
all-affected/root test compilation, default lint, format, whitespace and source
integrity pass. There are no newly executed run3 runtime cases.**

The existing SecretStore now requires authenticated encrypted-generation lookup,
actual conditional writes and owner-side material comparison. The same native
PostgreSQL operations already existed; the generic ports complete their real
memory/legacy/filesystem implementations and all actual wrappers. None means an
absent-only write. A changed reference returns no write; an error can still mean
that the write committed. Material and plaintext digests never cross comparison
ports. In-memory fixed-size digest comparison uses constant-time equality; no
comparison digest is persisted or exposed. Comparison does not establish expiry
eligibility, a runtime lease or authority.

PostgreSQL generic reads use real read-only transactions and preserve existing
legacy ciphertext/keys. Existing conditional writes already compared before
legacy migration; that behavior and reference bytes are unchanged. The private
hash helper is renamed only to avoid shadowing the new trait method. Filesystem
CAS uses the actual returned RecordVersion/Absent expectation, including between
independent persistent owners. The legacy memory implementation compares/writes
under its actual owner lock. No check-then-unconditional-put or local-mutex
substitute is introduced. All12 actual implementations satisfy their10/11
required methods; this source check supplements actual compilation and execution.

Two synthetic storage fixtures now delegate real encrypted memory persistence
and leases. The root seeded fixture provisions each intended fixture owner once,
then respects deletion, scope and actual one-shot state; it supplies no production
ownership or cross-process-lease acceptance. The explicit failing-lease fixture
returns its injected error for writes instead of fabricated persistence success.

## Actual evidence

Archive:`target/accepted-evidence/k3-secret-generation-ports-20261025/run-3/`.
Receipt SHA-256:`edde92c256c872f03f8a760190013f6672dedc6f7d6f7daeeca598adc98d26e1`.18 exact applied source/contract/changelog files,
full common-tree source inventory, exact commands/features/environment, prior
retained actual executables and final qualification are in the archive. Run2's
entire immutable source/results/logs remain retained. Run1 failed before
compilation on runner feature selection; run2 completed every runtime case and
then exposed the unused shared-test import under strict lint. All warnings were
resolved; no diagnostic, failing case or required backend was suppressed.

| Actual cases | Count |
| --- | ---: |
| Secrets library/contract/boundary | 63 |
| Actual Host secret obligations/credential egress | 10 |
| New actual native generation cases | 4 |
| Composition setup/custody/provider/DCR/HTTP helpers | 113 |
| Public callback/lifecycle/manual/WebUI consumers | 103 |
| Ordinary global Monty with current packaged content | 1 |
| Auth contracts/GSuite/Workflow consumers | 133 |
| Existing root host-managed Tool caller | 1 |
| Architecture dependency boundary | 1 |

The shared contract exercises two actual writers for absent and captured
successors, stale references after normal replacement and deletion/reinsertion,
foreign owners, real material comparison and ordinary fresh one-shot leases.
Independent legacy adapters retain their existing local lease maps; the fresh
lease is consumed through its issuing owner. This is not cross-process lease
recovery or the later generation-after-lease fix.

The four new native cases use actual PostgreSQL, independent real
PostgresRootFilesystem/FilesystemSecretStore owners, actual held row locks and
wrong encryption keys. Read-only projections finish while a write lock remains
held. A real protocol proxy cuts the server COMMIT acknowledgement after the
actual encrypted write commits; the independent owner reads the original, and
an absent-only changed retry cannot overwrite it. The legacy fixture keeps exact
old key/ciphertext bytes on reads and migrates only on the matching write. No
network-COMMIT simulation, silent database skip or zero-case pass is counted.

Final strict affected/root checks:86.07s; default:37.09s;
format:4.71s. These are observed validation times, not product deadlines.
The runner used one configured Cargo target and serial Screen/lock window,
Rust1.96.0 and the retained worker with529 exact source inputs plus the prior
separately qualified Network Cargo.lock dev-edge delta. Required executables
were preserved before capacity cleanup; once selective cleaning was insufficient,
the disposable dev cache was cleaned under the same lock. No competing Cargo,
retired target2, routine cleanup or remote compilation occurred.

## Remaining owner scope

This is an existing K3 step2 prerequisite. It does not make a normal mutable
handle an immutable original input, publish manual account/interaction/flow state
atomically, preserve original manual outputs, fix DCR partial-artifact disposition
or pin existing leases to an original credential generation. Those retain their
named K3 steps and acceptance, before issuer expiry/setup/Codex and K4–K7. The
prepared-value/immutable-input designs remain private candidates, not accepted
APIs or stored formats. No new crypto scheme, store, registry, queue, Tool,
permission, VM or Rust workflow loop was added. No full-plan, deployed/platform,
release, tag/commit/push or SSH acceptance is claimed.


## Follow-up limitation from the current source audit

The scoped sequential replacement/reinsertion and concurrent-writer cases above
do not establish the narrower window where a physical delete/recreate happens
after filesystem reference validation but before its Version-only CAS. Both
actual in-memory and PostgreSQL RootFilesystem restart a recreated path at
version1. Thus that condition alone can accept a later encrypted body under a
reused version. This is a confirmed source-level gap, not an observed failed
regression run yet. Existing K3 step2 owns the atomic version-and-body repair and
controlled real persistent tests. The historical receipt/case counts remain
unchanged; do not infer this interleaving is already closed.
