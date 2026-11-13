# Speedup2 build-flow review and integration — 2026-11-13

The user prioritizes review and integration of external package `04565b6f8`
from `/private/tmp/brassclaw-speedup2-package-report-20261113.md` over the next
compiler batch. Its original worktree/branch remain intact. The local plan
`speedup2.md` stays untracked; neither the plan nor unrelated operator work is
added to Main.

Root reads the complete original eight-file diff and runs its five existing
suites: 18 build-environment, 6 worker, 6 source-sync and 80 Sweepfix cases,
plus fifty classifier cases pass. These are unit/transport checks, including
explicit SDK fixtures; no Modal call or native cross-build is performed.
The reported 46 new classifier cases are actually 46 added lines, nineteen
new cases. Passing suites do not cover the following concrete review findings:

- The Remote collector exports every executable cache candidate by glob/mtime,
  always from debug, without current-invocation/package/target/profile custody.
  Accepted release/profile requests can therefore return unrelated old programs;
  an empty collection can still report success. The plan's glob requirement
  itself needs correction; it cannot establish qualified artifact identity.
- A leading global Cargo flag still bypasses the proposed subcommand classifier.
  Root's real pure-function probes return `--offline` for `cargo --offline check`
  and `-v` for both verbose clean forms. No destructive command is executed.
- All four actually embedded README translations are classified docs-only by
  the original patch. Root executes and retains those counterprobes separately.
- Remote test support also needs coherent profile/target syntax, immutable image
  worker identity, mode-specific qualification and original-only verified artifact
  recovery. Existing product-image activation cannot qualify a new test mode.

W-06 (`01a1d2fa-413a-76b2-a47a-f0953f25fd36`, isolated `f8e4`) supplies a safe
first checkpoint separately from the unfinished Remote work. Root reviews all
seven files and the two follow-up corrections: `65a14e513`, `1f3c132e6`,
`dd277b7ab`, based on exact `489f57998`. Main fast-forwards to
`dd277b7ab64abf7a0e5432f3c5a4421e56ade29f` without altering the four pending
Root development reports or unrelated operator changes. Remote controller and
worker blobs remain byte-identical to their pre-package Main versions.

The accepted checkpoint parses a bounded Cargo global grammar, including an
initial toolchain selector, before applying the existing local lock and both
15-GiB/90-percent disk gates. Package clean requires a real nonempty package
value in supported split/attached forms; global options, aliases, separators,
configuration and directory overrides cannot bypass custody. Unscoped doc
cleanup is rejected. Existing standalone test-manifest calls, top-level
inspection and settings inspection retain their supported behavior. No Cargo
process, cloud probe or source synchronization is started by settings inspection.

CI reads 298 literal immutable inputs from the existing release/seed owners and
checks release-inventory coverage. Its model-view disposition is not embedding
status. Embedded archived documents, data and translated READMEs trigger Reborn
validation; development policy, build-environment/handoff/status reports and
local Speedup plans remain unembedded. UTF-8 decoding is explicit. The existing
unconditional installer job gains the Python helper suites and classifier test;
action pins, permissions and other gates are unchanged.

Actual Root checks on integrated Main:

| Check | Result |
| --- | --- |
| Build environment/guard suite | 23 passed, 0 failed |
| Existing Remote worker suite | 5 passed, 0 failed; no new test mode |
| Source synchronization suite | 6 passed, 0 failed |
| Sweepfix suite | 80 passed; expected negative duplicate/malformed-TOML diagnostics retained |
| Scope classifier | 69 named scopes plus complete 298-input coverage passed |
| actionlint on changed workflow | passed, no diagnostics |
| Whitespace and exact input classification | passed; translated READMEs embedded, operator/development paths unembedded |

No Cargo compilation, image setup, cloud execution, activation, benchmark or
remote artifact qualification is claimed for this Python/Shell/CI change.
Existing installer/uninstaller implementation is unchanged and its applicable
prior file-operation evidence is reused. The package also exercises those suites
in its isolated checkout; unavailable macOS systemd validation remains explicit.

## Corrected Remote package integrated

Root reviews the full connected controller, worker, tests and documentation,
then returns three bounded follow-ups to W-06. The final branch tip is
`9b8421013380d7538fda72207fd563329e7afe9b`; Main merges it as
`9bb386191351ef5ebfac27594e41717ab3d6f4c7`. Its net six-file change preserves
the accepted local/CI checkpoint and Root's intervening dependency repairs.
The original external patch is retained in the branch history as `b53203e26`;
its original worktree and `04565b6f8` branch are untouched.

The bounded Apple composition-library test preparation now normalizes supported
JSON/profile syntax before admission and attributes its one executable to the
actual Cargo invocation's package, target, profile and features. It verifies
canonical path, regular-file identity, Mach-O arm64 executable header and a
private immutable snapshot. Source/image/embedded-worker identities are pinned;
repository synchronization cannot replace an old image's `/opt/brassclaw` worker.
No cache glob, mtime ordering or product-build activation selects a test binary.

Initial execution and status use one collector. Lost spawn acknowledgements,
interrupted downloads, terminal SDK results after durable worker completion and
expired SDK output reconcile the original UUID/Volume/environment, without a new
job or Cargo replay. Downloads verify size/SHA-256 before atomic publication.
Existing canonical regular artifact bytes are verified and reused instead of
retransmitting large binaries. Missing or unverifiable evidence stays unconfirmed;
output expiry does not establish a failed compilation. Only an actual terminal
SDK result plus confirmed absent worker completion can establish that failure.

A review hypothesis about an endless timeout poll is explicitly withdrawn:
installed Modal 1.6.3's `FunctionTimeoutError` inherits its own SDK `TimeoutError`,
not Python's built-in poll exception. Actual SDK source/MRO inspection confirms
the distinction. Tests model that real hierarchy; no synthetic overlapping
inheritance or claim of fixing a reproduced endless loop remains.

Actual Root checks on the merged source, with unchanged recorded helper hashes:

| Check | Result |
| --- | --- |
| Build environment/controller suite | 45 passed |
| Remote worker suite | 11 passed |
| Source synchronization suite | 6 passed |
| Sweepfix suite | 80 passed; expected negative-fixture warnings retained |

Logs, source identities and results are retained under
`target/validation/speedup2-remote-integrated-20261113/`. These tests use actual
local files/locks and explicitly synthetic SDK transport/Cargo JSON/header
fixtures. They are not real compiler images, cloud executions or native tests.
Prior unchanged classifier/inventory, installer, uninstaller and workflow
evidence remains applicable; it is not repeated merely for a second receipt.

Normal test offloading remains **DEACTIVATED**. A newly prepared exact worker
image, genuine Cargo-protocol interoperability, native loading/linking/unwind,
test fixtures and companion workers, actual selected execution and a connected
test-specific acceptance/activation path remain open. No paid image build,
cloud job, new activation or measured speed advantage occurs in this package.

Combined entry SP-02, canonical runner SP-03, optional artifact CAS SP-04,
native-image diagnosis SP-05, complete remote SP-06/SP-07 and test-machine
SP-11 acceptance remain open. SP-10 supplies the dual disk guard; it does not
implement general cache cleanup. This integration neither completes Speedup2
nor any Bob/v3 product plan. No worktree is deleted.
