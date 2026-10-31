# Original Auth-flow selection: scoped metadata-owner acceptance

29 October 2026. Four actual Rust cases/zero ignored from the shared compiled
Composition test image. This is historical **account-selection metadata** only,
not encrypted-generation, live credential eligibility, task-family/child,
native auth-wait, K3 or complete-plan acceptance.

## Boundary

The existing CredentialAccountRecordSource now exposes the readonly
requirement_selection_for_flow port, with unsupported owners closed by default.
Actual in-memory, PostgreSQL and record-filesystem Auth owners read the requested
protected original flow/custody and reuse the existing complete requirement and
receipt validators. They do not query a mutable selection head, enumerate today's
accounts, create another flow, publish a selection or grant runtime access.

Only a coherent completed original setup/selection returns its historical
account metadata. Pending/staged, failed/cancelled or unrelated historical flows
without exact metadata do not manufacture a selection. Half-present completed
custody fails. Wrong owner, original flow ID and full usage cannot substitute
another selection. Current account eligibility, authenticated encrypted source,
own-flow/gate/claim correlation and actual dispatch must be established by the
future consumer separately.

## Actual observations

The same verifier ran through four actual owning adapters:

| Owner | Actual result |
| --- | --- |
| InMemoryAuthProductServices | 1 passed/0 ignored, 0.009s. |
| Native PostgreSQL Auth records | 1 passed/0 ignored, 2.015s. |
| Memory-backed versioned filesystem Auth records | 1 passed/0 ignored, 0.015s. |
| Native PostgreSQL-backed filesystem Auth records | 1 passed/0 ignored, 1.983s. |

Each case checks pending state, completed A selection, a later B head, exact A
and B original-flow readback, full-usage mismatch, foreign owner, unknown ID,
cancelled and historical missing-proof flows, and malformed completed custody.
Fixture account handles are metadata sentinels; these tests neither lease token
values nor claim those handles constitute working encrypted credentials.

Root applied eight reviewed files in Exact K3-2's explicit apply-only window,
then returned that window without running Cargo. The owner built the common
checkout and supplied the retained image. The first combined build caught one
Root test typo: AuthFlowStatus::Cancelled, while the actual Auth variant is
Canceled (TurnStatus uses Cancelled). The owner corrected only that enum spelling.
Original proposals/logs are preserved; never restore them. No ignore, warning
allowance, fallback or production semantic change hides the failure.

Root ran only these four isolated metadata cases in Screen from the immutable
actual image, without Cargo or parallel runtime/model/provider tests. The image,
complete source inventory, exact eight subjects and logs were hashed before and
after execution. Own runner5162 is actually gone and Screen exited0. This is no
claim that the shared Cargo lock is free: Exact retains Source/Cargo and its
remaining combined runtime/strict checks continue. Final affected Auth strict
lint is not yet claimed by this receipt.

## Exact evidence

Receipt:
`/private/tmp/brassclaw-original-auth-selection-validation-20261029/owners-1/partial.json`
SHA256 `613e5baf807e30bd7d26d6496a07bda4d7589eb48ac96442724c836ec54cd676`.

Actual retained test image:
`/private/tmp/brassclaw-exact-capture-seam-validation-20261029/run-3/composition-tests`
SHA256 `d6c4d72dd90ea561097480b06925025bc0ab4b3205855340d27443dc88224f6a`.
Its default/postgres/root-llm-provider/skills-db/test-support build includes
tracing-test/no-env-filter. Complete source inventory SHA256
`50430602d5c708e7cbb6b665d6149bc96fca18bc8837c1cf88e57117827e9b55`.
Exact eight-subject inventory SHA256
`c101cb9454b0fc4381e9c9bf9c09fb42e9285a612b4d503daad27319757f98ef`.
The corrected custody-test source SHA256 is
`c84f9e4f36955ad428532e11d15ec21b6d433edd56df572d36bff4f904618d09`.

The original8 manifest/applied record and corrected successor are kept distinct
under `/private/tmp/brassclaw-original-auth-selection-20261029-0nhxwya8` and the
receipt. The current Root85 pointer remains its previous scoped apply record
until Exact's explicit final handoff; it is not this new combined compiler
receipt. No commit, push, release, remote deployment or complete-plan closure.

## Remaining consumer

Capture original source/absence with the task before effects; keep its original
previous selection when starting its own Auth setup. Read this exact completed
flow instead of today's mutable head. Bind actual source views to retained
native services, verify current eligibility/conditional generation use and own
flow/child/claim fencing, and continue the original native invocation once.
Consume Exact's prepared selection-before-runtime-binding seam; do not create
a second Factory capture or extend model metadata with arbitrary Tool secrets.
The private3 HostRuntime credential-service draft is not applied or qualified
by these4 cases. Ordinary global/UI/material/source-family acceptance remains open.
