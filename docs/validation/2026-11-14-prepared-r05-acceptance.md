# Prepared R05 accepted — 2026-11-14

Candidate `2228c28208c5b5e5d8b66b36f7b892d1e4654375` plus Root's single
stronger stale-head assertion, on accepted Main `5a8aadfe1`. Original prepared
commits: `ea3690a467575f9465ed628f95bedf56aba8665e` and
`ec09e6377c330f2ee3d065eaa2c1e27cbb9f1438`; integrated source commits
`443a7471b` and `2228c2820`. Root personally reviewed the connected readers,
CAS/publication callers and actual tests; no correction was delegated.

## Accepted behavior

Three existing PG Auth get/write/list log sites retain Serde category, line and
column instead of formatting the rejected payload. Public typed errors and
native Auth/Codex authority, encryption, account/head selection and effect
custody stay intact. Three real PostgreSQL regression cases prove safe corrupt
account lookup/list followed by actual CAS repair, retention of a late refresh
return after account revocation, and exact BackendConflict after publication of
a separately completed new login. The old return never overwrites either
revoked/new account and cannot dispatch another refresh.

Root tightened the final race assertion from any error to the actual
BackendConflict. The new handoff's cancellation/positive-HTTPS discussion
remains an explicitly unfinished owner seam, not implementation or a prerequisite
of this independent log repair. Local issuer tests use the existing explicit
controlled HTTP adapter and do not certify production DNS/TLS or OpenAI service.

## Actual checks

- Run75: Rust 1.100 strict all-targets Clippy, Skills/db-store,
  Engine/skills-db and Composition/postgres+skills-db, 143.017 seconds, zero
  warnings/errors; production and test feature profiles checked.
- Run76: actual native Rust 1.98.1 test programs, existing unchanged union of
  features including migrate-from-libsql/test-support, 745.759 seconds including
  snapshot custody, no warnings or unwind-limit diagnostics. Current global
  worker reused only after complete dependency/config/compiler/environment/hash
  equality, then staged beside the immutable test image.
- Two actual refresh-race cases pass in 8.580 and 13.503 seconds. Initial
  controller incorrectly addressed the account reader under `tests` rather than
  its real `refresh_tests` module and rejected that selector before execution.
  Failed controller/receipt preserved. Continuation executes only that missing
  exact test (2.036 seconds) and verifies/reuses the two passed original logs and
  the same image hash; no rebuild or duplicate passing test.

All three selected tests run, zero failed/ignored. Build and execution source
captures remain unchanged. Native Composition image SHA256:
`1e72b3049f5c8179e91965d7d7052070f83c38ee9ffb3da40a3300f5e9aa11ca`.

Original commands, toolchains/features/source manifests and full diagnostics:
`target/validation/bob-native-repair-20261105/run-75-prepared-r05-strict/`
and `run-76-prepared-r05-native/` (including both native-acceptance directories).
No new format, approval, catalogue or settings activation, general cancellation
cutover, full Bob-plan, platform, release or deployed acceptance follows.
