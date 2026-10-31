# K3 issuer access expiry through final credential injection — 26 October 2026

Status: **scoped acceptance passed; K3 and the full plans remain open**.
The implementing owner is Simplified's existing K3 step3 in
[the implementation journal](../plans/simplified-v3-implementation.md).
This extends the actual AuthProduct, Secret and host-runtime owners; it adds no
Recipe workflow, registry, approval grant, Rust agent loop or fallback.

## Implemented boundary

- The actual OAuth callback and refresh adapter captures a conservative origin
  immediately before its token request. Checked issuer lifetime arithmetic runs
  before secret writes. Omission remains unknown; zero is expired. Explicit null,
  negative, fractional, string and unrepresentable lifetimes fail before token
  publication. This is the supported numeric lifetime contract; see
  [RFC 6749 section 5.1](https://www.rfc-editor.org/rfc/rfc6749#section-5.1).
  Choosing the pre-request origin is BrassClaw's conservative policy.
- The exact optional deadline follows the access token through provider results,
  account creation/update, retained callback publication/recovery and refresh.
  A new token without lifetime clears the old access deadline. Omitted refresh
  rotation keeps its original refresh token. Recovery never restarts a lifetime
  or repeats an issuer exchange to repair publication.
- Expiry is derived eligibility, preserving raw account equality, timestamps and
  snapshot identity. Account lists, choices, recovery, runtime selection and
  GSuite consume that eligibility. Historical absent fields remain readable and
  serialize without inventing expiry or changing absent-field snapshots.
- Finite ordinary Secret values use a distinct authenticated purpose and explicit
  format. Unknown-lifetime tokens preserve their ordinary encoding, including
  JSON-looking raw text. PG, filesystem, scoped legacy and shared owners carry
  expiry; unsupported timed writers reject before writing. Outer metadata cannot
  remove or extend a newly authenticated finite deadline.
- Leases carry their selected generation's deadline. Actual issuance and consume
  recheck after awaited storage operations. A native committed consume whose
  acknowledgement arrives after expiry returns no material and stays consumed.
  The existing legacy usage counter now rejects signed overflow before updating
  usage metadata or releasing material.
- Runtime staging retains the issuer deadline and its original independent
  monotonic cleanup deadline. Actual staging, cache reuse, failed-injection
  restoration and final injection check eligibility. Restoration cannot restart
  either lifetime or overwrite a later staged selection. The compile-time
  zeroizing-material guard remains in force.

## Exact acceptance and failures retained

Receipt: `target/validation/issuer-expiry-20261026/run-8/accepted.json`, SHA256
`0cbddca057ab195d383c4053ca1a7658b96825f754728d954e3792db351f34e8`.
It records the complete source manifest, 67 owned input identities, retained
executable hashes, actual commands, log hashes and six original model files.
Only a nonembedded Root journal update differed during R8; compiler inputs did
not drift. Later nonembedded reports/status updates are not new runtime evidence.

Screen R8 is terminal and absent; its completed receipt was verified against
current owned source and actual executable bytes. Cargo used the configured
canonical NVMe target and shared serial lock without an inline target override.
Strict six-package test Clippy passed in 84.90 seconds; the combined selected
test build passed in 217.52 seconds. All **738 distinct selected Rust cases**
passed on those actual retained images:

| Selected suite | Passed |
| --- | ---: |
| Auth library / public contract | 5 / 87 |
| Secret owners and shared reference/custody contracts | 70 |
| Host runtime / obligation contract / HTTP egress contract | 219 / 33 / 37 |
| First-party library / GSuite | 51 / 34 |
| Actual crate dependency boundary | 1 |
| Native Secret references, expiry and generations / custody | 10 / 14 |
| DCR registration and cleanup | 32 |
| Actual provider callback and refresh | 16 |
| Runtime account selection | 15 |
| Native account/flow/refresh | 19 |
| Filesystem AuthProduct, including native PG-backed filesystem | 73 |
| Credential lifecycle | 8 |
| Protected original HTTP and actual Chromium | 5 |
| Unchanged peer-owned model inspector with six frozen originals | 9 |

The protected browser path emitted `native-dcr-original-browser: passed`, with
503/200/202/200/200 responses, retained original IDs/expiry, visible unknown
cleanup and the blocked-popup link case. This is the actual selected component
and protected HTTP path, not acceptance of the entire Extensions page.

Three pre-existing Docker-only HostRuntime cases are **unexecuted**, not passes:
the two container proxy/broker launch configurations and the unconfigured scoped
mount rejection. Their exact names are in the receipt. The complete Docker and
platform/release matrix remains required outside this local scope.

R1–R7 diagnostics/results remain intact. R1–R5 found missing imports/constructor
handoffs and test integration issues; their repairs preserve checks and remove
no warnings. R6 exposed the new shared fixture consuming a lease through another
independent legacy adapter. R7 ran the remaining independent suites directly on
R6 images without Cargo and found a real new-account callback publication that
discarded its original deadline. R8 qualifies both connected repairs: consume
through the issuing adapter, and copy the retained deadline in the shared
callback publisher. The latter is exercised through actual failed publication,
restart and reference-only native recovery with exactly one issuer exchange.

## Remaining work and scope limits

This closes the implemented issuer-expiry boundary, not original task-family
auth grants, dedicated requirement-to-account selection, complete account-source
reclamation, Codex session/SDK migration, adjustable technical settings, other
protocols, K4–K7, general validator/composition migration or durable recovery.
K3 step4 already owns the next end-to-end binding work, including the newly
traced loss of the manifest credential handle and ordinal display-name hazard.
Do not create a second selection store or infer an original grant from a label,
reference, raw JSON key or account's mere same-provider existence.

No new global Monty startup, real issuer, Linux/Windows execution, general
catalogue/package activation, deployment or release acceptance is claimed.
Embedded contract changes still participate in K5's owning package reconciliation.
The peer inspector's separate acceptance stays with its Prefix owner; these nine
cases do not establish accepted-design generation or Kohai serving.
