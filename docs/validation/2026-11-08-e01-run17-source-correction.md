# E-01 Native Run 17 source correction

Narrow follow-up to E-01 and its PreparedReference correction. Root's combined
source was `7c16d8fd46f856f8f93ef2c2d207b71d93925b84`. Transfer only this
correction commit; do not overwrite Root with this branch's older base.

## Observed diagnostics and exact changes

Read Root's terminal Native Run 17 log:
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261105/run-17-composition/compile-1.log`.
Worker compilation succeeded; Composition compilation stopped before its test
programs. This package did not run that build or any test.

- E0433: `bootstrap_inventory_qualification.rs` called `evidence::system_record`
  without a module alias. It now uses `crate::bootstrap_evidence::system_record`,
  consistent with the existing owner's qualified calls in this file. No new
  dependency, encoder, format or authority.
- E0308: `Context::executor` wrapped `restore_original`'s existing
  `Arc<ControlledExecutorConfiguration>` in a second Arc. It now returns the
  original Arc after the unchanged checksum check.
- Unused production import: `ComponentRevisionDraft` moved from the qualifier's
  parent imports to `bootstrap_checker_tests.rs`, where the authored-head test
  uses it. The fixture module already imports its own required type. No warning
  suppression or test-only missing-import workaround.

These are the only three implementation/test files changed. No architecture,
Recipe, schema, source behavior, execution, approval or test inventory changes.
The qualifier's source hash is intentionally different: its existing exact-byte
claim identity includes this source; no historical evidence bytes are rewritten
or reused as proof of the changed package.

## Targeted follow-through

Checked the actual owner signatures against E-01 call sites: controlled executor
restore/creation and checksum, verified worker capture, contained Request, global
root retention, revision retention/exact snapshots, deterministic retained Recipe
compilation, Tool-program preparation, source inspection/pure-component views,
and recorded receipt decoding. In particular, global-root retention returns a
definition while executor restoration returns an Arc; their different wrappers
are intentional. Checked explicit fixture/test imports and the shared evidence
encoder's visibility/signature. No additional concrete mismatch was found in
this source inspection; this is not compiler or native acceptance.

Actual checks: rustfmt with edition 2024 and `skip_children=true` on the three
changed Rust files, `git diff --check`, and generation/readback of the sibling
three-file SHA-256 manifest. No Cargo, DB, tests, Python helper execution, Modal
or paid job. Exact native selectors and remaining architectural boundaries are
unchanged from the original E-01 handoff. Root owns compilation/integration and
native acceptance.

The original 2026-11-06 manifest and the earlier 2026-11-08 PreparedReference
manifest remain historical package evidence. This sibling manifest supersedes
only the three initial-source entries changed by this correction.
