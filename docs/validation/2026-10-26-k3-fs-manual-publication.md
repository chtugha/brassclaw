# Ordinary filesystem manual-token original publication — 2026-10-26

Status: bounded ordinary filesystem/manual-path acceptance; **not full K3, browser
setup, platform, release or full-plan completion**.

The existing encrypted SecretStore retains original input, account selection and
result. The interaction row CAS chooses publication against cancellation; the
account record retains its exact manual receipt alongside refresh/callback journals.
Unknown acknowledgements reconcile those originals, never today's account or a
replacement token. Outputs are retained, not blindly deleted on errors. Completing
or canceling a chosen whole-flow operation settles that original decision. The lower
interaction operation still publishes only its account/interaction, with separate
flow completion. No filesystem multi-record transaction or new store/queue is claimed.

Existing-account changes require the validated binding and account snapshot captured
before challenge creation. Unbound setups create a new interaction-derived account;
the submit-time latest-reusable-account update is removed. Legacy bound proof comes
only from the retained original flow. An unproven completed legacy flow cannot
accept a raw token by falling back to current account metadata.

## Executed evidence

308 distinct freshly executed Rust cases pass, with no failures or ignored selected
cases. Twelve new cases use actual encrypted memory and native PostgreSQL-backed
filesystem owners: before-write rejection and lost post-commit acknowledgement at
all six writes; independent same/different-input races; cancellation at decision,
account, consumption and flow boundaries; bound/legacy snapshots; restarted owners;
and the actual facade's raw-versus-reference behavior. All 53 other durable cases,
including the five snapshot/ABA cases, pass separately without double counting.
Current native manual/refresh/custody, OAuth, secret-library, public manual/WebUI/
callback/lifecycle, Prefix-source and both global Monty callers also pass.

Strict full/root/tests checks pass (73.63s), PostgreSQL without RootLLM passes
(11.14s), default Composition passes (14.46s), plus format and whitespace. Actual
library/test build:194.24s; public targets:4.93s. The worker command returned its
cached current image in0.45s; this is not a fresh-worker compilation claim.

All twelve final author hashes, complete applicable source membership/identities
(9,735 recorded entries) and eight archived executables were independently checked.
The previous snapshot and accepted peer teaching receipts are explicit prerequisites;
no prior test executions count toward308. R1's collapsible-if lint and R2's missing
explicit test-type import remain preserved failures; both causes were repaired
without warning suppression before R3 acceptance.

Receipt:`target/accepted-evidence/k3-fs-manual-publication-20261026/run-3/accepted.json`.
SHA-256:`0e2a3e59dd34cd30dd98389032c43b17888c7f690c6d3965287c135d23d3a2b4`.

## Remaining owner work

K3 step2 still owns generic/fake/raw-facade migration, first setup identity before
HTTP/browser transport, explicit reference-only continuation, DCR disposition and
complete workflow qualification. Issuer expiry, final credential-generation/grant
fences, compatibility drain/reconciliation and K3–K7 remain open. Prefix design/
serving belongs to the other chat; no provider host/configuration, Git/tag/push,
release or SSH operation was performed by this package.
