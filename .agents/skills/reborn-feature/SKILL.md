---
name: reborn-feature
description: Plan and implement Reborn capabilities and their WebUI, settings, API, facade, and runtime integration using the final-v3 architecture. Start with Recipe/component reuse, then trace the required product and infrastructure wiring. Use for Reborn feature work; ordinary component-only authoring follows the repository component guides.
---

# Building a Reborn feature against final v3

This is a Codex development skill. BrassClaw runtime Skills have their own
preloadable function and association contracts; this file is development guidance.
Inventory existing components and trace the affected production path before editing.

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
Markdown, successful compilation, a persisted setting or candidate worker tests
do not establish activation, runtime uptake or full-v3 acceptance. Stop when the
coherent change and required relevant checks pass; expand only for unresolved
questions or binding acceptance requirements.
