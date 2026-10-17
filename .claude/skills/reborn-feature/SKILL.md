---
name: reborn-feature
description: Plan and implement Reborn capabilities and their WebUI, settings, API, facade, and runtime integration using the final-v3 architecture. Start with Recipe/component reuse, then trace the required product and infrastructure wiring. Use for Reborn feature work; ordinary component-only authoring follows the repository component guides.
---

# Building a Reborn feature against final v3

This is a Codex development skill. BrassClaw runtime Skills have their own
preloadable function and association contracts; this file is development guidance.
Inventory existing components and trace the affected production path before editing.
Maintain this `.agents/skills/reborn-feature/SKILL.md` as the canonical source;
keep `.claude/skills/reborn-feature/SKILL.md` byte-for-byte synchronized whenever
this guidance changes. Relative repository links work from either location.

## Start with the Recipe and choose the owning layer

Read the repository's `AGENTS.md`, [development policy](../../../docs/development-policy.md)
and the relevant parts of [simplified_v3.md](../../../simplified_v3.md).
Read affected crate `AGENTS.md` and `CLAUDE.md` files where present. The binding
v3 contracts take precedence over legacy scoped authorization, operation
approvals, per-chat VM descriptions and blanket validation commands.

For a new or changed capability, first present the candidate Recipe: existing
usages, ordered steps, typed inputs/results, completion and any missing primitive
or runner support. Search Recipes, variants, Skills, PythonCode, ToolSkills,
seeders and the supported catalogue before designing Rust services. Prefer an
existing variant, a new variant, or a Recipe composed from reusable components.
Before creating or changing components, read all four ground-truth guides:
[recipe.md](../../../recipe.md), [skills.md](../../../skills.md),
[tools.md](../../../tools.md) and [toolskills.md](../../../toolskills.md).

Read additional references only for the affected work:

| Work | Required reference |
| --- | --- |
| Component qualification, Q1/Q2 or activation | [validator_v3.md](../../../validator_v3.md); this checkout has no `validator.md`. |
| Component migration, reuse and canonical execution Recipes | [monty.composition.md](../../../docs/plans/monty.composition.md) and its linked implementation/Skill-Recipe appendices. |
| Prefix generation, profiles, model selection or Sempai knowledge | [prefix_v3_upgrade.md](../../../docs/plans/prefix_v3_upgrade.md). |
| Monty hosting, live settings or recovery | [simplified-v3 implementation plan](../../../docs/plans/simplified-v3-implementation.md) and [vendor compatibility contract](../../../vendor/monty-control/BRASSCLAW.md). |

Treat dated plan milestones as recorded evidence for their exact scope. Reinspect
current source and consumers; a pending statement in an older milestone or a newer
helper acceptance does not establish the current production status of every gate.

Choose the implementation path from the responsibility:

- **Task behavior:** Recipes and associated PythonCode own workflow sequencing,
  Tool calls, explicit LLM steps, replies and continuation. Add a Rust Tool only
  for an exact missing system primitive after considering existing usages.
- **Operator management:** settings, catalogue administration and control routes
  use the product facade and composition services. They may need Rust/API/UI
  changes without becoming a new workflow Tool. Task execution still enters the
  ordinary ingress and orchestrator path.
- **Infrastructure gaps:** repair binding, stores, lifecycle, resource accounting,
  transport or kernel enforcement in their owning layers. A missing runner
  contract does not justify hiding the task workflow in a specialized Rust Tool.

## Inventory existing integration points

Inspect actual signatures, adapters, feature gates and callers. A matching name
does not establish a working binding or an approved component combination.
Use task-specific searches of seeders and component stores as well as these
product integration starting points:

```bash
rg -n 'trait .*ProductFacade|pub trait .*Service|async fn ' crates/brassclaw_product_workflow/src
rg -n 'Reborn.*Admin|Reborn.*Facade|ProductCommandService|PgProviderRepo' crates/brassclaw_reborn_composition/src
rg -n 'WEBUI_V2_PATTERN_|fn .*_descriptor' crates/brassclaw_webui_v2/src/descriptors.rs
rg -n 'Swappable|Reload|SecretStore' crates/brassclaw_llm/src crates/brassclaw_reborn_config/src crates/brassclaw_secrets/src
rg -n 'GlobalMontyOwner|global_monty_startup|MontySettingsOwner' crates/brassclaw_reborn_composition/src
rg --files crates | rg '(builtin_bootstrap|seeder|seed|recipe|python_code|tool_skill)'
```

Reuse compatible ports and handles rather than introducing parallel traits.
Historical code is evidence of current support, not the target design. In
particular, final-v3 provider definitions, configuration and selection use
PostgreSQL through `PgProviderRepo` or a neutral port; `config.toml`, embedded
provider lists and file fallbacks are legacy paths to reconcile under v3 §10.
Keep secrets behind the existing secret broker.

For the affected subsystem, follow these current implementation seams rather
than creating another loader, catalogue, review owner or settings publisher:

| Subsystem | Source starting points |
| --- | --- |
| Retained workflow assembly | `brassclaw_engine/src/memory/retained_instruction.rs`, `retained_inputs.rs`, `retained_tools.rs`, `retained_reviews.rs`; `executor/retained_recipe.rs`. |
| Preload/export qualification | [retained_preload.rs](../../../crates/brassclaw_engine/src/executor/retained_preload.rs), [retained_source.rs](../../../crates/brassclaw_engine/src/executor/retained_source.rs); actual `brassclaw_monty_host` source inspection and invocation callers. |
| Catalogue capture and activation | [installed_monty_catalogue.rs](../../../crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs), [authored_monty_catalogue.rs](../../../crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs). |
| Submission and trusted review | Composition `pg_review_submission.rs`, `pg_workflow_review.rs`, `pg_association_review.rs`; retained revision, review and association stores in `brassclaw_skills`. |
| Live resource publication | [live_monty_settings.rs](../../../crates/brassclaw_reborn_composition/src/live_monty_settings.rs), `pg_monty_vm_settings.rs`; product settings DTOs, Monty service/transport owners and WebUI consumers. |
| Completed-turn review | Composition `completed_turn_review/{mod,delivery,backend,ports,reconciliation}.rs`, `pg_review_inspection.rs`. |
| MCP | Composition `mcp_recipe_catalogue.rs`, `mcp_command_qualification.rs`, `mcp_chat_bridge.rs`, `mcp_server_service.rs`, `mcp_provider_gateway.rs`; ingress listener wiring. |

These paths are source navigation, not an assertion that every final-v3 feature
is implemented or deployed. Trace feature gates, concrete store APIs and ordinary
callers before reusing any seam.

## Trace the complete affected path

For a management feature, trace one existing vertical before the first edit:

```text
browser (brassclaw_webui_v2_static)
  -> apiFetch -> webui_v2 descriptor/handler
  -> RebornServicesApi (brassclaw_product_workflow)
  -> port -> composition adapter
  -> supported store and live runtime/control handle
```

The composition path supplying that adapter is:

```text
brassclaw_reborn_cli/src/commands/serve.rs
  -> build_runtime_input_with_options -> RebornRuntimeInput
  -> build_reborn_runtime -> RebornRuntime
  -> build_webui_services(Arc<RebornRuntime>, ...) -> attached facades
```

Read the affected portions of `factory.rs`, `runtime.rs`, `runtime_input.rs`,
`webui.rs` and `commands/serve.rs`. Determine which store/control handle is
needed, who owns it, and whether it is already available. Thread required inputs
through existing `with_*` builders; construct consuming services inside composition
and expose facade-shaped handles rather than public raw substrate accessors.

For a task capability, trace ingress -> the already-running global Monty ->
matching -> IBS/composition -> pinned execution -> host/kernel -> reply/history.
Exactly one supervised global Monty service starts after migrations, component
seeding and integrity checks, before workers/producers/ingress accept work. Turns,
waits and cancellation retain isolated task contexts without replacing that VM.
Rust hosts/transports work; it must not add a second workflow loop or a per-chat
fallback. Inspect the ordinary caller; candidate tests alone do not prove uptake.

## Apply the component execution contracts

Use the four guides for detailed schemas and acceptance. During feature design:

- A runtime Skill is one Tool usage with prose and explicitly associated
  PythonCode exports/signatures, helpers/constants and dependencies. Load pinned
  definitions in dependency order without effects; invoke exports with typed data.
  Each real Skill needs a canonical execution Recipe and matching command.
- Eligible deterministic usages may be Tier 0; shell and subagent Recipes are
  always Tier 1. Keep creative reasoning in explicit LLM steps. Tier 2 is entered
  only by actual No-Match, never to bypass a failed match or unsupported runner.
- Recipe component steps reference one stable UUID each. A Rust ToolSkill binding
  step executes nothing and grants no permission; its immediately following
  matching PythonCode step invokes the usage. Pure logic needs no artificial
  binding. Independent Tool calls stay in separate execution steps; use the
  documented dependent-chain exception only with verified binding coverage.
- Use persisted `knowledge`/`stepnumber` syntax and step-local
  `inputs["local_name"]` bindings from recipe.md. Runtime values never become
  Python source. Validate recursive contracts, missing/null/default semantics,
  results and handoffs; isolate mutable state per task/attempt/invocation.
- Match and assemble from one coherent active, approved catalogue snapshot. Pin
  Recipe/variant/step links/input layout, exports, transitive dependencies, exact
  revisions/checksums, association approvals and actual retained Tool artifacts.
  Children, retries and resumed tasks keep that selection; do not read latest again.
- Authored installed-instance changes require trusted Q1, behavioral evidence and
  human Q2 for exact combinations. Bundled first-party components use the separate
  integrity-qualified `system_seed` bootstrap path without installed-instance Q2.
  Labels and manifests alone do not prove approval or grant dispatch authority.
- Only an actual No-Match enters Tier 2. Matching/DB errors, ambiguity and begun
  Recipe failures remain distinct. Preserve durable dispatch intent/counts,
  idempotency and confirmed/unresolved effects through waits and recovery. A timeout
  does not prove no effect; never replay completed effects or a whole Recipe after
  a crash. Fence stale generations before supervised reconciled replacement.
- If exposing Skill usages through MCP, follow skills.md's approved canonical
  Recipe discovery and completed-command chat ingress contract. MCP must not
  become a direct Python, IBS or Rust dispatch path.

Inspect the actual `python-preload/2` declaration reader and retained source
inspector before authoring exports. Use supported checksummed revision documents;
do not invent legacy INSERT fields or extend association formats by assumption.
Verify export signatures, fixed arguments, recursive contracts, declared imports,
dependency order, collisions and effect-free loading. The current inspector
conservatively rejects unsupported Tool-calling recursion, closures, first-class
aliases and repeatable dispatch patterns. Repair an actual missing adapter in its
owning layer before activating a usage; do not evade rejection with source rewriting.

`PreparedReplSeed` is first-feed compiler reuse, not complete catalogue-prefix
compilation or approval. Shared immutable bytecode must preserve fresh mutable
arguments/defaults/closures, task-owned bindings and old export selections.

## Authoring, review and coherent activation

Trace the existing submission and activation facade methods through their
composition/store implementations. Submit immutable unreviewed revisions with
exact predecessor/dependency references; submission must not mutate live legacy
rows, manufacture evidence or activate a catalogue. Keep structural Q1, behavioral
evidence, human Q2 and exact association approval distinct and owned by their
trusted writers. Model/Recipe authoring ports cannot write successful human Q2.

Activation consumes applicable existing approvals and checks assembly/runner
compatibility for a coherent generation; it must not rerun workflow effects or
turn into per-invocation review. Preserve unchanged approval identifiers and old
task generations. Publish routing, prefix and qualified MCP discovery coherently,
with revision conflicts and unsupported combinations failing before effects.
Use the installation owner's noncircular qualification path for bundled seeds;
a `source: system` label cannot qualify an installed-instance edit.

## Completed-turn Sempai review and MCP integration

Reuse the installation-owned completed-turn review Recipe and existing global
service delivery. Keep the completed prompt/response/effect bundle correlated
with its conversation, run and attempt, and pin the review's model/prefix selection.
Proposed components remain immutable and unreviewed until the separate authoring
gates pass. Preserve uncertain attempts, retained answers and quarantine state.
Inspection and reconciliation may settle only actual retained native observations;
they must not resubmit a provider request, replay Tools, re-admit a review, finish
a Recipe synthetically or activate a candidate. A reconciliation note is not proof
of an external effect or settlement after restart.

For MCP, declarations are not qualification or advertising. Reuse command
qualification and immutable discovery snapshots from the active Recipe catalogue;
do not expose raw Skill rows. Keep the server alive across requests. Kohai prepares
the final selected prefix before advertising the request-local provider connection,
then disconnects that exchange on complete answer or reconciled failure/cancellation.
Retain each advertised contract and selected generation through streaming and
concurrent calls. Calls enter new ordinary chats and return correlated terminal
results; lost replies or chat closure must not resend a completed effect.

## Prefix generation and serving features

Follow the prefix upgrade plan rather than adding another compiler or source
catalogue. Inventory complete authoritative originals and approved components,
with source hashes, applicability and provenance. Preserve originals separately
from model-facing teaching/compiler output; exclude private deployment data.
Do not silently trim the catalogue with row caps or architecture-heading slices.

Keep generation, validation, target/cache qualification and activation distinct.
Use immutable generations and the supported durable Recipe/job path; pin the
profile, model/design, source and compiler selections for each build and serving
request. Preserve predecessor generations needed by existing consumers. The six
planned profiles (`base-prompt`, `defensive`, `homeassistant`, `tomedo`, `sempai`,
`prefix-maker`) are target scope, not proof that their current APIs/jobs are shipped.
Inspect the current store, compiler and serving adapter before claiming uptake.

## Product integration checklist

Extend only the layers the feature actually needs:

| Layer | Existing seam and required work |
| --- | --- |
| Port | `brassclaw_product_workflow`: reuse or extend the feature port, DTOs and errors; re-export where required. |
| Facade | Extend `RebornServicesApi` and the real facade. Use fail-closed unavailable defaults where appropriate; verify production attachment and affected implementations rather than relying on defaults to hide missing wiring. |
| Adapter | `brassclaw_reborn_composition`: implement the supported port using existing stores/control handles; register under applicable current feature gates. |
| HTTP | `brassclaw_webui_v2`: route/pattern/descriptor, thin handler over `state.services()`, router mounting, and affected `tests/webui_v2_descriptors_contract.rs` expectations. |
| Wiring | Attach in `build_webui_services`; thread genuinely missing inputs through runtime/CLI composition. |
| Frontend | `brassclaw_webui_v2_static`: existing `apiFetch`, API modules, hooks and UI states; verify behavior and syntax of changed JavaScript. |

Feature gates describe current builds, not permission to invent final-v3 product
editions. Required services must be wired in the supported full product.
Composition must not depend on the v1 `src/` tree or unextracted legacy crates.
HTTP handlers use only `RebornServicesApi`; keep raw stores, dispatchers and
host-runtime handles inside composition rather than accessing them from handlers.

## Operator authority and live settings

- The valid instance-token operator administers all supported instance functions.
  Do not add user/tenant/project/feature-role checks or a multi-tenant mode.
  Preserve conversation/message/run/attempt identifiers for data relationships,
  cancellation, replies and idempotency; they are not Tool permission scopes.
  External channel senders do not inherit operator management authority.
- Retain token authentication, CSRF/origin protection, input validation, auditing,
  sandbox/network/secret enforcement and resource limits. Existing
  `read_policy`/`mutation_policy` helpers require inspection against the target;
  copying legacy scope metadata does not establish final-v3 authorization.
- Kernel checks apply current instance-wide Tool allow/block settings and technical
  constraints before every dispatch, including retries and running Recipes, across
  retained versions/aliases. Approval or ToolSkill binding never replaces that
  check. Run claims fence execution; they are not invocation approval leases.
- Supported resource and Tool settings changes take effect live. Validate the
  complete revisioned settings set, prevent lost updates, and coordinate store
  publication with runtime feasibility/acknowledgement. Desired/effective revisions,
  pending uptake and failures must be visible; UI save success proves persistence
  only. Preserve consumed task budgets across changes and continuations. Reject
  unsafe manual memory reductions; keep automatic reductions pending when required
  by Phase 3a. Do not substitute restart-to-apply or merely log a reload failure.

For resource changes, inspect the complete product settings DTOs and live owner:

- The default task limit is 600 seconds of executing VM time; distinguish
  preparation observations from execution and exclude idle/queue/external waits.
  It never becomes a deadline for the global orchestrator's lifetime.
- Shared-heap startup/manual budgets have a 512 MiB floor. Keep the finite
  physical worker backstop distinct from logical heap usage and host allocations.
  Preserve desired/effective revisions and retained work during pending reductions.
- Monty 1.0 removed allocation-count enforcement. Preserve retired configured
  values through the supported migration and report unsupported status; never
  reinterpret counts as bytes or pretend the retired setting still limits execution.
- Token budgets default to disabled. When disabled, remove artificial retrieval,
  assembly/history and task token caps while retaining accounting and actual model
  context/output limits.
- Coordinate heap, task, frame, queue, context and deadline policies through the
  existing durable settings publication boundary. Preserve accepted exchange frame
  contracts, completed answers, queued continuations and consumed budgets. Poll
  accepted host I/O while publication holds VM execution; do not replay effects to
  drain capacity or acknowledge a new policy merely because PostgreSQL saved it.

## Validate by the claim and report support honestly

Follow the development policy rather than building each crate by habit. Review
the complete change and affected consumers before compilation. Reuse passing
evidence unless a relevant change invalidates it; keep Cargo target/features/profile
stable and obey repository disk-space rules before executable checks.

- Prose-only changes: inspect consistency and links, then `git diff --check`.
- Component changes: schema/reference/routing and exact-combination validation,
  appropriate approval/bootstrap evidence, and behavior through the selected path.
- Rust changes: focused behavioral regression and affected-package linting with
  relevant features. API/dependency changes expand to affected consumers and
  architecture checks. Check changed frontend syntax with `node --check` and
  exercise the affected UI behavior.
- Database, lifecycle, security and live settings changes: real integration and
  ordinary production-caller acceptance, including the relevant failure cases.
  Read `LOCAL_TEST_ENV.md` if present before remote tests or provider setup.

Report implemented support, the path exercised and remaining gaps separately.
For affected preload/catalogue/settings/MCP/review work, choose relevant real
regressions proving retained generations, mutable-state isolation, live policy,
single-VM identity and no-replay behavior. Include invalid inputs, revision conflicts
and interrupted publication/settlement when those are the changed contracts. New
test names or source presence are not acceptance; distinguish helper, retained
worker, native database and ordinary authenticated product evidence.
Markdown, successful compilation, a persisted setting or candidate worker tests
do not establish activation, runtime uptake or full-v3 acceptance. Stop when the
coherent change and required relevant checks pass; expand only for unresolved
questions or binding acceptance requirements.
