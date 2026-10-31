# Extension elapsed expiry and protected Chromium — 26 October 2026

Status: the previously open elapsed-expiry and real Extension browser acceptance
for [the connected original handoff](2026-10-26-k3-extension-originals.md) now
passes. Requirement-specific account selection, DCR custody, issuer access-token
expiry/final use, remaining caps and full K3/plans/platform/release remain open.

Receipt:
`target/accepted-evidence/k3-extension-acceptance-20261026/run-3/accepted.json`;
SHA-256 `1d9352802a3569a143d0563449aaefd0a02e5ed73b38aa99db4b4f42d354a5c8`.
Independent readback verifies all 10 current author and retained source-copy
hashes, both packaged executable hashes and the actual Chromium completion marker.
The existing Prefix pair-target9 receipt/all nine authors were checked and remain
unchanged. This package changes test/CI inputs, not production Auth behavior.

## Actual elapsed-expiry cases

Four actual-time cases pass against the existing Fake, native PostgreSQL and
memory/native-backed filesystem owners. A shared test-only contract creates real
pending and completed originals with the same future expiry and waits for that
exact saved timestamp. Past expiry:

- metadata retains the same pair, scope, definition and first expiry;
- Expired is a read-derived Extension disposition, without rewriting the retained
  AwaitingUser record or extending its expiry;
- original setup cannot accept a later requested expiry and raw submission cannot
  publish a new account or original/output secret;
- reference cannot invent a completed result;
- an already completed original still confirms its stored result, without raw
  replay or loss of effect evidence.

Both filesystem cases also reject the actual first flow-projection write after
the interaction was retained. The projection remains absent until after elapsed
expiry; recovery then repairs only that proven original projection. Subsequent
raw/expiry-extension/reference attempts remain rejected, account listings remain
empty for that scope, and actual secret custody/output references remain absent.
The same cases assert no cleanup deletion or hidden account effect. Native SQL
independently confirms two flows/interactions and exactly the already completed
account; actual native secret custody/output lookup is absent for the expired one.

These are manual setup-flow expirations. They do not implement or qualify the
separate issuer access-token lifetime/final-egress requirements in K3 step3.

## Actual protected Extension Chromium

The existing native lifecycle test renders the real ConfigureModal, manual-token
controller, API, Query client and i18n through the ordinary protected WebUI bundle.
It uses the bundled NEAR AI manual requirement and the actual pre-existing account
created by its native HTTP setup. No fake chat/run/gate, saved state, persistence
owner, issuer request or model dispatch is introduced. Native assertions also
require the model gateway to have received zero requests.

Fixture middleware cuts only acknowledgements after the actual Product future
and native operation complete. The browser observes:

`503 → 200 → 503 → 200 → 409 → 200 → 200 → 503 → 200`.

It proves retained metadata before first transport, full reload with empty input,
original bound target/pair/invocation/first expiry, changed raw rejection after
completed publication, explicit token-free confirmation, and a deliberate new
bound setup canceled after its own setup acknowledgement is lost. Counts are
setup5/raw2/reference1/cancel1/generic0. The token intended for the canceled setup
is never transmitted. Safe storage/visible-output checks exclude all
three fixture token strings; audit records retain only correlation, field names
and privacy booleans, not raw values or fingerprints.

Native readback verifies both original flows, their exact retained binding to the
same account, preserved first expiry, completed versus canceled status and
continuation disposition. Exactly one account remains. The driver must emit
`native-extension-original-browser: passed`; an env-free native test is not browser
evidence. CI runs the same explicit fixture/driver through the existing native
browser job. Browser CLI0.27.1 and actual Chrome149 execute locally; no Windows/
Linux installed acceptance or Extension Tool egress/Recipe re-execution is claimed.

## Evidence reuse and preserved failures

R1 runs all 47 common Node cases: the earlier Chat/API/Extension cases46 plus the
new authoritative expired-ack controller case. Driver and both inline script
syntax checks and the actual CI step parse pass. R1 then fails on a test-module
import, repaired using the actual crate re-export without suppression.

R2 passes strict affected Root/Composition test lint44.38s, the actual library
build200.66s and all four elapsed-expiry cases (6.86s,7.92s,12.01s for the two FS
cases together). Its Runtime startup fails before Chromium because the reused
worker was not placed beside the archived test program. `installed_worker()`
intentionally resolves that packaged sibling rather than an environment/PATH hint.
This was a validation-package layout error, not a repaired production gate.

R3 verifies unchanged full relevant source membership/config and both actual
binary hashes, then copies both immutable executables into the proper sibling
layout. No Rust recompilation is performed. The real Chromium/native case passes
in46.32s; format4.55s and whitespace pass. R2's strict lint/build/four expiry cases
and R1's47 Node/HTML/driver/CI results are retained with exact input checks, not
claimed fresh in R3. The old failure archives are unchanged.

The worker is reused from accepted Prefix9, SHA
`a617985957ccb8c5f3f0263f3ffe9135d9e352a86b8235d4bf5e39eba7e431a8`;
the new Composition test image is
`efb15bf2082f33f4fae617ae5effabf29e181bfb7382566fd536e0962c895482`.
The only source successors since that peer acceptance are explicit test/CI files;
both changed owning files' production prefixes are byte-checked against Root33.
Cargo manifests/config/lock, worker sources and native protocol are unchanged.
Historical public/global cases remain scoped prerequisites, not fresh executions.
All Cargo work was serial in Screen under the common configured target/lock;
no clean, second target, warning suppression, Rust-loop fallback or deployment.
