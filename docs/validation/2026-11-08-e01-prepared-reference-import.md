# E-01 Native Run 16 import correction

This is a narrow follow-up to source package
`8ff7abc4788ca20e9e2010895022a685aa904ec2` (Root integration `0e2b53c30`).
Cherry-pick only this correction commit; do not transfer the older branch base
over Root's combined source.

## Diagnosis and correction

Root's Native Run 16 on `d5e18cf3509fee2784f1eb68db12e2eac5447002`
successfully built the native worker, then stopped during the joint test compile
before running test programs. Its three E0433 diagnostics at
`retained_inputs.rs:410,413,414` have one cause: the E-01-extracted input-edge
fingerprint verifier uses `PreparedReference` without importing that enum.

Original diagnostic:
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261105/run-16-composition/compile-1.log`.
This package read that log; it did not run the build.

The only implementation change adds `PreparedReference` to the existing
`super::typed_bindings` import block in
`crates/brassclaw_engine/src/memory/retained_inputs.rs`. This is the public enum
that owns the actual prepared layout's Input/Constant/Result variants, also used
through existing imports/reexports by the adjacent validator code. No behavior,
schema, approval, runner, fixture, visibility or test inventory changes.

## Targeted source review and actual checks

Rechecked the enum declaration and layout types, the fingerprint type's public
reexport, and the extracted verifier's three match arms. Rechecked imports and
parent-module access in the new E-01 execution, fixtures and native-test modules,
including the existing admission/runner, captured-preparation getters, test-only
catalogue fields, receipt source trait and failure-quiescence interfaces. No
additional concrete import/type/visibility defect was found by source inspection.
This is not a compiler result or proof that the combined package compiles.

Ran only `rustfmt --edition 2024 --config skip_children=true` on the changed Rust
file, `git diff --check`, and SHA-256 generation/readback. No Cargo, DB, tests,
Python helper execution, Modal or paid job. Root owns the next coherent compile
and the unchanged exact native selectors in the original E-01 handoff.

The sibling `.sha256` identifies the corrected implementation file. The original
2026-11-06 manifest remains the historical initial source-package manifest; this
correction supersedes only its `retained_inputs.rs` entry. All other E-01 source
bytes remain unchanged on this branch.
