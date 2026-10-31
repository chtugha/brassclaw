# Native manual cancellation disposition and build inputs — 2026-10-26

Status: scoped native cancellation/build prerequisite acceptance; **not original
setup transport, browser/reference continuation, full K3 or full-plan acceptance**.

Native abandonment retains consumed interactions and returns false for already
completed/failed/expired flows. Pending cancellation retains a timestamp in the
existing interaction record, with exact body/version CAS inside the existing
native transaction. Actual committed disposition reconciles a lost COMMIT reply;
no deletion or replacement follows uncertainty. Raw publication rejects canceled
inputs before account/secret effects. Existing optional fields remain backward
readable; first setup identity and other lifecycle cleanup still need their owners.

The Composition build script now declares its actual build.rs input. It reads no
package data, emits the same two [] stubs and preserves their mtimes when unchanged.
This removes its default package-wide rerun dependency on unrelated tests. Actual
build-script compilation and execution check initial content, preserved known past
mtimes on repeat, correction of differing bytes and propagation of a real directory/
read failure. This is mechanical source/output evidence, not an all-platform timing
benchmark. No extra Cargo target, cache or warning suppression was introduced.

## Executed acceptance

274 distinct freshly executed Rust cases pass, none failed or selected ignored:
86 Auth contracts,12 FS manual,12 native manual (nine prior plus three new),3
coordinator,53 durable,1 global Monty packaging,106 public auth/WebUI/callback/
lifecycle and1 dependency boundary. New actual native cases prove completed/consumed
retention, canceled disposition with zero account effect, and successful original
readback after existing AckCutTransport removes the real COMMIT acknowledgement.
A Ready-only cut is not used to claim an unobserved commit.

Strict full/root/Auth/tests73.60s, PostgreSQL-without-RootLLM10.94s, default14.50s,
format4.57s and whitespace pass. Actual library build191.79s, public4.84s,
Auth contracts0.19s. Worker command0.43s reused its cached current image; no fresh
worker-compiler claim. All six final author hashes, complete applicable source
membership/identities (9,737 entries) and eight Cargo executables were rechecked.
Build-script probe final image SHA-256:
`7ff4254fab22df9624d3f3301381e9133452289200675619609b6791c2da565d`.
No earlier Rust execution is counted toward274.

Receipt:`target/accepted-evidence/k3-manual-cancel-disposition-20261026/run-1/accepted.json`.
SHA-256:`2b3e5ce8377c97152fc200451e81d9737f886da3dea7f21955a71e7fcbaa5d7a`.

## Remaining work

K3 step2 still owns stable original interaction/flow/invocation IDs before HTTP,
first expiry, terminal setup readback, explicit confirmed-reference continuation,
actual browser handling and DCR disposition. This repair does not supply those
new ports or certify complete generic workflow support. Issuer expiry, final live
credential-generation/grant fences, compatible upgrade/recovery and K3–K7/full
plans remain open. No Git/tag/push/release/SSH/provider-host change occurred.
