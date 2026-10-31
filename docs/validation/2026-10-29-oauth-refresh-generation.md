# OAuth refresh input generation — scoped acceptance

29 October 2026. The current refresh consumer passes 652 distinct Rust cases,
zero ignored, actual Chromium acceptance and affected connected/default strict
checks. The final run executes the two new native generation cases; 650 positive
results are explicitly reused after reviewing the exact three test-only changes.
This is local development acceptance of this consumer, not full K3, deployment,
general task-family grants or live OAuth issuer acceptance.

## Implemented boundary

`OAuthProviderRefreshPreparation` carries deterministic output destinations and
the selected `OAuthRefreshSource`: exact resource scope, handle and authenticated
backend-owned encrypted record reference. The reference is opaque; it is neither
normalized nor assumed to be a fixed-length digest. Debug output redacts it.
Callback preparation keeps its separate existing contract.

The existing durable refresh action retains this source. Only its winning
`Dispatch` admission reconstructs the provider request/preparation from that
retained action. An earlier or later capture cannot replace it. `same_intent`
includes the source, preventing a completion from attaching to another selection.
The PostgreSQL owner authenticates the source in the original intent transaction.
The record-filesystem owner performs its real source preflight before intent CAS;
these are explicitly separate records, not a cross-record atomic transaction.

The actual Host provider uses `lease_once_if_record_reference` and the existing
consume-time generation, expiry and revocation checks. Replacement before native
admission rejects without an intent/effect. The tested replacement after admission
before lease, including same-plaintext ciphertext ABA, and mutation between lease and consume cannot reach
the transport. A lost admission acknowledgement reads the original; it never
issues another refresh. Confirmed/unresolved effect custody remains authoritative.

Legacy actions without `source` remain readable/reconcilable without backfilling
today's material. They cannot gain a new provider dispatch. Source metadata,
account snapshots and content references remain custody facts, not grants.
Request/account scope equality is checked before preparation and again when
reconstructing dispatch; copying the validated account scope is not substitution
of another request scope.

## Actual checks and reuse

| Scope | Distinct Rust cases |
| --- | ---: |
| Auth library and public contracts, including legacy codec | 120 |
| Product workflow library and public contracts | 207 |
| Existing Host provider/exchange cases | 16 |
| New native PostgreSQL and record-FS generation cases | 2 |
| External callback/lifecycle/public WebUI contracts | 57 |
| Native PostgreSQL Auth | 51 |
| Record-FS Auth | 88 |
| Google gates and provider composition | 13 |
| OAuth routes, including actual Chromium | 64 |
| DCR originals | 32 |
| Architecture dependency boundary | 1 |
| Current global Monty Match/No-Match lifecycle | 1 |
| **Total** | **652** |

Each new native case runs six scenarios through actual Host/provider code and
real encrypted account/secret stores: replacement before admission, replacement
after admission, ciphertext ABA, replacement before consume, lost begin
acknowledgement and valid refresh. Independently recreated native owners reread
original results after source deletion/change with an unavailable provider.
The existing `RecordingEgress` observes transport calls and supplies responses;
it is an in-process fixture, not a live external OAuth issuer. The real browser
case belongs to the existing route suite and is not counted as an extra Rust case.

Root R1 passed Auth120/Product207 and 16 provider cases. Its two new fixtures
failed at SecretHandle construction because scenario Debug names contained
uppercase letters. R2 passed the remaining 306 cases and retained that diagnostic.
The reviewed fix only lowercases the test identifier. The other two changed
inputs are Prefix's Oracle test and separate Python test helper. Production
sources/dependencies and unaffected caller bodies are unchanged. R3 runs the two
corrected native cases successfully in 3.96 seconds and reuses the current R7
global case exactly once. Failed runs remain failed; positive cases are attributed
separately, without replaying completed effects or rerunning unaffected suites.

The combined R7 block compiled the updated Composition test image and checked
affected connected/default configurations with warnings denied. Actual features,
toolchain, profile, producer commands, original images and applicability are in
the referenced receipts. Auth/Product images and the worker were preserved rather
than unnecessarily rebuilt. No Root Cargo execution was needed for these runs.

## Retained identities

- Final receipt: `/private/tmp/brassclaw-refresh-generation-validation-20261029/run-3/accepted.json`,
  SHA256 `838080ab50d1af4ef5d6a003eade4ca96d50ae1dc6d58462db566e252fe8e7d6`.
- Current input inventory: `6c96ebe758e52467b4fea9cfa0cc61d1d7155c23791a7492b0d3c9749b739c36`.
- Owned20 manifest: `2aadcec40bb041be8581179cd6f9033e2078260ffa9d4ed3573377c28e34613f`.
- Root36 current inputs: `03fb65b20037107f9df06dcd680c9ce2ccc30aad5bb012dd8f7fb13749a6b41c`.
- Actual Composition image: `55356202e6f97d4e9f4bb2ad7df0ac14531f3739260442c84936473a1b468f5b`.
- Actual sibling worker: `91a2165fa396dfd9513d15e886eaba2ed692f1a5b9ac472ef3fb734391e47343`.
- Combined R7 receipt: `/private/tmp/brassclaw-final-counter-validation-20261029/run-7/accepted.json`,
  SHA256 `a2af2da3849c997a08b58c843b88845686e8a485663401862aa3021875dfca51`.

These are retained local development archives, not portable release artifacts or
a guarantee of indefinite temporary-directory retention. Earlier Exact24 and
Google receipts keep their historical bytes and narrower claims. This source has
no new deployment, component activation, upgrade or platform acceptance.

## Remaining work

General Tool credential selection still needs original task-family/child/wait
capture, explicit absence, authenticated material generations and verified own
auth/refresh continuation through the existing native admission/host/broker owners.
Current policy, expiry and cancellation must remain effective through final use.
This refresh repair does not close that broader contract. Prefix Pair/Profile/
Generation acceptance, ordinary role attachment, remaining K3–K7, recovery,
upgrade/platform and complete-plan gates retain their existing owners.
