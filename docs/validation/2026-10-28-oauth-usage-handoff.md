# Extension OAuth usage handoff — 28 October 2026

Status: **scoped local acceptance passed; K3 step4 and the complete plans remain open**.

## Implemented boundary

New Google/DCR Extension setups resolve the complete requirement through the
existing Product Lifecycle facade and retain its content-based identity, explicit
New/existing-account target and previous selection in Auth original custody before
the challenge. Provider/scopes/label alone cannot select an account. Recovery and
cancellation verify the accepted original before consulting current declarations;
changed targets conflict and removed current metadata does not recreate a setup.

DCR's encrypted preparation context has a versioned usage-bearing form. Absent
usage retains the historical serialized shape/context; old originals are not
backfilled with selection authority. Native PostgreSQL and record-filesystem
callback publication consume the retained usage and previous head through their
existing owners. A repeated old callback neither exchanges again nor overwrites
a newer selection. No new catalogue, provider transport or pending registry was
introduced.

WebUI originals version 2 retain the explicit target. A displayed account change
cannot retarget an in-progress original. Legacy version-1 correlation remains
readable without an implicit upgrade; an explicit new setup uses the new contract.
Only redacted correlation is stored in the browser.

The connected compile repairs remove the obsolete implicit setup-account lookup's
exact-scope switch; unbound runtime ambiguity remains an error. Admission and Monty
now share the unchanged scope conversion from the transcript owner, available to
PostgreSQL test-support without skills-db. Supervisor-only error variants and
native HTTP fixtures have the same feature boundaries as their actual consumers.
No lint allowance or fabricated use was added.

## Evidence and reuse

Receipt: `target/validation/oauth-usage-20261028/run-6/accepted.json`, SHA256
`41a43fa8b722de674db140b893a79ad4823ec286571f22f15718c74474a64a55`.
The 29-file owned manifest SHA256 is
`065f467e71d2da48353bc4e87a594bd2f77ea5c1d7ed4dc9eac15d3e58b51bb6`;
complete selected-source inventory SHA256 is
`a992c191f9f2fb7e3f902170c56d952d015e86da690fcbba2ee06742a7842917`.
Originals and correction manifests remain under
`/private/tmp/brassclaw-oauth-usage-20261028-lkgmsur3/`.

| Evidence | Actual result |
| --- | --- |
| Fresh R6 Rust | 317: Product contracts 114; OAuth routes 19; native Auth 48; record-filesystem Auth 85; DCR regressions 32; runtime account selection 16; atomic transcript admission 2; global Monty Match/No-Match 1. Zero ignored. |
| Applicable R5 Rust | 187: Auth units/contracts 110 and Product units 77, actually executed with identical selected sources/images. These are reused results, not fresh R6 executions. |
| JavaScript | All 45 Extension cases fresh in R6, zero skipped; changed JS syntax checks pass. |
| Actual browser | The native PostgreSQL-backed original-setup component test executes Chromium through the pinned agent-browser driver. The required pass marker and observed HTTP sequence 503/200/202/200/200 establish lost-response recovery, reload, popup-blocked link and unknown-cleanup reconciliation. It is included in the 19 route tests. |
| Compilation/lint | Auth, Product and Composition test images and the actual worker were built serially from the common root. Full-feature affected lib/tests strict Clippy passes in R4. Default Composition lib/tests strict Clippy passes on the current source in R5. Formatting and final whitespace checks pass. |

Total: **504 distinct Rust cases and 45 JS cases**, including 317 fresh R6 Rust
and 187 applicable R5 Rust results. Chromium is a real nested acceptance scenario,
not an additional invented Rust-test count. External issuer egress is controlled;
real PostgreSQL/Auth/Secret/record-filesystem owners and public callback routing
are exercised. No live Google/Notion issuer or complete installed product is claimed.

R5 changes only two feature predicates from the R4 build: Busy/Deadline are
skills-db-only, and the native HTTP test module requires test/postgres/skills-db.
All predicates are true in the recorded full-feature build. Exact whole-file
comparison verifies these are the only differences, with no body, call, data or
compiled full-feature item change. R5 therefore retains the actual R4 images and
full-feature lint with this explicit applicability proof; default lint is fresh.
R6 verifies its complete source inventory is identical to R5 and reuses those
artifacts/checks, plus the 187 actual passing R5 cases. Image SHA256:
`efa519dcbfcc74080fe13a78ba2b6ae44c55163bc3a73156b099e9c9c7d747d2`;
worker SHA256:
`a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.
Both are retained in run-4; no mutable mirror is used for building.

R1 stopped at the mandatory capacity gate before Rust compilation. R2 diagnosed
the obsolete selector method; R3 exposed the scope-helper feature dependency; R4
exposed the remaining standard-feature warnings. Those failures remain preserved.
R5 compilation passed, then its direct Product test launch used the repository
root instead of Cargo's crate working directory. The source-file boundary test
correctly failed its file lookup: 113 passed/1 failed. R6 corrects only the runner
working directory and executes all 114 Product contracts successfully; the failed
case is never counted as passing R5 evidence.

The inherited canonical target and shared Cargo lock were used in Screen, with
no parallel Cargo or target override. Check metadata without sibling rlibs was
cleaned only when required by the capacity rule. The previous Google integration
image was retained byte-for-byte before its two old build-volume links were
removed; the historical private reference remains readable. Relocation and cleanup
inventories remain with the failed/continuing runs. No entire target was cleaned.
At completion no Screen/runner remains, the lock is free and the target volume
reports 57 GiB free/72% usage. Both Prefix manifests/all eight owned source files
remain unchanged.

## Exact remaining work

This accepts setup/callback metadata and the scoped original-controller browser
path, not complete runtime credential grants or K3 completion. Continue in the
existing K3 step4 owners:

1. Persist Google PKCE/provider originals through the encrypted provider/Secret
   owner; the route verifier cache remains process-local. Migrate generic Google
   start to caller-retained original IDs.
2. Reconcile DCR preparation written before an Auth flow exists. In particular,
   expiry after encrypted preparation but before prepare_dcr_registration can
   leave a retained preparation without a recoverable flow. The new pre-discovery
   expired-input rejection does not close that interrupted boundary. Recover the
   protected first definition/material and terminal retirement through the same
   original owner, without another provider registration or placeholder success.
3. Complete original task/family/child/wait requirement and authenticated material
   capture, conditional lease issuance/consumption and verified refresh lineage.
   Current-account enumeration is not original-family selection. Reuse the existing
   conditional record-reference issuance contract rather than another snapshot
   registry or a read-before/after lease check.

Full Extensions-page/global-startup browser acceptance, live issuers, platform,
deployment/release and remaining Prefix/Validator/Composition/K3–K7 gates stay open.
