# Authored catalogue activation

The operator catalogue API activates already validated usages. It does not run
Q1, ask an LLM to audit Recipe steps, create human-Q2 evidence or execute a
behavioral test during activation. Existing exact usage approvals are reused.
New or changed components/combinations must pass their component-validation
path first. Assembly checks cannot turn an unreviewed component into an approved
one.

`GET /api/webchat/v2/catalogue` returns the active generation and authored Recipe
IDs, complete `selections`, `packaged_recipe_ids` and `disabled_recipe_ids`.
`POST` to the same path accepts a complete replacement of the authored routing
set:

```json
{
  "activation_id": "<fresh canonical UUID>",
  "expected_generation": "<generation returned by GET>",
  "recipes": [
    {
      "recipe_id": "<stable Recipe UUID>",
      "workflow_class": "deterministic",
      "components": [
        {
          "uuid": "<component UUID>",
          "class_code": 21,
          "version": 1,
          "checksum": "<64 lowercase hexadecimal characters>"
        }
      ],
      "association_approvals": {
        "<stable Skill UUID>": "<existing exact association approval UUID>"
      }
    }
  ]
}
```

The component array must contain the Recipe and its entire immutable dependency
closure, with exactly one selected revision per UUID. These are selection
references in the management request; Recipe documents continue to use stable
UUIDs without embedded versions. Include every related Recipe when replacing a
shared component. Conflicting selections are rejected atomically. The workflow
class is `deterministic` or `requires_model`; shell and subagent usages cannot
be activated as deterministic workflows.

The authenticated instance operator supplies the decision. The request body
cannot supply an actor, successful audit report or fabricated approval record.
For authored usages, the reader resolves the existing trusted Q1, behavioral and
human-Q2 records for the exact combination. An unchanged bundled reply usage can
reuse its verified installation-owned approval; its approval-map entry may be
omitted or name that exact existing approval. A source/status label is never
sufficient.

All variants are assembled before publication. Typed handoffs, binding
adjacency, exact approval pins, registered Tool identity, retained implementation
identity and the v3 preload/export interface must agree. These are mechanical
assembly/runner checks. Independent approved usages are not behaviorally tested
again merely because another Recipe sequences them.

V136 retains immutable generation documents and activation receipts, and changes
one current-generation pointer in the same transaction as routing publication.
The API uses compare-and-swap. After an uncertain commit, retry exactly the same
activation ID and content. An identical old receipt does not republish a
withdrawn generation. Empty `recipes` withdraws the authored routing set while
keeping the packaged reply/history workflows.

Recipe-wide routing choices are now retained in `authored-monty-catalogue/3`.
In Settings → Validation queue → **Recipe intent routing**, load the current
selection, uncheck a Recipe to disable its ordinary intent matching, then publish.
The panel preserves the complete authored selections and uses the generation CAS.
After an uncertain response, retry the exact publication before refreshing.

The optional POST field `disabled_recipe_ids` has two meanings: omitted (or null)
preserves the previous routing choices; an array replaces them, and an empty
array explicitly re-enables all ordinary Recipes. IDs must be unique canonical
UUIDs belonging to a packaged ordinary Recipe, the prior/new authored selection,
or an existing retained disable. Internal history and the root orchestrator are
not ordinary routing targets. A pinned-subject deployment preserves routing
choices and cannot combine its publication with a routing edit.

Disabling removes a Recipe from the matching eligibility and qualified MCP
advertisement for the new generation. Its immutable revisions, approval pins and
physical intent rows remain retained. Internal reply/history still run when
needed; a routing choice is not a Tool-policy setting or an approval. A withdrawn
Recipe keeps its disabled rule, so reactivation without explicit routing changes
cannot re-enable it. Already captured tasks keep their prior routing selection.
Restoration retains the exact generation and rule set, including the preparation
policy under which they were published. Historical format-1/2 generations retain
their exact original representation.

This controls whole-Recipe routing, not individual expressions or component-body
overrides. Installation upgrades that change the packaged base still require the
compatible migration described below; the new rule set does not fabricate that
upgrade support. Native acceptance passed on 2026-10-13: actual PostgreSQL/operator HTTP rejects
unknown/duplicate/stale choices, rolls back pointer and receipt after a deliberate
publication rejection, preserves a captured task and one global VM, and restores
the policy after ordinary startup reseeding. Withdrawal/reintroduction preserves
disables; explicit re-enabling restores matching. Historical format-1/2 manifests
restore byte-for-byte. Existing authored activation/deployment/global regressions,
nine frontend state tests and strict affected test-target lint pass. Receipts are
`/private/tmp/brassclaw-routing-overrides-r3-accepted.json` and
`/private/tmp/brassclaw-routing-overrides-r4-accepted.json`. The latter differs only
by the expanded compatibility test and reuses the former’s unchanged-caller
results. This is local routing acceptance, with a controlled test model gateway;
rendered-browser, external-model, changed-seed-base upgrade and full-plan
acceptance remain separate.

Task capture commits actual normal matching against the same generation as its
component graph. Running tasks retain that selection, bindings and prefix;
withdrawal/replacement cannot rematch them or substitute new code. Each Tool
binding retains its actual dispatcher, and current global policy is still checked
before effects. Approval and loading do not enable a Tool. No new global Monty
VM, Rust workflow loop or Tier-2 error fallback is introduced.

MCP discovery refreshes on activation, restart and normal-task settlement. New
command advertising still requires the existing full-command qualification from
actual durable ordinary-chat observations. Until that generation is qualified,
its command list is empty. This is separate from component approval and does not
re-audit a Recipe's approved usages. MCP keeps its ordinary-chat relay boundary.
At ordinary task capture, the durable MCP command receipt must still agree with
the current advertised contract. A command changed or withdrawn before matching
fails before effects instead of entering another Recipe or Tier 2. A task that
has already captured its matching selection retains it across activation.

Current support is limited to graphs for the supported retained runner and
registered retained native Tool adapters. Standalone executable components whose
approval is not covered by a verified usage closure are rejected: legacy labels
and draft receipts cannot supply the missing standalone approval contract.
Packaged global reply/history identities are reserved; installation upgrades that
change the packaged base require an explicit compatible catalogue migration.
This API does not silently replace operator overrides or discard an incompatible
stored generation at restart.

Acceptance on 2026-10-10: the focused PostgreSQL/global-Monty/operator-ingress
test passed (73.18 seconds), including both variants, literal typed input,
unchanged review evidence, idempotent retry, stale/forged activation rejection,
effect-free restoration and completion of an already captured task across
withdrawal. The existing normal-match/No-Match/MCP chat regression passed
(184.57 seconds), as did four MCP discovery contract tests, both locked operator
route-policy tests and affected-library Clippy with warnings denied. Cargo and
cleanup batches used the shared cross-chat lock. These checks establish this
supported activation path; they do not complete the wider v3 conversion plan or
establish authored-command advertising without its required observations.


Catalogue consumer acceptance on 2026-10-13 additionally removes the matcher’s
separate fixed eligibility counters below admitted catalogue policy. Routing
metadata is checked in one complete database read with the original checksum
representation. A 9,000-template operator/global regression passes through its
last entry. Retained-store/parser/database constraints remain separate; larger
catalogue settings do not by themselves certify that every underlying limit is
adjustable. This adds no component approval, Tool permission or body-override
support.
