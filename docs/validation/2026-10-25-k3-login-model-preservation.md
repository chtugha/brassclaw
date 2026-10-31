# NEAR login model preservation — 2026-10-25

**Scoped acceptance:18 freshly executed Rust cases, current strict Root/
Composition/default checks, format, whitespace and exact source integrity pass.**

Fresh browser and wallet login previously prepared and committed model=None even
when reconnecting an already selected NEAR provider. That silently cleared the
operator's explicit model. Both channels now capture its intended model with
the original provider/role/credential context in the same repeatable-read snapshot,
then use it for retained SDK preparation and atomic role commit. A different
provider's model is excluded; choosing NEAR uses that definition's configured
default. Intervening role edits still conflict through the existing content CAS.
No original task is rematched, rebound or replayed.

The optional context field has serde default and is omitted when absent. Old
committed browser receipt JSON/proof therefore remains byte-compatible. Exact
original readback reconciles current desired settings and never reselects an old
model over a later edit. The existing model-selection reference algorithm is
factored into a pure helper over already read values; its public async consumer
is unchanged. Login reads roles once, with no additional database round trip.

## Observed caller evidence

Archive: `target/accepted-evidence/k3-login-model-20261025/run-1/`.
Receipt SHA-256: `cd3374897521d71537aeb2e8148e44f544f493f3e00e83e6327fffd64b01baad`.
Three changed Rust source/test files, source inventory, exact test executable,
worker, commands/results and original runner are archived. Canonical root/target,
Rust1.96.0 and serial screen/Cargo lock were used. Required disk-threshold cache
cleanup preserved executable evidence; no competing build tree was created.

Eight actual PostgreSQL/service/HTTP login cases pass22.17s. The new four-way
case completes browser and wallet authentication for both same/different real
provider rows and inspects the actual outgoing SDK request's model. Same-provider
selection remains explicit in settings and on the wire. A foreign model is
absent from the new selection; the wire uses NEAR's configured default. Legacy
session-file bytes remain unchanged. Existing original proof/COMMIT-loss,
rebuilt-owner, stale settings, failed publication, one-shot/timeout, redirects,
private errors and encrypted CAS cases rerun through the shared changed path.
Controlled issuer replies do not qualify real wallet signatures or model quality.

Nine actual settings-service cases pass3.50s and exercise existing role-reference
consumers. Ordinary native global startup/live model exchange passes36.58s with
current packaged content and one retained Monty owner. The exact worker remains
applicable after all530 original input comparisons, SHA
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

The actual test build takes253.14s following capacity cleanup. Strict current
Root/Composition tests lint with -D warnings passes109.54s; default Composition
passes36.55s. Workspace format4.63s, whitespace and current source integrity pass.
No ignored/zero-selected cases, suppressions, failed intermediate source runs or
additional replay pipeline occurred in this bounded successor. Unchanged SDK,
WebUI and secret-owner evidence retains its separately stated previous scope.

## Remaining work

This closes the connected model-preservation finding in the preceding
[wallet publication scope](2026-10-25-k3-nearai-wallet-publication.md).
It does not complete K3: the wallet signer UI, typed DB-owned auth setup, actual
Codex controller/encrypted rotating-session handoff and other protocol custody
remain. K4–K7 and full plans/release/platform/deployment acceptance remain open.
No new Recipe/Tool/queue/registry, global VM replacement, Rust-loop fallback or
remote test-machine operation was introduced.
