---
name: reborn-feature
description: Plan, implement, or fix Reborn capabilities and their WebUI, API, settings, facade, and runtime integration under the final-v3 architecture. Start with Recipe/component reuse and trace the affected product path. Use for Reborn feature integration; component-only authoring follows the repository component guides.
---

# Building a Reborn feature against final v3

This is a development skill, not a BrassClaw runtime Skill. It supplies repository
routing and integration rules; the linked policies and component guides own the
full contracts. Maintain this file as the canonical source and keep
`.claude/skills/reborn-feature/SKILL.md` byte-for-byte synchronized. Repository
links resolve from either skill directory.

## Establish the current gate and bounded change

Read root `AGENTS.md`, the [development policy](../../../docs/development-policy.md)
and the [current implementation status](../../../docs/implementation-status.md).
Reuse current session knowledge; reread only affected changes or unresolved facts.
Read the applicable crate `AGENTS.md`/`CLAUDE.md` and relevant plan sections, not
all subsystem documents for every edit. Architecture targets are requirements;
dated receipts establish only their exact observed scope.

For ongoing v3 implementation, check the [priority conformity gate in simplified_v3](../../../simplified_v3.md#vorrang-konformität-des-bereits-implementierten-stands-herstellen)
before continuing the ordinary remaining-work lists. Use its current completion
record; do not cache K-step status in this skill or redo accepted steps. While the
gate is open, implement the next authorized applicable correction in the existing
implementation identified by the relevant plan before regular plan continuation.
Do not turn a separate authorized doc fix or bounded maintenance request into
unsolicited implementation of every K-step.

For a new or changed capability, first present its candidate Recipe: reusable
usages, ordered steps, typed handoffs, completion and exact missing support.
Search the affected component families, variants, seeders and supported catalogue
before designing Rust services. Prefer a compatible existing usage/variant or an
explicit Recipe of reusable components. For infrastructure or operator-management
work, identify the affected Recipe/product path; do not invent a runtime Skill
or Tool to represent a settings endpoint or an agent-documentation edit.

Before creating or changing components, read all four ground-truth guides:
[recipe.md](../../../recipe.md), [skills.md](../../../skills.md),
[tools.md](../../../tools.md) and [toolskills.md](../../../toolskills.md).
Before editing shared code, identify the existing responsible implementation and
its boundary for each affected responsibility; trace its producer and consumers.
Use these areas for navigation, not as new architecture layers or authorities:

| Area | Responsibility |
| --- | --- |
| Recipes and components | Domain behavior, reusable usages, typed handoffs, workflow sequencing and domain completion. Review workflows express their checks through supported components. |
| Existing native infrastructure and kernel | Tools provide declared primitives. Existing services supply coherent selection, retained loading, authenticated persistence and observations, transport, lifecycle and effect custody. The kernel retains authority and technical enforcement. |
| Product, API and UI | Operator interaction, configuration, status and attachment to existing component and infrastructure interfaces. |

Read only relevant sections of [Composition](../../../docs/plans/monty.composition.md#concurrent-implementation-with-simplified-v3)
for components and consumers; [Simplified library integration](../../../simplified_v3.md#gemeinsame-runtime-verträge-für-die-komponentenmigration)
and [admission/recovery](../../../simplified_v3.md#gemeinsamer-admission--und-recoveryvertrag-für-produktaufträge)
for shared runtime/security changes; [Validator](../../../validator_v3.md#concurrent-implementation-with-simplified-v3)
and its [shared runtime support](../../../simplified_v3.md#gemeinsame-runtime-anbindung-der-komponentenprüfung)
for review contracts; [Prefix](../../../docs/plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion)
for prefix functionality; and [v3.5 handoffs](../../../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations)
for remaining workflow migration. Applicable Simplified Phases 0a/3a/6/8 and
section 10.1 retain their runtime, upgrade and provider contracts.

The Bob plans break down repairs to these existing implementations:
[B1](../../../Repair_bob_1_v3_at_k3.md) covers subjects, checks, evidence and
approval; [B2](../../../Repair_bob_2_v3_at_k3.md) covers qualified selection,
approval pins, readers and publication; [B3](../../../Repair_bob_3_v3_at_k3.md)
covers durable original work, lifecycle and native Codex/Auth integration.
They introduce no additional runtime authority or service.

Use [B1's shared dependency order](../../../Repair_bob_1_v3_at_k3.md#required-order-across-all-three-plans)
and current scoped evidence; do not cache repair completion in this skill.
`missing4_v3_at_k3.md` is superseded navigation/history, not a parallel checklist.

Extend the existing implementation for a missing responsibility. Other plans
consume its result and contribute applicable acceptance cases. Reuse qualified
loaders, stores, supervisors, settings publishers and evidence writers; do not
create another queue, VM, provider catalogue, approval path or workflow loop.

For new Tools and changed workflow handlers, apply the
[V35-08 semantic review](../../../path_to_v3.5.md#guidance-reconciliation-and-semantic-review)
in the existing review record: candidate Recipe, actual reuse identities, primitive
versus shared-support gap, the existing responsible implementation, its boundary
and applicable owning plan, retained binding/approval route and ordinary-caller
refusal/failure/no-replay evidence. Inspect actual method and export
responsibilities; a wrapper Recipe or host-call count proves no
workflow conversion.

## Inspect the actual producer, handoffs and consumer

Use bounded `rg`/`rg --files` searches for the requested behavior. The following
are navigation hints, not a mandatory repository-wide search batch:

| Affected area | Source starting points under `crates/` |
| --- | --- |
| Product contracts and WebUI | `brassclaw_product_workflow/src`, `brassclaw_webui_v2/src/descriptors.rs`, `brassclaw_webui_v2_static`. |
| Runtime/facade attachment | `brassclaw_reborn_composition/src/{factory,runtime,runtime_input,webui}.rs`; `brassclaw_reborn_cli/src/commands/serve.rs`. |
| Retained assembly and execution | `brassclaw_engine/src/memory/{retained_instruction,retained_inputs,retained_tools,retained_reviews}.rs`; `executor/{retained_preload,retained_source,retained_recipe}.rs`. |
| Catalogue and activation | Composition `installed_monty_catalogue.rs`, `authored_monty_catalogue.rs`, `component_deployment.rs`. |
| Submission and review | Composition `pg_review_submission.rs`, `pg_workflow_review.rs`, `pg_association_review.rs`; immutable revision/review/association stores in `brassclaw_skills`. |
| Runtime and settings | Composition `global_monty_startup.rs`, `live_monty_settings.rs`, `pg_monty_vm_settings.rs`; actual `brassclaw_monty_host` callers. |
| Provider and prefix jobs | Composition `pg_provider_repo.rs`, `llm_config_service.rs`, `prefix_job_submission.rs`, `prefix_job_definitions.rs`. |
| Completed-turn and MCP | Composition `completed_turn_review/`, `pg_review_inspection.rs`, `mcp_recipe_catalogue.rs`, `mcp_command_qualification.rs`, `mcp_chat_bridge.rs`, `mcp_provider_gateway.rs`; actual ingress listener wiring. |

Read the actual definition, selected feature/backend implementation and affected
callers before changing an interface. Trace inputs/results, errors, ownership,
transactions and side effects; find affected trait implementations. A similar
name, seeded row or old example does not prove registration, support or approval.
Resolve source-visible assumptions before compiling. Use a small experiment only
for a concrete uncertainty source inspection cannot settle.

For a management feature, follow the affected vertical:

```text
browser -> apiFetch -> descriptor/handler -> RebornServicesApi
        -> product port -> composition adapter -> store/live control owner
```

Use an existing comparable vertical to verify attachment. Read only affected
portions of factory/runtime/serve/webui. Reuse `with_*` builders and compatible
ports; expose facade-shaped handles. HTTP handlers use `state.services()`, not
raw stores, kernel dispatchers or host-runtime access. Composition must not depend
on the v1 `src/` tree or unextracted legacy crates.

For task work, follow ingress -> the already-running global Monty -> matching ->
IBS -> pinned execution -> host/kernel -> reply/history. Internal ResultOnly work
uses its qualified private admission and settlement contract, without fabricated
public chat matching or an assistant reply. One supervised global service starts
after migration/seeding/integrity and before producers/ingress; turns/children/waits
retain isolated task contexts. No per-chat replacement VM or Rust fallback loop.
For interpreter changes also read the [vendored compatibility contract](../../../vendor/monty-control/BRASSCLAW.md).

## Preserve the execution and approval boundaries

Apply the four guides to the affected graph; do not restate their full schemas in
new DTOs or invent fields for old constructors, Recipe JSON or association formats.

- A runtime Skill is one Tool usage: prose plus explicitly associated preloadable
  PythonCode, declared exports/signatures and helper/constant dependencies. It
  needs no dedicated Recipe; many consuming Recipes may reuse it. Only intended
  MCP exposure requires a qualified MCP-call Recipe/variant. Current deployment
  publishes complete Recipe graphs; do not claim standalone library activation.
  Pure helpers remain
  PythonCode and need no artificial zero-Tool Skill or ToolSkill.
- Each component step references one stable UUID using persisted
  `knowledge`/`stepnumber` and supported typed input mappings. A nonexecuting,
  nonauthorizing ToolSkill binding normally immediately precedes its matching
  PythonCode invocation. Loading definitions has no Tool/model/I/O effects.
- Pin one coherent approved catalogue selection, exact interface/export/code/
  association/approval/native artifact/ABI resolution, variant and input layout.
  Children, retries and resumes keep it. Mutable inputs/defaults/closures/results
  stay isolated per invocation; runtime values never become Python source.
- Inspect the actual `python-preload/2` reader, retained inspector and selected
  caller. The ordinary retained runner currently accepts flat, ordered execution:
  each selected static executable step may begin once, with at most one bound
  Tool dispatch per execution step and `stop`/`max_attempts: 1`. Recipe-level
  branching and repeated occurrences require qualified successor support. A
  dependent-chain or retry declaration alone enables neither. Recheck source and
  scoped acceptance before relying on an extension. This does not prohibit
  ordinary control flow within a permitted pure function or reuse of one component
  in distinct static steps; pure steps still need no Tool binding.
- A static step identifies a declaration in the selected workflow; its component
  UUID is not an execution occurrence ID. A dynamic occurrence is a host-verified
  execution of that step under the qualified flow. A Tool retry repeats the same
  logical invocation under its approved failure contract. A run/worker attempt
  fences execution; replacing it does not create a new occurrence, reset
  invocation counts or authorize replay. Use the actual shared identity contract,
  not invented fields or caller-selected IDs to disguise a retry as new work.
- Independent pure execution needs its own exact root/export/closure evidence;
  membership in a usage dependency graph is insufficient. Consume validator's
  [remaining pure-review contract](../../../docs/reborn/pure-component-review.md#exact-remaining-cutover-contract).
  Source facts, private check Recipes and sealed observations are prerequisites,
  not general Q1 or approval. Do not widen the closed compiler exception.
- Current global Tool policy and technical constraints apply at every dispatch,
  across retained versions/aliases. Claims/attempts fence execution, not Tool
  permission. Host denial, cancellation, exhausted budgets and unresolved effects
  remain authoritative even if Python catches an exception and returns success.
- Only genuine No-Match enters Tier 2; DB/matching/assembly errors, ambiguity and
  begun Recipe failures do not. Preserve original invocation/count/key/arguments
  and confirmed/unresolved effects. Timeout is not no-effect evidence; never
  replay a completed effect or whole Recipe to repair output/reply persistence.
- Deterministic eligible usages may be Tier 0; shell and subagent Recipes are
  Tier 1. Development reasoning is not restricted by product Tier-0 rules.

Follow the [own functional approval and successor compatibility contract](../../../skills.md#own-functional-approval-and-successor-compatibility):

- **Authored changes (A):** each changed revision needs own trusted Q1, observed
  behavior and human Q2. Skill and implementing PythonCode share one functional
  review with separate immutable IDs. If both change, coordinate their own
  approvals atomically; an unchanged approved partner or dependency is context,
  not another review candidate. Reuse implementing-code subchecks instead of
  creating a second full review.
- **Compatible A successors:** preserve contracts still needed by valid consumers
  for new use. Unchanged consumers keep their approvals; the qualified successor
  path needs no extra authored combination approval. Running/waiting/resumed tasks
  keep their exact original selection and artifacts.
- **Shipped qualification (S):** the installation owner independently qualifies
  Tool, then ToolSkill, then Skill/code together through noncircular
  `system_seed` integrity/Q1/behavior evidence. Retain exact Skill-usage
  qualification without additional installed or external-maintainer human Q2.
  Its dependency closure identifies tested bytes, not individual approvals for
  every member. Provenance belongs to the actual revision/usage, including mixed
  graphs; a label cannot turn an installed edit into S.
- **Recipe changes:** validate the supported workflows/variants and their actual
  handoffs, effects, failures and completion using approved components; do not
  reapprove unchanged constituent components.

These successor rules are targets, not proof of connected reader/writer support.
Inspect the selected review/publication consumers before relying on them. Existing
exact authored approval profiles retain their original meaning and scope until
qualified cutover; never reinterpret historical evidence or bypass a missing gate.

For validator changes, follow B1-04/B1-08 and B3-06/B3-07: bind the original
subject/candidate/check-set and selected graph to durable private work before
effects. A wake only notifies; the durable original request determines work.
Authenticate observations first, then construct the exact verdict only after
actual terminal native settlement and required custody acknowledgement. Never
await a task's terminal receipt from its own finish callback. Keep passed,
completed-negative, incomplete/infrastructure failure, cancelled/stale and
unresolved settlement distinct; observation or ResultOnly completion alone cannot
approve or graduate a component. Reconcile original status without replay.

For retained selection and approval carriage, follow
[B2-02's shared versioned contract and reader inventory](../../../Repair_bob_2_v3_at_k3.md#3-define-exact-approval-carriage-before-touching-readers).
[V35-13/14](../../../path_to_v3.5.md#two-distinct-shared-support-gaps) define required
flow and occurrence semantics and extend that same contract with the selection
implementation. Do not introduce a separate manifest or independently reinterpret
its fields.

Qualify dynamic execution as one connected extension of the existing selection,
interpreter, binding and effect-journal path. Monty selects the permitted
branch/item; native checks verify retained membership, typed edges, occurrence
identity and freshness. Record required dispatch intent/count before effects and
retain original arguments, deduplication identity, answer and confirmed/unresolved
outcome. Removing the started-step fence or trusting a caller-supplied loop index
is insufficient. Initial structured support remains sequential and stop-only;
it does not enable Tool retries or general crash replay. Supported waits retain
the original pending occurrence without replaying begun Tool effects. Workflow
qualification covers the allowed repeated usage; no new Q1/Q2 is created per
iteration. Preserve valid old flat selections.

Review the shared versioned schema and checksum domains before changing writers.
Connect serializers, strict nested readers, DB constraints, prepare/publication,
ordinary/private/child/wait/restore/recovery and MCP/Prefix consumers before
exposure. Preserve historical bytes, absence semantics and valid old profiles;
never normalize old records into a successor or assume unknown fields are ignored.
Resolve actual allocated format versions in their existing owner rather than
copying proposed version numbers as implemented support.

A source/status label, development test or task manifest supplies neither approval
nor authority. Submit immutable unreviewed revisions with exact predecessor and
dependency references; preserve overrides and old task selections. Successful
human evidence is written only through the authenticated exact-view review route.

Activation consumes existing valid evidence and runtime support; it does not
rerun effects or create per-invocation review. Follow [simplified §8.2](../../../simplified_v3.md#82-vorgeschlagene-webui-funktionen):
commit the approved catalogue/routing and durable affected-prefix invalidation,
then build the Prefix from that active snapshot. Qualified MCP discovery follows
its existing publication/command gates. On lost commit/reload/publication replies,
read back the original operation and show desired/effective/pending states; never
create a replacement activation, silently downgrade or report false runtime uptake.
Missing Prefix readiness blocks dependent model calls, not qualified model-free
builds or unrelated safe work. Old tasks keep their compatible pinned selection.

## Wire only the product layers the feature needs

| Layer | Required integration when affected |
| --- | --- |
| Product port/facade | Reuse/extend DTOs, errors and facade methods; update actual implementations/re-exports. Unavailable defaults cannot hide a required missing service. |
| Composition | Construct the adapter from existing stores/control handles and attach it in `build_webui_services`; thread only missing runtime/CLI inputs. |
| HTTP | Add or adjust route/pattern/descriptor, thin handler and mounting; update affected descriptor-contract expectations. |
| Frontend | Use existing `apiFetch`, API modules/hooks and explicit pending/error/effective states. Validate changed JavaScript syntax and affected UI behavior. |

Feature gates describe actual configurations, not extra final-v3 product editions.
The authenticated instance-token operator administers all supported functions;
external channel senders do not inherit that authority. Preserve token/CSRF/origin,
sandbox/network/secret and resource enforcement. Remove legacy role/scope/grant
checks only with their real replacement wired. Keep conversation/message/run/
attempt/source identity for correlation, cancellation, custody and idempotency.

Provider definitions/configuration/selection use PostgreSQL and the existing
revisioned owner, without file/compiled-list fallback. Resolve secrets through
protected host brokers, outside model inputs, results and snapshots. Settings
persistence alone is not uptake: preserve desired/effective revisions, feasibility,
acknowledgement, consumed task budgets and visible pending/failed publication.
Keep the 600-second executing-time task default, 512 MiB startup/manual heap floor,
finite physical backstop and independent host allocations. Preserve retired Monty
allocation-count values as migration evidence. Disabled discretionary token budgets
remove artificial retrieval/history/task quotas; technical model input/output and
context bounds still apply. Use existing acknowledged settings for necessary
operational limits, quotas and deadlines. Any missing setting needs its existing
DB/API/WebUI/live-ACK path, units and feasibility rules; do not add hidden local
caps. Protocol/representation bounds and the supported stop/one-dispatch contract
remain technical contracts. Reuse and, where required, extend the existing
final-request validation path. Keep unsupported branches rejected until their
connected implementation and qualification pass. For construction-specific
extensions, follow the Construction context handoff below.

## Apply the relevant domain handoff

- **Child work:** reuse claim-fenced explicit workflow/typed-input admission and
  its captured parent catalogue. Declare a finite recursive profile. Admission or
  `authorized:true` is not completion: consume the actual correlated terminal
  reply/failure, including absent/null distinctions. Wait yields shared capacity;
  unknown/cancelled results never cause an automatic replacement child.
- **Completed-turn Sempai:** reuse existing global delivery and native evidence/
  accounting custody. Review/proposal/iteration semantics remain Recipe-owned in
  v3.5. Candidate components stay unreviewed; reconciliation cannot resubmit model
  work, replay Tools, synthesize completion or activate a proposal.
- **MCP:** derive immutable listing/commands from qualified approved execution
  Recipes, not raw Skills. Calls send the listed completed command once through
  a new ordinary chat and forward its correlated result before non-destructive
  close. Recover lost send/delivery/close at the original chat without replay.
  The server remains instance-owned and alive. Kohai connects/advertises after
  final prefix addition immediately before model send, retaining the connection
  through intermediate Tool rounds; complete answer or owned error/cancel cleanup
  disconnects only that request. The final guard checks the actual transmitted
  request including Tool/advertisement metadata. No direct MCP execution bridge.
- **Prefix:** preserve complete authoritative originals/hashes/provenance separately
  from model-facing output. No row/heading truncation or private deployment data
  in model context. Profile metadata/source-only output/quarantine is not accepted
  design, cache qualification or serving activation. Reuse immutable job definitions
  and shared ResultOnly admission; identical IDs retain original explicit inputs/
  compiler selection, changed payload conflicts, deliberate new work gets a new ID.
  Provider-free qualified builds must not depend on an optional provider/prefix.
- **Construction context:** consume the
  [shared Prefixless successor](../../../docs/reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
  and its [Composition native delta](../../../docs/plans/monty.composition.implementation.md#sole-native-selectioncapturerequest-implementation-delta),
  [Simplified lifecycle contribution](../../../simplified_v3.md#open-prefixless-construction-successor--existing-owner-contribution)
  and [v3.5](../../../path_to_v3.5.md#open-construction-choice-handoff)
  handoffs. Maker -> Base -> Prefixless is only a visible editable form suggestion,
  never a runtime fallback chain. Bind the operator-submitted choice's exact
  generation or intentional absence, executing route/instructions and original job
  before effects;
  stale or incompatible choices fail without reselection. Required children/reviews
  use their own qualified retained context and legitimate lineage. For this
  construction successor, Prefix supplies qualified options, the operator-submitted
  job choice, construction qualification and product integration. Extend the
  existing model-usage implementation under Composition's native delta for
  selection, capture, request preparation, response and guard integration.
  Simplified supplies only genuinely missing shared task, child or continuation
  mechanics; consume those results through existing interfaces. Keep the full
  request guard effective with Prefixless. The Recipe editor
  retains supported Default/profile choices and adds explicit Prefixless only after
  qualification; no Recipe Generate button, baked-in generation or invented old-format
  tag. Preserve old jobs and independent reviewer contracts. Missing native support
  remains an open gate; prose, a declaration or profile metadata cannot enable it.
- **Upgrade/recovery:** contribute source/derived, review, command, native artifact
  and override references to the one migration/backup owner. Retaining an image or
  checksum does not prove compatible old native loading. Block unsafe upgrades or
  drain/reconcile first. Fence the old generation before supervised replacement;
  RAM is not a durable checkpoint and no whole-task replay repairs a crash.

## Complete one work package, then validate once sufficiently

Use the [development policy's evidence table](../../../docs/development-policy.md#choose-evidence-by-the-claim).
No extra planning document, permission round, subagent or reporting framework is
required by this skill. Preserve the user's scope and existing authorization;
component activation gates remain separate from authorization to develop a change.

1. Define the bounded expected behavior, affected producer/handoffs/consumers,
   existing applicable evidence and remaining uncertainty. Scale this to the fix.
2. Complete the connected code/contracts/wiring together. Review types, ownership,
   feature/backend variants and normal/missing/invalid/interrupted cases before
   compiling. Correct source-visible defects as a group. Compile earlier only
   when a concrete unresolved question or shared boundary warrants it.
3. Run the smallest sufficient batch for the claim. Reuse evidence only when its
   source/graph/artifact/configuration identity and scope still apply. Avoid stacking
   check/build/test/lint/release builds for identical targets; a test build can
   supply compilation evidence. Real DB/lifecycle/security claims need the actual
   backend/worker/production caller and relevant failure cases. Read
   `LOCAL_TEST_ENV.md` when present before remote tests or provider setup.
4. Read complete diagnostics and fix the connected causes before rerunning failed
   or invalidated checks. Verify selectors actually ran tests; skipped/zero-case
   checks do not pass acceptance. Do not add helper tests that merely mirror code.
5. Stop validation when required applicable checks pass and continue authorized
   work. Broaden only for new failures, unresolved risks or binding integration/
   release gates. Report actual exercised paths, limitations and unverified claims.

For ordinary prose/development-skill edits, inspect links/consistency/diff and run
`git diff --check`; validate skill metadata and mirror identity when updating this
skill. Do not run Cargo solely for prose. Embedded runtime/seed/document inputs
need their affected packaging/integrity checks before claiming product uptake,
including recompilation when their real consumer requires it. Fix introduced
warnings without suppression; unrelated baseline blockers remain visible.
