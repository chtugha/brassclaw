# Retain raw preflight selection through injection — 29 October 2026

The actual obligation prepare caller now retains its authenticated raw selections
through audit/resource preparation and passes those exact sources to injection.
A bound source is resolved once from its existing original-family owner. An
unbound legacy source is captured once at invocation preflight. Neither branch
can lease a later replacement during injection.

Both branches use the existing conditional lease/consume/staging/cleanup function.
Bound source-use checks, source/store expiry, missing-port rejection and actual
abort cleanup remain enforced. There is no unbounded lease call in the injection
implementation. The separate SecretStore forwarding API is unchanged.

This repairs an invocation race. Legacy selection is still later than task-family
preparation and supplies no own-flow, refresh, child or durable continuation
qualification. Missing bound sources do not fall back to legacy capture. No new
public interface, workflow, policy grant, registry or data schema is introduced.

## Actual validation

[The machine receipt](2026-10-29-legacy-raw-preflight.json) binds both changed
Host files, exact before/after/source/image/log hashes and the incoming Exact
Free-capture package. Both actual builds passed on the first compiler attempt.
Formatting, Host lib/tests Clippy, connected Composition lib/tests Clippy and
default Composition lib Clippy passed with warnings denied.

107 distinct fresh cases passed, with zero ignored in the selected executions:

- 102 Host cases: 18 obligation units, 3 retained-service units, 37 obligation
  contracts, 43 actual HTTP-egress contracts and 1 native identity/current-policy
  case. The new regression invokes real prepare with actual InMemorySecretStore,
  audit, leases and staging. A controlled writer replaces material during the
  AuditBefore await. The replacement receives no lease. With two handles, an
  earlier consumed/staged source is removed by actual abort cleanup. The fixture
  delegates real audit storage; it does not fabricate a Tool result.
- 5 Composition cases: real claimed PG account/raw source views, the native
  declared Free-capture probe with an inaccessible Prefix table, and actual
  Default/Selected-without-Base SDK consumers. These protect the incoming capture
  API and its closed dispatch gate, not active Free or native raw workflows.

All 25 incoming Exact source paths remain byte-identical. Their
[capture(parent, capture) API and V186](2026-10-29-exact-prefix-free-capture.md)
remain intact. No native Free dispatch or activation is enabled. Older global/
child results retain their own exact source scope and are not counted again.

Source writes used guarded, fsynced same-directory temporary files and atomic
replacement. Serial Screen `brassclaw-legacy-raw-preflight-r1` / Exec28070 exited0;
runners30088/30217 are gone and the common Cargo lock was independently checked
free. No cache cleanup, warning suppression, skipped case or failed attempt was
required. Source, images, worker and logs were reverified before this receipt.

Machine receipt SHA256:
`d33edb04a2931a8fed99483a9990675c9f92d595cf96cd983fe1e3f5e45303d4`.
Host source SHA256:
`675b1208c5f6683838a42d89f2a5005cc06fc04c363e136fb6c31660bcee9b5d`.
Composition source SHA256:
`8e8ea2ca8619603a9658f5ee144dcffe3d2412b0c4177141e287c077cf532c6f`.
Actual Composition image SHA256:
`42f548bd0045ef8d233f4138a53a24fbdecc1021da8fe2fffd239a90cbec651f`.

## Remaining work

Reuse [the existing family/raw port](2026-10-29-raw-source-family.md); do not add
another issuer or source registry. Actual native extension/ToolSkill workflows,
legacy migration to original task-family preparation, own-flow/refresh progression,
native Auth-wait production, multiple credentials, complete final-use/UI/recovery
and full K3/Exact/platform/release/deployment remain open in their owning plans.
