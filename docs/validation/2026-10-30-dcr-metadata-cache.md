# Acknowledged OAuth metadata-cache lifetime — 30 October 2026

[The machine receipt](2026-10-30-dcr-metadata-cache.json) binds ten changed sources, V190, the actual
worker/image, 7963 captured inputs and all logs. The existing execution-settings
owner now carries `oauth_dcr_metadata_cache_ttl_secs`, default600 seconds,
range0–4294967295. Zero disables reuse. SQL, native decoding, effective status
and actual EN/DE WebUI consume the same field; there is no second settings store.
The UI's two repeated zero-value field lists now share one local declaration.

Each metadata lookup reads acknowledged policy. A configured unavailable owner
cannot return cached metadata through a default fallback. Entries retain their
original age limit; current reductions apply on next lookup, increases cannot
extend old entries, and observed expired/disabled entries are removed. Monotonic
elapsed age replaces unchecked Instant addition. There is no periodic measurement
or cleanup task. Existing Auth expiry, PKCE, endpoints, clients and accepted
registration outcomes remain unchanged; cache settings cannot replay them.

## Observed validation

The first build batch passed548 distinct cases/0ignored:480 neutral contracts,
34 DCR consumers,33 native settings/migration cases and one actual global/runtime/
Chromium case. Actual worker and Composition builds, four JS syntax checks,
rustfmt and neutral/connected/default strict Clippy pass with warnings denied.
The new worker is `227a4b2c3880e51bda63b3a9c68c6e27271454d3e5cd7fa779d6d9fc71d5e624`. Old ece271… is immutable historical
evidence and must never overwrite the canonical executable.

The native HTTP/cache case uses PostgreSQL Auth explicitly, real encrypted
preparation and a shared real PG insert-fault helper. It proves unavailable
settings do not spend the registration claim; cache0, re-enable900 and u32MAX
uptake; no cache resurrection or extension past a captured1-second lifetime;
and unchanged original registration action. Actual HTTP reaches the host/network
private-address denial, not an invented successful issuer reply. Cached issuer
metadata is a data fixture. The previously accepted missing-flow gate recovery
is freshly rechecked with the same original expiry/provenance and no replay.

Actual PG upgrade checks preserve explicit0, u32MAX, revisions and complete
unrelated/custom INSERT defaults, including an explicit default0; missing fields
receive600 with one revision increment. Invalid/missing values are rejected.
Chromium runs the actual UI/router/global owner and checks zero/save/effective
revision, English/German help and full restoration. Original offline model
artifacts are hash-bound; no model/provider/server configuration is changed.

All14 other incoming Exact UI source hashes are unchanged. Its two locale files
have only the new control/help keys added; removing those exact lines reproduces
their original bytes. Historical Exact receipts retain their original scopes.
This does not establish a live OAuth issuer, complete Free workflow, upgrade/
platform support or full K3 acceptance. Setup lifetime600s, extension300s,
ProductAuth backend/browser waits, real browser expiry producers and ingress/rate
controls remain in existing K3 step6. Root keeps sole Cargo ownership through
[the central queue](../development-build-queue.md).
