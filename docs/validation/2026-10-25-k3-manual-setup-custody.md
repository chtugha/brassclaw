# Original manual setup-flow readback — 2026-10-25

**Scoped acceptance: 337 distinct passing Rust cases, comprising 204 fresh
executions and 133 exactly qualified independent Auth/GSuite/Workflow cases.
Final affected/root/default strict checks, format, whitespace and source integrity
pass without an intermediate compile, runtime or lint failure.**

The existing setup coordinator chooses one independent random Flow ID before the
first possible flow write, after the interaction port returns its original
challenge. An uncertain creation acknowledgement reads only that ID. Full scope,
kind, provider, challenge/expiry, continuation, binding and hash layout must match
the original definition. A foreign definition conflicts; missing or unavailable
readback remains unconfirmed. It neither creates a replacement flow nor abandons
an interaction whose flow may have committed. Canceled, terminal, inconsistent or
expired readback cannot become a fresh awaiting challenge. An already expired
initial request fails before any interaction/flow write.

Successful creation validates its returned record without an extra database read.
The native PG owner now delegates the same bounded helper used by filesystem and
port-backed callers. Existing IDs are never rewritten or derived again. The
public facade and HTTP request shapes are unchanged. No Tool, Recipe/VM, queue,
permission grant, crypto scheme, storage owner or Rust execution loop was added.

## Actual evidence

Archive: `target/accepted-evidence/k3-manual-setup-custody-20261025/run-1/`.
Receipt SHA-256: `be4ffe7f301f0fa4ffc5c0bf4a4438303761ff433074292a4aec6a682d87708f`.
Seven exact applied source/contract/changelog files, complete common-root source
inventory, retained executables, actual commands and logs identify the single
configured target and serial screen/lock window with Rust 1.96.0.

| Actual cases | Count | Evidence |
| --- | ---: | --- |
| Memory, native PG and persistent FS setup-original cases | 3 | fresh |
| Native PG refresh/callback/boundaries | 18 | fresh |
| Filesystem Auth/accounts/custody | 48 | fresh |
| Host/encrypted-store custody | 13 | fresh |
| Provider multiplexer | 6 | fresh |
| Auth/OAuth HTTP helpers | 11 | fresh |
| Callback/lifecycle/manual facade consumers | 22 | fresh |
| Actual public product-auth routers | 81 | fresh |
| Ordinary global Monty/current packaged-content caller | 1 | fresh |
| Architecture dependency boundary | 1 | fresh |
| Auth contracts/GSuite/Workflow | 133 | exact independent reused results |

Each of the three new tests exercises Original, Unavailable, Missing,
ForeignProvider, Canceled and Expired readback. These are 18 exercised scenarios
inside three Rust test functions, not 18 separately counted Rust cases. The
fully delegated flow wrapper drops an acknowledgement only after the actual
owner has stored the flow. Original recovery keeps one write/one exact-ID read,
retains persisted definition/state and allows the actual original interaction's
first submit. Other modes retain the real flow and return the appropriate
unconfirmed/conflict/canceled/expired error. A changed read projection cannot
substitute a different provider. This controlled port-level loss is not a claim
of physical network-COMMIT fault injection.

The native tests use real PostgreSQL metadata and actual
PostgresRootFilesystem/FilesystemSecretStore persistence. They do not certify
LocalFilesystem multi-record transactions or all deployed platforms. Existing
facade failure coverage now observes retained interaction after an unconfirmed
creation; expired setup rejects early, while stale-submit coverage still uses a
real existing expired lower-port interaction. Empty/duplicate/foreign input cases
remain. No ignored/zero-case suite is counted as pass.

Composition build: 190.12s. Final strict affected/root tests: 77.52s;
default: 14.55s; format: 4.59s. These are observed batch times,
not new product timeouts or task deadlines. The disk threshold was crossed before
the build: required executables were preserved and selective Composition cleanup
recovered about632 MiB to90%; no full cache cleanup was needed. No routine clean,
parallel Cargo or alternate target occurred.

Prior callback/refresh/Network and peer source-export successors remain intact.
The worker retains529 exact inputs plus the existing separately qualified
Cargo.lock Network dev-edge delta. Its executable SHA-256 remains
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

## Remaining owner scope

This closes original flow-creation readback after the known interaction challenge
within existing K3 step2. It does not establish transport-idempotent creation
before the first interaction acknowledgement, encrypted original manual input
proof, exact automatic/bound account selection after submit, atomic account/
interaction/flow success custody, or safe automatic HTTP submit-error/timeout
abandonment. Those remain before issuer expiry/final staging, dedicated setup/
original grants, Codex controller/sessions and K4–K7.

The same step2 owns DCR registration/flow-material partial-artifact custody:
current DCR create/store errors can still deregister after unknown successful
writes, with no durable cleanup disposition. That confirmed finding is recorded
in the existing owner journal; this setup correction neither changes nor
certifies DCR. No release, real issuer, platform-wide/deployment/full-plan
acceptance, Git commit/push/tag or SSH operation is claimed.
