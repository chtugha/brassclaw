# K3 — Retained embedding consumers, 2026-10-24

The bounded embedding successor passes **120 actual Rust cases**, **43 UI cases**,
ten JavaScript syntax checks, strict affected/root test lints, default-composition
lint, formatting and source/artifact integrity. No ignored cases are counted.
Evidence is retained in
`target/accepted-evidence/k3-embedding-consumers-20261024/run-1/`.
K3 and the priority gate remain open.

## Implemented and observed behavior

The memory source no longer reads credentials from `api_key_env`. It uses the
same authenticated host-store capture as the factory and compares opaque record
references for client reuse, without retaining an additional plaintext key copy.
The existing real credential/endpoint test now exercises both the factory and
ordinary memory source against a nonempty ambient `PATH` compatibility name.
Neither accepts that value instead of a stored credential.

The same PostgreSQL compatibility owner supplies a neutral embedding dispatch
guard to the existing cache through the existing factory/adapter. The utilities
crate reads no database, settings, environment or concrete HTTP provider. Before
cache use and raw submission it checks active provider/authentication/endpoint/
transport compatibility and the selected model's exact dimension/vector revision.
Live cache capacity and monotonic definition/generation fencing apply to already
retained clients. Partial batch hits cannot survive a retired generation across
a preparation await. A post-response check controls retention only; completed
vectors are returned to their original caller without replay.

Replaced or failed source clients permanently retire their caches and release
their map allocations. Compatible old operations can still finish against their
original model; live policy cannot re-enable retention in that retired cache.
The registered Memory Tool test exercises key-only rotation, a real authenticated
successor, current cache controls, compatible old space-A calls after selecting
space B, permanent retirement and deactivation of both retained spaces before
another HTTP request. Provider-free reads/search and complete large results
remain covered by the same actual PostgreSQL/filesystem/kernel caller.

The protocol test checks cache-off on an old client, vector revision changes,
new body/deadline constraints rejecting incompatible old clients, and actual HTTP
failure categories using freshly compatible clients. Errors are not substituted
for real service observations. Complete UTF-8 input, batch ordering, actual body
limits, refusal/timeout/redirect behavior and secret redaction remain exercised.

Database-selected text clients also reject legacy mutable `set_model`; the native
policy test verifies their publication/model remain unchanged. Shared text-cache,
publication, service, model-limit and discovery callers pass after the common
guard refactoring. The UI refreshes its exact canonical query on selection errors
as well as successful mutations, awaits that refresh, and exposes query loading/
errors in the inference summary. No stale mutation snapshot becomes canonical.

## Evidence identities and limits

- accepted.json: `ae1678a44b705904e7107e480babdf72dfed3393568adeb066da836235cf4a58`.
- source.json: `66a92cc42ba906fd4399904fe5f8fe46180e6aaa887d2ef51e4cf9fe91a05eb7`.
- validated-embedding-source.json: `aa1fb6ab44e0f65b224b3effe2cfd1fc1eac345e103d4929ca66b9c6c1b91fa6`.
- Actual composition executable: `d101cdf744ea104813b0c68b5498c4df0700ea7bcd6d3e34710cc4cb81897e63`.
- Actual embeddings executable: `6de3b1135affcc3e1710794bc2b6c74ffc7fc471ef3c0c2c0f53f226e3202dee`.

Fourteen changed compiler inputs are preserved as immutable evidence deltas.
The 530 worker inputs remain unchanged, and the original executable is hash
checked. Cargo ran serially in screen using the inherited canonical target;
executables were preserved before required targeted disk cleanup. The stricter
runner requires a positive executed Rust count and zero ignored cases.

The previous [provider consumer integration](2026-10-24-k3-provider-consumers.md)
retains its exact original full HTTP/Prefix and three real Ornith execution
receipts. They are not reattributed to this changed embedding source. This
successor qualifies the modified actual embedding/shared-guard callers, rather
than claiming a fresh full Prefix serving or deployment acceptance.

## Remaining K3 work

Embedding runtime publication is still explicitly untracked. Complete the
desired/effective/pending contract, role-selection concurrency, dedicated
session/broker compatibility and late claim/attempt checks at actual submission.
The task-wide model/target binding over later Recipe steps and waits is also
open: the qualified ordinary in-flight exchange is narrower. The ordinary
runtime still has `model_route_resolver: None`; Prefix confirms it has no
productive C2 binding port replacing that gap. Extend the existing task/child/
continuation snapshot and admission owner, with Prefix supplying its references
and final request guard; do not create a second provider registry, manifest,
model loop or admission system. K4–K7 and regular plan continuation follow this
priority work. Legacy credential migration must be qualified in K6/Phase 8 before
deployment; removed fallbacks alone do not establish safe upgrade support.
