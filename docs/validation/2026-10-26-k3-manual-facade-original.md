# Manual-token raw facade requires original proof — 2026-10-26

Status: scoped source/consumer acceptance; **not full K3, browser setup, native
cancellation, platform, release or complete-plan acceptance**.

The raw facade delegates every token to its selected original-input owner. A
completed flow and today's account no longer substitute for raw-input proof.
Same input returns its original result after account edits/revocation; changed
input conflicts, and replay does not dispatch another continuation. The separate
reference-only/browser migration remains open.

The in-memory contract adapter now models protected nonserializable SecretString
originals with owner-side comparison, retained result and prechallenge account
snapshot. Unbound setups create independent accounts. Original publication, not
current account status, supports later flow completion. This is test memory,
not new encrypted persistence. Generic abandonment reports an actually canceled
flow; this does not certify the native override or atomicity across unrelated ports.

## Validation and applicability

271 distinct positive Rust cases:107 fresh final executions (11 public manual,
42 WebUI auth,41 WebUI4201,7 callback,5 lifecycle and1 boundary) and164 qualified
individual R1 executions (86 Auth contracts,12 actual FS manual,9 native manual,
3 generic coordinator,53 durable and1 actual global Monty packaging case).
The source-only test adapter has three new contract cases; the real public facade
also tests unavailable original proof and changed input after continuation failure.

R1 remains FAILED honestly: a historical integration assertion treated every
identical replay as stale. The corrected case verifies exact replay, changed-input
conflict, no duplicate continuation and independent unbound accounts. Only that
independent integration-test file changed after R1. Every runtime/seed/indexed-doc/
Cargo/toolchain and other test input stayed identical; all actual images and
source membership were rechecked. Qualified successful individual results, logs,
hashes and the original failed receipt are retained in `qualified-reuse.json`.
The failed case is never counted as a pass; its repaired target ran freshly.

R1 full/root/Auth/tests and PostgreSQL-only strict lints passed and remain applicable
to unchanged inputs. R2 strict repaired target passes (34.52s), actual target build
66.91s, default Composition18.31s, format4.64s and whitespace. Seven final author
hashes,9,736 captured source entries and eight archived executables were checked.
No selected case is ignored and no warning is suppressed.

Receipt:`target/accepted-evidence/k3-manual-facade-original-20261026/run-2/accepted.json`.
SHA-256:`5bc44cf7d828939c42b8e6651ea9bcba342d3245fb8c86c5f2b059b4180f564e`.

## Remaining owning work

K3 step2 still owns pretransport setup/flow/invocation identity and first expiry,
explicit reference-only continuation, browser changed-token handling, DCR custody
and complete high-level/generic qualification. A connected native finding remains:
its abandon override treats Some(Completed) as success and deletes consumed
interaction proof; repair this with retained cancellation disposition before setup
retries are enabled. Native/FS live final-use fences, issuer expiry and cleanup/
controlled-upgrade recovery remain open. The complete K3–K7 and Prefix/full plans
are not closed. No Git/tag/push/release/SSH/provider-host change was performed.
