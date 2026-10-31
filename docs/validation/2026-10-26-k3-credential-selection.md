# K3 ambiguous credential selection and setup errors — 26 October 2026

Status: **necessary correction accepted; K3 step4 and the full plans stay open**.
This implements part of the existing
[dedicated-auth/original-grant step](../plans/simplified-v3-implementation.md),
not a second selection store, task workflow or permission grant.

## Behavior and affected callers

The existing runtime selector now returns `AccountSelectionRequired` for multiple
eligible accounts. A label, matching ownership, shared provider or later timestamp
cannot prove these are the same login or establish a selected credential. The
runtime resolver preserves `AuthRequired`; Extension readiness reports no connected
account instead of silently showing the newest one. Actual filesystem cases still
verify all70 records/session roots before asserting ambiguity: no truncation or
backend failure is relabelled as selection.

OAuth's existing exact update-scope boundary is applied before resolving ambiguity.
Accounts from another invocation therefore cannot prevent choosing a unique exact
update target. Unavailable lookup and ambiguous exact targets return their typed
503/409 failures before admitting a new flow. They no longer silently mean
"start without an update binding." Confirmed absence or an unavailable-to-requester
account keeps the existing distinct new-setup behavior; current ownership and
scope checks remain in force.

The native PostgreSQL regression verifies a unique exact-scope update beside a
different invocation, ambiguous ordinary runtime selection, lookup-only explicit
account projection and unchanged full original accounts/snapshots. A metadata
lookup is not a persisted requirement binding. Protected route tests exercise
actual status/body output, zero new flows on503/409, and preserved outside-scope
setup behavior.

The broader route selection found an older PKCE fixture that wrote only a Secret
under a random flow ID. The registry correctly rejects that unregistered identity.
The negative case now preserves that rejection. The positive case admits a real
DCR original, constructs a new route state with an empty PKCE cache, reads the
registry's exact verifier and compares its original retained hash. Its egress
adapter forbids every HTTP call after setup. No production original/custody check
was removed to satisfy the old fixture.

## Exact evidence

Receipt: `target/validation/credential-selection-20261026/run-3/accepted.json`,
SHA256 `ac76a27baebef6d95eed0dc0928bd0411e0fe61ab714d4cfac04145b94188dfd`.
It records five owned inputs (four Rust sources and the embedded changelog),
complete source/executable/log hashes and actual commands. During validation,
only the nonembedded handoff, existing journal and runtime-limit audit changed;
compiler inputs did not drift. The inspector source remains unchanged and its
separate Prefix acceptance is not broadened here.

One serial canonical-target batch passed strict six-package test Clippy in77.11s
and the selected test build in208.34s. The current retained Composition image is
`af4c58546d28834753281093bfaa2042471ba8b28cfbf4af9e54e61b3b742ced`.
Actual fresh executions cover **109 distinct Rust cases, zero ignored**:

| Suite | Passed |
| --- | ---: |
| Runtime credentials, including actual native PostgreSQL | 16 |
| Extension readiness | 5 |
| Filesystem AuthProduct, including native persistent callers | 73 |
| Protected auth routes and both PKCE cases | 10 |
| Original native HTTP and actual Chromium | 5 |

The protected browser emitted `native-dcr-original-browser: passed` and retained
the original IDs/expiry through503/200/202/200/200, blocked-popup handling and
visible unknown cleanup. R1's failed older fixture and R2's direct frozen-image
HTTP/Chromium sweep remain intact. No passing R3 result is inferred from them.

## Still required

Removing the unsafe chooser is not complete account selection. K3 step4 must still
persist exact server-validated requirement-to-account bindings, deliberate sharing
and original task-family credential generations across waits/children/recovery.
Unrelated same-provider existence cannot establish readiness, even when unique.
The existing journal records the dropped runtime handle, ordinal display-name
hazard and Google branch losing its original flow ID. Finish those real setup,
store, UI and runtime handoffs; do not stop at requiring authentication again or
invent another selection registry.

Complete source reclamation, Codex sessions, adjustable technical guards, K4–K7,
general validators/composition and durable recovery stay open. This local macOS/
native-PG/controlled-Chromium evidence is not Docker, Linux/Windows, global startup,
full Extensions-page, real issuer, Prefix serving, deployment or release acceptance.
