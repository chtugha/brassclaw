# Deploying reviewed component submissions

> **Current status — 2026-10-22:** use [the combined implementation and ownership
> index](../implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


Settings → Validation queue → **Deploy reviewed components** connects retained
Sempai/operator submissions to the existing live catalogue activation service.
Preparation verifies exact graphs and existing trusted usage approvals. Activation
is the explicit operator action; it does not produce Q1, behavioral or human-Q2
evidence, run an LLM, or execute/re-audit a Recipe's usages.

1. Retain the complete Recipe proposal through the component submission API.
   Its immutable dependency selection includes the reviewed Skills, PythonCode,
   ToolSkills and Tools it uses. New/changed authored combinations require the
   existing trusted Q1/behavior/human-Q2 path first. Verified bundled combinations
   reuse their installation-owned approval.
2. Enter the Recipe submission IDs, one per line, in the deployment panel. Select
   the workflow class and supply existing exact approval IDs as a JSON map from
   Skill UUID to approval UUID. `{}` is sufficient for verified bundled usages;
   it does not approve authored usages. Each batch uses the selected workflow class.
3. Click **Prepare deployment**. The panel loads the subjects/checksums and current
   generation. The server reads a consistent immutable snapshot, preserves other
   active Recipes, and validates compatible approved usages, bindings, exports and
   registered retained implementations. Nothing is activated or executed.
4. Review the exact selections, preserved/replaced Recipe counts and pinned source
   subjects. Click **Activate deployment**. Publication rechecks the subject bytes,
   exact dependency selections and replacement base inside the existing serialized
   generation-CAS transaction. Running tasks keep their previously pinned selections.
5. After an unknown commit, use **Retry exact activation**. The panel locks the
   original activation ID, generation and request bytes. A known stale-generation
   rejection permits preparing again. A retry of a recorded historical activation
   never republishes a withdrawn generation.

Preparation accepts **complete Recipe subjects**, including their dependency
closures. A standalone component is deployed by an approved Recipe graph that
references its exact revision. This does not implement a new native Tool loader
or standalone approval mechanism; unsupported/unapproved graphs fail closed. If a
shared component replacement affects several Recipes, include the coherent Recipe
subjects together. Conflicting old/new dependency selections are rejected; the
server never silently updates an unrelated workflow.

The retained subject's original `unreviewed` receipt remains an authoring receipt.
It is not rewritten to claim approval or activation. Trusted exact usage approval
and the immutable catalogue activation record establish those separate facts.

## Operator API

Read `GET /api/webchat/v2/catalogue` for `generation`, then inspect each subject
with `GET /api/webchat/v2/component-review-submissions/{submission_id}`. Prepare:

```json
{
  "activation_id": "<fresh canonical UUID>",
  "expected_generation": "<inspected current generation>",
  "recipes": [{
    "subject": {
      "submission_id": "<retained Recipe submission UUID>",
      "subject_checksum": "<exact checksum returned by inspection>"
    },
    "workflow_class": "deterministic",
    "association_approvals": {"<Skill UUID>": "<existing approval UUID>"}
  }]
}
```

Send this to `POST /api/webchat/v2/component-deployment`. The result contains an
`activation` request, preserved/replaced Recipe IDs and `catalogue_activated:false`.
Send that **unchanged** activation request to `POST /api/webchat/v2/catalogue`.
Its optional `deployment_subjects` are pinned again before publication and retained
in the existing immutable activation receipt. Empty pins preserve compatibility
with the general catalogue API. Raw catalogue replacement/withdrawal remains a
separate management operation; a pinned-subject deployment cannot drop or change
unrelated Recipes. It also preserves the active generation’s Recipe routing
disables. Use the separate routing panel for an explicit routing change.

Both endpoints are instance-operator only and retain the existing bearer,
origin/CSRF, body-limit, rate-limit and audit controls. The preparation route is
read-only despite using POST for structured selectors. Limits are 512 subjects
and the existing 14 MiB HTTP transport; evidence/graphs are never truncated to fit.
No new provider permission, Tool grant or runtime approval lease is introduced.

## Acceptance

Local acceptance passed with real PostgreSQL, authenticated operator ingress and
the ordinary global-Monty runtime. The native deployment test verifies effect-free
preparation, rejected altered source pins/graphs, exact active replacement bases,
preservation of unrelated Recipes, unchanged usage approval evidence, generation
conflicts, immutable subject pins in activation receipts, idempotent activation and
no republishing after withdrawal. Both deployed Recipes produce their exact replies
and retain their Recipe UUIDs in the settled task outcomes; a completed Tier-2
fallback cannot satisfy this check.

Six frontend state tests and two route/policy contract tests pass. Frontend checks
cover preparation-only behavior, duplicate activation clicks, unknown-commit retry
identity, stale-generation recovery and failed preparation. Strict affected-library
Clippy passes with warnings denied. The final matching worker/test capture has no
source changes during compilation; protected binaries and source hash manifests
are retained in `brassclaw-component-deployment-30pgbh7k` under the local temporary
directory. That capture includes the concurrently reviewed Cargo optimization
settings. The earlier unoptimized capture's macOS `__eh_frame` linker warning remains
in its build log; it was not suppressed.

This is local implementation evidence. No operator components were deployed on
the test machines, and no visual browser acceptance was performed. This feature
does not complete the future Sempai workflow conversion in `sempai_v3.5.md`.
