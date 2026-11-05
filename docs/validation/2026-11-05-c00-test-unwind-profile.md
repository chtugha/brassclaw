# C-00 test profile repair — source review only

Date: 2026-11-05. Base: `6a440d71555cefb16b55ad7beba58a57488bfa13`.
Package branch: `codex/c00-test-unwind-profile`.
Status: reviewed source candidate; native warning acceptance remains open.

## Cause and change

The retained Composition libtest from
`target/validation/bob-native-repair-20261105/run-10/` has SHA-256
`7836a434f64c0da5ca0c2dd35cfa273d077d86aa91e3c4c6566ccd5bcef8f837`,
20,050,884 bytes of `__eh_frame` and 348,854 FDEs totaling 19,750,896 bytes.
Composition already used optimization level 1. The batch compiler records show
477 of 479 distinct library crates at level 0, Composition at 1 and `sha2` at 3.

Read-only debug-map/FDE matching attributed 289,044 FDEs, or 15,259,404 bytes,
to exact object start addresses across many dependencies. Sample object DWARF
compilation units confirm Monty, LLM and WebUI ownership, including generics
whose symbol namespaces name other crates. The remaining 59,810 FDEs, or
4,491,492 bytes, were not attributed; they are not assumed to belong to
Composition. Archive section totals alone are not final linked contributions.
The diagnostic script, object attribution and sample CU metadata are retained at
`/private/tmp/brassclaw-c00-unwind-20261105/` with `PROPOSAL-1.md`. Root reviewed
the proposal and metadata before authorizing this exact source change.

The workspace manifest now sets `[profile.test] opt-level = 1` instead of the
Composition-only package override. Basic optimization can reduce out-of-line
functions and their unwind records throughout the actual generating graph.
The reduction needed to satisfy the Apple 24-bit DWARF offset range has not yet
been measured. No test was moved, gated, ignored or removed. No runtime,
dependency, feature, lockfile, API, linker or panic-strategy change is included.

This is build infrastructure, not a new product capability or component.
Recipe/PythonCode workflow ownership, kernel authority, retained selections and
native custody remain with their existing owners. A03 bootstrap/qualification,
B02 private Work/V207 and B2-09 deprecated-store migration are outside this package.

## Source checks and precedence

- Python `tomllib` parsed the before/after manifests and Cargo configuration.
  Structural comparison confirmed exactly the approved test-profile replacement
  in the manifest and no semantic change in `.cargo/config.toml`.
- The test profile inherits dev defaults. The existing `sha2` package override
  stays at 3; debug assertions, overflow checks and test unwinding remain enabled.
  Local debug=1, split-debuginfo=unpacked and incremental=true stay in Cargo config.
  Dev, release and dist profile tables are unchanged.
- Cargo configuration overrides manifest values; profile environment overrides
  take precedence over configuration. The repository/ancestor and Cargo-home
  configuration inspection found no competing test optimization setting.
  The inspected shell has no `CARGO_PROFILE_TEST_*`, `CARGO_PROFILE_DEV_*`,
  `CARGO_INCREMENTAL`, `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` override.
- Existing `reborn-tests.yml` jobs set debug=0 and some set incremental=0.
  They do not override test optimization. Those jobs and local/Modal launchers
  are unchanged. No Apple-specific compiler flag or second target was added.
- Reviewed the connected manifest/comment/documentation diff and relevant links;
  `git diff --check` passed before submission. No new assertion-only TOML suite
  was added. This source inspection does not establish effective compiler flags.

## Native acceptance still required

Root owns the actual native build and tests in the combined A03+B02 batch,
using the canonical target, existing lock and Screen. The first graph rebuild
needs new dependency artifacts; the earlier 163-second warm run is not its
estimate. Preserve old evidence and check the existing disk thresholds.

Use Run-10's actual feature union and unabridged Composition libtest. Acceptance
requires an actual successful link without the warning, retained unwind sections,
`__eh_frame` below 16,777,216 bytes, recorded FDE counts/sizes and new binary hash,
effective profile/check flags, and the complete expected test inventory. Execute
the existing concurrent-rejection and native ToolSkill-review caller cases from
that same binary, each selecting one real test, through the isolated native PG
environment. Reuse the combined build rather than starting a separate experiment.

No Cargo, native test, DB, Modal job or cleanup was run by C-00. No automatic
optimization increase is authorized if level 1 is insufficient. C-00/B3-12,
cross-platform, deployment and full Bob/v3.5 acceptance remain open.
