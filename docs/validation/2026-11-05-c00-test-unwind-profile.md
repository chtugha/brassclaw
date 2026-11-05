# C-00 test profile repair — scoped native acceptance

Date: 2026-11-05. Base: `6a440d71555cefb16b55ad7beba58a57488bfa13`.
Package branch: `codex/c00-test-unwind-profile`.
Source `36ce1c4c5` is integrated as `e212e0ba2`. The supervisor's native
profile/link/caller acceptance below passed on that exact source. Full B3-12,
A-03/B-02, other platforms and product/release acceptance remain open.

## Actual native acceptance

The supervisor ran `run-11-profile` in detached Screen with Rust 1.98.1 on
Apple ARM64, the configured canonical target and four Cargo jobs under the
existing local lock. The actual current `monty_worker` built first in 24.76
seconds and was retained after successful compilation. The known Run-10
Composition/LLM/WebUI feature union then compiled and linked in 779.70 seconds.
This was the first broad optimized-graph rebuild, not a warm timing comparison.
Both commands completed without warnings; the complete batch took 818.46 seconds.

- Actual Composition executable SHA-256:
  `26c553288d7f4d7b23fe9cbc2e0e6fd03997c1f7473cd756af99228915652db9`.
- Retained worker SHA-256:
  `74d692fe7181881ccfe1ed3b135a7d384fa4c1e59372666070a2dc6ada209def`.
- `__eh_frame`: **11,978,176 bytes (11.42 MiB)**, down from 20,050,884 bytes,
  with 4,799,040 bytes remaining below the 16-MiB encoding boundary.
  `__unwind_info` remains present at 2,284,120 bytes. No unwind strategy or
  linker suppression was introduced.
- Actual FDE count: 157,476, totaling 11,825,672 bytes. This measures unwind
  metadata; it is not a claim of smaller total code or faster compilation.
- Compiler records confirm opt1, debug1, debug assertions and overflow checks
  for the tested harness and inspected runtime owners, with `sha2` still opt3.
  All 1,353 prior test names remain; no test was removed or gated.
- The two specified real PostgreSQL caller tests each executed once and passed
  (2.52 and 1.54 seconds). The other listed tests were not executed in this batch.
- Captured source bytes and HEAD remained unchanged throughout the batch.

The exact source manifest, compiler JSON, complete logs, binaries, original
runner and terminal receipts are under
`target/validation/bob-native-repair-20261105/run-11-profile/`.
`profile-link-acceptance.json` and `batch-completed.json` both report success.
The source-manifest SHA-256 is
`706aadaff49136846047d5567b9c8d2305c229d54928e77a6e765ee26d8b2eee`.

This independent completed build boundary was checked while B-02 continued in
its isolated worktree. The policy's targeted intermediate-check exception
resolved the shared linker uncertainty before the larger runtime integration;
unchanged optimized dependencies can be reused. These receipts do not compile
or validate A-03/B-02. Their later combined executable must receive its own
link/section and runtime checks. New changelog text is embedded release input
and will be compiled with that pending integration; it is not part of this
already captured `e212e0ba2` executable.

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
The source proposal made no measured-reduction claim; the later native result
is recorded above. No test was moved, gated, ignored or removed. No runtime,
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

## Native acceptance procedure retained

Root owns native validation using the canonical target, existing lock and
Screen, including the later combined A03+B02 batch. The first graph rebuild
needs new dependency artifacts; the earlier 163-second warm run is not its
estimate. Preserve old evidence and check the existing disk thresholds.

Use Run-10's actual feature union and unabridged Composition libtest. Acceptance
requires an actual successful link without the warning, retained unwind sections,
`__eh_frame` below 16,777,216 bytes, recorded FDE counts/sizes and new binary hash,
effective profile/check flags, and the complete expected test inventory. Execute
the existing concurrent-rejection and native ToolSkill-review caller cases from
that same binary, each selecting one real test, through the isolated native PG
environment. Reuse the combined build rather than starting a separate experiment.

No Cargo, native test, DB, Modal job or cleanup was run by the C-00 package chat.
The supervisor's actual scoped acceptance is recorded above. No automatic
optimization increase is authorized for a future larger graph. Full B3-12,
cross-platform, deployment and full Bob/v3.5 acceptance remain open.
