# In-memory Auth fake: exact manual selection transitions

Status: **test-service transition fidelity accepted; productive consumer work remains open**.

The existing InMemoryAuthProductServices now prepares the manual account and
validates original flow custody and the captured requirement-head precondition
before mutating state. Account, head, completed flow, original receipt and
continuation become visible under its existing mutex. A stale choice neither
creates an account nor changes an existing bound account or emits a continuation.
Original account/custody remain available after later choices; raw repeats prove
their original input. Requested/bound scope metadata is retained without claiming
external issuer privileges. No AuthState/raw-secret clone or new store is added.

## Actual acceptance

Receipt: `target/validation/fake-requirement-publication-20261027/run-2/accepted.json`.
SHA256: `7be22f85e559ce4f57f11be088a5c2d99e59fcb7c75b1b9f83a1500e2942425b`.
Serial background screen used the inherited target and retained complete input
hashes and actual emitted test images. **98 fresh passes:** five unit cases and
all93 public Auth contract cases, zero ignored. Each new case was explicitly
observed: bare/stale new-account rejection without effects and original recovery
after later choice; stale bound selection without account mutation; and bound
scope preservation with old original recovery despite a replacement handle.

Auth all-target strict Clippy (`-D warnings`), formatting and whitespace pass.
R1 stopped before tests because a removed helper left ExtensionId unused; the
import was removed, without suppression. R1's failure/source archive remains.
No Compiler/Screen process remained after R2. Prefix Source6/Source2 stayed held
and unchanged. Native/filesystem production implementations were not changed
or requalified by this Auth-only batch; their separate actual receipts retain
their exact scopes.

## Remaining work

This is the actual fake being tested, not substitute evidence for production
transactions, CAS-only durability, provider calls or task-family grants. Other
OAuth/account-choice publishers, explicit sharing and original family/child/wait
selection and material-generation consumers remain open. No complete K3–K7,
Prefix serving, release/deployment, platform or full-plan acceptance is claimed.
