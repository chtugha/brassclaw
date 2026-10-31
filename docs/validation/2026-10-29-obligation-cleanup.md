# Independent obligation cleanup — Host acceptance

29 October 2026. One production source path, Host acceptance complete. This is
not native auth continuation, credential-family capture, K3 or full-v3 completion.

`BuiltinObligationHandler.abort` previously returned immediately when staged
secret cleanup failed, skipping independent resource release. That could strand
reserved concurrency after an unsuccessful call. It now attempts both cleanups,
returns failure if either fails and reports a second failure with redacted causes.
The direct `satisfy` release-error path also reports secondary cleanup failure.
Preparation preserves both original and cleanup causes when abort fails; unresolved
cleanup cannot be misreported as a safely resumable authentication pause.

The actual regression prepares network policy, secret material and a reserved
concurrency slot through `BuiltinObligationServices`. It deliberately poisons the
private injection-store mutex after preparation, then invokes the real abort.
Abort remains a Secret failure; reserved capacity returns to zero, network policy
is removed and the unusable store remains visibly poisoned. No recovery of the
poisoned state, fabricated cleanup success, Tool dispatch or permission bypass.

All 91 selected actual Rust cases pass, zero ignored:11 obligation/staging unit
cases including the regression,37 obligation caller contracts and43 actual Host
egress contracts. Host test images build in5.25s; affected lib/tests Clippy with
`-D warnings` passes in10.47s, formatting passes. The source guard covers the31
actual local Host dependency packages plus workspace configuration; it excludes
LLM/Reborn/Composition and remained unchanged while that owner edited its sources.
Cargo was serial, in Screen, from the common checkout and inherited canonical
target. There was no target override or parallel Cargo.

The first wrapper incorrectly expected the deliberate panic's test output to
remain on one line. All four commands had already succeeded, including the actual
11 cases. `host-2` verifies/reuses precisely their source, immutable images,
profiles and log hashes; it executes the remaining80 caller cases. No source
repair, compiler rerun or repeated11 tests was needed for this parser correction.
The complete original failed wrapper/positive command history remains in `host-1`.
The metadata continuation and remaining binary cases finished through Exec37138
with exit0. These91 results are not summed with historical overlapping Host90.

Exact evidence:

- Source proposal/preimage/apply history:
  `/private/tmp/brassclaw-obligation-cleanup-20261029-vsccvk4r/`.
- Terminal receipt:
  `/private/tmp/brassclaw-obligation-cleanup-validation-20261029/host-2/partial.json`,
  SHA256 `506c71a86b0b8ecf2b1456359e4b0af09047e1f9c22cc216170781da3d8d3eb7`.
- Source manifest SHA256
  `d5533ac819204d39d45bc0b39b4768b3387e15ea455b75a83e27f1ef38d5f922`.
- Current Root65 inventory is `current-root-inputs-cleanup.json` through
  `/private/tmp/brassclaw-root-reviewed-current.txt`, SHA256
  `8f61a5aa137941c0e93bc1248669eabd14c22ced217277af3ca84eadaa38b2b7`.

Own runners93289/93450 are actually gone, common lock independently free and all
captured sources rehashed unchanged. Source/Cargo was explicitly returned to the
Prefix owner. Current combined Composition/Reborn/native auth-origin checks remain
pending in that owner's batch. No Prefix source, Cargo configuration, API, component,
VM lifecycle or workflow changed here. No commit, push, tag, release or deployment.
