# Shared HTTP single-submission transport — 2026-10-25

**Scoped acceptance:49 freshly executed, nonignored Rust cases and current
strict Root/Composition/Network tests/default, format and integrity checks pass.**

Pinned Reqwest0.12.28 defaults to retrying certain protocol NACKs. The existing
hardened shared transport now explicitly disables that retry policy. A private
builder extractor preserves the production client factory, vetted DNS/IP pins,
proxy/redirect restrictions, timeouts and revisioned client cache. Connector
fallback before submission remains available. Eligible, explicitly accounted
retries remain with the caller; no new workflow, authority or resource knob is
introduced.

The native regression sends the complete POST body through the actual
ReqwestNetworkTransport and its cache. A local HTTP/2 peer reads that body and
returns REFUSED_STREAM. The default-policy positive control observes two complete
identical bodies and success. The hardened client observes exactly one body and
the classified transport failure, with one request's bytes and no response bytes.
Test-only h2c selection changes neither production negotiation nor TLS validation.
All fixture tasks are supervised; shutdown accepts only intentional cancellation.

## Evidence

Archive: `target/accepted-evidence/k3-network-no-replay-20261025/run-1/`.
Receipt SHA-256: `56fd38b607cb3571943c7c43d6886055ac9ee00b54510e48469a5be70501fe81`.
Six files were applied after exact base/proposal comparison. Source inventory,
runner, executables, commands, lock review and worker applicability are retained.
The common root/target, Rust1.96.0 and one serial screen/Cargo lock were used.
No intermediate source/test/lint failure or diagnostic suppression occurred.

| Actual execution | Passed |
| --- | ---: |
| Network unit tests, including physical NACK/control case | 16 |
| HTTP egress contracts | 20 |
| Network policy contracts | 11 |
| Dependency boundary contract | 1 |
| Ordinary global Monty/live-model packaged-content caller | 1 |

Locked offline dependency resolution passes. The lock changes exactly two
Network dev-dependency edges to already pinned h2/http packages; no version,
checksum or package is updated. The actual worker is reused only after comparison
of all530 inputs and this exact lock transformation outside its dependency graph;
the fresh global test uses the current packaged content. Worker SHA-256:
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Network test build5.53s, Composition test build194.66s and native global39.87s
pass. Strict Root/Composition/Network tests81.55s, default Composition19.10s,
workspace format4.71s, whitespace and protected source integrity pass.

## Remaining scope

This closes the shared transport's hidden protocol-NACK resend prerequisite in
Simplified K3. It does not certify every protocol/provider, real issuer behavior,
durable refresh result/account/flow custody, general recovery, deployment or full
K3. The existing AuthProduct owner still needs original pre-egress identities,
durably retained results before account publication, exact original reconciliation
and the remaining expiry/setup/session consumers. No commit, push, tag, release
or SSH operation was performed in this package.
