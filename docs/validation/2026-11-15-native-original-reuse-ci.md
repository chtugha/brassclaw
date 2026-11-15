# Native original reuse and CI regression repair — 2026-11-15

## Repair and observed cause

The unchanged global-runtime regression first failed with 194 Tool-journal rows
before restart and 200 afterwards. A read-only observer of that test's own
temporary clusters identified six repeated `builtin.pure_review.invoke_original`
calls. The private Pure Case reservation included mutable executor/preparation
settings, so a live settings edit selected a different reservation and repeated
completed qualification work. This was not a Recipe fallback or new user task.

The repair introduces a separate `/2` reservation identity covering the complete
immutable manifest except its three explicitly retained configuration fields.
The stored original still contains those fields. Readback checks the complete
immutable identity, canonical executor/checksum, preparation and matching settings
revision/policy, and restores that original before fixture inspection. Historical
`/1` records remain unchanged. Uncertain original execution still refuses replay;
actual Tool dispatch still uses current kernel policy.

CI additionally serializes only the full-instance CLI cases. Trusted Main pushes
can now save the existing dependency caches, including partial progress on job
failure. Pull requests remain read-only and the shared Clippy cache retains its
single all-feature writer. No application or CI deadline was increased.

The immutable-record test now uses `TRUNCATE ... CASCADE` to reach the actual
BEFORE TRUNCATE trigger rather than failing first at PostgreSQL's foreign-key
precheck. It checks the exact trigger message/SQLSTATE and persisted originals
after each rejected mutation, not only an earlier in-memory view.

## Actual validation and limits

Local evidence is under `target/validation/bob-native-repair-20261105/`:

- The unchanged Run125 binary reproduces the original restart failure;
  `acceptance-global-restart-diagnostic` preserves its journal observations.
- Strict Run128 fails on a test using a private capture constructor. The test is
  corrected through the existing retained codec; visibility is not broadened.
  Native Run129 is not started.
- Strict Run130 passes in 76.232 s with no warnings. Native Run131 compiles the
  actual Composition/ingress programs and worker in 658.707 s with no warnings.
- Run131 executes all three new configuration-integrity cases successfully. The
  unchanged full `native_global_runtime_retains_one_root_across_match_and_no_match`
  case passes in 341.329 s, including its original restart-count assertion,
  ordinary Match/No-Match, live settings, policy, MCP and History paths.
- Standalone control-harness all-targets Clippy Run132 passes on Rust 1.96 in
  4.047 s with no warnings. Both changed workflows pass actionlint.
- `target/validation/ci-trigger-regression-20261115/` records the actual current
  migration hashes and six UPDATE/DELETE/TRUNCATE rejections on Root's exclusively
  owned temporary PostgreSQL cluster. Every operation returns SQLSTATE 23514 and
  `component review records are immutable`; the original 2,107 evidence and 58
  approval rows remain unchanged. Start, queries and orderly `fast -w` stop pass;
  the PID file is absent afterwards. This is direct SQL acceptance, not a claim
  that the entire Rust association test suite was rerun.

The build/case Screens have ended. Their final notifications were accepted and
actually received, separately from successful command exit. No new Modal build
or compute was used. The separately requested old Modal package startup failed
at the shared observation deadline; see the [exact observation](2026-11-15-modal-canonical-native.md#additional-native-startup-observation--2026-11-15).

Run131 covers its captured source and embedded documents. The subsequent delta
contains CI workflow changes, the standalone SQL-test repair, an additional
Changelog entry and a redundant direct `cfg(test)` on the new test helper. The
helper already lives in a test-gated module; the local attribute makes that
boundary visible to the file-local No-Panics checker without changing its body
or production compilation. The Changelog is embedded by the Prefix release-document owner;
this receipt does not claim that Run131 contains the later release-note bytes or
qualifies a new Prefix/release package. Reporting/plan updates are development
records. No full GitHub CI success, deployment, complete shipped-library
qualification, authored approval activation or completed Bob plan is claimed.

## Concurrent source boundary

A separately user-commissioned chat is changing installation-time validation
transfer in this same checkout. Its `shipped_validation` implementation and
bootstrap/catalogue changes are not covered by Runs130–132 and are excluded from
this checkpoint. The passing original-reuse repair does not approve synthetic
provisioning evidence or establish that every shipped component has valid
release qualification. Integrate that work only after its own connected review.
