# Main CI scope repair — 2026-11-15

The two existing Reborn/Code Style workflows now also accept pushes to Main.
Direct pushes compare the actual before/after endpoints, including both sides
of moves. PR and merge-group comparisons retain their existing merge-base
semantics. A null initial base uses Git's empty tree; malformed, deleted or
unresolvable push endpoints fail scope discovery instead of producing success.

Manual/reusable Reborn invocations preserve their existing scope and defaults,
including false-valued inputs. The discriminator follows GitHub's documented
[input context](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts#inputs-context)
and [reusable workflow event payload](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_call).
Its live server-side behavior still requires an actual new Main run.
Separate concurrency keys prevent direct and reusable push invocations from
cancelling one another.

The shared classifier now connects declared migration/process/hooks/replay risk
owners to their existing Reborn gates, preserving prior legacy/core scope.
The actual hooks owner is `brassclaw_hooks_pg`; its old spelling remains covered.
Unembedded prose avoids Rust work. Embedded prose retains its real build scope.
No new CI service, permissions, branch rule or status context is introduced.

No-Panics supports explicit endpoint comparisons and is required for code pushes.
Both rollups reject failed scope discovery or missing/nonboolean outputs. Code
Style also requires its matrix, No-Panics and conditionally requested CLI checks
instead of accepting required skipped work. Existing version checks are unchanged.

## Local review and validation

- Full five-file source diff and connected callers/rollups reviewed by Root.
- Initial reviewed private patch SHA-256:
  `ce22dbe365e0b7110fb883e916514dac336a9bb351f3b1394f23c22b2274c1b3`.
  Root's final default actionlint/Shellcheck pass then found an unused Reborn
  `COMPARISON` variable. Its two unused assignments are removed; Code Style's
  genuinely consumed comparison output remains. No warning is suppressed.
- Private real Git/shell fixtures: 69 passing controls, including initial,
  multi-commit, sibling/amended, moved and deleted files; invalid endpoints;
  actual rollup refusal; and ten additional actual risk-owner commit cases.
- The complete classifier suite passes with 82 named cases, including actual
  embedded-input coverage. Forty-four declared risk-path families were checked.
- Integrated Main classifier suite passes; seven integrated No-Panics unit tests
  pass. Both complete integrated workflows pass actionlint.
- The small local fixtures create only their own repositories without remotes;
  no Cargo, model, Modal or GitHub Actions job is run by those checks.

Detailed retained local evidence is under
`/private/tmp/brassclaw-ci-proposal-01a1dc5d/`. The native lifetime acceptance is
separate; see [its exact scope](2026-11-15-global-monty-lifetime.md).

There were no observed Actions/check runs for the earlier exact Main
`6f4c6f4e040d93d1b9960e6e44547db00d156ee5`; zero runs is not a passing CI result.
Publication and subsequent live GitHub results must be reported separately.
This workflow change can schedule roughly 48 existing runner jobs for its own
code scope (including Linux, macOS and Windows), not a measured cost or speedup.
No historical run is dispatched again and no successful status is synthesized.
