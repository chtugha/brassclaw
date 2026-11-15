# Agents v3 documentation

This directory explains component authoring, retained execution and related
compatibility code. The source maps listed as reconciled below were checked on
2026-11-15. A documentation status is not component approval, deployment or
full-v3 acceptance.

## Authority and how to read this set

1. [recipe.md](../../recipe.md), [skills.md](../../skills.md),
   [tools.md](../../tools.md) and [toolskills.md](../../toolskills.md) govern
   component semantics, persisted schema, typed interfaces, approval, binding and
   native execution. Applicable later binding amendments retain their precedence.
2. [simplified_v3.md](../../simplified_v3.md) governs runtime/authorization targets;
   [path_to_v3.5.md](../../path_to_v3.5.md) assigns forward work and acceptance
   handoffs to existing owners. A plan requirement is not implementation proof.
3. [The implementation index](../implementation-status.md), its exact scoped
   receipts and actual current callers establish implemented/accepted boundaries.
   Preserve historical evidence with its original source/artifact/configuration
   scope. Source existence alone is not runtime or installed-instance evidence.
4. These subsystem guides summarize and link to those authorities. They cannot
   override them. Historical examples and older subplans are compatibility context,
   not alternate schemas, approval mechanisms or future execution architectures.
   [The development policy](../development-policy.md) governs development checks.

Ordinary PostgreSQL/skills-db startup already wires one global Monty service and
`GlobalMontyDriver`. Recipes and pinned exports own task sequencing; Rust owns
primitives, integrity, authority, transport, accounting and effect custody. Only
actual No-Match enters Tier 2. Runtime values are typed data, not Python source.
ToolSkill binding performs no operation and grants no permission. New tasks select
coherent active approved successors; existing tasks keep exact original pins.

## Documentation map

| Guide | Reading scope |
| --- | --- |
| [01 — Architecture](01-architecture-overview.md) | Reconciled source map: global lifecycle, component ownership and ordinary execution. |
| [02 — Intent](02-intent-system.md) | Reconciled source map: captured eligible catalogue matching, current result types, explicit error boundary. |
| [03 — Recipes](03-recipe-system.md) | Reconciled authoring/source map: variants, typed handoffs, workflow qualification and retained selection. |
| [04 — IBS](04-ibs.md) | Reconciled source map and partial design example: ordered IBS, exact layouts, exports, binding and stop-only support. |
| [05 — Skills](05-skills-system.md) | Reconciled interface/approval map: one-Tool usage, preload, current exact profiles and the own-functional successor. |
| [06 — Tools](06-tools-system.md) | Reconciled source map: primitive/native identity, registration, approval, live policy and effects. |
| [07 — PythonCode](07-pythoncode-system.md) | Reconciled source map: declarations, inspected exports, pure logic and evidence limits. |
| [08 — Actions](08-actions-system.md) | Older Action/step-machine compatibility walkthrough; verify its actual callers before reuse. It is not the retained Recipe execution specification. |
| [09 — Kohai/Sempai](09-sempai-kohai.md) | Binding prompt handoff and construction successor, with a separately labelled historical interceptor walkthrough. |
| [10 — Prefix](10-prefix-base-prompt.md) | Binding selection/design/construction contracts; historical placeholder/store/prewarm descriptions are not Serving acceptance. |
| [11 — Retrieval](11-retrieval-system.md) | Older retrieval/UNION/loop-bridge map; use 02/12 for ordinary captured routing and global execution. |
| [12 — Turn driver](12-agent-loop.md) | Reconciled source map: actual worker/global driver, task custody, shutdown and live resource contract. |
| [13 — Legacy orchestrator](13-orchestrator-default-py.md) | Historical compose/run script and compatibility context; ordinary retained execution is documented in 01/04/12. |
| [14 — Validation](14-validation-queue.md) | Reconciled evidence/queue map: actual producers, incomplete qualification, human review, class labels and release-reference scope. |
| [15 — Component catalogue](15-component-catalog.md) | Older storage/taxonomy walkthrough; current definitions and approval/selection rules are in the four component guides. |
| [16 — Kernel/composition](16-kernel-composition.md) | Older integration/source walkthrough; verify current runtime wiring and policy through 01/06/12 and the owning contracts. |
| [17 — Prefix UI](17-webui-prefix-tab.md) | Prefix contract plus historical UI/store details; current Serving/Generate acceptance remains with the Prefix owner. |
| [Conversion design](DOC_CONVERSION_MECHANISM_DESIGN.md) | Historical design/decisions plus a verified compatibility-source map, not a qualified current conversion workflow. |

The older sibling walkthroughs and named `subplan_*` files preserve historical
context. Their completion marks are not a second status authority. Follow the
current source maps and owning contracts when a legacy execution description
conflicts. Migration/support gaps stay explicit with their existing owner.

## Component reuse and review

A Skill comprises prose and an explicitly associated preloadable function
interface for one Tool usage. A Recipe provides ordering/data flow; pure PythonCode
needs no artificial Skill/ToolSkill. Each component step references one stable UUID
using persisted `knowledge` and `stepnumber` fields. Preload selected definitions
without effects, dependency-first with stable-UUID ties. Mutable data is isolated.

Current exact authored profiles retain their supported meaning. The binding
own-functional A successor reviews Skill/implementation together with separate IDs,
unchanged approved prerequisite context and human Q2; broader publication remains
an existing-owner cutover. Controlled S qualification independently qualifies Tool,
ToolSkill, then Skill/code and consuming workflows with integrity/Q1/behavior and
null human Q2. Labels never select S. Approval, activation, task selection and
current global Tool permission are separate.

Ordinary retained Tool steps currently permit one bound dispatch and stop-only
failure. A parsed retry declaration, helper include or historical enum does not
supply broader execution support. Preserve completed/unresolved effects and
original selections across waits and failure. See [04](04-ibs.md), [05](05-skills-system.md)
and [14](14-validation-queue.md) for the verified interfaces and owning limits.

## Relationship to forward work

Use the roadmap's actual [owner handoffs](../../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations):

- V35-01/02 reconcile current source/evidence and guidance, rather than repeating
  global startup or inventing another typed loader.
- V35-03/04 consume the existing validator's independent-pure approval work.
- V35-05/06 qualify the reusable reference workflow through the existing retained
  selection, preload, native implementation and publication paths.
- V35-13/14 own remaining qualified dynamic occurrences; Python syntax alone does
  not establish durable repeated-effect addressing.
- V35-17–19 consume the existing validator/review owners; a helper or nonempty-body
  runner does not prove the eight real class routes.
- V35-20 consumes the Prefix owner's exact compiler/qualification/Generate/Serving
  boundaries. Source-only qualification does not establish current serving.
- Shared lifecycle, authority, resource, connection and recovery work stays with
  its existing Simplified/v3.5 owners and scoped acceptance. No new owner/store
  follows from this documentation.

`saved_plan_to_v3.md` and dated subplans remain historical per-phase references
where superseded. They do not replace the current binding guides, amendments,
owning plans or implementation index. Do not retire an owning plan or claim its
remaining acceptance complete merely by replacing its old phase label with a V35 ID.

## Documentation content and release references

[The conversion design](DOC_CONVERSION_MECHANISM_DESIGN.md) contains earlier
source/converted-document proposals. Verified compatibility seeds and file/DB-event
wiring exist; their existence proves neither deployed doc-sync nor qualification
against current preload/approval requirements. Event refresh is not proof of an
idle-time Sempai optimization loop. Reuse compatible parts through current owners.

Document text belongs to **Docu, class 17**, not ExtensionCatalogue class 23.
A catalogue organizes the domain; reference capture, conversion, review,
publication and prefix generation are separate responsibilities.

The validation guide is embedded in the declared release reference inventory.
Its current [inventory entry](../../scripts/prefix/release-reference-inventory.json)
is **development-only**; do not silently promote it into a qualified model view.
Changed source changes exact reference bytes. Preserve old immutable revisions
and receipts; a reviewed documentation successor is not installed uptake or
permission to regenerate/activate a prefix. Existing source-package validation
can establish exact snapshot integrity without claiming component approval.
