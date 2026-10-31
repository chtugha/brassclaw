# Historical Prefix worker-reuse qualification — 26 October 2026

The historical 530-file worker applicability inventory is incomplete. It cannot
prove that all actual worker build inputs were unchanged. This limits source
applicability claims; it does not erase observed executions, rewrite immutable
receipts or establish that a particular historical test result was incorrect.

The actual `crates/brassclaw_monty_host/Cargo.toml` has normal local dependencies
on Model Encoding, Common and Resources. Resources brings additional filesystem
dependencies. The retained inventory copied into
`target/accepted-evidence/prefix-export-capacity-20261025/run-1/worker-source-applicability.json`
contains 530 entries but **zero** paths under each of
`crates/brassclaw_model_encoding/`, `crates/brassclaw_common/`,
`crates/brassclaw_resources/` and `crates/brassclaw_filesystem/`.
Host API and Monty Host entries alone do not cover that dependency graph.
Checking 529 unchanged entries plus the reviewed Cargo.lock difference therefore
does not complete the missing dependency qualification.

The earlier registered-source, archived-original, export-capacity and related
Prefix receipts retain their actual commands, binaries and observed outcomes.
Statements that all **recorded** hashes matched are limited to that incomplete
recorded set. Do not use those statements as proof of a complete unchanged
worker closure, current merged-source support or future worker reuse.

The [26 October physical-partition package](2026-10-26-prefix-token-partition.md)
built its actual worker anew, together with its current tests. That package does
not rely on the old 530-entry reuse proof. Its exact fresh-build/runtime evidence
remains applicable in its recorded scope; it does not retroactively qualify
unexecuted historical test cases.

Next Prefix validation must build the worker after the relevant current source
changes and freshly run the affected historical Prefix native cases. Retain that
new worker beside the actual test executables. If a later package instead reuses
a binary, qualify the complete conservative normal/build local dependency
closure and other build inputs, including inherited/target-specific dependencies,
local patches, manifests, lockfile, build scripts/assets and toolchain/features.
Never reuse the old handwritten list as the complete closure. Shared filesystem
and original-input custody qualification remains with its existing owner.

[Fresh current-case requalification](2026-10-26-prefix-leading-chat.md) now
passes the specifically listed export/source/HTTP/storage/preload/budget and
archive/graph/captured-selection native cases with an actually rebuilt worker.
This supersedes the old reuse justification only for those executed current
paths; it neither changes old receipts nor qualifies every older package or
unexecuted consumer. No new activation, provider call, deployment or cache-quality
claim follows from this review.
