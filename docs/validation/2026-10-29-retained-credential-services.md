# Retained credential service binding — 29 October 2026

The existing first-party snapshot now permits trusted composition to bind
explicit credential service views while retaining the same native registration,
kernel, policy, audit sinks, governor and staging stores. Binding neither selects
credentials nor grants access. Every snapshot constructor retains the concrete
built-in obligation handler; its immutable output detector is shared by Arc.

Both obligation preparation and invocation-service resolution preserve missing
original ports. The resolver propagates the original placement/access failure
before replacing an already available secret service. A new view cannot repair
denied modes or unavailable backends. Self-review caught and corrected a draft
that preserved None only in the later resolver; that draft was never applied.

`native_descriptor` checks the original package declaration, actual registered
binding and exact native implementation identity without constructing a consumer
runtime or resolving services. Runtime construction reuses the same declaration
checks. It performs no Tool or secret operation.

## Actual acceptance

One serial Screen session compiled the common checkout using the inherited
canonical target. Formatting and strict HostRuntime library/test Clippy passed
with test-support. Ninety-seven actual cases passed, none ignored:

- 13 obligation units, including two new cases for missing original ports and
  actual bound-generation staging followed by original-store/resource abort;
- 3 new actual LocalInvocationServicesResolver cases for absent, allowed,
  denied and missing-backend service placement;
- 37 existing obligation contracts and 43 actual HTTP-egress contracts;
- 1 extended actual native child case preserving descriptor, artifact, schema
  and identity across credential/child binding and registry removal; current
  kernel policy prevents dispatch and credential resolution.

The staging case consumes a real one-shot lease from its selected in-memory
SecretStore, verifies that the global store was not leased, and observes cleanup
through the original shared handoff and governor. This is lower service-owner
acceptance, not an activated global credential workflow.

Receipt: `/private/tmp/brassclaw-native-credential-services-validation-20261029/host-1/partial.json`,
SHA256 `aa9fa81b74988f92fd66a4c34e25755da789c4ccf356b2c3d4e3c66808a56d20`.
Source manifest SHA256
`bced4e3bfc4633a8dd39aa421472c1bdf7f3f990334bbd87ce1c32260c071020`.
Five applied paths are identified by applied-r2.json SHA256
`3c36d7466a5f926808604aa7895e26d67ca25f8ca7badd5439e3c7d782112569`
under the package pointed to by
`/private/tmp/brassclaw-native-credential-services-current.txt`.
The original three-path private manifest is historical; never replay it.

## Remaining boundary

Original task-family capture including explicit absence, actual source views,
own-flow/refresh progression, verified child inheritance and native auth-wait/
final-use/UI integration remain open. Current native built-in declarations have
empty runtime_credentials; this does not manufacture a verified native GSuite
registration or qualify its future migration. The readonly original-flow reader
has its [separate metadata-owner acceptance](2026-10-29-original-auth-flow-selection.md).
The [Exact preparation handoff](2026-10-29-exact-native-preparation.md) supplies
the existing capture/bind carriers; no second factory or source registry is added.

Separate captured-account draft and three prepared source tests remain private,
unwired and unrun. They are not part of these 97 results or the shared source.
No full K3, Prefix, platform, release or deployment completion is claimed.
