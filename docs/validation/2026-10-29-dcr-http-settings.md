# Acknowledged DCR HTTP settings — 29 October 2026

DCR server discovery, client registration and explicit cleanup now use
`oauth_dcr_timeout_millis` (default 30,000ms) and
`oauth_dcr_response_body_bytes` (default 32KiB). V189 extends the existing execution
settings/default/constraints, preserving explicit and partial overrides and
advancing only missing-field rows. Values are independent of token exchange/
refresh limits. Both clients reuse the same acknowledged snapshot reader and
normal factory-owned settings source; a configured unavailable owner never falls
back to defaults. No second settings registry, timer, workflow or authority exists.

Each new HTTP request captures one coherent acknowledged pair. An already
accepted GET retains its pair through an edit; the later authorization-server GET
(including legitimate fallback) captures the newer pair. Registration and cleanup
capture before consuming their respective durable one-shot claims. A missing
owner leaves a registration Prepared. Already submitted/unknown
request readback neither recaptures limits nor permits a resend. A genuinely new
cleanup request captures its current limits before its own claim.
The existing response guard also supplies the estimated-request egress limit.

Actual effective status and EN/DE WebUI controls/help expose both values, with
explicit overflow/no-resend semantics and the separate setup/browser/server
limits. The changes do not alter DCR lifetime/cache expiry or canonical Provider,
model/design/prefix/component approval, original metadata/credentials or routing.

## Evidence and applicability

[Machine receipt](2026-10-29-dcr-http-settings.json) SHA256 `8047db397a678a4f4a730b737cb8e4e48fd2f7104926db4e4b94fb16f981bbdb`.
Current source manifest SHA256 `c86a3c723659b865bd38e51e22380245bfa06043829381b1b3b71344570ae35a`.
Actual R2 Composition image `056037900ec13d9e5102d7e18bf76f426f41fc2203c14f0d88442dbef4d78b8b`;
actual current worker `ece271358a476130cb09d695e321eb485e00e1670e89a4704aad4098dee4cb7c`.

R2 freshly passes 33 DCR cases/0ignored and current connected lib/tests plus
default lib strict Clippy. The new native case starts the actual global runtime,
changes real acknowledged PG settings between two requests, verifies complete
old/new pairs and original registration identity, exercises real PG Auth Prepared/
Unknown claims and immutable original readback, and forwards POST/DELETE through
the actual host/network stack. All four actual egress returns are policy denials
for private issuer destinations; no successful issuer response is invented. Cached
issuer metadata is a data fixture, not registration evidence. Existing DCR
protocol/state fixtures retain their controlled scope. This is no live external
issuer or cleanup certificate.

R1's unchanged owners retain 480 neutral Host/Product cases and 69 further cases:
19 token-client, 7 provider-composition, 32 settings migrations (including real
V189), 10 incoming Exact model/setup-owner cases and 1 native global Match/No-Match
case. Actual Chromium on the real native facade/PG/global service verifies both
new fields, EN/DE help, save/effective revision and restoration. Final request/
phase wording is source-reviewed/syntax-checked; control/data behavior is unchanged.
These results keep R1's original image/source; they are not represented as R2
executions. R1's old grouped discovery probe and DCR group are superseded and not
counted again. Across scopes 582 distinct cases are retained, with zero ignored
in selected executions. Worker build, relevant lower strict, Rust formatting and
JavaScript syntax pass. Final source/artifact/worker/log identities, runner exit
and free common Cargo lock are independently verified before handoff.

All eight incoming [Exact setup-draft](2026-10-29-exact-model-setup-draft.md)
afterhashes remain unchanged; V188 and earlier migrations are untouched. The
combined scoped inventory has 182 paths. Source edits use guarded, fsynced
same-directory atomic replacement. A development edit guard stopped on a format
mismatch; its partial planned write was verified before completion. No compiler/
behavior failure, warning suppression, cache cleanup or production-guard weakening
was needed. Final whitespace review also resolved three pre-existing blank locale
separator lines with spaces. This changes no JS statement/key/string or line
count; final Node syntax passes and the exact tested/final source projection is
recorded separately. R2 images retain their actual original source subjects. The final per-request refinement precedes acceptance.

## Remaining work

K3 still owns DCR setup/cache lifetime, separate ProductAuth server/browser waits,
setup/ingress technical limits, complete own-flow/family/refresh/Auth-wait and
Codex session/controller/factory cutover. Exact HTTP/UI/Use/correlated autoimport,
design/review/Free and the full plans/platform/release/deployment remain open.
Reuse this completed HTTP settings path. No commit/tag/push or reinstall occurs.
