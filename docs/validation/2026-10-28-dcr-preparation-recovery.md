# DCR preparation recovery — 28 October 2026

Status: **scoped local acceptance passed; K3 and the complete plans remain open**.

## Implemented boundary

The existing Auth and encrypted SecretStore owners now reconcile a DCR
preparation that survived a rejected or unacknowledged first Auth-flow write.
Protected recovery verifies the complete explicit original correlation before
repairing a projection or cancelling it. Changed requirement, target or expiry
conflicts without a new flow or provider effect. Recovery does not require the
current Extension declaration.

An authenticated preparation and a missing flow alone are insufficient evidence
that no registration happened. First insertion checks the original generation
and absence of every later DCR artifact, then arbitrates through the existing
absent-flow CAS. Later artifact history prevents reconstruction and replay.
An existing exact flow wins unchanged; uncertain writes read back its original.

Expired or explicitly cancelled preparations can acquire a real terminal
projection with their original IDs, requirement, target, previous head and expiry.
This projection has no challenge, selected credential or dispatch authority. A
bound account need not still exist. The deliberately missing account snapshot is
accepted only for this suppressed, prepared, not-registered terminal state;
turning it into a pending state fails. The ordinary pending-flow expiry guard
remains effective. Existing cleanup retires the exact encrypted preparation and
retains its authenticated retirement receipt.

Live recovery restores the original pending projection without discovery,
registration or fresh PKCE. A subsequent explicit Start uses that preparation
for one registration; cancellation uses the existing cleanup path. Repeated
terminal operations do not repeat provider I/O or emit a successful continuation.
No new registry, workflow Tool, Rust-loop fallback or approval bypass was added.

## Evidence and applicability

Receipt: `target/validation/dcr-preparation-20261028/run-2/accepted.json`, SHA256
`eb73779777561562f60917ae00813229fd3f0f64056ecb8862fea182331f5e2c`.
The current 23-file owned manifest SHA256 is
`20818edc28dcaf96e298c70fd212ebd780a8c597287aa0cba5245945a4798e22`;
the complete selected-source inventory SHA256 is
`e243ae9a62e09eb005a73290eb6f13c6427d99235e2089d0cfdb266c5fd9b804`.
Originals, correction manifests and the exact applicability proof remain under
`/private/tmp/brassclaw-dcr-preparation-20261028-jyvc67ip/`.

| Evidence | Actual result |
| --- | --- |
| Fresh R2 Rust | 187: native Auth 49, record-filesystem Auth 86, HTTP routes 19, DCR regressions 32 and ordinary global Monty 1. Zero ignored. |
| Applicable current Auth batch | 112: library 5, public Auth contracts 95 and DCR contracts 12; actual current images and strict all-target Clippy passed in `auth-only-1`. |
| Applicable current Product batch | 207: library 77, Auth-interaction contracts 16 and Services contracts 114; actual images and strict lib/tests checks with `root-llm-provider` passed in `product-only-1`. |
| New native interruption cases | Both real storage backends exercise rejected first flow insertion, actual elapsed expiry, cancellation after deleting the selected account, live recovery without current declarations, competing inserts and later-artifact rejection. Each backend also cuts one actual terminal-write IdleReady acknowledgement and verifies exact readback and original retirement. These cases are included in the 49/86 totals. |
| Actual browser | The HTTP suite executes Chromium and requires the browser pass marker. Observed HTTP sequence: 503/200/202/200/200; original identity/expiry, popup-blocked link and visible unknown cleanup pass. Included in the 19 route cases. |
| Compilation | Auth/Product images, actual worker and Composition image built serially. Strict affected full-feature Composition lib/tests passed in R1; current standard-feature lib/tests passed fresh in R2. No warning suppression. |

Total: **506 distinct Rust cases: 187 fresh R2 and 319 applicable earlier
executions in this package**. The earlier Auth/Product batches are reused results,
not additional fresh R2 executions. Their receipt SHA256 values are respectively
`5101c03e29203b117e29cc4a947dcf631c5d6522e4965e57aad1e131a03ceedc` and
`657f563db6440210a20de240b28c2bdebd688d0f05a7ccc9b7b8e7b8ffc147e4`.
No JavaScript source changed; the actual browser scenario ran fresh.

R1 built the final embedded-document and six generated-Skill-prose changes plus
the Prefix owner's budget-receipt correction. Its subsequent standard-feature
test check correctly exposed two missing symbols: the new PostgreSQL test could
not reach a helper unnecessarily gated by `skills-db`. R2 changes exactly four
test files: the helper/re-exports follow their actual PostgreSQL parent, while
only the record-filesystem enum variant and match arms require `skills-db`.
The PostgreSQL test remains available with standard features. No function body,
production behavior, assertion or test selection was removed to obtain a pass.

The recorded full-feature test build has `test`, `postgres` and `skills-db` true.
An exact whole-tree delta and textual-transformation check proves identical
selected full-feature items and bodies. Auth/Product dependency libraries do not
compile Composition's test module. Their results, full-feature lint and actual
R1 images therefore remain applicable; the changed standard-feature boundary is
checked fresh. This is explicit applicability reuse, not a claim that R2 relinked
the image. Proof: `run-2/feature-gate-applicability.json`, SHA256
`0bdbc355ae17da3f5d49dcb1df03bf186bd826d6dce439ea2b0533d854351594`.

Actual retained images in run-1:

- Composition: `1bd7cc04c6d59c3de63fde571c271e35e631bacd03822519c7a235cbd1388e5e`.
- Worker: `a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.

Earlier Prefix R3 images became stale when embedded documents changed and were
used only for diagnosis. Those diagnostic passes are not counted here. All failed
checks, old manifests and outputs remain preserved. The final run verified source
and Prefix-manifest stability, positive test selections and final whitespace.
The canonical inherited target and shared Cargo lock were used in Screen.
Runner/process handles are absent, no Screen remains and the lock is verified
free; the target volume reports 53 GiB free/74% usage. No cache cleanup, commit,
push, release, deployment or remote compilation was needed for this package.

## Remaining work

This closes the interrupted DCR pre-flow preparation boundary identified in
[the earlier OAuth usage handoff](2026-10-28-oauth-usage-handoff.md). It does not
complete K3, all component-prose qualification or the broader plans.

Continue with durable Google PKCE/provider-client originals and caller-retained
generic Google setup IDs in the existing provider/SecretStore/Auth owners. Then
finish original task/family/child/wait requirement and authenticated material
capture, conditional lease issuance/consumption and verified refresh lineage.
Current-account enumeration is not an original-family selection. The existing
`OAuthClientMaterialSource` and conditional record-reference lease port are
available building blocks; do not add another registry or substitute current
provider credentials for an old setup.

The Prefix owner's workflow acceptance, final formatting/serving guards, general
pure approval, validator/subagent migration, recovery/upgrade, live issuers, full
product/browser, platform and release gates remain open. External issuer replies
in these tests are controlled; the PostgreSQL, SecretStore, Auth, filesystem and
HTTP owners are real. The ordinary global startup regression is scoped evidence,
not complete qualification of every changed packaged workflow.
