# B3-R02 — Provider UI baseline repair handoff

Source checkpoint for supervisor review, 2026-11-13. This closes the assigned
source/test repair only; integration and final native acceptance remain supervisor-owned.

## Source identity and diff

- Worktree: `/Users/ollama/.codex/worktrees/c530/brassclaw`.
- Branch: `codex/b3-r02-provider-ui-harness`.
- Starting Main: `1bc98c0d284e4f966b7c1d72c48ade78deb39866`.
- Tested source commit: `6fd7dcb334e3b5701bd03ec11b4e06963e1c39a9`.
- Changed source: `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-components.test.mjs`.
- Source diff: 215 insertions, 69 deletions in that one existing test file.
- Exact diff command: `git diff 1bc98c0d284e4f966b7c1d72c48ade78deb39866 6fd7dcb334e3b5701bd03ec11b4e06963e1c39a9 -- crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-components.test.mjs`.
- Tested file SHA-256: `ce0e6174d2997fceb5571892cd419f27bb93ec01e70b45f942c8a658bb640634`.

This handoff is a separate unembedded development document. No root status,
plan, CHANGELOG, architecture guide, Rust, migration or B2 file is changed.
No Main merge/push, subdelegation, Cargo, native compilation, second target,
Modal call/sync/image work or provider request was performed.

## Relevant architecture and ownership boundaries

The four component guides distinguish Recipe sequencing, one reusable Skill
usage with associated preloadable PythonCode, nonexecuting ToolSkill binding
and Rust primitives/live kernel policy. Task-start immutable selections and
original attempt/effect custody must survive later catalogue changes. A passing
UI projection test grants no Q1, human Q2, activation or Tool permission.
The v3.5 roadmap and implementation index keep these shared responsibilities
with their existing owners; this package introduces no new component or workflow.

The WebUI route layer consumes `RebornServicesApi`; native auth, secrets,
persistence and runtime publication remain behind their existing owners.
This repair uses existing provider projection/parser/transport modules and the
existing shallow template/hook test mechanism. It does not add a provider/auth
abstraction or change authentication. B3-09 class-21/22 submissions remain
inspection-only. Pure-approval/viewed-subject decisions stay with B1 and their
later UI integration; Skills/Engine retained-selection parsing, stores and V215
stay with B2-R01. The incoming review's rejected legacy graduation shortcut is
not restored.

## Diagnosis and connected repair

The exact initial command below reproduced **4 passed, 11 failed, zero skipped**
on the assigned baseline. Three ProviderManagement cases referenced the missing
`ProviderBootstrapStatus`, seven ProviderCard cases referenced the missing
`ProviderAuthStatus`, and one Codex onboarding case referenced that same missing
auth component. The old VM helper discarded all static imports and injected an
outdated flat context. These failures occurred before the intended assertions.

The existing product modules are `provider-bootstrap-status.js` and
`provider-auth-status.js`; they are not empty placeholder functions. The former
uses the original bootstrap receipt, existing subscription, strict target/result
parsers and original-job reader. The latter projects native auth observations
in compact and detailed views, returning null when the observation is absent.
The existing `provider-bootstrap.test.mjs` already drives real component logic
with controlled React scheduling and transport, including late responses and
unmount fencing. That coverage is executed in the final batch.

The repaired local VM loader resolves every named static import through an
explicit module-path namespace, checks its exported binding before evaluation,
and rejects unsupported import syntax instead of silently stripping it. This
also checks imports on branches that the selected render does not visit. It
loads the real AuthStatus, BootstrapStatus and PrefixModelSetupDraft source;
their provider/bootstrap/setup/API/transport libraries use actual ESM namespaces.
Existing shallow child/design-system boundaries and controlled React scheduling
remain explicit. Unexpected hook execution outside a fixture fails loudly.

ProviderCard now uses the real provider helper namespace as ProviderManagement
already did. Its old helper substitutions interpreted invented `configured` and
`missing` fixture fields. The custom fixture now supplies its real optional-key
and base-URL configuration; the missing-key fixture supplies `has_api_key: false`.
The existing negative/action assertions remain intact and run through the real
predicates. No product defect was exposed that required production source changes.

All original 15 cases remain. Eight added cases exercise the actual status
components through their ProviderManagement/Card/onboarding parents: absent
bootstrap target; absent/null auth; compact/detail current-with-unknown-lifetime,
pending setup, failed original refresh, missing account/custody recovery and
unsupported login; and missing/unsupported onboarding status. Checks preserve
provider identity, original refresh reference/outcome/publication/error code,
expiry visibility, support/recovery text and explicit serving-scope explanation.
These are observed-state fixtures, not simulated successful authentication.

## Actual validation

Environment: Node `v25.6.1`. No runtime JS source changed after this batch.
The receipt captures all tracked input hashes, HEAD, the test-file before/after
hashes, exact runner, complete logs and terminal results:
`/private/tmp/b3-r02-provider-ui-validation/` (`source.json`, `applied.json`,
`original-runner.py`, `01.log`–`03.log`, `completed.json`). Source unchanged: true.
The source commit contains exactly the test-file bytes from that receipt.

Baseline command, before changes:

```sh
node --test crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-components.test.mjs
```

Final connected batch (actual command, no skips/ignore/suppression):

```sh
node --test \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-components.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-runtime-status.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-mutation-status.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/llm-providers.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-bootstrap.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-status.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-login.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutations.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutation-auth.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/model-setup-draft.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useProviderLogin.test.mjs \
  crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useProviderLogin-query.test.mjs
node --check crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-components.test.mjs
git diff --check
```

Results: **109 passed, 0 failed/cancelled/skipped/todo**, including **23** actual
provider-components cases (the original 15 plus eight additions); all three
commands exit 0. Node test process wall time: 0.397 seconds. Syntax check: 0.044
seconds; whitespace check: 0.298 seconds. No failed intermediate corrected run
was suppressed or replaced. The known baseline failed before repair; the final
connected batch passed on its first execution.

## Self-review, limits and supervisor follow-up

The complete source diff was reviewed before the connected batch: module-path
resolution, missing exports/import forms, actual helper contracts/fixtures,
parent prop identity, absent/null observations, negative states and existing
callbacks. No assertion was deleted, no case skipped, and no auth/review/runtime
security check was weakened. Current scope is Node template/projection and
controlled hook/transport evidence. It is not DOM/browser or real React lifecycle,
issuer, credential, PostgreSQL, Monty, deployment or full Bob-3 acceptance.

`brassclaw_webui_v2_static/build.rs` embeds every file under `static/`, including
this `.test.mjs`. Therefore the changed test asset is a real packaging input even
though all production JS modules remain unchanged. The supervisor should review
the complete diff, include this asset change in the next coordinated native
static-package acceptance (`cargo test -p brassclaw_webui_v2_static`, existing
asset/router cases), and verify actual embedded bytes/input applicability before
claiming a new packaged executable. No new native auth/PG claim follows from
this test-only patch. Prior Run-59 cannot prove the changed test asset bytes.
Final lint/native check selection and integration remain supervisor-owned.
Full B3-09 decisions, B3-12 batch completion, B1/B2 cutovers and deployment remain
open. No external maintainer/human-Q2 action is needed to commit this development
harness fix; that does not approve installed authored components.
