# Acknowledged OAuth token transport limits — 29 October 2026

OAuth callback/token-refresh clients now capture `oauth_token_timeout_millis`
(default 30,000 ms) and `oauth_token_response_body_bytes` (default16 KiB) together
from the existing acknowledged Monty settings owner before network dispatch.
Factory composition wires the same owner to static, Google and DCR-backed token
clients. A configured unavailable owner blocks dispatch; only host-only local
composition without a VM/settings owner uses canonical defaults. Accepted requests
retain their captured pair. There is no new settings registry or polling loop.

V187 extends the existing JSON policy/default and constraints, preserving explicit
values and advancing only rows needing missing fields. Complete API updates,
observed effective limits and the actual WebUI consume both fields. EN/DE help
explains uptake, overflow, unknown timeout outcomes and the separate route/browser
waits. The response value also feeds the existing OAuth request-egress estimate
policy; this technical restriction has not been silently removed.

## Validation and actual scope

[Machine receipt](2026-10-29-oauth-token-transport.json) SHA256 `3315d68799c470c423a1a18e25d3564df341100e6c84746f1b073bdaafb5d55f`.
Final current source manifest SHA256 `bbe227cd746642b5a122efddb6553cd32f5f9f6c38b689518a9120978ff7db14`.
Actual R3 Composition image SHA256 `8491e5a59d130088f5f8235587aea9046a93e0685214c205088cb2286dc4c3d1`.
Actual worker SHA256 `601b976da41c65bb48d237b32db0bc6975e2b30056dae28c86f84ac2f9e5f205`.

- R3: 35 distinct fresh cases/0ignored. Actual native PostgreSQL V187 migration,
  original in-flight callback bounds across a live edit, refresh uptake, rejected
  invalid updates and unavailable owner, global Match/No-Match, 31 settings migration
  cases and live-settings publication pass. These are real stores/leases/host and
  network-policy calls; private issuer destinations remain denied, with no fake
  token response or Tool success.
- R2: 25 additional distinct OAuth/provider cases retain their original image/log
  scope, with unchanged production inputs. The real Chromium/native-facade/PG
  session verifies rendered values, EN/DE explanations, persisted/effective revision
  and restoration. Its frontend is unchanged in R3; it is not counted twice.
- R1: 480 neutral Host/Product contracts/0ignored plus their strict lint and actual
  worker build remain applicable to unchanged owner sources. Exact source/log
  identities are recorded; these are not represented as R3 executions.
- Final Rust format, JavaScript syntax, connected Composition lib/tests Clippy and
  default lib Clippy pass with warnings denied. Source, emitted images, worker and
  logs are verified; serial Screen runner exits and the common Cargo lock is free.

The first compilation rejected five private-field test accesses; existing facade
accessors repaired them without exposing runtime internals. The broad migration
batch exposed 15 pre-existing historical-fixture failures: replaying an older
migration overwrites its test table's default with historically correct older
JSON, then today's later CHECKs reject an INSERT. The corrected cases exercise
that real default with the exact historical migration CHECK in a separate native
PostgreSQL table. All checks and invalid/override/atomic-failure assertions on the
original current table stay intact. No old migration, production CHECK or warning
is suppressed. Current complete-schema insertion remains exercised by V187 and
native runtime/store cases. Failure logs remain retained.

All four incoming Exact prefill afterhashes remain unchanged. The scoped combined
inventory has 169 paths. Root's factory change is limited to settings-owner
construction/injection; older Exact capture contracts retain their own scope.

## Remaining ownership

K3 step6 still owns DCR registration/cleanup HTTP bounds (30 seconds/32 KiB),
ProductAuth server/browser waits, setup lifetimes and remaining ingress guards.
Own-flow/family advancement, actual native Auth-wait production, complete protocol/
policy/recovery and K3–K7 remain open. Exact native import/CAS/Use/design, active
Free/child/review and full Prefix/platform/release/deployment remain separate open
gates. No release, tag, commit, push or reinstall is performed by this package.
