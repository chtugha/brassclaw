# E-01 Run 20 supported-settings correction

Root's focused Run 20 completed two ACK-loss cases successfully, then the
originals/pruning/settings case failed at its attempted depth-65 publication.
Read the terminal `test-002.log` in
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261105/run-20-composition/focused-cases-1`.
The settings store correctly rejected the unsupported transport configuration.
This package ran no Cargo, DB, tests, Python or Modal.

## Cause and corrected acceptance claim

`REVISION_LIMITS.max_depth` is 64. Adding one produces a structurally valid
execution-settings DTO but not a supported live configuration:
`brassclaw_monty_host::process::runtime_bounds_supported` limits value depth to
`MAX_TRANSPORT_DEPTH` (64). `Service::validate_runtime_bounds_for_frame` checks
that predicate. The live settings store checks this before writing the desired
row and returns the exact observed Invalid classification. Raising the transport
ceiling is not part of E-01.

The connected native case now asserts that depth 65 is rejected with that
classification, then reads the settings back and verifies revision and execution
limits are unchanged. It captures the actual acknowledged preparation and checks
that its revision/policy still equal the accepted settings before inspecting the
retained pure component. This proves supported-profile metadata inspection and
safe rejection; it no longer claims a live depth-above-64 regression test. There
is no clamp, skip, successful fallback update or relaxed product constraint.

The original schema-parser separation remains implemented by the existing
immutable `REVISION_LIMITS` profile. This test does not claim native acceptance of
an unreachable larger-depth settings state merely to exercise that code change.

## Smaller acknowledged byte limit and original capacity

The later byte reduction remains derived from the actual serialized retained
claim, with checked `u64` conversion. It now explicitly asserts a positive,
strictly smaller setting below the retained claim's size. The existing
`execution_bounds` owner converts both current and proposed settings, and the
test verifies their complete VmBounds differ only in `max_value_bytes`, remain
valid, and fit the unchanged frame ceiling.

Source feasibility: the service's compatibility predicates require valid bounds,
supported depth, value/source/stdout ceilings fitting the frame, source capacity
covering the retained root, and compatible response/slice/actor settings. Only
the positive value-byte ceiling decreases. Depth, source/stdout bounds, frame,
actor and deadlines retain their already accepted inputs. No minimum value-byte
bound is derived from root *source* size: `max_source_bytes` is the separate
predicate. Full intrinsic DTO/native-representation validation still runs.
This establishes the source feasibility of the proposed reduction, not a native
pass; the actual update must still succeed normally.

After the update, the test now captures acknowledged preparation and verifies
the returned settings revision, complete policy and converted VmBounds before
the pool-size-one read. It then explicitly verifies each returned context keeps
the original larger byte capacity, alongside unchanged operation/approval bytes,
unchanged admission/observation counts, terminal pruning and authored-head
preservation. Historical immutable metadata larger than the current task budget
is therefore still the intended counterexample; no checker may be re-executed to
manufacture new evidence under the smaller budget.

## Shared finding for Root/F01

Read-only inspected F01's `bootstrap_pure_checker_tests.rs` in its worktree. Its
similar smaller-byte case changes only `max_value_bytes` and does not attempt
depth 65. The same source feasibility argument applies if its derived byte count
is positive and strictly smaller. Root/F01 should distinguish persisted desired
settings from actual acknowledged preparation and verify the latter before
claiming smaller-live-budget acceptance. No F01 file or shared product code was
edited; no F01 native pass is claimed.

## Handoff and actual checks

Only `bootstrap_checker_tests.rs` changes; selector and test inventory are
unchanged. Checked actual DTO types: revisions `u64`, settings depth `u32`,
settings bytes `u64`, VmBounds bytes `usize`, UtilityLimits fields under `bounds`,
and ContractLimits bytes under `max_bytes`. Checked existing capture policy/
revision getters and the existing error variant/signature.

Ran rustfmt edition 2024 with `skip_children=true`, `git diff --check` and sibling
SHA-256 manifest generation/readback. No native execution receipt. The sibling
manifest supersedes the checker-test entry from the earlier length-correction
manifest; earlier manifests remain historical. Transfer only this correction
over Root's accepted E-01 source, not this branch's older base. Root owns the
next coherent compile and prerequisite/focused native acceptance.
