# Google Extension OAuth original identity — 28 October 2026

Status: **scoped local acceptance passed; K3 step4 and the complete plans remain open**.
Work started on 27 October and completed on 28 October. The existing Product/Auth
owners implement this correction; no Recipe loop, catalogue, pending map or
provider transport was added. Prefix implementation remains with its other chat.

## Result and source

Google Extension setup now requires the browser-retained flow ID, retains the
first accepted state/PKCE/expiry/update binding, and supports Start/Recover/Cancel
through the existing original controller. Auth-facade recovery authenticates the
stored flow and its state/label/scopes before current configuration or account
lookup. Changed explicit definitions conflict; missing recovery/cancellation
cannot create a flow. Cancellation and repeated starts retain terminal outcomes.
A concurrent creator cannot overwrite the accepted verifier with its losing
candidate. A lost native initial-write acknowledgement reconciles the actual
original before installing its matching verifier. Responses expose no URL for a
terminal/expired flow. Google uses no DCR remote-registration cleanup projection.

The configure dialog now uses the same original controller for Google and DCR.
The old Google-only hook/API producer is removed; actual consumers and pairing
regressions were checked. Browser reload preserves the original ID and expiry,
and session storage contains only safe correlation metadata, never URL/state or
credential material. The controller test harness now associates each inline
button with its following label, including the existing Check button.

The ten-file source manifest and original copies are retained at
`/private/tmp/brassclaw-google-original-qs1ek7y2/`; manifest SHA256
`7325c0d5d493c3123d5385354f174e31b63c6b3dc8bea045facf55167fb144be`.
The full compiler-input inventory is in the accepted run. Both Prefix manifests
and all eight of their source files were verified unchanged.

## Acceptance and honest reuse

Receipt: `target/validation/google-original-20261027/run-3/accepted.json`, SHA256
`242adaba6b1a41db434bd22d7d388b9db365b96f8991f560ec0010dfbdfca740`.
One configured Cargo target and the existing serial lock were used in Screen;
no target override, parallel Cargo execution or warning suppression was used.

| Evidence | Actual scope |
| --- | --- |
| Fresh R3 Rust | 86 cases: real native autocommit-INSERT acknowledgement loss 1, record-filesystem Auth 84, global Monty Match/No-Match 1. No ignored cases. |
| Reused actual Rust | R1 protected OAuth routes 19 and R2 individually passed native Auth cases 46. Source comparison proves only the repaired final test body changed; production and other test inputs remain identical. The failed R2 case is excluded and its repaired successor runs fresh. |
| JavaScript | All 43 Extension cases fresh in R3, including the Google original controller; syntax checks pass for the four affected JS/test files. These are controller/library tests, not rendered browser acceptance. |
| Compiler/lint | Fresh R3 full-feature affected lib/tests strict Clippy, standard library strict Clippy, formatting and whitespace pass. |
| Current actual artifacts | Composition test image SHA256 `23b6d5d811282f3dd284eca227acd1834174779dbae9f6e8aa75e5ec5f5e0ecd`; unchanged actual worker `a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`. Current executables are retained under the private package directory. |

Total: 151 distinct Rust cases and 43 JS cases, **not 151 freshly executed R3 Rust
cases**. R1 stopped on a wrong native test selector with zero selected cases;
that was not accepted. R2 ran 46 native cases successfully but its new loss test
correctly failed because an explicit COMMIT cut never fired for this autocommit
INSERT. R3 uses the real bound initial-flow payload and IdleReady boundary;
`cut_count == 1` remains required. The real tokio-postgres driver awaits that
ReadyForQuery after CommandComplete, so the test exercises actual lost write
acknowledgement. No SQL result or provider outcome is fabricated. R1/R2 logs,
source identities and failures remain preserved. No product-code correction was
needed after the initial strict/runtime pass; only the test boundary changed.

The new shared route scenario executes through in-memory, native PostgreSQL and
native Postgres-record-filesystem Auth owners. It covers invalid/missing IDs,
repeated start, changed definitions, missing/foreign original, recovery after
configuration removal, cancellation, terminal no-rearm, concurrent first starts
and retained matching verifier. It does not contact an external Google issuer.

During the build the mandatory 90% disk threshold was crossed. The current test
and worker images were retained, then only 965 disposable check-metadata files
without sibling rlibs were removed (1,020,206,817 bytes), restoring 90% before the
next Cargo command. The exact inventory is in
`capacity-check-metadata-clean.json`. Default strict subsequently left 19 GiB free
and 91% usage; reapply the existing capacity gate before another compilation.
No executable, archive, runtime evidence or full target directory was cleaned.

## Required continuation in the existing owners

This correction does **not** complete OAuth requirement grants. Carry the complete
server-resolved requirement, explicit target and previous-head custody into the
first Google/DCR original; recover before any current-manifest lookup. Existing
legacy originals must not acquire grants by backfill or label/provider inference.
Reuse the already implemented native/FS OAuth head publishers.

Google setup PKCE remains in the existing process-local route cache. The surviving
flow/hash is not a restart-safe verifier original. Replace that boundary through
the existing Google provider/SecretStore owner and immutable original/retirement
contracts, including independent creators, lost acknowledgements and callbacks.
The generic non-Extension Google route's new-ID behavior is not migrated here.
No external issuer, full HTTP/browser, deployment, cross-platform or complete
Original-family/material-generation consumer acceptance is claimed.

Then complete original task/child/wait requirement and authenticated material
capture, conditional lease issuance and verified refresh lineage in the existing
admission/Auth/Secret owners. Runtime's current-account enumeration and
unconstrained lease are still open. No whole-plan completion is established.
