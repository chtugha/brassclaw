# DCR original gate preparation recovery — 30 October 2026

[The machine receipt](2026-10-30-dcr-gate-recovery.json) binds the two changed sources, actual image,
worker, source manifest and logs. The gate now reads authenticated encrypted
preparation when its Auth flow is absent, before deriving a fresh expiry, PKCE
or provider defaults. It verifies the exact gate continuation and excludes
setup/update-bound preparation. The existing exact original verifier remains
unchanged. Accepted account label/scopes/expiry and preparation references survive
retry; readable originals do not become replacement registrations.

Run2 compiled and freshly passed34 distinct DCR cases, zero ignored: the new
native case and33 other regressions. Connected lib/tests and default lib strict
Clippy pass with warnings denied; rustfmt passes. The unchanged worker remains
`ece271358a476130cb09d695e321eb485e00e1670e89a4704aad4098dee4cb7c`. No new migration, component activation,
Rust workflow loop, fallback or live issuer response is introduced.

The new actual PostgreSQL trigger rejects the first flow insert after real
encrypted preparation commits. The trigger is removed before assertions/retry.
The retry retains the exact original across changed provider defaults and missing
acknowledged settings. Missing settings leave the native registration Prepared;
a later actual host/network denial records one POST outcome as Unknown. Further
readback performs no POST. A foreign SetupOnly original is rejected before flow
publication/dispatch. Issuer metadata is a data fixture; no successful HTTP/Tool
response is fabricated. This is scoped original-boundary acceptance, not complete
native Auth-wait, workflow-family, issuer or crash-recovery qualification.

Run1 compiled, then failed while installing its fault trigger. Source inspection
found the fixture omitted `with_tenant_id`, selecting Filesystem Auth despite
PostgreSQL settings/global hosting. Both DCR native fixtures now explicitly select
PostgreSQL Auth; the current native HTTP regression passes there too. The earlier
[HTTP receipt](2026-10-29-dcr-http-settings.json) remains historical and cannot
establish PostgreSQL Auth ownership. Its HTTP-policy/settings observations retain
their actual scope. Run1 remains failed, never relabelled or suppressed. Ordinary
CLI/serve composition already supplies the explicit tenant.

All16 incoming [Exact HTTP/UI source hashes](2026-10-30-exact-model-setup-ui.json)
are unchanged. Its earlier complete checks retain their own source/image scope;
they are not counted again here. Current evidence: `/private/tmp/brassclaw-dcr-gate-validation-20261030/run-2`.

Root retains all Cargo execution under the new [central queue](../development-build-queue.md).
No build window is returned. Setup/cache lifetime settings, related browser
producers, remaining K3–K7/Exact/Prefix/platform/release/deployment stay open.
