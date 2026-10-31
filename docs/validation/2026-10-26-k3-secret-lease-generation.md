# K3 selected Secret generations — 26 October 2026

Status: **scoped acceptance passed**.395 distinct passing Rust cases, strict
affected test Clippy and actual Chromium/native Auth HTTP checks establish the
selected-generation prerequisite. The historical cancellation test is repaired
and passed through actual PostgreSQL. Issuer expiry, task-family credential
selection, K3 and the complete plans remain open.

## Implemented boundary

Native PostgreSQL leases retain the exact authenticated encrypted generation
after legacy upgrade while holding the actual secret row lock. Consumption
compares that generation under its existing row lock before releasing material;
it acquires no advisory writer lock after the row lock. Old encrypted leases
without a selected generation fail closed but remain explicitly revocable.

Filesystem leases likewise retain the selected generation and authenticate the
complete scope, lease ID, handle, deadlines and lifecycle state through the
existing crypto owner. Removing the proof or copying a reference cannot create
a valid selected lease. Initial issuance is conditional on absence. Subsequent
lease/session CAS compares the version and encrypted body from the same backend
read, preventing a restored revoked row at a reused numeric version from being
overwritten by a stale consume. No new store, task workflow, public API or
configured lifetime was introduced.

The shared real-owner contract covers same-byte replacement, delete/recreate,
competing consumes and actual concurrent replacement. The legacy memory adapter
keeps its actual UUID/usage guard; its split read is not described as releasing
a replacement when record_usage rejects that replacement before return.
Additional checks exercise genuine old encrypted PG/FS leases, proof/metadata
tampering, native consumption COMMIT acknowledgement loss and authentic sealed
revocation restored at the same version on memory and PostgreSQL RootFilesystem.

## Actual validation and failure disposition

The six owned source hashes, before bytes and exact applied receipt are retained
under `/private/tmp/brassclaw-secret-lease-generation-20261026/`. R1 captured the
whole build source inventory in
`target/validation/secret-lease-generation-20261026/run-1/source.json`, the direct
Cargo commands/features/toolchain in progress.json and all three emitted test
images with SHA256 identities. Strict Secrets/HostRuntime/Composition test Clippy
passed with warnings denied; the three actual library test builds passed.

R1 executed Secrets68 and HostRuntime215 with zero failures. HostRuntime's three
existing Docker-daemon cases were ignored and are not qualified here. The runner
then stopped on its inappropriate blanket zero-ignored assertion. That terminal
failure is preserved; no product change or compiler rerun repaired a nonexistent
HostRuntime failure.

R2/R3 verify and execute the exact immutable R1 images without Cargo while Prefix
R4 owns the shared source window. They also verify the six Root source hashes
and both external browser inputs, and explicitly record concurrent Prefix source
drift. These are frozen-image results, not certification of the later merged
Prefix tree or global startup. Actual results:

| Selected scope | Observed result |
| --- | --- |
| Native reference/generation | 8 passed, zero ignored |
| Native custody/index/retirement | 14 passed, zero ignored |
| DCR registration/cleanup | 32 passed, zero ignored |
| OAuth provider/callback/refresh | 13 passed, zero ignored |
| Runtime account selection | 14 passed, zero ignored |
| Native account/flow/refresh | 17 passed, 1 failed, zero ignored |
| Credential lifecycle | 8 passed, zero ignored |
| Protected Auth HTTP/original browser | 5 passed, zero ignored |

The failed native test is
`native_manual_abandon_surfaces_each_failed_transition_and_preserves_retry_state`.
Its second fault trigger still attaches BEFORE DELETE and its final assertion
expects the original interaction to disappear. The current legitimate custody
owner instead conditionally updates canceled_at and retains the original. Keep
both transition-failure assertions; target the actual interaction UPDATE and
assert preserved original fields, cancellation and repeat idempotence. Production
cancellation must not be weakened or returned to destructive deletion.

The actual Chromium completion marker is present. HTTP observations are
503/200/202/200/200; original identity and first expiry survive retry/reload,
blocked popup exposes the safe link and unknown cleanup remains visible. Native
assertions retain the exact registration/cleanup counts. This covers the existing
component/protected HTTP/native Auth/Secret path with a controlled issuer, not
the complete Extensions page, successful live issuer or remote deployment.

## Terminal repair and evidence reuse

After the Prefix owner's actual terminal R4/window return, Root applied the
guarded single test correction. Both actual failure assertions remain; the
second trigger now rejects the interaction UPDATE. Each rejection preserves the
entire original and revision. Successful abandonment retains only its new cancel
timestamp/revision; repetition retains the same full original/cancellation and
revision. Production code was not changed to satisfy this historical fixture.

Root R4 strict Composition test Clippy passed in47.91s, the actual library test
build in138.34s and the exact real PostgreSQL regression in4.63s (one actual pass,
zero failed/ignored). The other17 native refresh cases retain their actual R2
passes; the failed case is counted once using its repaired R4 result. R1's68
Secret and215 HostRuntime passes, R2's81 other selected passes and17 unchanged
native refresh passes, R3's13 lifecycle/HTTP passes and R4'sone repaired case
give395 distinct passing cases. The three unexecuted Docker tests stay separate.

`target/validation/secret-lease-generation-20261026/run-4/accepted.json` SHA256
`3d6c66bcd5bb5b672e83ee8e218388af42b90b7ee4cb52a17677ce85651d8873`
records the complete count/reuse/result mapping. Its seven owned source bytes
are archived under owned-source/. All R1/R4 executable hashes and current owned
source hashes were verified before recording acceptance. Frozen R1 reuse is
explicit; this is not blanket certification of later concurrent Prefix changes.

Continue the existing K3 steps3/4: authenticated issuer expiry through both
backends/final injection and original task-family grant selection. No full-plan,
Prefix serving, platform or release acceptance follows from this prerequisite.
